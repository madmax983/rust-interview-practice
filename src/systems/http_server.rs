//! # HTTP Server Implementation
//!
//! Implements a minimal multithreaded HTTP/1.1 server from scratch without external dependencies.
//! Includes request parsing, threading models (thread pool), and response formatting.
//!
//! **Replaces Crates:** `hyper`, `actix-web`, `axum`
//!
//! **Real-world Usage:**
//! - Web applications and APIs
//! - Microservices communication
//! - CDN edge nodes handling raw HTTP traffic
//!
//! **Why build it yourself?**
//! Understanding how raw bytes over a TCP stream translate into structured HTTP Requests
//! and Responses demystifies the web. Building a thread pool for handling concurrent
//! connections teaches you about synchronization primitives (`Mutex`, channels) and
//! the blocking nature of synchronous I/O.
//!
//! # Architecture
//!
//! ```text
//! Client(s)                  Server                     Thread Pool
//!   │                          │                             │
//!   ├─── TCP Connection ──────►│                             │
//!   │                          │─── Job (TCP Stream) ───────►│
//!   │                          │                             │─── Thread 1 (Parsing & Handling)
//!   ├─── TCP Connection ──────►│─── Job (TCP Stream) ───────►│
//!   │                          │                             │─── Thread 2 (Parsing & Handling)
//! ```
//!
//! **Invariants:**
//! 1. The ThreadPool maintains a fixed number of worker threads.
//! 2. Requests are parsed according to a simplified HTTP/1.1 specification.
//! 3. Responses are properly formatted with standard HTTP/1.1 status lines and headers.
//!
//! **Time/Space Complexity:**
//! - **Request Parsing:** O(N) where N is the length of the request headers and body. Space: O(N) to store parsed data.
//! - **Request Handling:** Depends on the specific handler.
//!
//! **Benchmarking Notes:**
//! This server can be benchmarked using tools like `wrk` or `hey`. For microbenchmarking the parser,
//! use `criterion` or `std::hint::black_box` with `std::time::Instant` to measure bytes parsed per microsecond.
//!
//! **Design Decisions and Tradeoffs:**
//! - **Blocking I/O:** This implementation uses synchronous I/O and standard threads. While simpler to understand,
//!   it doesn't scale to thousands of concurrent connections (C10k problem) like asynchronous I/O (epoll/kqueue) does.
//! - **Request Parsing:** We parse everything into a `Request` struct in memory. Production servers might stream
//!   large bodies or use zero-copy parsing to avoid allocations.
//! - **Thread Pool:** Uses a `mpsc` channel to distribute work. `Mutex` is used to allow multiple workers to receive from the channel.

use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;

// =========================================================================================
// Thread Pool Implementation
// =========================================================================================

type Job = Box<dyn FnOnce() + Send + 'static>;

/// A worker in the thread pool that executes jobs.
struct Worker {
    _id: usize,
    thread: Option<thread::JoinHandle<()>>,
}

impl Worker {
    /// Creates a new Worker.
    fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Job>>>) -> Self {
        // RUST INSIGHT: We use `Arc<Mutex<Receiver>>` because the receiver must be shared
        // among multiple threads. In safe Rust, `mpsc::Receiver` does not implement `Sync`,
        // meaning we need a `Mutex` to safely share it and pull messages concurrently.
        let thread = thread::spawn(move || {
            loop {
                let message = receiver.lock().expect("Mutex poisoned").recv();

                match message {
                    Ok(job) => {
                        // GOTCHA: It's important to drop the Mutex guard before executing the job.
                        // If we held the lock while executing, the thread pool would become sequential!
                        // RUST INSIGHT: The explicit lock drop is naturally handled here since we
                        // don't keep the guard around, but in complex scenarios `drop(guard)` might be needed.
                        job();
                    }
                    Err(_) => {
                        // Channel disconnected, meaning the pool is shutting down
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

/// A Thread Pool for executing tasks concurrently.
pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: Option<mpsc::Sender<Job>>,
}

impl ThreadPool {
    /// Creates a new ThreadPool with the specified number of threads.
    ///
    /// # Panics
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

    /// Executes the given closure on a worker thread.
    ///
    /// # Panics
    /// Panics if the internal channel has been closed.
    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);
        if let Some(sender) = &self.sender {
            sender.send(job).expect("Failed to send job to worker");
        }
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        // Drop the sender to signal workers to stop waiting for new jobs
        drop(self.sender.take());

        for worker in &mut self.workers {
            if let Some(thread) = worker.thread.take() {
                // Wait for the worker to finish its current job
                let _ = thread.join();
            }
        }
    }
}

// =========================================================================================
// HTTP Request & Response Models
// =========================================================================================

/// HTTP Methods
#[derive(Debug, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Put,
    Delete,
    Unknown(String),
}

impl From<&str> for Method {
    fn from(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "GET" => Self::Get,
            "POST" => Self::Post,
            "PUT" => Self::Put,
            "DELETE" => Self::Delete,
            other => Self::Unknown(other.to_string()),
        }
    }
}

/// An HTTP Request
#[derive(Debug)]
pub struct Request {
    pub method: Method,
    pub path: String,
    pub version: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl Request {
    /// Parses an HTTP request from a stream.
    ///
    /// # Errors
    /// Returns an error if the request is malformed or reading fails.
    pub fn parse<R: Read>(stream: &mut BufReader<R>) -> Result<Self, String> {
        let mut request_line = String::new();
        stream
            .read_line(&mut request_line)
            .map_err(|e| format!("Failed to read request line: {e}"))?;

        let parts: Vec<&str> = request_line.split_whitespace().collect();
        if parts.len() != 3 {
            return Err("Invalid request line".to_string());
        }

        let method = Method::from(parts[0]);
        let path = parts[1].to_string();
        let version = parts[2].to_string();

        let mut headers = HashMap::new();
        loop {
            let mut header_line = String::new();
            stream
                .read_line(&mut header_line)
                .map_err(|e| format!("Failed to read header: {e}"))?;

            let trimmed = header_line.trim_end();
            if trimmed.is_empty() {
                break; // End of headers
            }

            if let Some(idx) = trimmed.find(':') {
                let key = trimmed[..idx].trim().to_lowercase();
                let value = trimmed[idx + 1..].trim().to_string();
                headers.insert(key, value);
            }
        }

        let mut body = Vec::new();
        if let Some(content_length) = headers
            .get("content-length")
            .and_then(|s| s.parse::<usize>().ok())
        {
            body.resize(content_length, 0);
            stream
                .read_exact(&mut body)
                .map_err(|e| format!("Failed to read body: {e}"))?;
        }

        Ok(Self {
            method,
            path,
            version,
            headers,
            body,
        })
    }
}

/// HTTP Status Codes
#[derive(Debug, Clone, Copy)]
pub enum StatusCode {
    Ok = 200,
    BadRequest = 400,
    NotFound = 404,
    InternalServerError = 500,
}

impl StatusCode {
    #[must_use]
    pub const fn reason_phrase(self) -> &'static str {
        match self {
            Self::Ok => "OK",
            Self::BadRequest => "Bad Request",
            Self::NotFound => "Not Found",
            Self::InternalServerError => "Internal Server Error",
        }
    }
}

impl Display for StatusCode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", *self as u16, self.reason_phrase())
    }
}

/// An HTTP Response
#[derive(Debug)]
pub struct Response {
    pub status: StatusCode,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl Response {
    #[must_use]
    pub fn new(status: StatusCode, body: impl Into<Vec<u8>>) -> Self {
        let body = body.into();
        let mut headers = HashMap::new();
        headers.insert("Content-Length".to_string(), body.len().to_string());

        Self {
            status,
            headers,
            body,
        }
    }

    /// Serializes the response into raw bytes for sending over a stream.
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        let mut res = format!("HTTP/1.1 {}\r\n", self.status).into_bytes();

        for (key, value) in self.headers {
            res.extend_from_slice(format!("{key}: {value}\r\n").as_bytes());
        }

        res.extend_from_slice(b"\r\n");
        res.extend_from_slice(&self.body);

        res
    }
}

// =========================================================================================
// Server Implementation
// =========================================================================================

/// Handler trait for processing requests
pub trait Handler: Send + Sync {
    fn handle(&self, req: Request) -> Response;
}

/// A simple function-based handler
pub struct FnHandler<F>(pub F);

impl<F> Handler for FnHandler<F>
where
    F: Fn(Request) -> Response + Send + Sync,
{
    fn handle(&self, req: Request) -> Response {
        (self.0)(req)
    }
}

/// A minimal multithreaded HTTP server
pub struct HttpServer {
    pool: ThreadPool,
    handler: Arc<dyn Handler>,
}

impl HttpServer {
    /// Creates a new HttpServer with the specified worker count and request handler.
    #[must_use]
    pub fn new(workers: usize, handler: impl Handler + 'static) -> Self {
        Self {
            pool: ThreadPool::new(workers),
            handler: Arc::new(handler),
        }
    }

    /// Handles a single client connection.
    ///
    /// # Panics
    /// Panics if the stream cannot be cloned.
    fn handle_connection(mut stream: TcpStream, handler: Arc<dyn Handler>) {
        let request = {
            let mut reader = BufReader::new(&mut stream);
            Request::parse(&mut reader)
        };

        match request {
            Ok(req) => {
                let response = handler.handle(req);
                let _ = stream.write_all(&response.into_bytes());
            }
            Err(_) => {
                let response = Response::new(StatusCode::BadRequest, "Bad Request");
                let _ = stream.write_all(&response.into_bytes());
            }
        }
        let _ = stream.flush();
    }

    /// Starts the server on the specified address.
    /// This method blocks indefinitely.
    ///
    /// # Panics
    /// Panics if the server fails to bind to the address.
    pub fn run(&self, addr: &str) {
        let listener = TcpListener::bind(addr).expect("Failed to bind to address");

        // PRODUCTION NOTE: In a production server, we wouldn't block indefinitely without
        // a graceful shutdown mechanism (e.g., using an AtomicBool or cancellation token).
        for stream in listener.incoming().flatten() {
            let handler = Arc::clone(&self.handler);
            self.pool.execute(move || {
                Self::handle_connection(stream, handler);
            });
        }
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `hyper`: Uses asynchronous I/O (`tokio`) for high scalability, handles chunked encoding, HTTP/2, and is highly optimized for zero-copy where possible.
// - `actix-web` / `axum`: Build on top of `hyper` (or similar) to provide routing, middleware, state management, and declarative handler macros.
//
// What's missing vs. production:
// - Asynchronous I/O (epoll/kqueue) for C10k+ scaling.
// - Pipelining, Keep-Alive, HTTP/2 or HTTP/3 support.
// - Streaming bodies (we load the entire body into memory).
// - Robust error handling (e.g., timeouts, preventing Slowloris attacks).
// - Advanced routing and middleware.
//
// Suggested next steps:
// 1. Swap the synchronous thread pool with an asynchronous reactor (epoll).
// 2. Add an HTTP router to map specific paths to specific handlers.

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_thread_pool() {
        let pool = ThreadPool::new(4);
        let counter = Arc::new(Mutex::new(0));

        for _ in 0..10 {
            let counter = Arc::clone(&counter);
            pool.execute(move || {
                let mut num = counter.lock().unwrap();
                *num += 1;
            });
        }

        // We can't deterministically wait for all jobs to finish without a barrier or
        // shutting down the pool. Dropping the pool will wait for all workers.
        drop(pool);

        assert_eq!(*counter.lock().unwrap(), 10);
    }

    #[test]
    fn test_request_parsing() {
        let raw_request =
            b"GET /hello HTTP/1.1\r\nHost: localhost\r\nUser-Agent: curl/7.68.0\r\n\r\n";
        let mut reader = BufReader::new(Cursor::new(raw_request));

        let req = Request::parse(&mut reader).unwrap();

        assert_eq!(req.method, Method::Get);
        assert_eq!(req.path, "/hello");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.get("host").unwrap(), "localhost");
        assert_eq!(req.headers.get("user-agent").unwrap(), "curl/7.68.0");
        assert!(req.body.is_empty());
    }

    #[test]
    fn test_request_parsing_with_body() {
        let raw_request = b"POST /submit HTTP/1.1\r\nContent-Length: 11\r\n\r\nHello World";
        let mut reader = BufReader::new(Cursor::new(raw_request));

        let req = Request::parse(&mut reader).unwrap();

        assert_eq!(req.method, Method::Post);
        assert_eq!(req.body, b"Hello World");
    }

    #[test]
    fn test_response_formatting() {
        let res = Response::new(StatusCode::Ok, "Success");
        let bytes = res.into_bytes();
        let res_str = String::from_utf8(bytes).unwrap();

        assert!(res_str.contains("HTTP/1.1 200 OK\r\n"));
        assert!(res_str.contains("Content-Length: 7\r\n"));
        assert!(res_str.ends_with("\r\nSuccess"));
    }

    #[test]
    fn test_handler() {
        let handler = FnHandler(|req: Request| {
            if req.path == "/test" {
                Response::new(StatusCode::Ok, "Test passed")
            } else {
                Response::new(StatusCode::NotFound, "Not Found")
            }
        });

        let req1 = Request {
            method: Method::Get,
            path: "/test".to_string(),
            version: "HTTP/1.1".to_string(),
            headers: HashMap::new(),
            body: Vec::new(),
        };

        let res1 = handler.handle(req1);
        assert_matches_status(res1.status, StatusCode::Ok);

        let req2 = Request {
            method: Method::Get,
            path: "/unknown".to_string(),
            version: "HTTP/1.1".to_string(),
            headers: HashMap::new(),
            body: Vec::new(),
        };

        let res2 = handler.handle(req2);
        assert_matches_status(res2.status, StatusCode::NotFound);
    }

    fn assert_matches_status(status: StatusCode, expected: StatusCode) {
        assert_eq!(status as u16, expected as u16);
    }
}
