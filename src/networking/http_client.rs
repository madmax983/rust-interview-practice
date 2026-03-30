//! # HTTP/1.1 Client Implementation
//!
//! Implements a synchronous HTTP/1.1 client over raw TCP sockets from scratch.
//!
//! **Replaces Crates:** `reqwest::blocking`, `ureq`
//!
//! **Real-world Usage:**
//! - Making REST API calls without bringing in a large async runtime like Tokio.
//! - Bootstrapping connections in embedded systems.
//! - Service-to-service communication in synchronous architectures.
//!
//! **Why build it yourself?**
//! Building an HTTP client teaches you how to construct raw HTTP requests, handle
//! TCP connections directly, and most importantly, parse HTTP responses. It exposes
//! the complexities of `Content-Length`, `Transfer-Encoding: chunked`, and robust
//! header parsing, demystifying the text-based nature of the web.

use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{TcpStream, ToSocketAddrs}; // ToSocketAddrs is required by TcpStream::connect implicitly
use std::str::FromStr;
use std::time::Duration;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Flow:
//
//      Client Code
//           │
//           ▼
//    HttpRequestBuilder ──► [Construct HTTP String]
//                                   │
//                                   ▼
//                             [TCP Stream]
//                                   │
//                                   ▼
//                            Server Response
//                                   │
//                                   ▼
//    HttpResponse::parse ◄── [Raw Bytes via BufReader]
//
//
// Invariants:
// 1. All headers key names are treated as case-insensitive (normalized to lowercase).
// 2. The body is parsed based strictly on `Content-Length` or `Transfer-Encoding: chunked`.
// 3. Network timeouts are enforced on both read and write operations.
//
// Complexity:
// - Time: `O(N)` where N is the length of the response payload (due to reading bytes).
// - Space: `O(N)` to buffer the response body and headers in memory.
//
// Tradeoffs:
// - **Synchronous Blocking:** This client blocks the current thread during network I/O.
//   It is not suitable for high-concurrency scraping where an async client (e.g., Tokio + Hyper)
//   would be required to multiplex connections.
// - **Memory Loading:** The entire response body is loaded into memory (`Vec<u8>`). A production
//   client would offer a streaming interface (`impl Read`) for large files to keep `O(1)` memory.

// =========================================================================================
// Traits & Types
// =========================================================================================

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
}

impl std::fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HttpMethod::Get => write!(f, "GET"),
            HttpMethod::Post => write!(f, "POST"),
            HttpMethod::Put => write!(f, "PUT"),
            HttpMethod::Delete => write!(f, "DELETE"),
        }
    }
}

/// Represents an HTTP/1.1 Request
pub struct HttpRequest {
    pub method: HttpMethod,
    pub url: String,
    pub headers: HashMap<String, String>,
    pub body: Option<Vec<u8>>,
}

/// Represents an HTTP/1.1 Response
#[derive(Debug)]
pub struct HttpResponse {
    pub status_code: u16,
    pub reason_phrase: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

/// A trait defining the core capabilities of an HTTP client.
pub trait HttpClient {
    fn send(&self, request: HttpRequest) -> io::Result<HttpResponse>;
}

// =========================================================================================
// Implementation
// =========================================================================================

/// A synchronous HTTP client over raw TCP.
pub struct TcpHttpClient {
    timeout: Duration,
}

impl TcpHttpClient {
    #[must_use]
    pub fn new(timeout: Duration) -> Self {
        Self { timeout }
    }

    /// Helper to parse a URL into host, port, and path.
    /// Note: A real implementation would use the `url` crate.
    fn parse_url(url: &str) -> io::Result<(String, u16, String)> {
        let url = url.strip_prefix("http://").unwrap_or(url);

        let mut parts = url.splitn(2, '/');
        let host_port = parts.next().unwrap_or("");
        let path = format!("/{}", parts.next().unwrap_or(""));

        let mut host_port_parts = host_port.splitn(2, ':');
        let host = host_port_parts.next().unwrap_or("").to_string();
        let port = match host_port_parts.next() {
            Some(p) => u16::from_str(p)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Invalid port"))?,
            None => 80,
        };

        if host.is_empty() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid URL"));
        }

        Ok((host, port, path))
    }
}

impl HttpClient for TcpHttpClient {
    fn send(&self, request: HttpRequest) -> io::Result<HttpResponse> {
        let (host, port, path) = Self::parse_url(&request.url)?;
        let addr = format!("{}:{}", host, port);

        // RUST INSIGHT:
        // By using `ToSocketAddrs`, we seamlessly support DNS resolution. If `host` is a domain name,
        // it will resolve to an IP address blocking the current thread until resolution completes.
        let mut stream = TcpStream::connect(addr)?;
        stream.set_read_timeout(Some(self.timeout))?;
        stream.set_write_timeout(Some(self.timeout))?;

        // 1. Construct Request
        let mut req_bytes = Vec::new();

        // Request Line
        write!(&mut req_bytes, "{} {} HTTP/1.1\r\n", request.method, path)?;

        // Host Header (Required by HTTP/1.1)
        write!(&mut req_bytes, "Host: {}\r\n", host)?;

        // User Headers
        for (k, v) in &request.headers {
            write!(&mut req_bytes, "{}: {}\r\n", k, v)?;
        }

        // Body Headers
        if let Some(body) = &request.body {
            if !request
                .headers
                .keys()
                .any(|k| k.eq_ignore_ascii_case("content-length"))
            {
                write!(&mut req_bytes, "Content-Length: {}\r\n", body.len())?;
            }
        }

        // End of Headers
        write!(&mut req_bytes, "Connection: close\r\n\r\n")?;

        // 2. Send Request
        stream.write_all(&req_bytes)?;
        if let Some(body) = &request.body {
            stream.write_all(body)?;
        }
        stream.flush()?;

        // 3. Read Response
        // GOTCHA: We must use a BufReader to safely read line-by-line without consuming the body bytes.
        let mut reader = BufReader::new(stream);
        let mut line = String::new();

        // Read Status Line
        reader.read_line(&mut line)?;
        if line.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Empty response",
            ));
        }

        // Parse Status Line (e.g., "HTTP/1.1 200 OK")
        let mut parts = line.splitn(3, ' ');
        let _version = parts
            .next()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Missing HTTP version"))?;
        let status_code_str = parts
            .next()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Missing status code"))?;
        let status_code = u16::from_str(status_code_str)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid status code"))?;
        let reason_phrase = parts.next().unwrap_or("").trim().to_string();

        // Read Headers
        let mut headers = HashMap::new();
        loop {
            line.clear();
            reader.read_line(&mut line)?;
            if line == "\r\n" || line == "\n" {
                break; // End of headers
            }

            if let Some((k, v)) = line.split_once(':') {
                headers.insert(k.trim().to_lowercase(), v.trim().to_string());
            }
        }

        // Read Body
        let mut body = Vec::new();

        // RUST INSIGHT:
        // Safely handling the body requires checking both chunked encoding and content-length.
        // We avoid underflow/overflow panics by explicitly handling `parse()` errors.
        if let Some(enc) = headers.get("transfer-encoding") {
            if enc.eq_ignore_ascii_case("chunked") {
                loop {
                    line.clear();
                    reader.read_line(&mut line)?;
                    let chunk_size_str = line.trim().split(';').next().unwrap_or("");
                    let chunk_size = usize::from_str_radix(chunk_size_str, 16).map_err(|_| {
                        io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!("Invalid chunk size: {}", chunk_size_str),
                        )
                    })?;

                    if chunk_size == 0 {
                        // Read trailing headers if any (usually just \r\n)
                        reader.read_line(&mut line)?;
                        break;
                    }

                    // GOTCHA: Read exactly `chunk_size` bytes. Using `read_to_end` here would hang or corrupt data.
                    let mut chunk = vec![0; chunk_size];
                    reader.read_exact(&mut chunk)?;
                    body.extend_from_slice(&chunk);

                    // Consume the trailing \r\n after the chunk data
                    let mut crlf = [0; 2];
                    reader.read_exact(&mut crlf)?;
                }
            } else {
                // Unknown transfer encoding, fallback to reading to end
                reader.read_to_end(&mut body)?;
            }
        } else if let Some(len_str) = headers.get("content-length") {
            let len = usize::from_str(len_str).map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidData, "Invalid content-length")
            })?;
            body.resize(len, 0);
            reader.read_exact(&mut body)?;
        } else {
            // No length specified, read until connection closes (HTTP/1.0 style or Connection: close)
            reader.read_to_end(&mut body)?;
        }

        Ok(HttpResponse {
            status_code,
            reason_phrase,
            headers,
            body,
        })
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to `reqwest::blocking`:
// - `reqwest` uses a robust connection pool (keep-alive). This implementation establishes a new
//   TCP connection per request (`Connection: close`).
// - `reqwest` handles HTTPS (TLS) transparently via `rustls` or `native-tls`. This implementation
//   only supports plain HTTP.
// - `reqwest` supports redirects, cookies, and advanced proxy configurations.
//
// What's missing vs. production:
// 1. TLS/HTTPS Support (Crucial for the modern web).
// 2. Connection Pooling (Keep-Alive) to avoid TCP handshake overhead on sequential requests.
// 3. Streaming Response Bodies (returning an `impl Read` instead of collecting into `Vec<u8>`).
// 4. Robust URL parsing (handling query parameters, encoding).
//
// Suggested next steps:
// 1. Add `rustls` to support `https://` URLs.
// 2. Implement a `ConnectionPool` that hands out active `TcpStream`s keyed by host.

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::thread;

    fn spawn_test_server(handler: impl FnMut(TcpStream) + Send + 'static) -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        let mut handler = handler;
        thread::spawn(move || {
            if let Ok((stream, _)) = listener.accept() {
                handler(stream);
            }
        });

        port
    }

    #[test]
    fn test_parse_url() {
        assert_eq!(
            TcpHttpClient::parse_url("http://example.com").unwrap(),
            ("example.com".to_string(), 80, "/".to_string())
        );

        assert_eq!(
            TcpHttpClient::parse_url("http://example.com:8080/api/v1").unwrap(),
            ("example.com".to_string(), 8080, "/api/v1".to_string())
        );

        assert_eq!(
            TcpHttpClient::parse_url("example.com/path").unwrap(),
            ("example.com".to_string(), 80, "/path".to_string())
        );
    }

    #[test]
    fn test_basic_get_request() {
        let port = spawn_test_server(|mut stream| {
            let mut buf = [0; 1024];
            stream.read(&mut buf).unwrap();

            let response = "HTTP/1.1 200 OK\r\nContent-Length: 13\r\n\r\nHello, World!";
            stream.write_all(response.as_bytes()).unwrap();
        });

        let client = TcpHttpClient::new(Duration::from_secs(1));
        let req = HttpRequest {
            method: HttpMethod::Get,
            url: format!("http://127.0.0.1:{}", port),
            headers: HashMap::new(),
            body: None,
        };

        let res = client.send(req).unwrap();
        assert_eq!(res.status_code, 200);
        assert_eq!(res.reason_phrase, "OK");
        assert_eq!(res.body, b"Hello, World!");
    }

    #[test]
    fn test_chunked_transfer_encoding() {
        let port = spawn_test_server(|mut stream| {
            let mut buf = [0; 1024];
            stream.read(&mut buf).unwrap();

            let response = "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n\
4\r\n\
Wiki\r\n\
5\r\n\
pedia\r\n\
E\r\n\
 in \r\nchunks.\r\n\
0\r\n\
\r\n";
            stream.write_all(response.as_bytes()).unwrap();

            // Allow the client time to read before we close the stream
            std::thread::sleep(Duration::from_millis(50));
        });

        let client = TcpHttpClient::new(Duration::from_secs(1));
        let req = HttpRequest {
            method: HttpMethod::Get,
            url: format!("http://127.0.0.1:{}", port),
            headers: HashMap::new(),
            body: None,
        };

        let res = client.send(req).unwrap();
        assert_eq!(res.status_code, 200);
        assert_eq!(res.body, b"Wikipedia in \r\nchunks.");
    }
}
