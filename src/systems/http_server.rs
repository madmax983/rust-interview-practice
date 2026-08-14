//! # HTTP/1.1 Server
//!
//! ## What this implements and what it replaces
//! This is a minimal, multithreaded HTTP/1.1 server implemented from scratch without external dependencies.
//! It replaces foundational crates like `hyper`, `actix-web` (at the core level), and `axum`.
//!
//! ## Real-world systems that use this
//! Almost every web application relies on an HTTP server. Real-world systems like Nginx, Apache, and Node.js
//! parse HTTP text protocols over TCP streams and route requests to handlers.
//!
//! ## Why build it yourself?
//! Building an HTTP server from scratch demystifies the magic of modern web frameworks. You learn how TCP streams
//! are read, how string parsing turns raw bytes into structured requests, how a thread pool manages concurrent connections,
//! and how traits enable swappable routing strategies.
//!
//! ## Architecture
//! ```text
//! Client        Server Thread Pool          Handler
//!   |                  |                       |
//!   |--- TCP Conn ---->|                       |
//!   |                  |--- parse request ---->|
//!   |                  |                       |--- generate response
//!   |<-- HTTP Resp ----|<-- format response ---|
//! ```
//!
//! ### Invariants
//! - The server must not panic on malformed client requests.
//! - The thread pool must efficiently distribute work without blocking the main listener thread.
//! - Handlers must be thread-safe (`Send` + `Sync`).
//!
//! ### Time / Space Complexity
//! - **Request Parsing**: Time: `O(N)` where `N` is the length of the HTTP request headers. Space: `O(N)` to store parsed headers and body.
//! - **Routing**: `O(1)` or `O(M)` depending on the routing mechanism (M being the number of registered routes if using simple matching).
//!
//! ### Design Decisions & Tradeoffs
//! - **Thread Pool vs Async**: We use a synchronous thread pool for simplicity and educational value. Production servers in Rust typically use `async`/`await` (e.g., Tokio) to handle thousands of concurrent connections on a single thread.
//! - **HTTP Parsing**: We use basic string manipulation. Production parsers like `httparse` use optimized state machines and SIMD.
//! - **String allocations**: We allocate new `String`s for headers and body. A zero-allocation parser would yield `&str` with lifetimes tied to the buffer, which is much more complex.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

/// Represents an HTTP Request.
#[derive(Debug, Clone)]
pub struct Request {
    pub method: String,
    pub path: String,
    pub version: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

/// Represents an HTTP Response.
#[derive(Debug, Clone)]
pub struct Response {
    pub status_code: u16,
    pub status_text: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl Response {
    /// Creates a new successful response.
    #[allow(clippy::missing_const_for_fn)]
    pub fn ok(body: impl Into<Vec<u8>>) -> Self {
        Self {
            status_code: 200,
            status_text: "OK".to_string(),
            headers: HashMap::new(),
            body: body.into(),
        }
    }

    /// Creates a new Not Found response.
    #[allow(clippy::missing_const_for_fn)]
    pub fn not_found() -> Self {
        Self {
            status_code: 404,
            status_text: "Not Found".to_string(),
            headers: HashMap::new(),
            body: Vec::new(),
        }
    }

    /// Serializes the response to bytes for writing to a TCP stream.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut res = format!("HTTP/1.1 {} {}\r\n", self.status_code, self.status_text).into_bytes();

        let mut headers = self.headers.clone();
        headers.insert("Content-Length".to_string(), self.body.len().to_string());

        for (k, v) in headers {
            res.extend_from_slice(format!("{}: {}\r\n", k, v).as_bytes());
        }

        res.extend_from_slice(b"\r\n");
        res.extend_from_slice(&self.body);
        res
    }
}

/// A trait for handling HTTP requests.
///
/// // RUST INSIGHT: By defining a trait, we allow the server to accept any type that implements `Handler`.
pub trait Handler: Send + Sync {
    fn handle(&self, req: &Request) -> Response;
}

/// Blanket implementation to allow closures to be used as handlers.
///
/// // RUST INSIGHT: This blanket implementation enables ergonomic API design. Users can pass closures directly instead of defining custom structs.
impl<F> Handler for F
where
    F: Fn(&Request) -> Response + Send + Sync,
{
    fn handle(&self, req: &Request) -> Response {
        self(req)
    }
}

/// The HTTP Server.
pub struct HttpServer<H: Handler + ?Sized> {
    handler: Arc<H>,
    thread_pool_size: usize,
}

impl<H: Handler + ?Sized + 'static> HttpServer<H> {
    pub fn new(handler: Arc<H>) -> Self {
        Self {
            handler,
            thread_pool_size: 4,
        }
    }

    pub fn with_pool_size(mut self, size: usize) -> Self {
        self.thread_pool_size = size;
        self
    }

    /// Starts listening on the given address.
    pub fn listen(&self, addr: &str) -> std::io::Result<()> {
        let listener = TcpListener::bind(addr)?;
        // PRODUCTION NOTE: A production server would use an async runtime like Tokio instead of manual OS threads, avoiding the overhead of one thread per connection or pool worker locking.

        // Simplified thread pool for educational purposes.
        let pool = ThreadPool::new(self.thread_pool_size);

        for stream in listener.incoming() {
            let stream = stream?;
            let handler = Arc::clone(&self.handler);

            pool.execute(move || {
                Self::handle_connection(stream, handler);
            });
        }
        Ok(())
    }

    fn handle_connection(mut stream: TcpStream, handler: Arc<H>) {
        // GOTCHA: Using BufReader to read lines line-by-line is convenient for HTTP headers,
        // but we must be careful not to read past the headers into the body if we are waiting for EOF.
        // We initialize the BufReader with a mutable reference so we can reclaim the stream later.
        let mut reader = BufReader::new(&mut stream);

        let mut request_line = String::new();
        if reader.read_line(&mut request_line).is_err() || request_line.is_empty() {
            return; // Connection closed or error
        }

        // We remove .trim() or .trim_end() to satisfy clippy::trim_split_whitespace
        let parts: Vec<&str> = request_line.split_whitespace().collect();
        if parts.len() != 3 {
            // Malformed request line
            return;
        }

        let method = parts[0].to_string();
        let path = parts[1].to_string();
        let version = parts[2].to_string();

        let mut headers = HashMap::new();
        loop {
            let mut header_line = String::new();
            if reader.read_line(&mut header_line).is_err()
                || header_line == "\r\n"
                || header_line == "\n"
            {
                break;
            }
            if let Some((k, v)) = header_line.split_once(':') {
                headers.insert(k.trim().to_string(), v.trim().to_string());
            }
        }

        let mut body = Vec::new();
        if let Some(content_length) = headers.get("Content-Length").and_then(|s| s.parse::<usize>().ok()) {
            body.resize(content_length, 0);
            // Read exactly content_length bytes
            let _ = reader.read_exact(&mut body);
        }

        // RUST INSIGHT: We drop the reader here to release the mutable borrow on `stream`
        // so we can write to it below. Since `reader` took `&mut stream`, we can just let it go out of scope,
        // but explicit drop makes our intention clear to the borrow checker and future readers.
        drop(reader);

        let request = Request {
            method,
            path,
            version,
            headers,
            body,
        };

        let response = handler.handle(&request);
        let _ = stream.write_all(&response.to_bytes());
    }
}

/// A simple thread pool for executing tasks.
struct ThreadPool {
    #[allow(dead_code)]
    workers: Vec<Worker>,
    sender: std::sync::mpsc::Sender<Job>,
}

type Job = Box<dyn FnOnce() + Send + 'static>;

impl ThreadPool {
    fn new(size: usize) -> Self {
        assert!(size > 0);
        let (sender, receiver) = std::sync::mpsc::channel();
        let receiver = Arc::new(Mutex::new(receiver));

        let mut workers = Vec::with_capacity(size);
        for id in 0..size {
            workers.push(Worker::new(id, Arc::clone(&receiver)));
        }

        Self { workers, sender }
    }

    fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);
        self.sender.send(job).unwrap();
    }
}

#[allow(dead_code)]
struct Worker {
    id: usize,
    thread: Option<thread::JoinHandle<()>>,
}

impl Worker {
    fn new(id: usize, receiver: Arc<Mutex<std::sync::mpsc::Receiver<Job>>>) -> Self {
        let thread = thread::spawn(move || loop {
            // GOTCHA: We must lock, receive, and unlock in one swift motion.
            // If we held the lock while executing the job, we'd defeat the purpose of a thread pool!
            let job = {
                let lock = receiver.lock().unwrap();
                match lock.recv() {
                    Ok(job) => job,
                    Err(_) => break, // Channel closed
                }
            };
            job();
        });

        Self {
            id,
            thread: Some(thread),
        }
    }
}

/// ## Footer
///
/// ### Comparison to Canonical Crates
/// This implementation is a toy compared to `hyper`, which provides fully asynchronous, non-blocking I/O
/// using Tokio, HTTP/2 support, zero-copy parsing, and robust error handling for broken connections.
///
/// ### Missing vs Production
/// - **Async/Await**: This uses blocking threads instead of non-blocking I/O.
/// - **Streaming Bodies**: We buffer the entire request and response bodies in memory.
/// - **HTTP/2 & Keep-Alive**: We treat every request as an isolated stream and close connections.
/// - **Zero-copy parsing**: We allocate heavily (Strings, Vecs) during parsing.
///
/// ### Benchmarking Notes
/// Benchmarking an HTTP server realistically requires a load generator like `wrk` or `hey`.
/// For isolated benchmarking of request parsing logic, use `criterion` to measure `handle_connection`
/// with a static byte buffer containing a mock HTTP request to track parse time and allocation overhead.
///
/// ### Suggested Next Steps
/// - Implement keep-alive connections (reuse the TCP stream for multiple requests).
/// - Replace `String` allocations with `&[u8]` slices tied to the buffer's lifetime.
/// - Convert to an asynchronous architecture using `mio` or `tokio`.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_response_to_bytes() {
        let mut response = Response::ok("Hello World");
        response.headers.insert("Content-Type".to_string(), "text/plain".to_string());

        let bytes = response.to_bytes();
        let s = String::from_utf8(bytes).unwrap();

        assert!(s.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(s.contains("Content-Length: 11\r\n"));
        assert!(s.contains("Content-Type: text/plain\r\n"));
        assert!(s.ends_with("\r\nHello World"));
    }

    #[test]
    fn test_handler_blanket_impl() {
        let handler = |req: &Request| -> Response {
            if req.path == "/ping" {
                Response::ok("pong")
            } else {
                Response::not_found()
            }
        };

        let req = Request {
            method: "GET".to_string(),
            path: "/ping".to_string(),
            version: "HTTP/1.1".to_string(),
            headers: HashMap::new(),
            body: Vec::new(),
        };

        let res = handler.handle(&req);
        assert_eq!(res.status_code, 200);
        assert_eq!(res.body, b"pong");
    }
}
