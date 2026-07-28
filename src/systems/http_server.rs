//! # HTTP Server Implementation
//!
//! Implements a minimal, multithreaded HTTP/1.1 server from scratch without external dependencies.
//!
//! **Replaces Crates:** `hyper`, `actix-web`, `axum` (at a very low level)
//!
//! **Real-world Usage:**
//! - Core logic inside web frameworks
//! - Reverse proxies (e.g., Nginx, `HAProxy`)
//! - Embedded systems requiring lightweight web interfaces
//!
//! **Why build it yourself?**
//! Building an HTTP server from raw TCP streams teaches you how text protocols actually work over the wire.
//! You learn about parsing headers, chunking data, threading models (thread-per-connection vs thread pools),
//! and how abstractions like `Request` and `Response` are built from plain bytes.

use std::collections::HashMap;
use std::fmt::Write as FmtWrite;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Diagram:
//
//     Client       Server (TcpListener)       ThreadPool (Workers)
//       │                 │                         │
//       │── TCP SYN ─────>│                         │
//       │<─ SYN-ACK ──────│                         │
//       │── ACK ─────────>│                         │
//       │                 │                         │
//       │── HTTP GET ────>│──── TcpStream ─────────>│
//       │                 │                         │── Read Stream
//       │                 │                         │── Parse HTTP Request
//       │                 │                         │── Generate Response
//       │<─ HTTP 200 OK ──│<─── write_all ──────────│
//
// Invariants:
// 1. ThreadPool manages a fixed number of worker threads to prevent resource exhaustion.
// 2. Incoming connections are placed in a channel; idle workers pick them up.
// 3. The server must handle malformed requests gracefully (return 400 Bad Request) rather than crashing.
// 4. HTTP Headers are separated by `\r\n`, and the body is separated by `\r\n\r\n`.
//
// Complexity (per request):
// ┌──────────────┬──────────────┬────────┐
// │ Operation    │ Time         │ Space  │
// ├──────────────┼──────────────┼────────┤
// │ Parse Req    │ O(N) bytes   │ O(N)   │
// │ Routing      │ O(1)         │ O(1)   │
// │ Write Res    │ O(M) bytes   │ O(M)   │
// └──────────────┴──────────────┴────────┘
// N = size of request headers, M = size of response.
//
// Design Decisions:
// - **Concurrency**: Basic ThreadPool using standard library `mpsc` and `Mutex`.
//   - *Alternative*: Async/await with non-blocking I/O (epoll). Much more scalable but complex.
// - **Parsing**: Manual string splitting. A production server uses state machines (like `httparse`) for zero-allocation parsing.

/// Represents an HTTP Method.
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Unknown,
}

impl From<&str> for HttpMethod {
    fn from(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "GET" => Self::Get,
            "POST" => Self::Post,
            "PUT" => Self::Put,
            "DELETE" => Self::Delete,
            _ => Self::Unknown,
        }
    }
}

/// Represents an HTTP Request.
#[derive(Debug, Clone)]
pub struct HttpRequest {
    pub method: HttpMethod,
    pub path: String,
    pub version: String,
    pub headers: HashMap<String, String>,
    pub body: String,
}

impl HttpRequest {
    /// Parses a raw byte stream into an `HttpRequest`.
    ///
    /// # Errors
    /// Returns an error message if parsing fails.
    pub fn parse(raw_request: &[u8]) -> Result<Self, &'static str> {
        let request_str = std::str::from_utf8(raw_request).map_err(|_| "Invalid UTF-8")?;

        // HTTP headers and body are separated by \r\n\r\n
        let mut parts = request_str.splitn(2, "\r\n\r\n");
        let head_part = parts.next().ok_or("Malformed request")?;
        let body_part = parts.next().unwrap_or("");

        let mut lines = head_part.lines();

        // Parse request line: METHOD PATH VERSION
        let request_line = lines.next().ok_or("Empty request line")?;
        let mut req_parts = request_line.split_whitespace();

        let method = HttpMethod::from(req_parts.next().ok_or("Missing method")?);
        let path = req_parts.next().ok_or("Missing path")?.to_string();
        let version = req_parts.next().ok_or("Missing version")?.to_string();

        // Parse headers
        let mut headers = HashMap::new();
        for line in lines {
            if line.is_empty() {
                continue;
            }
            if let Some((k, v)) = line.split_once(':') {
                headers.insert(k.trim().to_string(), v.trim().to_string());
            }
        }

        Ok(Self {
            method,
            path,
            version,
            headers,
            body: body_part.to_string(),
        })
    }
}

/// Represents an HTTP Response.
pub struct HttpResponse {
    pub status_code: u16,
    pub status_text: String,
    pub headers: HashMap<String, String>,
    pub body: String,
}

impl HttpResponse {
    #[must_use]
    pub fn new(status_code: u16, status_text: &str, body: &str) -> Self {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "text/plain".to_string());
        headers.insert("Content-Length".to_string(), body.len().to_string());

        Self {
            status_code,
            status_text: status_text.to_string(),
            headers,
            body: body.to_string(),
        }
    }

    #[must_use]
    pub fn ok(body: &str) -> Self {
        Self::new(200, "OK", body)
    }

    #[must_use]
    pub fn not_found() -> Self {
        Self::new(404, "Not Found", "404 Not Found")
    }

    #[must_use]
    pub fn bad_request() -> Self {
        Self::new(400, "Bad Request", "400 Bad Request")
    }

    /// Converts the response into bytes suitable for writing to a TCP stream.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut response_str = format!("HTTP/1.1 {} {}\r\n", self.status_code, self.status_text);

        for (k, v) in &self.headers {
            let _ = write!(response_str, "{k}: {v}\r\n");
        }

        response_str.push_str("\r\n");
        response_str.push_str(&self.body);

        response_str.into_bytes()
    }
}

type Job = Box<dyn FnOnce() + Send + 'static>;

/// A simple thread pool for handling connections.
pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: Option<std::sync::mpsc::Sender<Job>>,
}

impl ThreadPool {
    /// Creates a new `ThreadPool`.
    ///
    /// # Panics
    /// Panics if size is 0.
    #[must_use]
    pub fn new(size: usize) -> Self {
        assert!(size > 0);

        let (sender, receiver) = std::sync::mpsc::channel();
        let receiver = Arc::new(Mutex::new(receiver));

        let mut workers = Vec::with_capacity(size);
        for _ in 0..size {
            workers.push(Worker::new(Arc::clone(&receiver)));
        }

        Self {
            workers,
            sender: Some(sender),
        }
    }

    /// Executes a job on the thread pool.
    ///
    /// # Panics
    /// Panics if the channel is disconnected.
    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);
        if let Some(sender) = &self.sender {
            sender.send(job).unwrap();
        }
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        drop(self.sender.take());

        for worker in &mut self.workers {
            if let Some(thread) = worker.thread.take() {
                thread.join().unwrap();
            }
        }
    }
}

struct Worker {
    thread: Option<thread::JoinHandle<()>>,
}

impl Worker {
    fn new(receiver: Arc<Mutex<std::sync::mpsc::Receiver<Job>>>) -> Self {
        let thread = thread::spawn(move || {
            loop {
                let message = receiver.lock().unwrap().recv();

                match message {
                    Ok(job) => {
                        job();
                    }
                    Err(_) => {
                        break;
                    }
                }
            }
        });

        Self {
            thread: Some(thread),
        }
    }
}

/// A type alias for the route handler map to avoid type complexity.
type RouteMap = HashMap<String, Arc<dyn Fn(&HttpRequest) -> HttpResponse + Send + Sync>>;

/// A minimal HTTP Server.
pub struct HttpServer {
    address: String,
    pool: ThreadPool,
    // Basic routing: Path -> Handler
    routes: Arc<RouteMap>,
}

impl HttpServer {
    /// Creates a new HTTP server.
    #[must_use]
    pub fn new(address: &str, thread_count: usize) -> Self {
        Self {
            address: address.to_string(),
            pool: ThreadPool::new(thread_count),
            routes: Arc::new(HashMap::new()),
        }
    }

    /// Adds a route to the server. (Builder pattern for tests/setup).
    /// Note: This consumes self and returns a new server, simple for setup but not dynamic.
    ///
    /// # Panics
    /// Panics if a route is added after the server has started and shared its routes `Arc`.
    #[must_use]
    pub fn add_route<F>(mut self, path: &str, handler: F) -> Self
    where
        F: Fn(&HttpRequest) -> HttpResponse + Send + Sync + 'static,
    {
        let routes = Arc::get_mut(&mut self.routes).expect("Cannot add route after server start");
        routes.insert(path.to_string(), Arc::new(handler));
        self
    }

    /// Handles a single TCP connection.
    fn handle_connection(mut stream: TcpStream, routes: &Arc<RouteMap>) {
        let mut buffer = [0; 1024];

        // PRODUCTION NOTE: Real servers read in loops to handle large requests,
        // use timeouts, and parse incrementally. We just do a single block read.
        if let Ok(bytes_read) = stream.read(&mut buffer) {
            if bytes_read == 0 {
                return;
            }

            let response = match HttpRequest::parse(&buffer[..bytes_read]) {
                Ok(request) => routes
                    .get(&request.path)
                    .map_or_else(HttpResponse::not_found, |handler| handler(&request)),
                Err(_) => HttpResponse::bad_request(),
            };

            let _ = stream.write_all(&response.to_bytes());
            let _ = stream.flush();
        }
    }

    /// Starts listening for connections (Blocking).
    ///
    /// # Panics
    /// Panics if the address cannot be bound.
    pub fn run(&self) {
        let listener = TcpListener::bind(&self.address).unwrap();

        for stream in listener.incoming().flatten() {
            let routes_clone = Arc::clone(&self.routes);
            self.pool.execute(move || {
                Self::handle_connection(stream, &routes_clone);
            });
        }
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `hyper`: Uses async/await (Tokio) and state machines (httparse) for non-blocking, zero-allocation parsing.
// - `actix-web`/`axum`: Provide rich routing, middleware, and type-safe extractors on top of `hyper`.
//
// Missing vs. Production:
// - **Zero-Allocation Parsing**: We allocate strings heavily (`to_string()`, `split`, etc.).
// - **Keep-Alive**: Connections are immediately closed (no `Connection: keep-alive` support).
// - **Chunked Encoding**: No support for large bodies or streams.
// - **Async/Await**: Uses OS threads which don't scale to 10k concurrent connections (C10K problem).
//
// Next Steps:
// 1. Swap `String` parsing for `httparse` to avoid allocations.
// 2. Refactor to use the async runtime built previously.
//
// Benchmarking Note:
// To benchmark the server's throughput, use a tool like `wrk` or `oha`.
// Locally, you can spin up the server and run: `wrk -t4 -c100 -d10s http://localhost:8080/`
// to measure requests per second and observe thread contention in the pool.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_request_parsing() {
        let raw = b"GET /hello HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let req = HttpRequest::parse(raw).unwrap();

        assert_eq!(req.method, HttpMethod::Get);
        assert_eq!(req.path, "/hello");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(
            req.headers.get("Host").map(std::string::String::as_str),
            Some("localhost")
        );
    }

    #[test]
    fn test_http_request_with_body() {
        let raw = b"POST /submit HTTP/1.1\r\nContent-Length: 5\r\n\r\nhello";
        let req = HttpRequest::parse(raw).unwrap();

        assert_eq!(req.method, HttpMethod::Post);
        assert_eq!(req.body, "hello");
    }

    #[test]
    fn test_malformed_request() {
        let raw = b"MALFORMED_GARBAGE";
        assert!(HttpRequest::parse(raw).is_err());
    }

    #[test]
    fn test_http_response_generation() {
        let res = HttpResponse::ok("Hello World");
        let bytes = res.to_bytes();
        let s = std::str::from_utf8(&bytes).unwrap();

        assert!(s.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(s.contains("Content-Length: 11\r\n"));
        assert!(s.ends_with("\r\n\r\nHello World"));
    }
}
