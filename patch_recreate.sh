cat << 'EOF2' > src/systems/http_server.rs
//! # Multithreaded HTTP/1.1 Server
//!
//! Implements a minimal, multithreaded HTTP/1.1 server from scratch.
//!
//! **Replaces Crates:** `hyper`, `actix-web`, `axum`
//!
//! **Real-world Usage:**
//! - Web servers handling concurrent HTTP requests.
//! - Foundation for REST APIs and web frameworks.
//! - Load balancers and reverse proxies parsing HTTP headers.
//!
//! **Why build it yourself?**
//! Building an HTTP server from raw TCP streams forces you to understand the HTTP/1.1 protocol,
//! string parsing, threading models, and synchronization. You'll see exactly how bytes on a socket
//! turn into structured requests and responses, and how a thread pool manages concurrent connections.
//!
//! # Architecture
//!
//! ```text
//!    TCP Listener
//!         │
//!         ▼ (accept)
//!    [Incoming TCP Stream] ───► Thread Pool (Queue)
//!                                    │
//!               ┌────────────────────┼────────────────────┐
//!               ▼                    ▼                    ▼
//!          [Worker Thread]      [Worker Thread]      [Worker Thread]
//!               │
//!               ▼
//!       Read & Parse HTTP
//!       (Method, URI, Headers)
//!               │
//!               ▼
//!            Router ──► Match Route ──► Handler
//!                                          │
//!                                          ▼
//!               ┌──────────────────────────┘
//!               ▼
//!       Generate Response
//!       (Status, Headers, Body)
//!               │
//!               ▼
//!         Write to Stream
//! ```
//!
//! **Invariants:**
//! 1. `ThreadPool` bounds the maximum number of concurrent threads.
//! 2. `Router` dispatches requests based on exact match of HTTP Method and URI path.
//! 3. Requests without a matched route return a `404 Not Found`.
//! 4. Unparsable requests return a `400 Bad Request`.
//!
//! **Complexity (Routing):**
//! - **Time:** O(1) assuming `HashMap` lookup for routes.
//! - **Space:** O(R) where R is the number of routes.
//!
//! **Design Decisions:**
//! - **Thread Pool:** Uses a task queue rather than `std::thread::spawn` per request to prevent OS resource exhaustion.
//! - **Parsing:** Implements minimal parsing using basic string splitting. A production server would use a zero-copy parser like `httparse`.
//! - **Handlers:** Uses boxed closures for flexibility, implementing the Command pattern for routes.
//!
//! # Footer
//!
//! **Comparison to `hyper` / `axum`:**
//! - Canonical crates use asynchronous I/O (`tokio`) to handle millions of connections efficiently. This implementation uses synchronous I/O and OS threads, which is simpler but less scalable due to context switching overhead and memory per thread.
//! - Canonical crates have robust HTTP/1.1 parsing (handling chunked encoding, pipelines, keep-alive).
//!
//! **Missing from production:**
//! - Keep-Alive connection pooling.
//! - Async/await support.
//! - Robust error handling (e.g., timeouts, malicious payloads).
//! - Zero-copy request parsing.
//!
//! **Next Steps:**
//! - Implement an async version using `epoll` / `kqueue`.
//! - Add middleware support (logging, auth).
//!
//! **Benchmarking Note:**
//! To benchmark this server, start it locally and use `wrk` or `ab` (Apache Bench):
//! `wrk -t4 -c100 -d10s http://127.0.0.1:8080/hello`

use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

// =========================================================================================
// Thread Pool
// =========================================================================================

type Job = Box<dyn FnOnce() + Send + 'static>;

struct TaskQueue {
    queue: VecDeque<Job>,
    shutdown: bool,
}

/// A Thread Pool for executing jobs concurrently.
pub struct ThreadPool {
    workers: Vec<thread::JoinHandle<()>>,
    shared_state: Arc<(Mutex<TaskQueue>, Condvar)>,
}

impl ThreadPool {
    /// Creates a new `ThreadPool`.
    ///
    /// # Panics
    ///
    /// Panics if `size` is zero.
    #[must_use]
    pub fn new(size: usize) -> Self {
        assert!(size > 0, "ThreadPool size must be greater than zero");

        let shared_state = Arc::new((
            Mutex::new(TaskQueue {
                queue: VecDeque::new(),
                shutdown: false,
            }),
            Condvar::new(),
        ));

        let mut workers = Vec::with_capacity(size);
        for _ in 0..size {
            let state = Arc::clone(&shared_state);
            workers.push(thread::spawn(move || {
                loop {
                    let job = {
                        let (lock, cvar) = state.as_ref();
                        let mut task_queue = lock.lock().unwrap();

                        while task_queue.queue.is_empty() && !task_queue.shutdown {
                            task_queue = cvar.wait(task_queue).unwrap();
                        }

                        if task_queue.shutdown && task_queue.queue.is_empty() {
                            break; // Shutting down and no more jobs
                        }

                        let job = task_queue.queue.pop_front().unwrap();
                        // resolve clippy::significant_drop_tightening
                        drop(task_queue);
                        job
                    };

                    // RUST INSIGHT: We execute the job without holding the lock.
                    // This is essential for concurrency!
                    job();
                }
            }));
        }

        Self {
            workers,
            shared_state,
        }
    }

    /// Executes a job on the thread pool.
    ///
    /// # Panics
    ///
    /// Panics if the internal task queue mutex is poisoned or if a condition variable wait fails.
    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);
        let (lock, cvar) = self.shared_state.as_ref();
        let mut task_queue = lock.lock().unwrap();
        task_queue.queue.push_back(job);
        drop(task_queue); // Release lock before notifying
        cvar.notify_one();
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        {
            let (lock, cvar) = self.shared_state.as_ref();
            let mut task_queue = lock.lock().unwrap();
            task_queue.shutdown = true;
            drop(task_queue);
            cvar.notify_all();
        }

        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

// =========================================================================================
// HTTP Parsing & Types
// =========================================================================================

/// Standard HTTP Methods.
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum Method {
    Get,
    Post,
    Put,
    Delete,
    Unknown(String),
}

impl From<&str> for Method {
    fn from(s: &str) -> Self {
        match s {
            "GET" => Self::Get,
            "POST" => Self::Post,
            "PUT" => Self::Put,
            "DELETE" => Self::Delete,
            _ => Self::Unknown(s.to_string()),
        }
    }
}

/// An incoming HTTP Request.
#[derive(Debug)]
pub struct Request {
    pub method: Method,
    pub path: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl Request {
    /// Parses an HTTP request from a reader.
    ///
    /// # Errors
    ///
    /// Returns an error message if parsing fails.
    pub fn parse(mut reader: impl BufRead) -> Result<Self, String> {
        let mut request_line = String::new();
        reader
            .read_line(&mut request_line)
            .map_err(|e| e.to_string())?;

        // clippy::trim_split_whitespace handled by split_whitespace
        let mut parts = request_line.split_whitespace();
        let method_str = parts.next().ok_or("Missing method")?;
        let path = parts.next().ok_or("Missing path")?.to_string();

        let method = Method::from(method_str);

        let mut headers = HashMap::new();
        loop {
            let mut header_line = String::new();
            reader
                .read_line(&mut header_line)
                .map_err(|e| e.to_string())?;
            let header_line = header_line.trim_end();
            if header_line.is_empty() {
                break; // End of headers
            }

            if let Some((key, value)) = header_line.split_once(':') {
                headers.insert(key.trim().to_lowercase(), value.trim().to_string());
            }
        }

        let mut body = Vec::new();
        // GOTCHA: Without checking `Content-Length`, `read_to_end` would block
        // indefinitely waiting for EOF on keep-alive connections.
        if let Some(len) = headers
            .get("content-length")
            .and_then(|s| s.parse::<usize>().ok())
        {
            body.resize(len, 0);
            reader.read_exact(&mut body).map_err(|e| e.to_string())?;
        }

        Ok(Self {
            method,
            path,
            headers,
            body,
        })
    }
}

/// An outgoing HTTP Response.
#[derive(Debug)]
pub struct Response {
    pub status_code: u16,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl Response {
    /// Creates a new `Response`.
    #[must_use]
    pub fn new(status_code: u16, body: impl Into<Vec<u8>>) -> Self {
        let mut headers = HashMap::new();
        let body_vec = body.into();
        headers.insert("Content-Length".to_string(), body_vec.len().to_string());
        headers.insert("Connection".to_string(), "close".to_string()); // Simple server, no keep-alive

        Self {
            status_code,
            headers,
            body: body_vec,
        }
    }

    /// Writes the response to a writer.
    ///
    /// # Errors
    ///
    /// Returns a `std::io::Error` if writing fails.
    pub fn write_to(&self, mut writer: impl Write) -> std::io::Result<()> {
        let reason = match self.status_code {
            200 => "OK",
            400 => "Bad Request",
            404 => "Not Found",
            500 => "Internal Server Error",
            _ => "Unknown",
        };

        write!(
            writer,
            "HTTP/1.1 {status_code} {reason}\r\n",
            status_code = self.status_code,
            reason = reason
        )?;
        for (key, value) in &self.headers {
            write!(writer, "{key}: {value}\r\n")?;
        }
        write!(writer, "\r\n")?;
        writer.write_all(&self.body)?;
        writer.flush()?;
        Ok(())
    }
}

// =========================================================================================
// Router & Server
// =========================================================================================

type Handler = Box<dyn Fn(&Request) -> Response + Send + Sync>;

/// HTTP Router mapping methods and paths to handlers.
pub struct Router {
    routes: HashMap<(Method, String), Handler>,
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}

impl Router {
    /// Creates a new `Router`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            routes: HashMap::new(),
        }
    }

    /// Adds a route to the router.
    pub fn add_route<F>(&mut self, method: Method, path: &str, handler: F)
    where
        F: Fn(&Request) -> Response + Send + Sync + 'static,
    {
        self.routes
            .insert((method, path.to_string()), Box::new(handler));
    }

    /// Handles a request, dispatching to the appropriate handler or returning 404.
    #[must_use]
    pub fn handle(&self, request: &Request) -> Response {
        self.routes
            .get(&(request.method.clone(), request.path.clone()))
            .map_or_else(
                || Response::new(404, "Not Found"),
                |handler| handler(request),
            )
    }
}

/// A Multithreaded HTTP Server.
#[allow(clippy::module_name_repetitions)]
pub struct HttpServer {
    address: String,
    router: Arc<Router>,
    thread_pool: ThreadPool,
}

impl HttpServer {
    /// Creates a new `HttpServer`.
    ///
    /// # Panics
    ///
    /// Panics if `pool_size` is zero.
    #[must_use]
    pub fn new(address: &str, router: Router, pool_size: usize) -> Self {
        Self {
            address: address.to_string(),
            router: Arc::new(router),
            thread_pool: ThreadPool::new(pool_size),
        }
    }

    /// Runs the HTTP server, blocking the current thread.
    ///
    /// # Errors
    ///
    /// Returns a `std::io::Error` if binding the address fails.
    pub fn run(&self) -> std::io::Result<()> {
        let listener = TcpListener::bind(&self.address)?;

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let router = Arc::clone(&self.router);
                    self.thread_pool.execute(move || {
                        Self::handle_connection(stream, &router);
                    });
                }
                Err(_e) => {
                    // In a production server, we would log the error
                }
            }
        }
        Ok(())
    }

    fn handle_connection(mut stream: TcpStream, router: &Router) {
        // PRODUCTION NOTE: We use `BufReader` for efficient line-by-line reading.
        // To reuse the stream for writing, we initialize it with a mutable reference
        // and explicitly drop the reader to reclaim the stream for `.write_to()`.
        let response = {
            let mut reader = BufReader::new(&mut stream);
            let res = Request::parse(&mut reader).map_or_else(
                |_| Response::new(400, "Bad Request"),
                |request| router.handle(&request),
            );
            drop(reader);
            res
        };

        let _ = response.write_to(&mut stream);
    }
}

// =========================================================================================
// Tests
// =========================================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_thread_pool_execution() {
        let pool = ThreadPool::new(4);
        let counter = Arc::new(AtomicUsize::new(0));

        for _ in 0..10 {
            let c = Arc::clone(&counter);
            pool.execute(move || {
                c.fetch_add(1, Ordering::SeqCst);
            });
        }

        // Wait a bit for threads to finish
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert_eq!(counter.load(Ordering::SeqCst), 10);
    }

    #[test]
    fn test_request_parsing_valid() {
        let raw_request = b"POST /api/data HTTP/1.1\r\nHost: localhost\r\nContent-Length: 5\r\n\r\nhello";
        let reader = Cursor::new(raw_request);
        let req = Request::parse(reader).expect("Failed to parse valid request");

        assert_eq!(req.method, Method::Post);
        assert_eq!(req.path, "/api/data");
        assert_eq!(req.headers.get("host").unwrap(), "localhost");
        assert_eq!(req.headers.get("content-length").unwrap(), "5");
        assert_eq!(req.body, b"hello");
    }

    #[test]
    fn test_request_parsing_empty_body() {
        let raw_request = b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let reader = Cursor::new(raw_request);
        let req = Request::parse(reader).unwrap();
        assert_eq!(req.method, Method::Get);
        assert!(req.body.is_empty());
    }

    #[test]
    fn test_router_dispatch() {
        let mut router = Router::new();
        router.add_route(Method::Get, "/hello", |_req| Response::new(200, "world"));

        let req_valid = Request {
            method: Method::Get,
            path: "/hello".to_string(),
            headers: HashMap::new(),
            body: Vec::new(),
        };
        let resp_valid = router.handle(&req_valid);
        assert_eq!(resp_valid.status_code, 200);
        assert_eq!(resp_valid.body, b"world");

        let req_invalid = Request {
            method: Method::Get,
            path: "/not-found".to_string(),
            headers: HashMap::new(),
            body: Vec::new(),
        };
        let resp_invalid = router.handle(&req_invalid);
        assert_eq!(resp_invalid.status_code, 404);
    }

    #[test]
    fn test_response_serialization() {
        let resp = Response::new(200, "ok");
        let mut buf = Vec::new();
        resp.write_to(&mut buf).unwrap();

        let output = String::from_utf8(buf).unwrap();
        assert!(output.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(output.contains("Content-Length: 2\r\n"));
        assert!(output.ends_with("\r\nok"));
    }
}
EOF2
