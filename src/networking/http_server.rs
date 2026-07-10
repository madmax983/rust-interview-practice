//! # HTTP/1.1 Server Implementation
//!
//! Implements a synchronous, multi-threaded HTTP/1.1 server from scratch.
//! It handles raw TCP connections, parses HTTP requests, and generates formatted responses
//! including support for chunked transfer encoding.
//!
//! **Replaces Crates:** `hyper`, `tiny_http`, `rouille`
//!
//! **Real-world Usage:**
//! - Foundation of web frameworks (Axum, Actix, Rocket).
//! - Embedded devices exposing a control interface.
//! - Local development tools serving static files.
//!
//! **Why build it yourself?**
//! Parsing HTTP/1.1 manually teaches you about the text-based nature of the web.
//! You'll grapple with framing (Content-Length vs Chunked Encoding), header parsing complexity
//! (multiline headers, case insensitivity), and the request/response lifecycle.
//! It demystifies how a raw TCP stream becomes a structured request and how to speak the
//! universal language of the internet.

use crate::concurrency::thread_pool::ThreadPool;
use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::sync::Arc;

/// Hard cap on the accepted request body size (100 MB). Bounding allocations
/// against this before `vec![0; n]` prevents an attacker-controlled Content-Length
/// or chunked size line from panicking/aborting (and thus draining) a pool worker.
const MAX_BODY_SIZE: usize = 100 * 1024 * 1024;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Flow:
//
//      Client
//        │
//        ▼
//   [TCP Handshake]
//        │
//        ▼
//   [Raw Bytes]  ──►  BufReader  ──►  HttpRequest::parse()
//                                            │
//                                            ▼
//                                     Handler Function
//                                            │
//                                            ▼
//   [Raw Bytes]  ◄──  Writer     ◄──  HttpResponse::to_bytes()
//
//
// Invariants:
// 1. All headers key names are treated as case-insensitive (normalized to lowercase).
// 2. A request body is read only if `Content-Length` or `Transfer-Encoding` is present.
// 3. Responses always contain a valid status line and headers terminated by `\r\n\r\n`.
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ Parse Request │ O(Header +  │ O(Header +  │
// │               │   Body)     │   Body)     │
// ├───────────────┼─────────────┼─────────────┤
// │ Serialize Resp│ O(Body)     │ O(Body)     │
// └───────────────┴─────────────┴─────────────┘
//
// Design Decisions:
// - **Threading Model**: ThreadPool.
//   - *Tradeoff*: Limits concurrency but prevents OOM.
//   - *Downside*: Blocking I/O means one slow client can occupy a thread.
//   - *Alternative*: Non-blocking I/O with an event loop (Tokio/Mio).
// - **Parsing**: `BufReader` with `read_line` for headers.
//   - *Tradeoff*: Allocates strings.
//   - *Alternative*: Zero-copy parsing using a cursor over a byte buffer (httparse crate style).
// - **Storage**: `Vec<u8>` for body.
//   - *Tradeoff*: Loads entire body into memory.
//   - *Alternative*: Streaming body reader.

/// Represents an HTTP Request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    pub method: String,
    pub path: String,
    pub version: String,
    pub headers: HashMap<String, String>,
    pub params: HashMap<String, String>,
    pub body: Vec<u8>,
}

/// Represents an HTTP Response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status_code: u16,
    pub status_text: String,
    pub headers: HashMap<String, String>,
    pub body: Option<Vec<u8>>,
}

/// specific handler for processing requests.
pub trait Handler: Send + Sync + 'static {
    fn handle(&self, req: HttpRequest) -> HttpResponse;
}

impl<F> Handler for F
where
    F: Fn(HttpRequest) -> HttpResponse + Send + Sync + 'static,
{
    fn handle(&self, req: HttpRequest) -> HttpResponse {
        self(req)
    }
}

/// A simple HTTP/1.1 Server.
pub struct HttpServer<H: Handler> {
    listener: TcpListener,
    handler: Arc<H>,
    pool: ThreadPool,
}

impl<H: Handler> HttpServer<H> {
    /// Creates a new HTTP Server bound to the given address with a request handler.
    ///
    /// # Errors
    /// Returns an error if the listener cannot bind to the given address.
    pub fn new<A: ToSocketAddrs>(addr: A, handler: H, pool_size: usize) -> io::Result<Self> {
        let listener = TcpListener::bind(addr)?;
        let pool = ThreadPool::new(pool_size);
        Ok(Self {
            listener,
            handler: Arc::new(handler),
            pool,
        })
    }

    /// Starts the server loop.
    ///
    /// # Errors
    /// Returns an error if the local address cannot be read from the listener.
    pub fn run(&self) -> io::Result<()> {
        println!("Server listening on {}", self.listener.local_addr()?);

        for stream in self.listener.incoming() {
            match stream {
                Ok(stream) => {
                    let handler = Arc::clone(&self.handler);

                    self.pool.execute(move || {
                        if let Err(e) = Self::handle_connection(stream, &handler) {
                            // Don't log unexpected EOF which is normal connection close
                            if e.kind() != io::ErrorKind::UnexpectedEof {
                                eprintln!("Error handling connection: {e}");
                            }
                        }
                    });
                }
                Err(e) => eprintln!("Connection failed: {e}"),
            }
        }
        Ok(())
    }

    fn handle_connection(mut stream: TcpStream, handler: &Arc<H>) -> io::Result<()> {
        let mut reader = BufReader::new(stream.try_clone()?);

        loop {
            let request = match HttpRequest::parse(&mut reader) {
                Ok(Some(req)) => req,
                Ok(None) => return Ok(()), // Clean EOF
                Err(e) => {
                    // Send 400 Bad Request
                    let response = HttpResponse::new(
                        400,
                        "Bad Request",
                        Some(format!("Error parsing request: {e}").into_bytes()),
                    );
                    // Ignore write error on broken pipe
                    let _ = stream.write_all(&response.to_bytes());
                    return Ok(());
                }
            };

            println!("Request: {} {}", request.method, request.path);

            // Check for Connection: close
            let close_connection = request
                .headers
                .get("connection")
                .is_some_and(|v| v.to_lowercase() == "close");

            let response = handler.handle(request);
            stream.write_all(&response.to_bytes())?;

            if close_connection {
                break;
            }
        }
        Ok(())
    }
}

impl HttpRequest {
    /// Reads and parses an HTTP request from the given reader.
    /// Returns `Ok(None)` if the stream ends cleanly at the start of a request.
    ///
    /// # Errors
    /// Returns an error if the underlying stream fails or the request is
    /// malformed (bad request line, headers, or content length).
    pub fn parse<R: Read>(reader: &mut BufReader<R>) -> io::Result<Option<Self>> {
        // GOTCHA: `read_line` appends to the string. If we reused a buffer, we'd need to clear it.
        // It also includes the newline characters, which we must trim.
        let mut first_line = String::new();
        let bytes_read = reader.read_line(&mut first_line)?;

        if bytes_read == 0 {
            return Ok(None);
        }

        // Handle empty lines (some clients send newlines as keep-alive ping or before request)
        while first_line.trim().is_empty() {
            first_line.clear();
            let bytes = reader.read_line(&mut first_line)?;
            if bytes == 0 {
                return Ok(None);
            }
        }

        let parts: Vec<&str> = first_line.split_whitespace().collect();
        if parts.len() < 3 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid request line",
            ));
        }

        let method = parts[0].to_string();
        let path = parts[1].to_string();
        let version = parts[2].to_string();

        let mut headers = HashMap::new();
        // ⚡ BOLT OPTIMIZATION: Hoist `String::new()` out of the loop and reuse the capacity
        // via `line.clear()` to eliminate dynamic heap allocations per header line.
        let mut line = String::new();
        loop {
            line.clear();
            reader.read_line(&mut line)?;

            if line == "\r\n" || line == "\n" {
                break;
            }

            // Trim trailing newline and spaces
            let trimmed_line = line.trim_end();
            if trimmed_line.is_empty() {
                break;
            }

            if let Some((key, value)) = trimmed_line.split_once(':') {
                headers.insert(key.trim().to_lowercase(), value.trim().to_string());
            }
        }

        // Handle body
        let mut body = Vec::new();

        // RUST INSIGHT: Security - Request Smuggling
        // RFC 7230: If a message is received with both a Transfer-Encoding and a Content-Length header field,
        // the Transfer-Encoding overrides the Content-Length.
        if let Some(transfer_encoding) = headers.get("transfer-encoding") {
            if transfer_encoding.contains("chunked") {
                // Basic chunked reader implementation
                // ⚡ BOLT OPTIMIZATION: Hoist `String::new()` out of the loop and reuse the capacity
                // via `size_line.clear()` to eliminate dynamic heap allocations per chunk.
                let mut size_line = String::new();
                loop {
                    size_line.clear();
                    let bytes_read = reader.read_line(&mut size_line)?;
                    if bytes_read == 0 {
                        break; // EOF
                    }

                    let size_str = size_line.trim();
                    if size_str.is_empty() {
                        continue;
                    }

                    let size = usize::from_str_radix(size_str, 16).map_err(|_| {
                        io::Error::new(io::ErrorKind::InvalidData, "Invalid chunk size")
                    })?;

                    if size == 0 {
                        // Read trailing CRLF
                        size_line.clear();
                        reader.read_line(&mut size_line)?;
                        break;
                    }

                    // SECURITY: Bound per-chunk and cumulative body size before
                    // allocating. A malicious client can send a hex size line like
                    // `ffffffffffffffff` (usize::MAX); `vec![0; size]` would then
                    // panic/abort, killing this pool worker (the pool does not
                    // catch_unwind, so repeated requests drain the pool -> DoS).
                    if size > MAX_BODY_SIZE || body.len().saturating_add(size) > MAX_BODY_SIZE {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "Request body too large",
                        ));
                    }

                    let mut chunk = vec![0; size];
                    reader.read_exact(&mut chunk)?;
                    body.extend(chunk);

                    // Read trailing CRLF after chunk
                    size_line.clear();
                    reader.read_line(&mut size_line)?;
                }
            }
        } else if let Some(content_length) = headers.get("content-length")
            && let Ok(len) = content_length.parse::<usize>()
            && len > 0
        {
            // SECURITY: Bound the declared body size before allocating. Without
            // this, `Content-Length: 9223372036854775807` (or usize::MAX) makes
            // `vec![0; len]` panic/abort and kill a pool worker.
            if len > MAX_BODY_SIZE {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Request body too large",
                ));
            }
            let mut buffer = vec![0; len];
            reader.read_exact(&mut buffer)?;
            body = buffer;
        }

        Ok(Some(Self {
            method,
            path,
            version,
            headers,
            params: HashMap::new(),
            body,
        }))
    }
}

impl HttpResponse {
    #[must_use]
    pub fn new(status_code: u16, status_text: &str, body: Option<Vec<u8>>) -> Self {
        Self {
            status_code,
            status_text: status_text.to_string(),
            headers: HashMap::new(),
            body,
        }
    }

    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut response = Vec::new();

        // Status line
        write!(
            &mut response,
            "HTTP/1.1 {} {}\r\n",
            self.status_code, self.status_text
        )
        .unwrap();

        // Headers
        let is_chunked = self
            .headers
            .get("Transfer-Encoding")
            .map(std::string::String::as_str)
            == Some("chunked")
            || self
                .headers
                .get("transfer-encoding")
                .map(std::string::String::as_str)
                == Some("chunked");

        for (key, value) in &self.headers {
            write!(&mut response, "{key}: {value}\r\n").unwrap();
        }

        // Content-Length or Transfer-Encoding
        if let Some(body) = &self.body {
            if !is_chunked
                && !self.headers.contains_key("Content-Length")
                && !self.headers.contains_key("content-length")
            {
                write!(&mut response, "Content-Length: {}\r\n", body.len()).unwrap();
            }
        } else if !is_chunked
            && !self.headers.contains_key("Content-Length")
            && !self.headers.contains_key("content-length")
        {
            write!(&mut response, "Content-Length: 0\r\n").unwrap();
        }

        write!(&mut response, "\r\n").unwrap();

        if let Some(body) = &self.body {
            if is_chunked {
                // Chunk the body
                // For demonstration, we'll send it as one chunk if it's small.
                if !body.is_empty() {
                    write!(&mut response, "{:x}\r\n", body.len()).unwrap();
                    response.extend_from_slice(body);
                    write!(&mut response, "\r\n").unwrap();
                }
                write!(&mut response, "0\r\n\r\n").unwrap();
            } else {
                response.extend_from_slice(body);
            }
        }

        response
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `hyper`: Hyper is a low-level HTTP implementation used by most Rust web frameworks. It is asynchronous,
//   supports HTTP/1.1, H2, and H3, and is heavily optimized. Our implementation is synchronous and simplified.
// - `tiny_http`: Similar in spirit to this implementation but offers more features and robustness.
//
// Missing vs. Production:
// - **Async I/O**: Essential for high-concurrency performance.
// - **Security**: No protection against Slowloris, large payload DoS, or malformed header attacks (beyond basic parsing).
// - **Full RFC Compliance**: We skip many headers, status codes, and edge cases.
//
// Next Steps:
// 1. Port to `mio` for non-blocking I/O.
// 2. Implement HTTP/2 support.

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::thread;

    #[test]
    fn test_parse_simple_get() {
        let input = b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let mut reader = BufReader::new(Cursor::new(input));
        let req = HttpRequest::parse(&mut reader).unwrap().unwrap();

        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.get("host"), Some(&"localhost".to_string()));
        assert!(req.body.is_empty());
    }

    #[test]
    fn test_parse_post_with_body() {
        let input = b"POST /submit HTTP/1.1\r\nContent-Length: 11\r\n\r\nHello World";
        let mut reader = BufReader::new(Cursor::new(input));
        let req = HttpRequest::parse(&mut reader).unwrap().unwrap();

        assert_eq!(req.method, "POST");
        assert_eq!(req.body, b"Hello World");
    }

    #[test]
    fn test_parse_rejects_huge_content_length() {
        // i64::MAX Content-Length. Before the fix `vec![0; len]` panicked/aborted,
        // killing a pool worker. Now parse must return an error (-> 400).
        let input = b"POST / HTTP/1.1\r\nContent-Length: 9223372036854775807\r\n\r\n";
        let mut reader = BufReader::new(Cursor::new(input));
        let err = HttpRequest::parse(&mut reader).expect_err("huge content-length must error");
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn test_parse_rejects_oversized_chunk_size() {
        // usize::MAX chunk-size line. Before the fix `vec![0; size]` panicked.
        let input = b"POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\nffffffffffffffff\r\n";
        let mut reader = BufReader::new(Cursor::new(input));
        let err = HttpRequest::parse(&mut reader).expect_err("oversized chunk must error");
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn test_response_serialization() {
        let mut resp = HttpResponse::new(200, "OK", Some(b"Hello".to_vec()));
        resp.headers
            .insert("Server".to_string(), "RustServer".to_string());

        let bytes = resp.to_bytes();
        let s = String::from_utf8(bytes).unwrap();

        assert!(s.contains("HTTP/1.1 200 OK"));
        assert!(s.contains("Server: RustServer"));
        assert!(s.contains("Content-Length: 5"));
        assert!(s.ends_with("\r\n\r\nHello"));
    }

    #[test]
    fn test_response_chunked() {
        let mut resp = HttpResponse::new(200, "OK", Some(b"Hello".to_vec()));
        resp.headers
            .insert("Transfer-Encoding".to_string(), "chunked".to_string());

        let bytes = resp.to_bytes();
        let s = String::from_utf8(bytes).unwrap();

        assert!(s.contains("Transfer-Encoding: chunked"));
        assert!(!s.contains("Content-Length"));
        // Chunk size 5 is '5' in hex
        assert!(s.contains("5\r\nHello\r\n"));
        assert!(s.ends_with("0\r\n\r\n"));
    }

    #[test]
    fn test_server_handle_connection() {
        // Find a free port
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        struct TestHandler;
        impl Handler for TestHandler {
            fn handle(&self, req: HttpRequest) -> HttpResponse {
                if req.path == "/" {
                    HttpResponse::new(
                        200,
                        "OK",
                        Some(b"Welcome to the Rust HTTP Server!".to_vec()),
                    )
                } else {
                    HttpResponse::new(404, "Not Found", None)
                }
            }
        }

        let handler = Arc::new(TestHandler);
        let handler_clone = Arc::clone(&handler);

        // Spawn server thread
        let server_handle = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            // We need a dummy handler since we can't create an HttpServer without arguments or access its internal method easily
            // But wait, the method is static on the struct if H is known.
            // Actually, handle_connection is an associated function.
            let _ = HttpServer::<TestHandler>::handle_connection(stream, &handler_clone);
        });

        // Client
        let mut client = TcpStream::connect(addr).unwrap();
        client
            .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .unwrap();

        let mut buffer = Vec::new();
        client.read_to_end(&mut buffer).unwrap();

        let response = String::from_utf8(buffer).unwrap();
        assert!(response.contains("HTTP/1.1 200 OK"));
        assert!(response.contains("Welcome to the Rust HTTP Server!"));

        server_handle.join().unwrap();
    }

    #[test]
    fn test_chunked_reader_eof_safety() {
        // Chunk size 5, but EOF immediately
        let input = b"POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n5\r\n";
        let mut reader = BufReader::new(Cursor::new(input));

        // Should return error due to UnexpectedEof in read_exact
        let result = HttpRequest::parse(&mut reader);
        assert!(result.is_err());
    }

    #[test]
    fn test_chunked_reader_infinite_loop_bug() {
        // Chunk size line is empty (EOF) immediately after headers
        let input = b"POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n";
        let mut reader = BufReader::new(Cursor::new(input));

        // Should return empty body, not loop infinitely
        let req = HttpRequest::parse(&mut reader).unwrap().unwrap();
        assert!(req.body.is_empty());
    }
}
