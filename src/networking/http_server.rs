//! # HTTP/1.1 Server Implementation
//!
//! Implements a multi-threaded HTTP/1.1 server from scratch using `std::net` and `std::thread`.
//! Handles request parsing, response formatting, and basic connection management.
//!
//! **Replaces Crates:** `hyper` (server-side core), `tiny_http`, `actix-web` (at a very low level)
//!
//! **Real-world Usage:**
//! - Embedded devices where a full async runtime is too heavy.
//! - Internal tools where dependency minimalism is preferred.
//! - Understanding how `nginx` or `apache` handle parsing and framing at the socket level.
//!
//! **Why build it yourself?**
//! Building an HTTP server demystifies the text-based protocol that powers the web.
//! You learn about:
//! - **Framing**: How to separate requests (Content-Length vs Chunked).
//! - **Parsing**: Robustly handling unstructured text streams.
//! - **Concurrency**: Managing client connections with threads (vs async tasks).
//! - **Socket I/O**: Blocking vs non-blocking reads and buffering.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Diagram:
//
//      Client                      Server (Main Thread)          Worker Thread
//         │                               │                            │
//         ├─── TCP Connect ──────────────►│                            │
//         │                               ├─── spawn ─────────────────►│
//         ├─── HTTP Request ──────────────┼───────────────────────────►│
//         │                               │                            │
//         │                               │          ┌─────────────────┴─────────────────┐
//         │                               │          │ 1. Read & Parse (BufReader)       │
//         │                               │          │ 2. Route / Handle Request         │
//         │                               │          │ 3. Build Response                 │
//         │                               │          │ 4. Write Response (TcpStream)     │
//         │                               │          └─────────────────┬─────────────────┘
//         ◄─── HTTP Response ─────────────┼────────────────────────────┘
//         │                               │
//
// Invariants:
// 1. Each connection is handled by a separate thread (thread-per-connection model).
// 2. Request parsing blocks until the full header section is received.
// 3. Body reading respects `Content-Length`.
// 4. Response writing ensures atomic writes (mostly) via buffering.
//
// Complexity:
// ┌─────────────┬───────────────┬──────────────────────────────────┐
// │ Component   │ Complexity    │ Notes                            │
// ├─────────────┼───────────────┼──────────────────────────────────┤
// │ Parse       │ O(N)          │ N = header size + body size      │
// │ Routing     │ O(1) / O(M)   │ Depends on handler logic         │
// │ Concurrency │ O(T)          │ T = number of active threads     │
// └─────────────┴───────────────┴──────────────────────────────────┘
//
// Design Decisions:
// - **Blocking I/O**: Used for simplicity. A production server would use `mio`/`tokio` for non-blocking Event Loop.
// - **Thread-per-Request**: Simple but doesn't scale to 10k connections (C10k problem).
// - **Parsing**: Strict line-based parsing for headers.

/// Represents an HTTP Request.
#[derive(Debug, Clone)]
pub struct HttpRequest {
    pub method: String,
    pub uri: String,
    pub version: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

/// Represents an HTTP Response.
#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status_code: u16,
    pub status_text: String,
    pub headers: HashMap<String, String>,
    pub body: Option<Vec<u8>>,
}

impl HttpResponse {
    /// Creates a new Response builder with status 200 OK.
    pub fn new(status_code: u16, status_text: &str) -> Self {
        Self {
            status_code,
            status_text: status_text.to_string(),
            headers: HashMap::new(),
            body: None,
        }
    }

    /// Sets a header.
    pub fn with_header(mut self, key: &str, value: &str) -> Self {
        self.headers.insert(key.to_string(), value.to_string());
        self
    }

    /// Sets the body. automatically sets Content-Length.
    pub fn with_body(mut self, body: Vec<u8>) -> Self {
        self.headers
            .insert("Content-Length".to_string(), body.len().to_string());
        self.body = Some(body);
        self
    }

    /// Writes the response to the given stream.
    // RUST INSIGHT: Taking `&mut impl Write` allows testing with `Vec<u8>` or `Cursor`
    // instead of needing a real TCP stream.
    pub fn write_to(&self, writer: &mut impl Write) -> std::io::Result<()> {
        write!(
            writer,
            "HTTP/1.1 {} {}\r\n",
            self.status_code, self.status_text
        )?;

        for (key, value) in &self.headers {
            write!(writer, "{}: {}\r\n", key, value)?;
        }

        write!(writer, "\r\n")?;

        if let Some(body) = &self.body {
            writer.write_all(body)?;
        }

        writer.flush()?;
        Ok(())
    }
}

/// A simple HTTP 1.1 Server.
pub struct HttpServer {
    addr: String,
}

impl HttpServer {
    /// Creates a new server bound to the given address.
    pub fn new(addr: &str) -> Self {
        Self {
            addr: addr.to_string(),
        }
    }

    /// Starts the server. This function blocks indefinitely.
    ///
    /// The `handler` is a thread-safe closure that processes requests.
    // RUST INSIGHT: `F: Fn(...) + Send + Sync + 'static` is the classic trait bound
    // for a closure that can be shared across threads. `Arc` is needed to share ownership
    // of the handler function itself across multiple worker threads.
    pub fn listen<F>(&self, handler: F) -> std::io::Result<()>
    where
        F: Fn(HttpRequest) -> HttpResponse + Send + Sync + 'static,
    {
        let listener = TcpListener::bind(&self.addr)?;
        println!("Server listening on {}", self.addr);

        let handler = Arc::new(handler);

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let handler_clone = Arc::clone(&handler);
                    // PRODUCTION NOTE: In a real server, we'd use a ThreadPool here to limit
                    // the number of concurrent connections and avoid OS thread exhaustion.
                    // For learning, we spawn a new thread per connection.
                    thread::spawn(move || {
                        if let Err(e) = handle_connection(stream, handler_clone) {
                            eprintln!("Error handling connection: {}", e);
                        }
                    });
                }
                Err(e) => {
                    eprintln!("Connection failed: {}", e);
                }
            }
        }
        Ok(())
    }
}

/// Handles a single connection.
fn handle_connection<F>(mut stream: TcpStream, handler: Arc<F>) -> std::io::Result<()>
where
    F: Fn(HttpRequest) -> HttpResponse,
{
    // RUST INSIGHT: `BufReader` is essential for performance. Reading byte-by-byte from
    // a TCP stream is extremely slow due to syscall overhead.
    // We scope the reader to ensure it is dropped before we write to the stream,
    // avoiding a borrow checker error (immutable borrow of `stream` via reader vs mutable borrow for writing).
    let request_result = {
        let mut reader = BufReader::new(&stream);
        parse_request(&mut reader)
    };

    // Parse Request
    match request_result {
        Ok(request) => {
            // Log request
            // println!("{} {}", request.method, request.uri);

            // Generate Response
            let response = handler(request);

            // Write Response
            response.write_to(&mut stream)?;
        }
        Err(e) => {
            // Bad Request
            let response = HttpResponse::new(400, "Bad Request")
                .with_body(format!("Failed to parse request: {}", e).into_bytes());
            response.write_to(&mut stream)?;
        }
    }
    Ok(())
}

/// Parses an HTTP request from the reader.
fn parse_request<R: BufRead>(reader: &mut R) -> Result<HttpRequest, String> {
    // 1. Read Request Line: "GET /index.html HTTP/1.1"
    let mut request_line = String::new();
    reader.read_line(&mut request_line).map_err(|e| e.to_string())?;

    if request_line.trim().is_empty() {
        return Err("Empty request line".to_string());
    }

    let parts: Vec<&str> = request_line.trim().split_whitespace().collect();
    if parts.len() < 3 {
        return Err("Invalid request line format".to_string());
    }

    let method = parts[0].to_string();
    let uri = parts[1].to_string();
    let version = parts[2].to_string();

    // 2. Read Headers
    let mut headers = HashMap::new();
    let mut content_length = 0;
    let mut is_chunked = false;

    loop {
        let mut line = String::new();
        reader.read_line(&mut line).map_err(|e| e.to_string())?;

        let line = line.trim();
        if line.is_empty() {
            break; // End of headers
        }

        // GOTCHA: Headers are case-insensitive, but we store them as provided (usually).
        // A production parser should normalize keys to lowercase for easier lookup.
        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim().to_string();
            let value = value.trim().to_string();

            if key.eq_ignore_ascii_case("Content-Length") {
                content_length = value.parse().unwrap_or(0);
            } else if key.eq_ignore_ascii_case("Transfer-Encoding") && value.eq_ignore_ascii_case("chunked") {
                is_chunked = true;
            }

            headers.insert(key, value);
        }
    }

    // 3. Read Body
    let mut body = Vec::new();

    if is_chunked {
        // Handle Chunked Encoding
        loop {
            let mut size_line = String::new();
            reader.read_line(&mut size_line).map_err(|e| e.to_string())?;

            // Parse hex size
            let size_str = size_line.trim();
            if size_str.is_empty() { continue; } // robustness
            let chunk_size = usize::from_str_radix(size_str, 16).map_err(|_| "Invalid chunk size".to_string())?;

            if chunk_size == 0 {
                // End of chunks. Read trailing CRLF (or trailers)
                let mut trailer = String::new();
                reader.read_line(&mut trailer).map_err(|e| e.to_string())?;
                break;
            }

            // Read chunk data
            let mut chunk = vec![0; chunk_size];
            reader.read_exact(&mut chunk).map_err(|e| e.to_string())?;
            body.extend_from_slice(&chunk);

            // Read trailing CRLF after data
            let mut crlf = vec![0; 2];
            reader.read_exact(&mut crlf).map_err(|e| e.to_string())?;
            if crlf != b"\r\n" {
                return Err("Expected CRLF after chunk".to_string());
            }
        }
    } else if content_length > 0 {
        // Handle Content-Length
        // GOTCHA: Don't use `read_to_end` here because the connection is kept alive!
        // `read_to_end` waits for EOF (connection close). We must read exactly `content_length` bytes.
        let mut buffer = vec![0; content_length];
        reader.read_exact(&mut buffer).map_err(|e| e.to_string())?;
        body = buffer;
    }

    Ok(HttpRequest {
        method,
        uri,
        version,
        headers,
        body,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_response_writing() {
        let response = HttpResponse::new(200, "OK")
            .with_header("Content-Type", "text/plain")
            .with_body(b"Hello".to_vec());

        let mut buffer = Vec::new();
        response.write_to(&mut buffer).unwrap();

        let output = String::from_utf8(buffer).unwrap();
        assert!(output.contains("HTTP/1.1 200 OK"));
        assert!(output.contains("Content-Type: text/plain"));
        assert!(output.contains("Content-Length: 5"));
        assert!(output.ends_with("\r\n\r\nHello"));
    }

    #[test]
    fn test_parse_request() {
        let raw_request = "POST /api/data HTTP/1.1\r\n\
                           Host: localhost\r\n\
                           Content-Length: 11\r\n\
                           \r\n\
                           Hello World";
        let mut reader = Cursor::new(raw_request);
        let request = parse_request(&mut reader).unwrap();

        assert_eq!(request.method, "POST");
        assert_eq!(request.uri, "/api/data");
        assert_eq!(request.version, "HTTP/1.1");
        assert_eq!(request.headers.get("Host").unwrap(), "localhost");
        assert_eq!(request.body, b"Hello World");
    }

    #[test]
    fn test_parse_chunked_request() {
        // "Wiki" in chunks: 4\r\nWiki\r\n5\r\npedia\r\nE\r\n in\r\n\r\nchunks.\r\n0\r\n\r\n
        let raw_request = "POST /wiki HTTP/1.1\r\n\
                           Host: localhost\r\n\
                           Transfer-Encoding: chunked\r\n\
                           \r\n\
                           4\r\n\
                           Wiki\r\n\
                           5\r\n\
                           pedia\r\n\
                           0\r\n\
                           \r\n";

        let mut reader = Cursor::new(raw_request);
        let request = parse_request(&mut reader).unwrap();

        assert_eq!(request.method, "POST");
        assert_eq!(request.body, b"Wikipedia");
    }

    #[test]
    fn test_server_integration() {
        // Start server in background
        // We use a manual listener loop here instead of `HttpServer::listen` because
        // we need to know the random port assigned by the OS (port 0), and `HttpServer::listen`
        // blocks and doesn't expose the bound port.

        // We need to know the port to connect.
        // Since we can't easily get the bound port from the simple `listen` function
        // without modifying it to return the listener or port, we'll manually bind for this test
        // or just pick a high port and hope it's free.
        // Better: Bind a listener first, get port, then run loop.

        // Modifying the design for testability:
        // A robust test would bind a listener, get the port, then pass the listener to the server.
        // For this exercise, let's just pick a port that is likely free, or use a slightly different pattern.

        // Let's rely on a helper for the test.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let addr = format!("127.0.0.1:{}", port);

        thread::spawn(move || {
            let handler = Arc::new(|req: HttpRequest| {
                if req.uri == "/hello" {
                    HttpResponse::new(200, "OK").with_body(b"World".to_vec())
                } else {
                    HttpResponse::new(404, "Not Found")
                }
            });

            for stream in listener.incoming() {
                if let Ok(stream) = stream {
                    let handler = Arc::clone(&handler);
                    thread::spawn(move || {
                        handle_connection(stream, handler).unwrap();
                    });
                }
            }
        });

        // Give server a moment to start (though bind happened already)
        thread::sleep(std::time::Duration::from_millis(100));

        // Connect as client
        let mut stream = TcpStream::connect(&addr).unwrap();
        stream.write_all(b"GET /hello HTTP/1.1\r\nHost: localhost\r\n\r\n").unwrap();

        let mut reader = BufReader::new(&stream);
        let mut response_line = String::new();
        reader.read_line(&mut response_line).unwrap();

        assert_eq!(response_line, "HTTP/1.1 200 OK\r\n");
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `hyper`: Uses `tokio` for async I/O, `httparse` for zero-copy parsing, and handles keep-alive/pipelining.
// - `actix-web`: Builds on top of `tokio` with an actor-like model.
//
// Missing vs. Production:
// - **Keep-Alive**: This server closes the connection (conceptually) or relies on client to close,
//   actually our implementation doesn't strictly close it but read_line might hang if client keeps connection open
//   without sending data. Real servers handle `Connection: keep-alive` logic explicitly.
// - **Timeouts**: No read/write timeouts. Malicious clients (Slowloris) can hang threads.
// - **Security**: No TLS support. No prevention against massive headers (DoS).
//
// Next Steps:
// 1. Implement a ThreadPool to limit concurrency.
// 2. Add `Connection: keep-alive` support (loop in `handle_connection`).
// 3. Support Chunked Transfer Encoding.
