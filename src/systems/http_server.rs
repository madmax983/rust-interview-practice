//! # HTTP Server
//!
//! What this implements: A multithreaded HTTP/1.1 server from scratch without external dependencies.
//! Crates it replaces: `hyper`, `actix-web`, `axum`
//! Real-world systems: Nginx, Apache HTTP Server, Rust web frameworks.
//! Why build it yourself: Understanding how TCP streams are parsed into HTTP requests, how a thread pool manages concurrent connections, and how responses are formatted over the wire is fundamental to web development. It demystifies the "magic" of modern web frameworks.
//!
//! ## Architecture
//!
//! ```text
//! [Client 1] \                                      / [Worker 1] -> Handler
//! [Client 2] -> [TcpListener (Main Thread)] -> [Queue] -> [Worker 2] -> Handler
//! [Client 3] /                                      \ [Worker 3] -> Handler
//! ```
//!
//! ### Invariants
//! - The server must gracefully handle malformed requests without crashing.
//! - The thread pool must distribute connections among available workers.
//! - Connections must be closed after handling (HTTP/1.0 style or simple HTTP/1.1 without Keep-Alive for simplicity).
//!
//! ### Time/Space Complexity
//! - Request Parsing: Time: O(N) where N is request size. Space: O(N) to store headers and body.
//! - Thread Pool Dispatch: Time: O(1). Space: O(W) where W is number of workers.
//!
//! ### Design Decisions
//! - **Thread Pool**: We implement a basic `ThreadPool` using `std::thread` and `mpsc::channel` to avoid spawning a new thread per request (which is vulnerable to `DoS`).
//! - **Traits**: We use a `Handler` trait to allow swappable request routing strategies.
//! - **Parsing**: We use standard `BufReader` and string manipulation. A production parser would use state machines or zero-copy parsing (like `httparse`).

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;

/// HTTP Method
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Unknown(String),
}

impl From<&str> for HttpMethod {
    fn from(s: &str) -> Self {
        match s {
            "GET" => Self::Get,
            "POST" => Self::Post,
            "PUT" => Self::Put,
            "DELETE" => Self::Delete,
            other => Self::Unknown(other.to_string()),
        }
    }
}

/// Represents an HTTP Request.
#[derive(Debug)]
pub struct HttpRequest {
    pub method: HttpMethod,
    pub path: String,
    pub version: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl HttpRequest {
    /// Parses an HTTP request from a stream.
    #[must_use]
    pub fn parse(stream: &mut TcpStream) -> Option<Self> {
        let mut reader = BufReader::new(stream);
        let mut request_line = String::new();

        // RUST INSIGHT: `BufReader` allows efficient line-by-line reading from a TCP stream.
        // GOTCHA: We must be careful not to read indefinitely if the client sends no newline.
        // A production server limits the max header size.
        if reader.read_line(&mut request_line).ok()? == 0 {
            return None; // Connection closed
        }

        let parts: Vec<&str> = request_line.split_whitespace().collect();
        if parts.len() != 3 {
            return None; // Malformed request line
        }

        let method = HttpMethod::from(parts[0]);
        let path = parts[1].to_string();
        let version = parts[2].to_string();

        let mut headers = HashMap::new();
        loop {
            let mut header_line = String::new();
            if reader.read_line(&mut header_line).ok()? == 0 {
                break;
            }

            let trimmed = header_line.trim_end();
            if trimmed.is_empty() {
                break; // End of headers
            }

            if let Some((key, value)) = trimmed.split_once(':') {
                headers.insert(key.trim().to_lowercase(), value.trim().to_string());
            }
        }

        let mut body = Vec::new();
        if let Some(length) = headers
            .get("content-length")
            .and_then(|s| s.parse::<usize>().ok())
            .filter(|&l| l > 0)
        {
            #[allow(unknown_lints, clippy::read_zero_byte_vec)]
            {
                body.resize(length, 0);
                // GOTCHA: `read_exact` blocks until exactly `length` bytes are read.
                let _ = reader.read_exact(&mut body);
            }
        }

        Some(Self {
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
    pub status_code: u16,
    pub status_text: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl HttpResponse {
    #[must_use]
    pub fn new(status_code: u16, status_text: &str, body: impl Into<Vec<u8>>) -> Self {
        let body_vec = body.into();
        let mut headers = HashMap::new();
        headers.insert("Content-Length".to_string(), body_vec.len().to_string());

        Self {
            status_code,
            status_text: status_text.to_string(),
            headers,
            body: body_vec,
        }
    }

    /// Writes the response to a TCP stream.
    ///
    /// # Errors
    /// Returns an error if writing to the stream fails.
    pub fn write_to(&self, stream: &mut TcpStream) -> std::io::Result<()> {
        use std::fmt::Write as _;
        let mut response = String::new();
        let _ = write!(
            response,
            "HTTP/1.1 {} {}\r\n",
            self.status_code, self.status_text
        );

        for (key, value) in &self.headers {
            let _ = write!(response, "{key}: {value}\r\n");
        }

        response.push_str("\r\n");

        stream.write_all(response.as_bytes())?;
        stream.write_all(&self.body)?;
        stream.flush()?;

        Ok(())
    }
}

impl Default for HttpResponse {
    fn default() -> Self {
        Self::new(200, "OK", "")
    }
}

/// Trait for handling HTTP requests.
pub trait Handler: Send + Sync {
    fn handle(&self, request: HttpRequest) -> HttpResponse;
}

impl<F> Handler for F
where
    F: Fn(HttpRequest) -> HttpResponse + Send + Sync,
{
    fn handle(&self, request: HttpRequest) -> HttpResponse {
        self(request)
    }
}

type Job = Box<dyn FnOnce() + Send + 'static>;

/// A simple thread pool to handle concurrent connections.
pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: Option<mpsc::Sender<Job>>,
}

impl ThreadPool {
    /// Creates a new `ThreadPool`.
    ///
    /// # Panics
    /// Panics if size is 0.
    #[must_use]
    pub fn new(size: usize) -> Self {
        assert!(size > 0, "ThreadPool size must be > 0");

        let (sender, receiver) = mpsc::channel();
        // RUST INSIGHT: Arc<Mutex<>> allows multiple workers to safely share the receiver endpoint.
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

    /// Executes a job in the thread pool.
    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);
        if let Some(sender) = &self.sender {
            // Ignore send errors if workers have disconnected
            let _ = sender.send(job);
        }
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        // Drop the sender to signal workers to stop.
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
                // PRODUCTION NOTE: lock().unwrap() will panic if the mutex is poisoned (another thread panicked while holding it).
                // A production server should handle worker panics by restarting the worker.
                let message = receiver.lock().expect("Mutex poisoned").recv();

                match message {
                    Ok(job) => {
                        job();
                    }
                    Err(_) => {
                        // Channel closed, time to shut down.
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

/// A simple HTTP server.
pub struct HttpServer<H: Handler> {
    handler: Arc<H>,
}

impl<H: Handler + 'static> HttpServer<H> {
    /// Creates a new HTTP Server with the given handler.
    #[must_use]
    pub fn new(handler: H) -> Self {
        Self {
            handler: Arc::new(handler),
        }
    }

    /// Starts listening on the given address.
    ///
    /// # Errors
    /// Returns an error if binding to the address fails.
    pub fn listen<A: ToSocketAddrs>(&self, addr: A) -> std::io::Result<()> {
        let listener = TcpListener::bind(addr)?;
        // PRODUCTION NOTE: Hardcoded thread pool size.
        // In reality, this should be configurable or match num_cpus.
        let pool = ThreadPool::new(4);

        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };

            let handler = Arc::clone(&self.handler);

            pool.execute(move || {
                if let Some(request) = HttpRequest::parse(&mut stream) {
                    let response = handler.handle(request);
                    let _ = response.write_to(&mut stream);
                } else {
                    let response = HttpResponse::new(400, "Bad Request", "Malformed request");
                    let _ = response.write_to(&mut stream);
                }
            });
        }

        Ok(())
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `hyper`: Uses asynchronous I/O (tokio), zero-copy parsing (`httparse`), and complex state machines for HTTP/1.1 and HTTP/2.
// - `axum` / `actix-web`: High-level routing, middleware, and extractor systems built on top of async runtimes.
//
// Missing vs. Production:
// - **Asynchronous I/O**: We use thread-per-connection (via ThreadPool) which blocks. Production servers use epoll/kqueue (via tokio/mio).
// - **Keep-Alive**: We close the connection (implicitly or don't wait for more) rather than persisting it for multiple requests.
// - **HTTP/2 & HTTP/3**: Not supported.
// - **Security**: No TLS support, no limits on request body size or header size (vulnerable to Slowloris).
//
// Benchmarking Notes:
// - To benchmark this, you could spawn the server in a background thread and use a tool like `wrk`
//   or `bombardier` to measure requests per second (RPS) and latency.
// - Alternatively, in a Rust benchmark (e.g., using `criterion`), you could mock the `TcpStream`
//   or connect locally and time the round trip using `std::time::Instant::now()`.
//
// Next Steps:
// 1. Port this to use `mio` and non-blocking sockets.
// 2. Add an HTTP router (e.g., `Router::new().route("/hello", get(handler))`).
// 3. Implement zero-copy parsing for headers.

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::net::{TcpListener, TcpStream};
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_http_request_parsing() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let t = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let req = HttpRequest::parse(&mut stream).unwrap();
            assert_eq!(req.method, HttpMethod::Post);
            assert_eq!(req.path, "/data");
            assert_eq!(req.version, "HTTP/1.1");
            assert_eq!(req.headers.get("content-length").unwrap(), "5");
            assert_eq!(req.body, b"hello");
        });

        let mut client = TcpStream::connect(addr).unwrap();
        client
            .write_all(b"POST /data HTTP/1.1\r\nHost: localhost\r\nContent-Length: 5\r\n\r\nhello")
            .unwrap();
        client.flush().unwrap();

        t.join().unwrap();
    }

    #[test]
    fn test_malformed_request_parsing() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let t = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let req = HttpRequest::parse(&mut stream);
            assert!(req.is_none());
        });

        let mut client = TcpStream::connect(addr).unwrap();
        client.write_all(b"INVALID_LINE\r\n\r\n").unwrap();
        client.flush().unwrap();

        t.join().unwrap();
    }

    #[test]
    fn test_thread_pool_execution() {
        let pool = ThreadPool::new(4);
        let counter = Arc::new(Mutex::new(0));

        for _ in 0..20 {
            let c = Arc::clone(&counter);
            pool.execute(move || {
                // Simulate some work
                thread::sleep(Duration::from_millis(10));
                let mut num = c.lock().unwrap();
                *num += 1;
            });
        }

        // Drop the pool, which waits for all workers to finish.
        drop(pool);

        assert_eq!(*counter.lock().unwrap(), 20);
    }
}
