//! # HTTP/1.1 Server
//!
//! ## What this implements and what it replaces
//! This is a multithreaded HTTP/1.1 server built entirely from scratch using only standard library primitives (`std::net`, `std::thread`).
//! It acts as a foundational replacement for crates like `hyper`, `actix-web`, or `axum`.
//!
//! ## Real-world systems that use this
//! Every major web service and proxy (Nginx, Envoy, Apache) runs on similar foundational HTTP parsing and request-dispatching models. Rust web frameworks abstract these raw TCP/HTTP mechanics away.
//!
//! ## Why build it yourself?
//! Building an HTTP server from the TCP socket up demystifies the "magic" of web frameworks. You learn how bytes on a wire become structured requests, how concurrency is managed per connection, and how to safely handle shared state across threads.
//!
//! ## Architecture
//!
//! ```text
//! Client      TCP Connection      Server Thread         Application
//!   |               |                   |                    |
//!   |--- GET ------>|==== TCP ====>|-- Parse HTTP -->|--- handle() --->|
//!   |               |                   |                    |
//!   |<-- 200 OK ----|<==== TCP ====<|-- Format res <-|<-- Response <---|
//! ```
//!
//! ## Invariants
//! * Connections must not block the main listener loop.
//! * Malformed HTTP requests must not panic the server; they should return a 400 Bad Request.
//! * Handlers must be thread-safe (`Send + Sync`) to allow concurrent processing.
//!
//! ## Complexity
//! * **Request Parsing**: Time O(N) where N is the number of bytes in the headers. Space O(M) where M is the size of the request headers and body.
//! * **Routing**: Time O(1) for our single handler architecture. Space O(1).
//!
//! ## Design Decisions
//! * **Thread per connection vs Thread pool vs Async**: We use a simple `std::thread::spawn` for simplicity. A production server would use an async runtime (like `tokio` and `hyper`) to handle C10K efficiently without OS thread overhead.
//! * **Borrowing over Cloning**: We use `BufReader<&mut TcpStream>` to read the stream without taking ownership of it, allowing us to drop the reader and write back to the stream safely.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;

/// An HTTP request.
#[derive(Debug, Clone)]
pub struct Request {
    pub method: String,
    pub path: String,
    pub version: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

/// An HTTP response.
#[derive(Debug, Clone)]
pub struct Response {
    pub status_code: u16,
    pub status_text: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl Response {
    pub fn new(status_code: u16, status_text: &str, body: impl Into<Vec<u8>>) -> Self {
        Self {
            status_code,
            status_text: status_text.to_string(),
            headers: HashMap::new(),
            body: body.into(),
        }
    }
}

/// A trait for handling HTTP requests.
// RUST INSIGHT: The `Send + Sync` bounds are strictly enforced by the compiler, guaranteeing
// that implementors are safe to be shared and executed concurrently across multiple threads.
pub trait Handler: Send + Sync {
    fn handle(&self, request: &Request) -> Response;
}

// RUST INSIGHT: Blanket implementation allows users to pass simple closures instead of requiring them to implement a custom struct.
// This allows ergonomic closure usage by consumers while maintaining a robust, swappable trait-based design under the hood.
impl<F> Handler for F
where
    F: Fn(&Request) -> Response + Send + Sync,
{
    fn handle(&self, request: &Request) -> Response {
        self(request)
    }
}

/// A simple multithreaded HTTP Server.
pub struct Server<H: ?Sized> {
    handler: Arc<H>,
}

// RUST INSIGHT: By using `H: Handler + ?Sized`, we explicitly relax the implicit `Sized` bound on the generic parameter.
// This resolves E0277 when passing a dynamically dispatched trait object (e.g., `Arc<dyn Handler>`) to the Server.
impl<H: Handler + ?Sized + 'static> Server<H> {
    pub fn new(handler: Arc<H>) -> Self {
        Self { handler }
    }

    /// Runs the server on the given address. Blocks the current thread.
    pub fn run(&self, addr: &str) -> std::io::Result<()> {
        let listener = TcpListener::bind(addr)?;
        self.serve(listener)
    }

    /// Serves requests using the provided listener.
    pub fn serve(&self, listener: TcpListener) -> std::io::Result<()> {
        // PRODUCTION NOTE: A real server would use a thread pool or async tasks to avoid the overhead of spawning a thread per connection.
        for stream_result in listener.incoming() {
            match stream_result {
                Ok(stream) => {
                    let handler = Arc::clone(&self.handler);
                    // GOTCHA: Thread per connection is vulnerable to DoS attacks if too many connections are opened concurrently.
                    thread::spawn(move || {
                        if let Err(e) = Self::handle_connection(stream, handler) {
                            eprintln!("Error handling connection: {}", e);
                        }
                    });
                }
                Err(e) => {
                    eprintln!("Failed to accept connection: {}", e);
                }
            }
        }
        Ok(())
    }

    fn handle_connection(mut stream: TcpStream, handler: Arc<H>) -> std::io::Result<()> {
        // RUST INSIGHT: We borrow `stream` mutably to read from it without taking ownership.
        // This avoids cloning the stream, which is an OS-level operation.
        let mut reader = BufReader::new(&mut stream);
        let mut request_line = String::new();

        if reader.read_line(&mut request_line)? == 0 {
            return Ok(()); // Connection closed
        }

        // Memory rule: remove .trim() before .split_whitespace() to resolve clippy::trim_split_whitespace
        let parts: Vec<&str> = request_line.split_whitespace().collect();
        if parts.len() != 3 {
            let _ = stream.write_all(b"HTTP/1.1 400 Bad Request\r\n\r\n");
            return Ok(());
        }

        let method = parts[0].to_string();
        let path = parts[1].to_string();
        let version = parts[2].to_string();

        let mut headers = HashMap::new();
        loop {
            let mut header_line = String::new();
            if reader.read_line(&mut header_line)? == 0 {
                break;
            }
            let header_line = header_line.trim_end();
            if header_line.is_empty() {
                break;
            }
            if let Some((key, value)) = header_line.split_once(':') {
                headers.insert(key.trim().to_lowercase(), value.trim().to_string());
            }
        }

        let mut body = Vec::new();
        // Memory rule: avoid unstable let_chains. We use `.and_then()` for Option chaining.
        if let Some(content_length) = headers.get("content-length").and_then(|len| len.parse::<usize>().ok()) {
            body.resize(content_length, 0);
            reader.read_exact(&mut body)?;
        }

        let request = Request {
            method,
            path,
            version,
            headers,
            body,
        };

        // RUST INSIGHT: We explicitly drop the reader here to release the mutable borrow on `stream`.
        // This is strictly required to satisfy the borrow checker and reclaim the stream for `.write_all()` within the exact same lexical scope.
        drop(reader);

        let mut response = handler.handle(&request);

        response.headers.entry("Content-Length".to_string()).or_insert_with(|| response.body.len().to_string());
        response.headers.entry("Connection".to_string()).or_insert_with(|| "close".to_string());

        // Format and send the response
        let status_line = format!("HTTP/1.1 {} {}\r\n", response.status_code, response.status_text);
        stream.write_all(status_line.as_bytes())?;

        for (key, value) in &response.headers {
            let header_line = format!("{}: {}\r\n", key, value);
            stream.write_all(header_line.as_bytes())?;
        }

        stream.write_all(b"\r\n")?;
        stream.write_all(&response.body)?;

        Ok(())
    }
}

// ============================================================================
// Footer:
//
// Comparison to canonical crates:
// * `hyper` and `axum` are asynchronous and run on top of `tokio`. They use non-blocking I/O and state machines (Futures) to handle tens of thousands of concurrent connections on a few threads.
// * They have rigorous, spec-compliant HTTP parsing (e.g., using `httparse`) that handles chunked transfer encoding, HTTP/2, and keep-alive, which we simplified here.
//
// Missing vs Production:
// * Async I/O (epoll/kqueue) for C10K scale.
// * Connection Keep-Alive and pooling.
// * HTTP/2 multiplexing support.
// * Rigorous timeout and header size limits to prevent Slowloris attacks.
//
// Benchmarking Note:
// A robust benchmark for this server would involve using a load testing tool like `wrk` or `hey` to send a high volume of requests.
// Within Rust, you could write a `criterion` benchmark in a `benches/` directory to measure the latency of `Server::handle_connection` in isolation using `std::hint::black_box` to prevent compiler optimizations on the mock request bytes.
//
// Benchmarking Note:
// A robust benchmark for this server would involve using a load testing tool like `wrk` or `hey` to send a high volume of requests.
// Within Rust, you could write a `criterion` benchmark in a `benches/` directory to measure the latency of `Server::handle_connection` in isolation using `std::hint::black_box` to prevent compiler optimizations on the mock request bytes.
//
// Suggested Next Steps:
// * Integrate a custom ThreadPool to limit the number of active threads.
// * Add robust routing capabilities with path parameters (e.g., `/users/:id`).
// * Support chunked transfer encoding for streaming responses.
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_server_responds() {
        let handler = Arc::new(|req: &Request| {
            if req.path == "/hello" {
                Response::new(200, "OK", "Hello, World!")
            } else {
                Response::new(404, "Not Found", "Page not found")
            }
        });

        let server = Server::new(handler);
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        thread::spawn(move || {
            let _ = server.serve(listener);
        });

        // Give the listener thread a tiny bit of time (it's mostly instantaneous though)
        thread::sleep(std::time::Duration::from_millis(10));

        // Test 200 OK route
        let mut client = TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
        client.write_all(b"GET /hello HTTP/1.1\r\nHost: localhost\r\n\r\n").unwrap();

        let mut response = String::new();
        client.read_to_string(&mut response).unwrap();

        assert!(response.contains("HTTP/1.1 200 OK"));
        assert!(response.contains("Hello, World!"));

        // Test 404 Not Found route
        let mut client2 = TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
        client2.write_all(b"GET /unknown HTTP/1.1\r\nHost: localhost\r\n\r\n").unwrap();

        let mut response2 = String::new();
        client2.read_to_string(&mut response2).unwrap();

        assert!(response2.contains("HTTP/1.1 404 Not Found"));
        assert!(response2.contains("Page not found"));
    }

    #[test]
    fn test_server_post_request() {
        let handler = Arc::new(|req: &Request| {
            if req.method == "POST" && req.path == "/echo" {
                // Return body back as response
                Response::new(200, "OK", req.body.clone())
            } else {
                Response::new(400, "Bad Request", "")
            }
        });

        let server = Server::new(handler);
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        thread::spawn(move || {
            let _ = server.serve(listener);
        });

        thread::sleep(std::time::Duration::from_millis(10));

        let mut client = TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
        let request_payload = b"POST /echo HTTP/1.1\r\nContent-Length: 12\r\n\r\nHello Server";
        client.write_all(request_payload).unwrap();

        let mut response = String::new();
        client.read_to_string(&mut response).unwrap();

        assert!(response.contains("HTTP/1.1 200 OK"));
        assert!(response.ends_with("Hello Server"));
    }

    #[test]
    fn test_malformed_request() {
        let handler = Arc::new(|_req: &Request| {
            Response::new(200, "OK", "Should not reach here")
        });

        let server = Server::new(handler);
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        thread::spawn(move || {
            let _ = server.serve(listener);
        });

        thread::sleep(std::time::Duration::from_millis(10));

        let mut client = TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
        client.write_all(b"JUST_A_BAD_REQUEST\r\n\r\n").unwrap();

        let mut response = String::new();
        client.read_to_string(&mut response).unwrap();

        assert!(response.contains("HTTP/1.1 400 Bad Request"));
    }
}
