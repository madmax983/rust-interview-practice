//! # HTTP/1.1 Client Implementation
//!
//! Implements a synchronous HTTP/1.1 client from scratch over raw TCP sockets.
//! It handles request serialization, manual HTTP/1.1 response parsing, safe boundary extraction,
//! and chunked transfer decoding.
//!
//! **Replaces Crates:** `reqwest::blocking`, `ureq`
//!
//! **Real-world Usage:**
//! - Communicating with REST APIs and web services.
//! - Fetching data in CLI tools, web scrapers, and automated scripts.
//! - Microservice-to-microservice internal communication.
//!
//! **Why build it yourself?**
//! Understanding how a client constructs the raw text of an HTTP request and reliably
//! extracts headers and body data from a raw TCP stream is fundamental to networking.
//! Handling edge cases like chunked encoding and preventing underflow/overflow panics
//! during byte slice parsing teaches you safe, robust network programming.

use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Flow:
//
//     User Code
//        │
//        ▼
//  HttpClient::send(Request)
//        │
//        ▼
//  [TCP Connection] ──► Write Request string to Socket
//        │
//        ▼
//  [Raw Bytes] ◄── read from Socket ◄── BufReader
//        │
//        ▼
//  Response::parse() ──► Extract Status Line, Headers, Body
//
//
// Invariants:
// 1. Requests must be properly formatted HTTP/1.1 strings ending with `\r\n\r\n`.
// 2. `Host` header must be sent with the request.
// 3. Responses are parsed case-insensitively for headers.
// 4. Safe boundary extraction ensures we never panic when searching for `\r\n\r\n`.
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ Serialize Req │ O(Headers + │ O(Headers + │
// │               │   Body)     │   Body)     │
// ├───────────────┼─────────────┼─────────────┤
// │ Parse Resp    │ O(Resp Size)│ O(Resp Size)│
// └───────────────┴─────────────┴─────────────┘
//
// Design Decisions:
// - **Synchronous I/O**: Blocking TCP sockets.
//   - *Tradeoff*: Simple to write and reason about, but thread-blocks on network I/O.
//   - *Alternative*: Non-blocking async I/O (like Tokio) for handling many concurrent requests.
// - **Parsing Strategy**: `BufReader` with `read_line` for headers, then exact byte reads for body.
//   - *Tradeoff*: Allocates strings for headers, but avoids complex zero-copy state machines.
// - **Chunked Decoding**: We manually parse hexadecimal chunk sizes.

/// Represents an HTTP Method.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Put,
    Delete,
    Patch,
}

impl std::fmt::Display for Method {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Method::Get => "GET",
            Method::Post => "POST",
            Method::Put => "PUT",
            Method::Delete => "DELETE",
            Method::Patch => "PATCH",
        };
        write!(f, "{}", s)
    }
}

/// Represents an HTTP Request to be sent.
#[derive(Debug, Clone)]
pub struct Request {
    pub method: Method,
    pub url: String,
    pub headers: HashMap<String, String>,
    pub body: Option<Vec<u8>>,
}

impl Request {
    /// Creates a new request builder-style.
    #[must_use]
    pub fn new(method: Method, url: &str) -> Self {
        Self {
            method,
            url: url.to_string(),
            headers: HashMap::new(),
            body: None,
        }
    }

    /// Adds a header to the request.
    #[must_use]
    pub fn header(mut self, key: &str, value: &str) -> Self {
        self.headers.insert(key.to_string(), value.to_string());
        self
    }

    /// Sets the body of the request.
    #[must_use]
    pub fn body(mut self, body: Vec<u8>) -> Self {
        self.body = Some(body);
        self
    }
}

/// Represents a parsed HTTP Response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    pub status_code: u16,
    pub status_text: String,
    pub version: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

/// Trait defining the behavior of our HTTP client.
pub trait HttpClient {
    /// Sends an HTTP request and returns the parsed response.
    fn send(&self, req: Request) -> io::Result<Response>;
}

/// A synchronous HTTP client.
pub struct SyncHttpClient {
    timeout: Option<Duration>,
}

impl SyncHttpClient {
    /// Creates a new client with an optional timeout.
    #[must_use]
    pub fn new(timeout: Option<Duration>) -> Self {
        Self { timeout }
    }

    /// Parses the URL into (host, port, path).
    /// Note: This is a very naive parser for educational purposes.
    fn parse_url(url: &str) -> io::Result<(String, u16, String)> {
        // Remove "http://"
        let without_scheme = url
            .strip_prefix("http://")
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Only HTTP is supported"))?;

        // Find path
        let (host_port, path) = match without_scheme.find('/') {
            Some(idx) => {
                let (hp, p) = without_scheme.split_at(idx);
                (hp, p.to_string())
            }
            None => (without_scheme, "/".to_string()),
        };

        // Find port
        let (host, port) = match host_port.find(':') {
            Some(idx) => {
                let (h, p) = host_port.split_at(idx);
                let port_num = p[1..].parse::<u16>().map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidInput, "Invalid port number")
                })?;
                (h.to_string(), port_num)
            }
            None => (host_port.to_string(), 80), // Default HTTP port
        };

        Ok((host, port, path))
    }
}

impl HttpClient for SyncHttpClient {
    fn send(&self, req: Request) -> io::Result<Response> {
        let (host, port, path) = Self::parse_url(&req.url)?;
        // ⚡ BOLT OPTIMIZATION: Avoid intermediate string allocation for DNS resolution.
        // Replaced `format!("{}:{}", host, port).to_socket_addrs()` with tuple `(host.as_str(), port).to_socket_addrs()`.
        let mut addrs = (host.as_str(), port).to_socket_addrs()?;
        let socket_addr = addrs
            .next()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Could not resolve host"))?;

        let mut stream = if let Some(timeout) = self.timeout {
            let s = TcpStream::connect_timeout(&socket_addr, timeout)?;
            s.set_read_timeout(Some(timeout))?;
            s.set_write_timeout(Some(timeout))?;
            s
        } else {
            TcpStream::connect(socket_addr)?
        };

        // Serialize request
        let mut request_bytes = Vec::new();
        write!(&mut request_bytes, "{} {} HTTP/1.1\r\n", req.method, path)?;

        // Ensure Host header is present
        let mut has_host = false;
        for (key, value) in &req.headers {
            if key.eq_ignore_ascii_case("host") {
                has_host = true;
            }
            write!(&mut request_bytes, "{}: {}\r\n", key, value)?;
        }

        if !has_host {
            write!(&mut request_bytes, "Host: {}\r\n", host)?;
        }

        // Handle body headers
        if let Some(body) = &req.body
            && !req
                .headers
                .keys()
                .any(|k| k.eq_ignore_ascii_case("content-length"))
        {
            write!(&mut request_bytes, "Content-Length: {}\r\n", body.len())?;
        }

        write!(&mut request_bytes, "Connection: close\r\n\r\n")?;

        if let Some(body) = &req.body {
            request_bytes.extend_from_slice(body);
        }

        // Send request
        stream.write_all(&request_bytes)?;

        // Parse response
        let mut reader = BufReader::new(stream);
        Response::parse(&mut reader)
    }
}

impl Response {
    /// Parses an HTTP response from a `BufReader`.
    pub fn parse<R: Read>(reader: &mut BufReader<R>) -> io::Result<Self> {
        // Read status line
        let mut status_line = String::new();
        let bytes_read = reader.read_line(&mut status_line)?;
        if bytes_read == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Empty response",
            ));
        }

        let parts: Vec<&str> = status_line.trim_end().splitn(3, ' ').collect();
        if parts.len() < 2 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid status line",
            ));
        }

        let version = parts[0].to_string();
        let status_code = parts[1]
            .parse::<u16>()
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid status code"))?;
        let status_text = if parts.len() == 3 {
            parts[2].to_string()
        } else {
            String::new()
        };

        // Read headers
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

            let trimmed_line = line.trim_end();
            if trimmed_line.is_empty() {
                break;
            }

            if let Some((key, value)) = trimmed_line.split_once(':') {
                headers.insert(key.trim().to_lowercase(), value.trim().to_string());
            }
        }

        // Read body
        let mut body = Vec::new();

        // RUST INSIGHT: Safely parsing transfer encoding.
        // We prioritize Transfer-Encoding over Content-Length as per RFC 7230.
        // GOTCHA: It is easy to mistakenly read `Content-Length` even when `chunked` encoding is used.
        if let Some(transfer_encoding) = headers.get("transfer-encoding") {
            if transfer_encoding.contains("chunked") {
                // ⚡ BOLT OPTIMIZATION: Hoist `String::new()` out of the loop and reuse the capacity
                // via `size_line.clear()` to eliminate dynamic heap allocations per chunk.
                let mut size_line = String::new();
                loop {
                    size_line.clear();
                    let bytes_read = reader.read_line(&mut size_line)?;
                    if bytes_read == 0 {
                        break; // EOF
                    }

                    // Remove possible trailing comments (';') and whitespace
                    let size_str = size_line.split(';').next().unwrap_or("").trim();
                    if size_str.is_empty() {
                        continue;
                    }

                    // Parse hex size
                    let size = usize::from_str_radix(size_str, 16).map_err(|_| {
                        io::Error::new(io::ErrorKind::InvalidData, "Invalid chunk size")
                    })?;

                    if size == 0 {
                        // End of chunks, read trailing CRLF
                        let mut crlf = [0u8; 2];
                        let _ = reader.read_exact(&mut crlf);
                        break;
                    }

                    // Read exactly `size` bytes
                    let mut chunk = vec![0; size];
                    reader.read_exact(&mut chunk)?;
                    body.extend_from_slice(&chunk);

                    // Read trailing CRLF after chunk
                    let mut crlf = [0u8; 2];
                    let _ = reader.read_exact(&mut crlf);
                }
            } else {
                // Unknown transfer encoding, try reading to end
                reader.read_to_end(&mut body)?;
            }
        } else if let Some(content_length_str) = headers.get("content-length") {
            let len = content_length_str.parse::<usize>().map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidData, "Invalid content length")
            })?;

            if len > 0 {
                // Prevent extreme memory allocation attacks (e.g., trying to allocate 10GB)
                // PRODUCTION NOTE: A real client like reqwest might stream large bodies instead of buffering them.
                // We'll place a sane hard limit for this educational implementation.
                const MAX_BODY_SIZE: usize = 100 * 1024 * 1024; // 100 MB
                if len > MAX_BODY_SIZE {
                    return Err(io::Error::new(
                        io::ErrorKind::OutOfMemory,
                        "Response body too large",
                    ));
                }

                body.resize(len, 0);
                reader.read_exact(&mut body)?;
            }
        } else {
            // No content-length or transfer-encoding, read until EOF (common for HTTP/1.0 or closed connections)
            reader.read_to_end(&mut body)?;
        }

        Ok(Response {
            status_code,
            status_text,
            version,
            headers,
            body,
        })
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `reqwest`: The standard for HTTP clients in Rust. It supports async (via hyper) and blocking IO,
//   handles HTTP/2, connection pooling, automatic redirects, TLS, and complex multipart forms.
// - `ureq`: A minimal, blocking-only HTTP client. Our implementation shares a similar philosophy but
//   lacks features like TLS, redirects, and proxy support.
//
// What's missing vs. production:
// - **HTTPS (TLS) Support**: We only handle unencrypted HTTP over port 80.
// - **Connection Pooling (Keep-Alive)**: We create a new TCP connection for every request and close it.
// - **Redirect Following**: We don't automatically follow 301/302 redirects.
// - **Compression Decoding**: Production clients automatically decode gzip/brotli/deflate bodies.
//
// Next steps:
// 1. Add support for parsing `https://` and wrapping the `TcpStream` in `rustls`.
// 2. Implement connection pooling by caching `TcpStream`s keyed by host:port.
// 3. Add streaming response bodies instead of buffering entire responses into memory.
//
// Benchmarking Note:
// Use `criterion` to benchmark HTTP request parsing and serialization overhead.
// Measure the `to_socket_addrs` DNS resolution separately, as it can be highly variable.
// `std::hint::black_box` should be used around parsed results to ensure the optimizer doesn't
// eliminate the parsing loop.

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_parse_url_valid() {
        let (host, port, path) = SyncHttpClient::parse_url("http://example.com/api/data").unwrap();
        assert_eq!(host, "example.com");
        assert_eq!(port, 80);
        assert_eq!(path, "/api/data");

        let (host, port, path) = SyncHttpClient::parse_url("http://localhost:8080/test").unwrap();
        assert_eq!(host, "localhost");
        assert_eq!(port, 8080);
        assert_eq!(path, "/test");

        let (host, port, path) = SyncHttpClient::parse_url("http://example.com").unwrap();
        assert_eq!(host, "example.com");
        assert_eq!(port, 80);
        assert_eq!(path, "/");
    }

    #[test]
    fn test_parse_url_invalid() {
        assert!(SyncHttpClient::parse_url("https://example.com").is_err());
        assert!(SyncHttpClient::parse_url("example.com").is_err());
        assert!(SyncHttpClient::parse_url("http://example.com:abc").is_err());
    }

    #[test]
    fn test_request_builder() {
        let req = Request::new(Method::Post, "http://example.com/login")
            .header("Content-Type", "application/json")
            .body(b"{\"user\":\"test\"}".to_vec());

        assert_eq!(req.method, Method::Post);
        assert_eq!(req.url, "http://example.com/login");
        assert_eq!(req.headers.get("Content-Type").unwrap(), "application/json");
        assert_eq!(req.body.unwrap(), b"{\"user\":\"test\"}");
    }

    #[test]
    fn test_parse_simple_response() {
        let raw_response = b"HTTP/1.1 200 OK\r\n\
                           Content-Type: text/plain\r\n\
                           Content-Length: 5\r\n\
                           \r\n\
                           Hello";
        let mut reader = BufReader::new(Cursor::new(raw_response));
        let resp = Response::parse(&mut reader).unwrap();

        assert_eq!(resp.version, "HTTP/1.1");
        assert_eq!(resp.status_code, 200);
        assert_eq!(resp.status_text, "OK");
        assert_eq!(resp.headers.get("content-type").unwrap(), "text/plain");
        assert_eq!(resp.headers.get("content-length").unwrap(), "5");
        assert_eq!(resp.body, b"Hello");
    }

    #[test]
    fn test_parse_chunked_response() {
        let raw_response = b"HTTP/1.1 200 OK\r\n\
                           Transfer-Encoding: chunked\r\n\
                           \r\n\
                           4\r\n\
                           Wiki\r\n\
                           5\r\n\
                           pedia\r\n\
                           e\r\n\
                           in \r\n\
                           \r\n\
                           chunks.\r\n\
                           0\r\n\
                           \r\n";
        let mut reader = BufReader::new(Cursor::new(raw_response));
        let resp = Response::parse(&mut reader).unwrap();

        assert_eq!(resp.status_code, 200);
        assert_eq!(
            String::from_utf8(resp.body).unwrap(),
            "Wikipediain \r\n\r\nchunks."
        );
    }

    #[test]
    fn test_parse_no_body() {
        let raw_response = b"HTTP/1.1 404 Not Found\r\n\
                           Content-Length: 0\r\n\
                           \r\n";
        let mut reader = BufReader::new(Cursor::new(raw_response));
        let resp = Response::parse(&mut reader).unwrap();

        assert_eq!(resp.status_code, 404);
        assert!(resp.body.is_empty());
    }

    #[test]
    fn test_parse_eof_body() {
        let raw_response = b"HTTP/1.1 200 OK\r\n\
                           \r\n\
                           Some Data Unbounded";
        let mut reader = BufReader::new(Cursor::new(raw_response));
        let resp = Response::parse(&mut reader).unwrap();

        assert_eq!(resp.status_code, 200);
        assert_eq!(resp.body, b"Some Data Unbounded");
    }
}
