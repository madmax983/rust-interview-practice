//! # HTTP Server Implementation
//!
//! A minimal multithreaded HTTP/1.1 server from scratch without external dependencies.
//!
//! **Replaces Crates:** `hyper`, `actix-web`, `axum`
//!
//! **Real-world Usage:**
//! - REST APIs
//! - Microservices
//! - Static file servers
//!
//! **Why build it yourself?**
//! Building an HTTP server from scratch demystifies network programming. It teaches you how TCP sockets
//! work, how to parse text-based protocols like HTTP/1.1, and how to manage concurrency with thread pools
//! rather than relying on heavy frameworks.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────┐        ┌──────────────┐        ┌──────────────┐
//! │ TcpListener │───────►│ Thread Pool  │───────►│ HTTP Handler │
//! │ (Accepts)   │        │ (Workers)    │        │ (Routing)    │
//! └─────────────┘        └──────────────┘        └──────────────┘
//! ```
//!
//! **Invariants:**
//! 1. Threads must be reused via a pool to prevent unbounded thread creation.
//! 2. Requests must read Headers fully before processing.
//! 3. Responses must include proper Content-Length and CRLF formatting.
//!
//! **Time/Space Complexity:**
//! - Accept Connection: O(1) time
//! - Request Parse: O(N) where N is header size
//! - Routing: O(1) time for basic paths
//!
//! **Design Decisions and Tradeoffs:**
//! - Uses a basic fixed-size thread pool.
//! - Uses `BufReader` mapped onto `&mut TcpStream` to read lines efficiently without cloning the stream.
//!
//! # Comparison to Canonical Crates
//! - **hyper**: Asynchronous (Tokio), handles HTTP/2, streaming bodies, connection pooling.
//! - **Missing here**: Async IO, Keep-Alive, HTTP/2, Chunked Transfer Encoding.
//! - **Next Steps**: Integrate with the custom async runtime.

// PRODUCTION NOTE: For benchmarking, use a tool like `wrk` or `oha` with
// many concurrent connections to test the thread pool saturation and latency.
// A microbenchmark with `criterion` on the request parser could test raw throughput.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;

// =========================================================================================
// Data Types
// =========================================================================================

pub struct HttpRequest {
    pub method: String,
    pub path: String,
    pub headers: HashMap<String, String>,
}

pub struct HttpResponse {
    pub status_code: u16,
    pub status_text: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl HttpResponse {
    #[must_use]
    pub fn new(status_code: u16, status_text: &str, body: Vec<u8>) -> Self {
        let mut headers = HashMap::new();
        headers.insert("Content-Length".to_string(), body.len().to_string());
        Self {
            status_code,
            status_text: status_text.to_string(),
            headers,
            body,
        }
    }
}

// =========================================================================================
// Thread Pool
// =========================================================================================

type Job = Box<dyn FnOnce() + Send + 'static>;

struct Worker {
    _id: usize,
    thread: Option<thread::JoinHandle<()>>,
}

impl Worker {
    fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Job>>>) -> Self {
        let thread = thread::spawn(move || {
            loop {
                let job = {
                    let guard = receiver.lock().unwrap();
                    match guard.recv() {
                        Ok(j) => j,
                        Err(_) => break,
                    }
                };

                job();
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
    #[must_use]
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
        if let Some(sender) = &self.sender {
            let _ = sender.send(job);
        }
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        drop(self.sender.take());
        for worker in &mut self.workers {
            if let Some(thread) = worker.thread.take() {
                let _ = thread.join();
            }
        }
    }
}

// =========================================================================================
// HTTP Handler & Server
// =========================================================================================

pub trait HttpHandler: Send + Sync + 'static {
    fn handle(&self, req: HttpRequest) -> HttpResponse;
}

pub struct HttpServer {
    listener: TcpListener,
    handler: Arc<dyn HttpHandler>,
    pool: ThreadPool,
}

impl HttpServer {
    /// # Panics
    /// Panics if the address cannot be bound.
    #[must_use]
    pub fn new(addr: &str, handler: impl HttpHandler, workers: usize) -> Self {
        let listener = TcpListener::bind(addr).expect("Should bind to address");
        Self {
            listener,
            handler: Arc::new(handler),
            pool: ThreadPool::new(workers),
        }
    }

    pub fn run_once(&self) {
        if let Ok((mut stream, _)) = self.listener.accept() {
            let handler = Arc::clone(&self.handler);
            self.pool.execute(move || {
                Self::handle_connection(&mut stream, &*handler);
            });
        }
    }

    fn handle_connection(stream: &mut TcpStream, handler: &dyn HttpHandler) {
        let mut reader = BufReader::new(&mut *stream);
        let mut request_line = String::new();

        // RUST INSIGHT: We avoid cloning the TcpStream by mapping a BufReader onto a
        // mutable reference `&mut *stream`. We must explicitly drop `reader` later to reclaim the stream for writing.
        if reader.read_line(&mut request_line).is_err() || request_line.is_empty() {
            return;
        }

        let parts: Vec<&str> = request_line.split_whitespace().collect();
        if parts.len() != 3 {
            return;
        }

        let method = parts[0].to_string();
        let path = parts[1].to_string();

        let mut headers = HashMap::new();
        loop {
            let mut header_line = String::new();
            // GOTCHA: HTTP headers end with a blank line (CRLF). We must break the loop when an empty line is encountered.
            if reader.read_line(&mut header_line).is_err() || header_line.trim().is_empty() {
                break;
            }
            if let Some(colon_idx) = header_line.find(':') {
                let key = header_line[..colon_idx].trim().to_string();
                let value = header_line[colon_idx + 1..].trim().to_string();
                headers.insert(key, value);
            }
        }

        drop(reader);

        let request = HttpRequest {
            method,
            path,
            headers,
        };

        let response = handler.handle(request);

        let mut response_bytes = Vec::new();
        response_bytes.extend_from_slice(
            format!(
                "HTTP/1.1 {} {}\r\n",
                response.status_code, response.status_text
            )
            .as_bytes(),
        );
        for (k, v) in &response.headers {
            response_bytes.extend_from_slice(format!("{k}: {v}\r\n").as_bytes());
        }
        response_bytes.extend_from_slice(b"\r\n");
        response_bytes.extend_from_slice(&response.body);

        let _ = stream.write_all(&response_bytes);
    }
}

// =========================================================================================
// Tests
// =========================================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    struct TestHandler;
    impl HttpHandler for TestHandler {
        fn handle(&self, req: HttpRequest) -> HttpResponse {
            if req.path == "/" {
                HttpResponse::new(200, "OK", b"Hello, World!".to_vec())
            } else {
                HttpResponse::new(404, "Not Found", b"Not Found".to_vec())
            }
        }
    }

    #[test]
    fn test_http_server() {
        let server = HttpServer::new("127.0.0.1:0", TestHandler, 2);
        let addr = server.listener.local_addr().unwrap();

        let handle = thread::spawn(move || {
            server.run_once();
        });

        let mut stream = TcpStream::connect(addr).unwrap();
        stream
            .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .unwrap();

        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();

        assert!(response.contains("HTTP/1.1 200 OK"));
        assert!(response.contains("Hello, World!"));

        let _ = handle.join();
    }
}
