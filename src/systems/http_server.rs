//! # HTTP Server Implementation
//!
//! Implements a minimal multithreaded HTTP/1.1 server from scratch.
//!
//! **Replaces Crates:** `hyper`, `actix-web`, `axum`
//!
//! **Real-world Usage:**
//! - Web backend services
//! - Reverse proxies (Nginx/Envoy internals)
//! - Embedded devices exposing a web interface
//!
//! **Why build it yourself?**
//! Understanding HTTP from the TCP socket up demystifies web frameworks. You'll learn
//! how bytes on a wire become structured requests, how a thread pool manages concurrent
//! connections without spawning unbounded threads, and how routing maps URLs to code.
//!
//! # Architecture
//!
//! ```text
//! Client      TcpListener          ThreadPool (Workers)
//!   │              │                     │
//!   ├─── TCP ─────►│─── Accept ─────────►│
//!   │              │                     ├──► Read HTTP Request (BufReader)
//!   │              │                     ├──► Route to Handler
//!   │◄── HTTP ─────│◄── Write Response ──┤
//!   │              │                     │
//! ```
//!
//! **Invariants:**
//! 1. The `ThreadPool` maintains exactly `N` worker threads.
//! 2. Incoming connections are placed in a channel queue; if full, TCP backlog handles it.
//! 3. Responses are always well-formed HTTP/1.1 (CRLF terminated headers).
//!
//! **Time/Space Complexity:**
//! - **Accept Connection:** O(1) Time, O(1) Space
//! - **Parse Request:** O(N) Time (bytes read), O(N) Space (allocating headers/body)
//! - **Route Match:** O(M) Time (path string comparison), O(1) Space
//!
//! **Design Decisions and Tradeoffs:**
//! - Uses blocking I/O with a thread pool instead of async I/O. Async (like `tokio`)
//!   scales better to 10k+ connections, but OS threads + blocking I/O is simpler
//!   and perfectly fine for hundreds of concurrent requests.
//! - Request parsing reads headers eagerly into a `HashMap`. Production servers often
//!   use zero-copy parsers (like `httparse`) holding references to a pre-allocated buffer.
//!
//! # Footer
//!
//! - **Canonical Crates:** `hyper` for low-level HTTP, `actix-web`/`axum` for high-level frameworks.
//! - **Missing Features:** Keep-alive, chunked transfer encoding, zero-copy parsing, HTTP/2 multiplexing.
//! - **Next Steps:** Implement keep-alive by looping the request parser over the same stream until the client closes it. Implement a zero-copy parser.
//! - **Benchmarking:** To benchmark this HTTP server, one could use a load testing tool like `wrk` or `hey` against a running instance. Alternatively, one could use `std::hint::black_box` and `std::time::Instant::now()` in a tight loop within a test to measure request parsing and routing overhead in isolation.

#![allow(clippy::module_name_repetitions)]

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;

/// The HTTP Method of a request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Unsupported(String),
}

impl HttpMethod {
    /// Returns the string representation of the HTTP method.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Delete => "DELETE",
            Self::Unsupported(s) => s,
        }
    }
}

/// A parsed HTTP Request.
#[derive(Debug, Clone)]
pub struct Request {
    pub method: HttpMethod,
    pub path: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl Request {
    /// Parses an HTTP request from a TCP stream.
    ///
    /// # Errors
    /// Returns an error if the request format is invalid or reading from the stream fails.
    pub fn parse(stream: &mut TcpStream) -> std::io::Result<Option<Self>> {
        // GOTCHA:
        // We use a BufReader to read line-by-line efficiently.
        // However, we must explicitly drop it after parsing so we can reclaim
        // the mutable reference to `TcpStream` for writing the response later.
        let mut reader = BufReader::new(stream);
        let mut request_line = String::new();
        let bytes_read = reader.read_line(&mut request_line)?;

        if bytes_read == 0 {
            drop(reader);
            return Ok(None);
        }

        let parts: Vec<&str> = request_line.split_whitespace().collect();
        if parts.len() != 3 {
            drop(reader);
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid request line",
            ));
        }

        let method = match parts[0] {
            "GET" => HttpMethod::Get,
            "POST" => HttpMethod::Post,
            "PUT" => HttpMethod::Put,
            "DELETE" => HttpMethod::Delete,
            other => HttpMethod::Unsupported(other.to_string()),
        };

        let path = parts[1].to_string();
        let mut headers = HashMap::new();

        loop {
            let mut line = String::new();
            reader.read_line(&mut line)?;
            if line == "\r\n" || line == "\n" || line.is_empty() {
                break;
            }
            if let Some((key, value)) = line.split_once(':') {
                headers.insert(key.trim().to_string(), value.trim().to_string());
            }
        }

        let mut body = Vec::new();
        // PRODUCTION NOTE:
        // A production HTTP server must limit the maximum body size to prevent
        // Out-Of-Memory (OOM) Denial-of-Service attacks from clients sending massive
        // `Content-Length` headers. We blindly allocate here for simplicity.
        if let Some(len) = headers
            .get("Content-Length")
            .and_then(|s| s.parse::<usize>().ok())
        {
            body.resize(len, 0);
            reader.read_exact(&mut body)?;
        }

        drop(reader);

        Ok(Some(Self {
            method,
            path,
            headers,
            body,
        }))
    }
}

/// An HTTP Response to be sent to the client.
#[derive(Debug, Clone)]
pub struct Response {
    pub status_code: u16,
    pub status_text: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl Default for Response {
    fn default() -> Self {
        Self::new(200, "OK")
    }
}

impl Response {
    /// Creates a new HTTP response.
    #[must_use]
    pub fn new(status_code: u16, status_text: &str) -> Self {
        Self {
            status_code,
            status_text: status_text.to_string(),
            headers: HashMap::new(),
            body: Vec::new(),
        }
    }

    /// Writes the HTTP response to a TCP stream.
    ///
    /// # Errors
    /// Returns an error if writing to the stream fails.
    pub fn write_to(&self, stream: &mut TcpStream) -> std::io::Result<()> {
        let mut response_bytes = Vec::new();

        write!(
            response_bytes,
            "HTTP/1.1 {} {}\r\n",
            self.status_code, self.status_text
        )?;

        for (key, value) in &self.headers {
            write!(response_bytes, "{key}: {value}\r\n")?;
        }

        write!(response_bytes, "Content-Length: {}\r\n", self.body.len())?;
        write!(response_bytes, "\r\n")?;
        response_bytes.write_all(&self.body)?;

        stream.write_all(&response_bytes)?;
        stream.flush()?;
        Ok(())
    }
}

/// A trait for processing an HTTP request and returning a response.
pub trait Handler: Send + Sync + 'static {
    fn handle(&self, req: &Request) -> Response;
}

// Implement `Handler` for any closure that matches the signature.
impl<F> Handler for F
where
    F: Fn(&Request) -> Response + Send + Sync + 'static,
{
    fn handle(&self, req: &Request) -> Response {
        (self)(req)
    }
}

/// HTTP Router mapping routes to handlers.
#[derive(Default)]
pub struct Router {
    // RUST INSIGHT:
    // Using `Arc<dyn Handler>` allows dynamic dispatch for handlers of different
    // types (functions, closures, structs) while ensuring they are thread-safe (`Send + Sync`).
    routes: HashMap<String, Arc<dyn Handler>>,
}

impl Router {
    /// Creates a new Router.
    #[must_use]
    pub fn new() -> Self {
        Self {
            routes: HashMap::new(),
        }
    }

    /// Adds a route to the router.
    pub fn add_route<H: Handler>(&mut self, method: &HttpMethod, path: &str, handler: H) {
        let key = format!("{} {path}", method.as_str());
        self.routes.insert(key, Arc::new(handler));
    }

    /// Routes an incoming request to the appropriate handler.
    #[must_use]
    pub fn route(&self, req: &Request) -> Response {
        let key = format!("{} {}", req.method.as_str(), req.path);
        self.routes.get(&key).map_or_else(|| Response::new(404, "Not Found"), |handler| handler.handle(req))
    }
}

type Job = Box<dyn FnOnce() + Send + 'static>;

/// A simple thread pool for handling concurrent requests.
pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: Option<mpsc::Sender<Job>>,
}

impl ThreadPool {
    /// Creates a new `ThreadPool`.
    ///
    /// # Panics
    ///
    /// Panics if `size` is 0.
    #[must_use]
    pub fn new(size: usize) -> Self {
        assert!(size > 0, "ThreadPool size must be greater than 0");

        let (sender, receiver) = mpsc::channel();
        let receiver = Arc::new(Mutex::new(receiver));

        let mut workers = Vec::with_capacity(size);
        for id in 0..size {
            workers.push(Worker::new(id, Arc::clone(&receiver)));
        }

        Self {
            workers,
            sender: Some(sender),
        }
    }

    /// Executes a job on the thread pool.
    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);
        if let Some(sender) = &self.sender {
            // Ignore send errors if workers have panicked/shut down
            let _ = sender.send(job);
        }
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        // Drop sender so workers' recv() fails and they exit
        drop(self.sender.take());

        for worker in &mut self.workers {
            if let Some(thread) = worker.thread.take() {
                let _ = thread.join();
            }
        }
    }
}

struct Worker {
    _id: usize,
    thread: Option<thread::JoinHandle<()>>,
}

impl Worker {
    fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Job>>>) -> Self {
        let thread = thread::spawn(move || {
            loop {
                let message = {
                    // RUST INSIGHT:
                    // Using a Mutex to protect the Receiver ensures only one thread
                    // pulls a job at a time. The lock is immediately dropped after recv()
                    // unblocks, allowing other threads to wait.
                    let guard = receiver.lock().unwrap();
                    let msg = guard.recv();
                    // clippy::significant_drop_tightening requires explicit drop
                    drop(guard);
                    msg
                };

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
            _id: id,
            thread: Some(thread),
        }
    }
}

/// A minimal HTTP Server.
pub struct HttpServer {
    router: Arc<Router>,
    thread_pool: ThreadPool,
}

impl HttpServer {
    /// Creates a new `HttpServer`.
    #[must_use]
    pub fn new(router: Router, threads: usize) -> Self {
        Self {
            router: Arc::new(router),
            thread_pool: ThreadPool::new(threads),
        }
    }

    /// Handles a single incoming TCP connection.
    pub fn handle_connection(router: &Arc<Router>, mut stream: TcpStream) {
        match Request::parse(&mut stream) {
            Ok(Some(req)) => {
                let response = router.route(&req);
                let _ = response.write_to(&mut stream);
            }
            Ok(None) => {}
            Err(_) => {
                let response = Response::new(400, "Bad Request");
                let _ = response.write_to(&mut stream);
            }
        }
    }

    /// Runs the server on the provided TCP listener.
    ///
    /// # Errors
    /// Returns an error if accepting a connection fails.
    pub fn run_with_listener(self, listener: &TcpListener) -> std::io::Result<()> {
        for stream in listener.incoming() {
            let stream = stream?;
            let router = Arc::clone(&self.router);

            self.thread_pool.execute(move || {
                Self::handle_connection(&router, stream);
            });
        }
        Ok(())
    }

    /// Binds the server to an address and runs it.
    ///
    /// # Errors
    /// Returns an error if binding to the address fails.
    pub fn run(self, addr: &str) -> std::io::Result<()> {
        let listener = TcpListener::bind(addr)?;
        self.run_with_listener(&listener)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_server_routing() {
        let mut router = Router::new();
        router.add_route(&HttpMethod::Get, "/hello", |_req: &Request| {
            let mut res = Response::new(200, "OK");
            res.body = b"Hello, World!".to_vec();
            res
        });

        router.add_route(&HttpMethod::Post, "/echo", |req: &Request| {
            let mut res = Response::new(200, "OK");
            res.body = req.body.clone();
            res
        });

        let server = HttpServer::new(router, 2);

        // Bind to an ephemeral port
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        // Spawn server thread
        std::thread::spawn(move || {
            let _ = server.run_with_listener(&listener);
        });

        // Give server a moment to start (though binding is already complete)
        std::thread::sleep(Duration::from_millis(50));

        // Make GET request
        let mut client = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
        client
            .write_all(b"GET /hello HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .unwrap();

        let mut response = String::new();
        client.read_to_string(&mut response).unwrap();

        assert!(response.contains("HTTP/1.1 200 OK"));
        assert!(response.contains("Hello, World!"));

        // Make POST request
        let mut client = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
        client
            .write_all(b"POST /echo HTTP/1.1\r\nContent-Length: 4\r\n\r\nEcho")
            .unwrap();

        let mut response = String::new();
        client.read_to_string(&mut response).unwrap();

        assert!(response.contains("HTTP/1.1 200 OK"));
        assert!(response.contains("Echo"));

        // Make 404 request
        let mut client = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
        client
            .write_all(b"GET /missing HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .unwrap();

        let mut response = String::new();
        client.read_to_string(&mut response).unwrap();

        assert!(response.contains("HTTP/1.1 404 Not Found"));
    }

    #[test]
    fn test_threadpool_shutdown() {
        let pool = ThreadPool::new(4);
        let counter = Arc::new(Mutex::new(0));

        for _ in 0..10 {
            let c = Arc::clone(&counter);
            pool.execute(move || {
                let mut num = c.lock().unwrap();
                *num += 1;
            });
        }

        // wait briefly for threads to process
        std::thread::sleep(Duration::from_millis(50));
        drop(pool); // explicitly drop to test shutdown

        assert_eq!(*counter.lock().unwrap(), 10);
    }
}
