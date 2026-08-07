//! # HTTP Server
//!
//! What this implements and what crate(s) it replaces:
//! This is a minimal multithreaded HTTP/1.1 server from scratch without external dependencies.
//! It replaces crates like `hyper`, `actix-web`, and `axum`.
//!
//! Real-world systems that use this:
//! All web applications require an HTTP server. High-performance servers like Nginx, Apache,
//! and application servers built in Node, Java, or Go operate on these exact principles.
//!
//! Why build it yourself?
//! Understanding the anatomy of an HTTP request, the complexity of parsing strings into
//! structured data, and how to multiplex incoming connections over a thread pool provides
//! deep insight into web server performance and concurrency models.
//!
//! ## Architecture
//!
//! ```text
//! Incoming TCP -> Thread Pool -> Worker Thread -> Parse HTTP -> Route -> Handler -> Format Response -> TCP Write
//! ```
//!
//! - **Thread Pool**: Maintains a fixed number of worker threads to avoid OS thread-creation overhead.
//! - **Request Parsing**: Reads from a `TcpStream` via `BufReader`, extracting headers and body.
//! - **Routing**: Simple hash-map-based exact-path routing.
//!
//! ## Invariants
//! - Worker threads must outlive the server's main execution context or shut down cleanly.
//! - HTTP parsing must handle malformed requests gracefully without panicking.
//!
//! ## Time/Space Complexity
//! - **Routing**: O(1) time complexity using a HashMap.
//! - **Parsing**: O(N) time where N is the length of the HTTP request.
//! - **Space**: O(N) space to buffer the request body and headers.
//!
//! ## Inline Annotations
//! - `// RUST INSIGHT:` How Rust's ownership model protects thread-shared data without data races.
//! - `// GOTCHA:` Proper stream borrowing and reading.
//! - `// PRODUCTION NOTE:` Discussing missing HTTP features (e.g., keep-alive, chunked transfer encoding).

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

/// Represents an HTTP method.
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Other(String),
}

/// Represents an HTTP request.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct HttpRequest {
    pub method: HttpMethod,
    pub path: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl HttpRequest {
    /// Parses an HTTP request from a buffered reader.
    /// # Errors
    /// Returns an error if the request format is invalid.
    pub fn parse(reader: &mut impl BufRead) -> Result<Self, String> {
        let mut request_line = String::new();
        reader.read_line(&mut request_line).map_err(|e| e.to_string())?;

        let parts: Vec<&str> = request_line.split_whitespace().collect();
        if parts.len() != 3 {
            return Err("Invalid request line".to_string());
        }

        let method = match parts[0] {
            "GET" => HttpMethod::Get,
            "POST" => HttpMethod::Post,
            "PUT" => HttpMethod::Put,
            "DELETE" => HttpMethod::Delete,
            other => HttpMethod::Other(other.to_string()),
        };

        let path = parts[1].to_string();

        let mut headers = HashMap::new();
        loop {
            let mut header_line = String::new();
            reader.read_line(&mut header_line).map_err(|e| e.to_string())?;
            let header_line = header_line.trim_end();
            if header_line.is_empty() {
                break;
            }
            if let Some((key, value)) = header_line.split_once(':') {
                headers.insert(key.trim().to_lowercase(), value.trim().to_string());
            }
        }

        let mut body = Vec::new();
        // GOTCHA: We must parse content-length carefully to know how many bytes to read
        // since HTTP streams might not close immediately.
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

/// Represents an HTTP response.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct HttpResponse {
    pub status_code: u16,
    pub status_text: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl HttpResponse {
    #[must_use]
    pub fn ok(body: impl Into<Vec<u8>>) -> Self {
        Self {
            status_code: 200,
            status_text: "OK".to_string(),
            headers: HashMap::new(),
            body: body.into(),
        }
    }

    #[must_use]
    pub fn not_found() -> Self {
        Self {
            status_code: 404,
            status_text: "Not Found".to_string(),
            headers: HashMap::new(),
            body: Vec::new(),
        }
    }

    #[must_use]
    pub fn bad_request() -> Self {
        Self {
            status_code: 400,
            status_text: "Bad Request".to_string(),
            headers: HashMap::new(),
            body: Vec::new(),
        }
    }

    /// Writes the response to a writer.
    /// # Errors
    /// Returns an error if the underlying writer fails.
    pub fn write_to(&self, writer: &mut impl Write) -> std::io::Result<()> {
        write!(writer, "HTTP/1.1 {} {}\r\n", self.status_code, self.status_text)?;
        for (key, value) in &self.headers {
            write!(writer, "{key}: {value}\r\n")?;
        }
        write!(writer, "Content-Length: {}\r\n\r\n", self.body.len())?;
        writer.write_all(&self.body)?;
        Ok(())
    }
}

pub trait Handler: Send + Sync {
    fn handle(&self, req: &HttpRequest) -> HttpResponse;
}

impl<F> Handler for F
where
    F: Fn(&HttpRequest) -> HttpResponse + Send + Sync,
{
    fn handle(&self, req: &HttpRequest) -> HttpResponse {
        self(req)
    }
}

type Job = Box<dyn FnOnce() + Send + 'static>;

struct Worker {
    _id: usize,
    thread: Option<thread::JoinHandle<()>>,
}

impl Worker {
    fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Job>>>) -> Self {
        let thread = thread::spawn(move || loop {
            let message = receiver.lock().unwrap().recv();
            match message {
                Ok(job) => {
                    // RUST INSIGHT: We execute the job outside the lock by using `recv`
                    // to acquire ownership, dropping the lock when the statement finishes.
                    job();
                }
                Err(_) => {
                    // Channel closed
                    break;
                }
            }
        });
        Self {
            _id: id,
            thread: Some(thread),
        }
    }
}

pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: Option<mpsc::Sender<Job>>,
}

impl ThreadPool {
    /// Creates a new thread pool.
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

    /// Executes a job in the thread pool.
    /// # Panics
    /// Panics if the thread pool has been shut down or job fails to send.
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

pub struct HttpServer {
    handlers: HashMap<String, Arc<dyn Handler>>,
    pool: ThreadPool,
}

impl Default for HttpServer {
    fn default() -> Self {
        Self::new(4)
    }
}

impl HttpServer {
    #[must_use]
    pub fn new(thread_pool_size: usize) -> Self {
        Self {
            handlers: HashMap::new(),
            // PRODUCTION NOTE: A real server might use a dynamic thread pool
            // or an async runtime like Tokio instead of a standard thread pool.
            pool: ThreadPool::new(thread_pool_size),
        }
    }

    #[must_use]
    pub fn route<P, H>(mut self, path: P, handler: H) -> Self
    where
        P: Into<String>,
        H: Handler + 'static,
    {
        self.handlers.insert(path.into(), Arc::new(handler));
        self
    }

    /// Handles a single client connection.
    pub fn handle_client(mut stream: TcpStream, handlers: Arc<HashMap<String, Arc<dyn Handler>>>) {
        let mut reader = BufReader::new(&mut stream);
        match HttpRequest::parse(&mut reader) {
            Ok(req) => {
                // GOTCHA: We must drop the reader to reclaim mutable access to `stream` for writing.
                // Alternatively, we could write via stream while reader exists, but dropping is cleaner.
                drop(reader);
                let response = if let Some(handler) = handlers.get(&req.path) {
                    handler.handle(&req)
                } else {
                    HttpResponse::not_found()
                };
                let _ = response.write_to(&mut stream);
            }
            Err(_) => {
                drop(reader);
                let response = HttpResponse::bad_request();
                let _ = response.write_to(&mut stream);
            }
        }
    }

    /// Runs the HTTP server, listening on the provided `TcpListener`.
    pub fn run(self, listener: TcpListener) {
        let handlers = Arc::new(self.handlers);
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let handlers_clone = Arc::clone(&handlers);
                    self.pool.execute(move || {
                        Self::handle_client(stream, handlers_clone);
                    });
                }
                Err(e) => {
                    eprintln!("Connection failed: {e}");
                }
            }
        }
    }
}

// -----------------------------------------------------------------------------
// Comparison & Next Steps
// -----------------------------------------------------------------------------
//
// How this compares to canonical crates (hyper, actix-web, axum):
// - Hyper is asynchronous (Tokio-based) and uses advanced zero-copy parsers (httparse).
// - This implementation uses blocking I/O and synchronous threads, limiting scalability.
// - Memory allocation here is naive (creating Strings and Vecs for every request).
//
// What's missing vs production:
// - Keep-Alive connection reuse.
// - Asynchronous I/O to handle thousands of concurrent connections (C10k problem).
// - Chunked Transfer-Encoding and robust URI parsing.
// - Zero-copy string parsing.
//
// Suggested next steps:
// - Refactor to use an async runtime.
// - Implement a router tree (Trie/Radix tree) for path parameters (e.g. `/users/:id`).
//
// Benchmarking note:
// The parsing logic can be benchmarked using `criterion` (e.g., parsing a static buffer).
// The full server can be load tested using tools like `wrk`, `hey`, or `ab` to measure
// requests per second and latency percentiles under concurrent load.

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn test_http_request_parse() {
        let req_data = b"GET /hello HTTP/1.1\r\nHost: localhost\r\nContent-Length: 5\r\n\r\nworld";
        let mut reader = BufReader::new(&req_data[..]);
        let req = HttpRequest::parse(&mut reader).unwrap();
        assert_eq!(req.method, HttpMethod::Get);
        assert_eq!(req.path, "/hello");
        assert_eq!(req.headers.get("host").unwrap(), "localhost");
        assert_eq!(req.headers.get("content-length").unwrap(), "5");
        assert_eq!(req.body, b"world");
    }

    #[test]
    fn test_http_request_parse_no_body() {
        let req_data = b"POST /api HTTP/1.1\r\nHost: example.com\r\n\r\n";
        let mut reader = BufReader::new(&req_data[..]);
        let req = HttpRequest::parse(&mut reader).unwrap();
        assert_eq!(req.method, HttpMethod::Post);
        assert_eq!(req.path, "/api");
        assert_eq!(req.headers.get("host").unwrap(), "example.com");
        assert!(req.body.is_empty());
    }

    #[test]
    fn test_http_response_format() {
        let mut resp = HttpResponse::ok(b"hello".to_vec());
        resp.headers.insert("X-Custom".to_string(), "Value".to_string());

        let mut buf = Vec::new();
        resp.write_to(&mut buf).unwrap();
        let resp_str = String::from_utf8(buf).unwrap();
        assert!(resp_str.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(resp_str.contains("Content-Length: 5\r\n"));
        assert!(resp_str.contains("X-Custom: Value\r\n"));
        assert!(resp_str.ends_with("\r\n\r\nhello"));
    }

    #[test]
    fn test_server_routing() {
        let server = HttpServer::new(2)
            .route("/test", |_: &HttpRequest| HttpResponse::ok(b"test passed".to_vec()));

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let server_thread = thread::spawn(move || {
            let handlers = Arc::new(server.handlers);
            if let Ok((stream, _)) = listener.accept() {
                HttpServer::handle_client(stream, handlers);
            }
        });

        let mut client = TcpStream::connect(addr).unwrap();
        client.write_all(b"GET /test HTTP/1.1\r\n\r\n").unwrap();

        let mut response_data = String::new();
        client.read_to_string(&mut response_data).unwrap();
        assert!(response_data.contains("test passed"));

        server_thread.join().unwrap();
    }
}
