//! # HTTP Server Implementation
//!
//! Implements a minimal multithreaded HTTP/1.1 server from scratch without external dependencies,
//! including request parsing, a custom thread pool for concurrent request handling, and response formatting.
//!
//! **Replaces Crates:** `hyper`, `actix-web`, `axum` (at their foundational level)
//!
//! **Real-world Usage:**
//! - Web servers and reverse proxies (Nginx, Apache, Caddy)
//! - Application frameworks serving web traffic
//! - Embedded servers for IoT devices or admin dashboards
//!
//! **Why build it yourself?**
//! Building an HTTP server from scratch demystifies how "the web" actually works over plain TCP.
//! You learn how to manually parse text-based protocols, how to manage threads and concurrency to avoid
//! blocking on slow clients, and how Rust's ownership model naturally guides you away from data races
//! when sharing state across a thread pool.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure / Threading Model:
//
//      Main Thread                    Worker Threads (ThreadPool)
//      ┌───────────┐                  ┌───────────┐
//      │TcpListener├─ Accept ─┐    ┌─►│ Worker 1  │──► Parse Request
//      └───────────┘          │    │  └───────────┘       │
//                             ▼    │                      ▼
//      ┌───────────┐      ┌──────┐ │  ┌───────────┐   Build Response
//      │ mpsc::Tx  ├─Send─► Queue├─┼─►│ Worker 2  │       │
//      └───────────┘      └──────┘ │  └───────────┘       ▼
//                                  │                  Write to Stream
//                                  │  ┌───────────┐
//                                  └─►│ Worker N  │
//                                     └───────────┘
//
// HTTP Request Format:
//   METHOD /path HTTP/1.1\r\n
//   Header-Key: Header-Value\r\n
//   \r\n
//   <Body>
//
// Invariants:
// 1. The thread pool has a fixed number of workers.
// 2. Main thread only accepts connections and dispatches them; it never reads/writes directly, ensuring it never blocks.
// 3. Each connection is fully handled (read, parsed, responded, closed) by a single worker thread.
//
// Complexity:
// ┌───────────────┬───────────────────┬─────────────┐
// │ Component     │ Time              │ Space       │
// ├───────────────┼───────────────────┼─────────────┤
// │ Connection    │ O(1) to accept    │ O(1)        │
// │ Req Parsing   │ O(N) length of req│ O(N)        │
// │ ThreadPool    │ O(1) dispatch     │ O(W) workers│
// └───────────────┴───────────────────┴─────────────┘
//
// Design Decisions:
// - **Concurrency**: Fixed-size ThreadPool with an `mpsc` channel.
// - **Request Parsing**: `BufReader` initialized with `&mut stream` to avoid cloning or taking ownership unnecessarily.
// - **Memory**: Use `String` and `HashMap` for simplicity in parsing headers, though a zero-copy approach (like `httparse`) is faster.

/// Represents an HTTP Request.
#[derive(Debug, PartialEq, Eq)]
pub struct Request {
    pub method: String,
    pub path: String,
    pub version: String,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
}

/// Represents an HTTP Response.
#[derive(Debug, PartialEq, Eq)]
pub struct Response {
    pub status_code: u16,
    pub status_text: String,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
}

impl Response {
    /// Creates a simple response with standard headers.
    pub fn new(status_code: u16, status_text: &str, body: Option<String>) -> Self {
        let mut headers = HashMap::new();
        if let Some(ref b) = body {
            headers.insert("Content-Length".to_string(), b.len().to_string());
        }
        headers.insert("Connection".to_string(), "close".to_string());

        Self {
            status_code,
            status_text: status_text.to_string(),
            headers,
            body,
        }
    }

    /// Serializes the response into raw bytes for writing to a TCP stream.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut res = format!("HTTP/1.1 {} {}\r\n", self.status_code, self.status_text);

        for (k, v) in &self.headers {
            res.push_str(&format!("{}: {}\r\n", k, v));
        }

        res.push_str("\r\n");
        if let Some(ref b) = self.body {
            res.push_str(b);
        }

        res.into_bytes()
    }
}

/// Parses an incoming raw HTTP request from a TCP Stream.
pub fn parse_request(stream: &mut TcpStream) -> Result<Request, String> {
    // RUST INSIGHT: We initialize BufReader with `&mut stream` instead of `stream.try_clone().unwrap()`.
    // This perfectly abides by Rust's borrowing rules: the reader borrows the stream mutably for the read phase,
    // and we can drop the reader explicitly to regain access to `stream` for writing.
    let mut reader = BufReader::new(stream);

    let mut request_line = String::new();
    reader.read_line(&mut request_line).map_err(|e| e.to_string())?;

    if request_line.is_empty() {
        return Err("Empty request line".to_string());
    }

    // GOTCHA: HTTP lines end in \r\n. We must trim whitespace before parsing.
    let parts: Vec<&str> = request_line.trim_end().split_whitespace().collect();
    if parts.len() != 3 {
        return Err("Malformed request line".to_string());
    }

    let method = parts[0].to_string();
    let path = parts[1].to_string();
    let version = parts[2].to_string();

    let mut headers = HashMap::new();
    let mut content_length = 0;

    loop {
        let mut header_line = String::new();
        reader.read_line(&mut header_line).map_err(|e| e.to_string())?;

        let trimmed = header_line.trim_end();
        if trimmed.is_empty() {
            // End of headers reached
            break;
        }

        if let Some((k, v)) = trimmed.split_once(':') {
            let key = k.trim().to_string();
            let value = v.trim().to_string();

            if key.eq_ignore_ascii_case("content-length") {
                content_length = value.parse::<usize>().unwrap_or(0);
            }

            headers.insert(key, value);
        }
    }

    let mut body = None;
    if content_length > 0 {
        let mut body_buf = vec![0; content_length];
        reader.read_exact(&mut body_buf).map_err(|e| e.to_string())?;
        body = String::from_utf8(body_buf).ok();
    }

    // Explicitly drop the reader to satisfy the borrow checker and return full mutable access
    // to the stream to the caller (who will write the response).
    drop(reader);

    Ok(Request {
        method,
        path,
        version,
        headers,
        body,
    })
}

// =========================================================================================
// Thread Pool Implementation
// =========================================================================================

// PRODUCTION NOTE: A real-world web framework like `actix-web` relies on `tokio` (an async runtime)
// to multiplex thousands of connections over a small thread pool using non-blocking I/O (epoll/kqueue).
// Our implementation uses blocking I/O with 1 thread per active connection, which doesn't scale to
// 10k concurrent connections (the C10k problem) but is rock solid for standard workloads.

type Job = Box<dyn FnOnce() + Send + 'static>;

struct Worker {
    _id: usize,
    thread: Option<thread::JoinHandle<()>>,
}

impl Worker {
    fn new(_id: usize, receiver: Arc<Mutex<mpsc::Receiver<Job>>>) -> Self {
        // RUST INSIGHT: We use Arc<Mutex<Receiver>> because multiple threads need to safely
        // share the receiving end of the channel. The Mutex ensures only one thread is locking
        // and retrieving a job at a time.
        let thread = thread::spawn(move || loop {
            let message = receiver.lock().unwrap().recv();

            match message {
                Ok(job) => {
                    // Execute the job
                    job();
                }
                Err(_) => {
                    // Sender disconnected; shut down worker
                    break;
                }
            }
        });

        Self {
            _id,
            thread: Some(thread),
        }
    }
}

pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: Option<mpsc::Sender<Job>>,
}

impl ThreadPool {
    /// Create a new ThreadPool.
    ///
    /// The size is the number of threads in the pool.
    ///
    /// # Panics
    ///
    /// The `new` function will panic if the size is zero.
    pub fn new(size: usize) -> Self {
        assert!(size > 0);

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

    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);
        self.sender.as_ref().unwrap().send(job).unwrap();
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        // Drop the sender to close the channel and signal workers to break their loop
        drop(self.sender.take());

        for worker in &mut self.workers {
            if let Some(thread) = worker.thread.take() {
                thread.join().unwrap();
            }
        }
    }
}

// =========================================================================================
// Server Implementation
// =========================================================================================

/// Trait representing a generic handler for HTTP requests.
/// This allows swappable strategies for routing and handling.
pub trait Handler: Send + Sync + 'static {
    fn handle(&self, request: Request) -> Response;
}

// Blanket implementation for closures that match the signature.
// This is a common Rust pattern (used heavily in hyper/tower) to allow ergonomic closure usage
// while still relying on a robust trait-based design under the hood.
impl<F> Handler for F
where
    F: Fn(Request) -> Response + Send + Sync + 'static,
{
    fn handle(&self, request: Request) -> Response {
        (self)(request)
    }
}

pub struct HttpServer {
    pool: ThreadPool,
}

impl HttpServer {
    pub fn new(workers: usize) -> Self {
        Self {
            pool: ThreadPool::new(workers),
        }
    }

    /// Handles a single client connection.
    fn handle_connection<H>(mut stream: TcpStream, handler: Arc<H>)
    where
        H: Handler + ?Sized,
    {
        match parse_request(&mut stream) {
            Ok(request) => {
                let response = handler.handle(request);
                let _ = stream.write_all(&response.to_bytes());
            }
            Err(_) => {
                let error_response = Response::new(400, "Bad Request", None);
                let _ = stream.write_all(&error_response.to_bytes());
            }
        }
        // stream is dropped here, closing the TCP connection.
    }

    /// Starts the server and listens for incoming connections.
    /// This method runs indefinitely until the listener is dropped.
    pub fn run<H, A>(&self, addr: A, handler: H) -> std::io::Result<()>
    where
        A: std::net::ToSocketAddrs,
        H: Handler,
    {
        let listener = TcpListener::bind(addr)?;
        let handler_arc = Arc::new(handler);

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let handler_clone = Arc::clone(&handler_arc);
                    self.pool.execute(move || {
                        Self::handle_connection(stream, handler_clone);
                    });
                }
                Err(e) => {
                    eprintln!("Failed to accept connection: {}", e);
                }
            }
        }
        Ok(())
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `hyper`: Uses asynchronous I/O (via Tokio), supports HTTP/2, streaming bodies, and zero-copy parsing.
// - `actix-web` / `axum`: Build on top of `hyper` with routing, middleware, and request extraction.
//
// Missing vs. Production:
// - **Zero-Copy Parsing**: This implementation allocates heavily (`String`, `HashMap`) while parsing. A real server parses slices (`&[u8]`).
// - **Async I/O**: Blocking I/O prevents scaling past thousands of concurrent connections.
// - **HTTP/1.1 Keep-Alive**: We currently close the connection after every request.
// - **Routing**: We expect a single handler function; a framework would provide a router to dispatch paths.
// - **Security**: This implementation is vulnerable to DoS attacks. A malicious client could cause an OOM panic by sending a massive `Content-Length`, or trigger a Slowloris attack by trickling headers slowly. Production servers enforce strict limits on header sizes, body sizes, and timeouts.
// - **Binary Payloads**: By storing the body as `Option<String>`, we silently fail or corrupt non-UTF8 binary data (like images). A real server uses `Vec<u8>` or streaming bodies.
//
// Benchmarking Note:
// To benchmark this, you would typically use an external tool like `wrk` or `hey` against a running instance of the server,
// rather than an in-process Rust benchmark (like `criterion`), to accurately measure network I/O, thread pool dispatch, and parsing overhead.
// Example: `wrk -t4 -c100 -d10s http://127.0.0.1:8080/`
//
// Next Steps:
// 1. Swap the synchronous `TcpListener` and ThreadPool for `tokio::net::TcpListener` and async/await.
// 2. Use `httparse` to replace manual request parsing and avoid allocations.
// 3. Implement a rudimentary router structure (e.g., `Router::get("/path", handler)`).

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::net::{TcpListener, TcpStream};
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_response_to_bytes() {
        let res = Response::new(200, "OK", Some("Hello World".to_string()));
        let bytes = res.to_bytes();
        let res_str = String::from_utf8(bytes).unwrap();

        assert!(res_str.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(res_str.contains("Content-Length: 11\r\n"));
        assert!(res_str.contains("Connection: close\r\n"));
        assert!(res_str.ends_with("\r\nHello World"));
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

        // Wait briefly for threads to finish
        thread::sleep(Duration::from_millis(50));
        assert_eq!(*counter.lock().unwrap(), 10);
    }

    // A mock stream to test request parsing without needing a real TCP connection.
    // In actual tests, spinning up a local listener is sometimes easier to simulate complete IO.

    #[test]
    fn test_server_integration() {
        // Spin up the server in a background thread
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        thread::spawn(move || {
            let pool = ThreadPool::new(2);
            // In tests we need a concrete type for Arc
            let handler: Arc<dyn Handler> = Arc::new(|req: Request| {
                if req.path == "/hello" {
                    Response::new(200, "OK", Some("World".to_string()))
                } else {
                    Response::new(404, "Not Found", None)
                }
            });

            for stream in listener.incoming().take(2) {
                let stream = stream.unwrap();
                let handler_clone = Arc::clone(&handler);
                pool.execute(move || {
                    HttpServer::handle_connection(stream, handler_clone);
                });
            }
        });

        // Let server start
        thread::sleep(Duration::from_millis(50));

        // Make a successful request
        let mut client = TcpStream::connect(addr).unwrap();
        client.write_all(b"GET /hello HTTP/1.1\r\nHost: localhost\r\n\r\n").unwrap();

        let mut response = String::new();
        client.read_to_string(&mut response).unwrap();

        assert!(response.contains("HTTP/1.1 200 OK"));
        assert!(response.contains("World"));

        // Make a not-found request
        let mut client2 = TcpStream::connect(addr).unwrap();
        client2.write_all(b"GET /nowhere HTTP/1.1\r\nHost: localhost\r\n\r\n").unwrap();

        let mut response2 = String::new();
        client2.read_to_string(&mut response2).unwrap();

        assert!(response2.contains("HTTP/1.1 404 Not Found"));
    }
}
