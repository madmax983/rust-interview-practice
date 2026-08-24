//! # HTTP Server Implementation
//!
//! Implements a minimal multithreaded HTTP/1.1 server from scratch without external dependencies,
//! including request parsing, threading models, and response formatting.
//!
//! **Replaces Crates:** `hyper`, `actix-web`, `axum` (fundamental core)
//!
//! **Real-world Usage:**
//! - Web backend frameworks
//! - API gateways and reverse proxies (e.g., Nginx, Envoy)
//! - Microservice communication (REST APIs)
//!
//! **Why build it yourself?**
//! Building an HTTP server from scratch demystifies the magic of modern web frameworks.
//! You learn exactly how raw bytes from a TCP socket are parsed into structured requests,
//! how concurrency is managed via a thread pool to avoid blocking, and how to construct
//! valid HTTP responses. It also highlights Rust's strengths in zero-cost abstractions,
//! string handling, and safe concurrency.
//!
//! // =========================================================================================
//! // Architecture
//! // =========================================================================================
//!
//! Data Structure:
//!
//! //!     ┌─────────────────┐  //!     ┌──────────────┐  //!     ┌─────────────┐
//! //!     │   TCP Listener  │ ───► │  Thread Pool │ ───► │   Workers   │
//! //!     └─────────────────┘  //!     └──────────────┘  //!     └─────────────┘
//!                                                       //!     │
//! //!     ┌─────────────────┐  //!     ┌──────────────┐  //!     ┌─────────────┐
//! //!     │ HTTP Response   │ ◄─── │    Router//!     │ ◄─── │ HTTP Request│
//! //!     └─────────────────┘  //!     └──────────────┘  //!     └─────────────┘
//!
//! Invariants:
//! 1. The server must handle multiple concurrent connections without blocking the main thread.
//! 2. Requests must be parsed correctly according to the HTTP/1.1 specification (basic subset).
//! 3. Malformed requests should result in appropriate error responses (e.g., 400 Bad Request).
//! 4. The thread pool must shut down gracefully, joining all worker threads.
//!
//! Complexity:
//! ┌───────────────┬───────────┬────────┐
//! │ Operation //!     │ Time  //!     │ Space  │
//! ├───────────────┼───────────┼────────┤
//! │ Parse Request │ O(N)  //!     │ O(N)   │ N = request length
//! │ Route Request │ O(1)  //!     │ O(1)   │ Assuming simple path matching
//! │ Handle Conn   │ O(1)* //!     │ O(N)   │ *Excluding user handler time
//! └───────────────┴───────────┴────────┘
//!
//! Design Decisions and Tradeoffs:
//! - **Thread Pool vs. Async:** This implementation uses a classic thread pool (OS threads)
//!   for simplicity and to demonstrate concurrent shared state. Production servers in Rust
//!   typically use asynchronous I/O (e.g., `tokio`) for much higher concurrency with lower
//!   overhead, at the cost of significantly higher complexity.
//! - **String Parsing:** We do basic string splitting for parsing. A production server would
//!   use a highly optimized parser (like `httparse`) to avoid unnecessary allocations and
//!   handle complex HTTP/1.1 edge cases.
//! - **Routing:** Simple trait-based routing is used. Modern frameworks use advanced
//!   macro-based or type-safe routing.
//!
//! // =========================================================================================
//! // Canonical Comparisons & Missing Features
//! // =========================================================================================
//!
//! - **`hyper`:** The canonical low-level HTTP library in Rust. It is asynchronous, highly
//!   optimized, and supports HTTP/2 and HTTP/3. Our implementation is synchronous and HTTP/1.1 only.
//! - **Missing Features:**
//!   - Keep-Alive connections (we close after one request).
//!   - Chunked transfer encoding.
//!   - Advanced header parsing and validation.
//!   - TLS/HTTPS support.
//!   - Streaming request/response bodies.

use std::collections::HashMap;
use std::fmt;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

// =========================================================================================
// Thread Pool
// =========================================================================================

/// A job to be executed by the thread pool.
type Job = Box<dyn FnOnce() + Send + 'static>;

/// A worker in the thread pool that executes jobs.
struct Worker {
    _id: usize,
    thread: Option<thread::JoinHandle<()>>,
}

impl Worker {
    fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Job>>>) -> Self {
        // RUST INSIGHT: We use `thread::spawn` with a closure that loops continuously,
        // waiting for jobs from the channel.
        let thread = thread::spawn(move || loop {
            // GOTCHA: We must lock the receiver, receive the job, and drop the lock
            // *before* executing the job to allow other workers to receive jobs concurrently.
            // If we hold the lock during execution, the thread pool becomes effectively single-threaded.
            let message = receiver.lock().unwrap().recv();

            match message {
                Ok(job) => {
                    // println!("Worker {id} got a job; executing.");
                    job();
                }
                Err(_) => {
                    // println!("Worker {id} disconnected; shutting down.");
                    break;
                }
            }
        });

        Worker {
        _id: id,
            thread: Some(thread),
        }
    }
}

/// A simple thread pool for concurrent request handling.
pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: Option<mpsc::Sender<Job>>,
}

impl ThreadPool {
    /// Creates a new ThreadPool.
    ///
    /// # Panics
    ///
    /// Panics if `size` is 0.
    #[must_use]
    pub fn new(size: usize) -> Self {
        assert!(size > 0, "ThreadPool size must be greater than 0");

        let (sender, receiver) = mpsc::channel();
        // RUST INSIGHT: `Arc<Mutex<T>>` allows multiple threads to safely share ownership
        // and access the single receiver endpoint of the channel.
        let receiver = Arc::new(Mutex::new(receiver));

        let mut workers = Vec::with_capacity(size);

        for id in 0..size {
            workers.push(Worker::new(id, Arc::clone(&receiver)));
        }

        ThreadPool {
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
        // We use unwrap here because if the receiver is dropped, we want the thread pool to fail.
        self.sender.as_ref().unwrap().send(job).unwrap();
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        // Drop the sender to close the channel and signal workers to shut down.
        drop(self.sender.take());

        for worker in &mut self.workers {
            // println!("Shutting down worker {}", worker.id);

            // RUST INSIGHT: We take the thread handle out of the Option, leaving None in its place,
            // so we can call `join()` on it. This consumes the handle safely.
            if let Some(thread) = worker.thread.take() {
                thread.join().unwrap();
            }
        }
    }
}

// =========================================================================================
// HTTP Protocol Definitions
// =========================================================================================

/// HTTP Methods
#[derive(Debug, PartialEq, Eq, Clone)]
#[derive(Hash)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Unsupported(String),
}

impl From<&str> for HttpMethod {
    fn from(s: &str) -> Self {
        match s {
            "GET" => HttpMethod::Get,
            "POST" => HttpMethod::Post,
            "PUT" => HttpMethod::Put,
            "DELETE" => HttpMethod::Delete,
            _ => HttpMethod::Unsupported(s.to_string()),
        }
    }
}

impl fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HttpMethod::Get => write!(f, "GET"),
            HttpMethod::Post => write!(f, "POST"),
            HttpMethod::Put => write!(f, "PUT"),
            HttpMethod::Delete => write!(f, "DELETE"),
            HttpMethod::Unsupported(s) => write!(f, "{s}"),
        }
    }
}

/// HTTP Status Codes
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum HttpStatus {
    Ok = 200,
    Created = 201,
    BadRequest = 400,
    NotFound = 404,
    InternalServerError = 500,
}

impl HttpStatus {
    #[must_use]
    pub const fn reason_phrase(self) -> &'static str {
        match self {
            HttpStatus::Ok => "OK",
            HttpStatus::Created => "Created",
            HttpStatus::BadRequest => "Bad Request",
            HttpStatus::NotFound => "Not Found",
            HttpStatus::InternalServerError => "Internal Server Error",
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
    pub body: Vec<u8>,
}

impl HttpRequest {
    /// Parses an HTTP request from a TCP stream.
    ///
    /// # Errors
    /// Returns an error if the request is malformed or the stream cannot be read.
    pub fn parse(stream: &mut TcpStream) -> Result<Self, String> {
        // PRODUCTION NOTE: This reads everything into a fixed buffer for simplicity.
        // A production server would dynamically size the buffer, handle chunked transfers,
        // and parse headers more efficiently without allocating strings for everything.
        let mut buffer = [0; 4096];
        let bytes_read = stream.read(&mut buffer).map_err(|e| e.to_string())?;

        if bytes_read == 0 {
            return Err("Empty request".to_string());
        }

        // RUST INSIGHT: We find the boundary using sliding windows on raw bytes.
        // This avoids bugs where invalid UTF-8 in headers alters the string length
        // when using `String::from_utf8_lossy`, causing a panic on slice indices.
        let header_body_separator = b"\r\n\r\n";
        let sep_index = buffer[..bytes_read]
            .windows(4)
            .position(|window| window == header_body_separator)
            .ok_or("Invalid HTTP request format")?;

        let header_bytes = &buffer[..sep_index];
        let body_start = sep_index + 4;
        let body = buffer[body_start..bytes_read].to_vec();

        let header_section = String::from_utf8_lossy(header_bytes);
        let mut lines = header_section.lines();

        // Parse Request Line (e.g., "GET / HTTP/1.1")
        let request_line = lines.next().ok_or("Missing request line")?;
        let mut parts = request_line.split_whitespace();

        let method = parts.next().ok_or("Missing method")?.into();
        let path = parts.next().ok_or("Missing path")?.to_string();
        let version = parts.next().ok_or("Missing version")?.to_string();

        // Parse Headers
        let mut headers = HashMap::new();
        for line in lines {
            if let Some(colon_idx) = line.find(':') {
                let key = line[..colon_idx].trim().to_lowercase();
                let value = line[colon_idx + 1..].trim().to_string();
                headers.insert(key, value);
            }
        }

        Ok(HttpRequest {
            method,
            path,
            version,
            headers,
            body,
        })
    }
}

/// Represents an HTTP Response.
#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub version: String,
    pub status: HttpStatus,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl HttpResponse {
    #[must_use]
    pub fn new(status: HttpStatus) -> Self {
        HttpResponse {
            version: "HTTP/1.1".to_string(),
            status,
            headers: HashMap::new(),
            body: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_body(mut self, body: &str) -> Self {
        self.body = body.as_bytes().to_vec();
        self.headers.insert("Content-Length".to_string(), self.body.len().to_string());
        // Default content type
        self.headers.entry("Content-Type".to_string()).or_insert_with(|| "text/plain".to_string());
        self
    }

    #[must_use]
    pub fn with_header(mut self, key: &str, value: &str) -> Self {
        self.headers.insert(key.to_string(), value.to_string());
        self
    }

    /// Serializes the response and writes it to a stream.
    ///
    /// # Errors
    /// Returns an error if writing to the stream fails.
    pub fn write_to(&self, stream: &mut TcpStream) -> std::io::Result<()> {
        let status_code = self.status as u16;
        let reason_phrase = self.status.reason_phrase();

        let mut response_string = format!("{} {} {}\r\n", self.version, status_code, reason_phrase);

        for (key, value) in &self.headers {
            response_string.push_str(&format!("{key}: {value}\r\n"));
        }

        response_string.push_str("\r\n");

        stream.write_all(response_string.as_bytes())?;
        stream.write_all(&self.body)?;
        stream.flush()?;

        Ok(())
    }
}

// =========================================================================================
// Router and Server
// =========================================================================================

/// Trait for handling HTTP requests.
pub trait RequestHandler: Send + Sync {
    fn handle(&self, request: &HttpRequest) -> HttpResponse;
}

impl<F> RequestHandler for F
where
    F: Fn(&HttpRequest) -> HttpResponse + Send + Sync,
{
    fn handle(&self, request: &HttpRequest) -> HttpResponse {
        self(request)
    }
}

/// A simple HTTP Server.
pub struct HttpServer {
    router: Arc<HashMap<(HttpMethod, String), Box<dyn RequestHandler>>>,
    not_found_handler: Arc<Box<dyn RequestHandler>>,
}

impl HttpServer {
    #[must_use]
    pub fn new() -> Self {
        HttpServer {
            router: Arc::new(HashMap::new()),
            not_found_handler: Arc::new(Box::new(|_req: &HttpRequest| -> HttpResponse {
                HttpResponse::new(HttpStatus::NotFound).with_body("404 Not Found")
            })),
        }
    }
}

impl Default for HttpServer {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for configuring the `HttpServer`.
pub struct HttpServerBuilder {
    router: HashMap<(HttpMethod, String), Box<dyn RequestHandler>>,
    not_found_handler: Option<Box<dyn RequestHandler>>,
}

impl HttpServerBuilder {
    #[must_use]
    pub fn new() -> Self {
        HttpServerBuilder {
            router: HashMap::new(),
            not_found_handler: None,
        }
    }

    #[must_use]
    pub fn route<H>(mut self, method: HttpMethod, path: &str, handler: H) -> Self
    where
        H: RequestHandler + 'static,
    {
        self.router.insert((method, path.to_string()), Box::new(handler));
        self
    }

    #[must_use]
    pub fn get<H>(self, path: &str, handler: H) -> Self
    where
        H: RequestHandler + 'static,
    {
        self.route(HttpMethod::Get, path, handler)
    }

    #[must_use]
    pub fn post<H>(self, path: &str, handler: H) -> Self
    where
        H: RequestHandler + 'static,
    {
        self.route(HttpMethod::Post, path, handler)
    }

    #[must_use]
    pub fn not_found<H>(mut self, handler: H) -> Self
    where
        H: RequestHandler + 'static,
    {
        self.not_found_handler = Some(Box::new(handler));
        self
    }

    #[must_use]
    pub fn build(self) -> HttpServer {
        let not_found: Box<dyn RequestHandler> = self.not_found_handler.unwrap_or_else(|| {
            Box::new(|_req: &HttpRequest| -> HttpResponse {
                HttpResponse::new(HttpStatus::NotFound).with_body("404 Not Found")
            })
        });

        HttpServer {
            router: Arc::new(self.router),
            not_found_handler: Arc::new(not_found),
        }
    }
}

impl Default for HttpServerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpServer {
    /// Starts the server on the specified address.
    ///
    /// # Errors
    /// Returns an error if it cannot bind to the address.
    pub fn listen<A: ToSocketAddrs>(&self, addr: A, workers: usize) -> std::io::Result<()> {
        let listener = TcpListener::bind(addr)?;
        let pool = ThreadPool::new(workers);

        // println!("Server listening...");

        for stream in listener.incoming() {
            let stream = stream?;

            // Clone Arcs to move into the thread
            let router_clone = Arc::clone(&self.router);
            let not_found_clone = Arc::clone(&self.not_found_handler);

            pool.execute(move || {
                Self::handle_connection(stream, router_clone, not_found_clone);
            });
        }

        Ok(())
    }

    fn handle_connection(
        mut stream: TcpStream,
        router: Arc<HashMap<(HttpMethod, String), Box<dyn RequestHandler>>>,
        not_found: Arc<Box<dyn RequestHandler>>,
    ) {
        match HttpRequest::parse(&mut stream) {
            Ok(request) => {
                let handler = router.get(&(request.method.clone(), request.path.clone()));

                let response = match handler {
                    Some(h) => h.handle(&request),
                    None => not_found.handle(&request),
                };

                if let Err(e) = response.write_to(&mut stream) {
                    eprintln!("Failed to write response: {e}");
                }
            }
            Err(e) => {
                // Return a 400 Bad Request
                let response = HttpResponse::new(HttpStatus::BadRequest)
                    .with_body(&format!("Bad Request: {e}"));
                let _ = response.write_to(&mut stream);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::net::TcpListener;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_http_method_display() {
        assert_eq!(HttpMethod::Get.to_string(), "GET");
        assert_eq!(HttpMethod::Post.to_string(), "POST");
        assert_eq!(HttpMethod::Unsupported("PATCH".to_string()).to_string(), "PATCH");
    }

    #[test]
    fn test_http_method_from_str() {
        assert_eq!(HttpMethod::from("GET"), HttpMethod::Get);
        assert_eq!(HttpMethod::from("POST"), HttpMethod::Post);
        assert_eq!(HttpMethod::from("PATCH"), HttpMethod::Unsupported("PATCH".to_string()));
    }

    #[test]
    fn test_http_status_reason_phrase() {
        assert_eq!(HttpStatus::Ok.reason_phrase(), "OK");
        assert_eq!(HttpStatus::NotFound.reason_phrase(), "Not Found");
    }

    #[test]
    fn test_thread_pool_execution() {
        let pool = ThreadPool::new(4);
        let counter = Arc::new(Mutex::new(0));

        for _ in 0..10 {
            let counter_clone = Arc::clone(&counter);
            pool.execute(move || {
                let mut num = counter_clone.lock().unwrap();
                *num += 1;
            });
        }

        // Wait a bit for threads to finish
        thread::sleep(Duration::from_millis(50));
        assert_eq!(*counter.lock().unwrap(), 10);
    }

    #[test]
    fn test_response_builder() {
        let response = HttpResponse::new(HttpStatus::Ok)
            .with_header("X-Custom", "Value")
            .with_body("Hello, World!");

        assert_eq!(response.status, HttpStatus::Ok);
        assert_eq!(response.headers.get("X-Custom").unwrap(), "Value");
        assert_eq!(response.headers.get("Content-Length").unwrap(), "13");
        assert_eq!(String::from_utf8_lossy(&response.body), "Hello, World!");
    }

    // Helper for testing parsing
    fn start_mock_server_and_send_request(request_str: &[u8]) -> Option<HttpRequest> {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        let (tx, rx) = mpsc::channel();

        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let req = HttpRequest::parse(&mut stream).ok();
                tx.send(req).unwrap();
            }
        });

        let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
        stream.write_all(request_str).unwrap();

        rx.recv_timeout(Duration::from_secs(1)).unwrap_or(None)
    }

    #[test]
    fn test_parse_valid_get_request() {
        let req_str = b"GET /hello HTTP/1.1\r\nHost: localhost\r\nUser-Agent: test\r\n\r\n";
        let req = start_mock_server_and_send_request(req_str).expect("Failed to parse request");

        assert_eq!(req.method, HttpMethod::Get);
        assert_eq!(req.path, "/hello");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.get("host").unwrap(), "localhost");
        assert!(req.body.is_empty());
    }

    #[test]
    fn test_parse_valid_post_request_with_body() {
        let req_str = b"POST /submit HTTP/1.1\r\nContent-Length: 11\r\n\r\nhello world";
        let req = start_mock_server_and_send_request(req_str).expect("Failed to parse request");

        assert_eq!(req.method, HttpMethod::Post);
        assert_eq!(req.path, "/submit");
        assert_eq!(req.body, b"hello world");
    }

    #[test]
    fn test_server_routing() {
        // We will start a real server on a random port for a quick integration test
        let server = HttpServerBuilder::new()
            .get("/test", |_req: &HttpRequest| {
                HttpResponse::new(HttpStatus::Ok).with_body("Test OK")
            })
            .build();

        // We use a channel to signal that the server is listening, avoiding flaky sleeps
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        // Since `server.listen` binds inside, we must drop it, but we retry if it fails
        drop(listener);

        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            // Give the OS a tiny moment to actually free the port just in case
            thread::sleep(Duration::from_millis(10));
            // In a real robust test, you'd want to use `TcpListener::bind` first, then pass it to the server
            // But since our API takes an address, we will just start it and let the test proceed.
            let _ = tx.send(());
            let _ = server.listen(format!("127.0.0.1:{port}"), 2);
        });

        let _ = rx.recv();
        thread::sleep(Duration::from_millis(50)); // Wait for server to start

        // Test GET /test
        let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
        stream.write_all(b"GET /test HTTP/1.1\r\n\r\n").unwrap();

        let mut buf = [0; 1024];
        let n = stream.read(&mut buf).unwrap();
        let response = String::from_utf8_lossy(&buf[..n]);
        assert!(response.contains("200 OK"));
        assert!(response.contains("Test OK"));

        // Test 404
        let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
        stream.write_all(b"GET /unknown HTTP/1.1\r\n\r\n").unwrap();

        let mut buf = [0; 1024];
        let n = stream.read(&mut buf).unwrap();
        let response = String::from_utf8_lossy(&buf[..n]);
        assert!(response.contains("404 Not Found"));
    }
}
