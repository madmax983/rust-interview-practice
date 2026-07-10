//! # Remote Procedure Call (RPC) Framework
//!
//! Implements a minimal, synchronous Remote Procedure Call (RPC) server and client from scratch.
//! It supports multiplexed client requests over a single TCP connection and dynamic method dispatch on the server.
//!
//! **Replaces Crates:** `tonic`, `tarpc`, `jsonrpsee`
//!
//! **Real-world Usage:**
//! - Microservice to microservice communication
//! - Language-agnostic API endpoints (when standard formats like JSON-RPC or gRPC are used)
//! - Internal control planes for distributed systems
//!
//! **Why build it yourself?**
//! Building an RPC framework teaches you how to map local function calls into network packets and back.
//! You learn about framing (how to separate messages in a continuous TCP stream), multiplexing (routing multiple asynchronous requests over one connection using correlation IDs), and dynamic dispatch (invoking registered functions by string name).

use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock, mpsc};
use std::thread;

/// Maximum accepted payload length for a single RPC frame (64 MB). The length
/// prefix is attacker-controlled; bounding it before `vec![0; len]` prevents a
/// huge value from panicking (capacity overflow) or aborting (OOM).
const MAX_FRAME_SIZE: usize = 64 * 1024 * 1024;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure Diagram:
//
//      Client                                Server
//      ======                                ======
//      Call(Method, Args) ──┐
//                           ▼
//  [mpsc::Sender]       [TCP Stream]      [TCP Stream]
//     Wait on RX ◄──┐       │                   │
//                   │       ▼                   ▼
//  [Background   ◄──┴── [Reader]          [Dispatcher] ──► Lookup Handler in HashMap
//    Thread]            (Matches ID)            │          Execute Handler
//                                               ▼
//                       [Writer] ◄────────[Response]
//
// Protocol Framing (Text-based for simplicity, like JSON-RPC but manual):
// [ID]\n[Method]\n[Payload Length]\n[Payload Bytes]
//
// Invariants:
// 1. Every request must have a unique ID per client connection.
// 2. The server must respond with the exact ID it received.
// 3. The client's background reader must route the response to the correct waiting channel based on the ID.
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ Call (Client) │ O(Payload)  │ O(Payload)  │
// │ Dispatch (Srv)│ O(1) Lookup │ O(1) Lookup │
// └───────────────┴─────────────┴─────────────┘
//
// Design Decisions:
// - **Framing**: Newline-delimited text header + sized binary payload.
//   - *Alternative*: Fully binary (e.g., Protobuf/gRPC) or fully text (JSON). We chose a hybrid for easy parsing without dependencies.
// - **Multiplexing**: The client spawns a background thread to read from the TCP stream and uses a `HashMap` of channels to wake up waiting callers.
//   - *Tradeoff*: Requires thread synchronization (`Mutex`) on the write path and a dedicated read thread.
// - **Server Concurrency**: Spawns a new thread per connection.
//   - *Alternative*: Tokio/async for high concurrency without OS thread overhead.

/// Represents an RPC Request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpcRequest {
    pub id: u64,
    pub method: String,
    pub payload: Vec<u8>,
}

/// Represents an RPC Response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpcResponse {
    pub id: u64,
    pub is_error: bool,
    pub payload: Vec<u8>,
}

/// A handler trait for dynamic dispatch on the server.
pub trait RpcHandler: Send + Sync + 'static {
    /// Handles a request payload and produces a response payload.
    ///
    /// # Errors
    /// Returns an error string if the handler fails to process the request.
    fn handle(&self, payload: &[u8]) -> Result<Vec<u8>, String>;
}

impl<F> RpcHandler for F
where
    F: Fn(&[u8]) -> Result<Vec<u8>, String> + Send + Sync + 'static,
{
    fn handle(&self, payload: &[u8]) -> Result<Vec<u8>, String> {
        self(payload)
    }
}

// =========================================================================================
// Wire Protocol Parsing & Serialization
// =========================================================================================

impl RpcRequest {
    /// Serializes the request to a byte buffer.
    fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        // Format: ID\nMETHOD\nPAYLOAD_LEN\nPAYLOAD
        write!(
            &mut buf,
            "{}\n{}\n{}\n",
            self.id,
            self.method,
            self.payload.len()
        )
        .unwrap();
        buf.extend_from_slice(&self.payload);
        buf
    }

    /// Parses a request from a buffered reader.
    fn parse<R: Read>(reader: &mut BufReader<R>) -> io::Result<Option<Self>> {
        let mut id_str = String::new();
        if reader.read_line(&mut id_str)? == 0 {
            return Ok(None); // EOF
        }
        let id = id_str
            .trim()
            .parse::<u64>()
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid ID"))?;

        let mut method = String::new();
        reader.read_line(&mut method)?;
        let method = method.trim().to_string();

        let mut len_str = String::new();
        reader.read_line(&mut len_str)?;
        let len = len_str
            .trim()
            .parse::<usize>()
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid Length"))?;

        // SECURITY: Bound the untrusted length prefix before allocating.
        if len > MAX_FRAME_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Frame payload too large",
            ));
        }

        let mut payload = vec![0; len];
        reader.read_exact(&mut payload)?;

        Ok(Some(Self {
            id,
            method,
            payload,
        }))
    }
}

impl RpcResponse {
    /// Serializes the response to a byte buffer.
    fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        // Format: ID\nIS_ERROR\nPAYLOAD_LEN\nPAYLOAD
        let err_flag = i32::from(self.is_error);
        write!(
            &mut buf,
            "{}\n{}\n{}\n",
            self.id,
            err_flag,
            self.payload.len()
        )
        .unwrap();
        buf.extend_from_slice(&self.payload);
        buf
    }

    /// Parses a response from a buffered reader.
    fn parse<R: Read>(reader: &mut BufReader<R>) -> io::Result<Option<Self>> {
        let mut id_str = String::new();
        if reader.read_line(&mut id_str)? == 0 {
            return Ok(None); // EOF
        }
        let id = id_str
            .trim()
            .parse::<u64>()
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid ID"))?;

        let mut err_str = String::new();
        reader.read_line(&mut err_str)?;
        let is_error = err_str.trim() == "1";

        let mut len_str = String::new();
        reader.read_line(&mut len_str)?;
        let len = len_str
            .trim()
            .parse::<usize>()
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid Length"))?;

        // SECURITY: Bound the untrusted length prefix before allocating.
        if len > MAX_FRAME_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Frame payload too large",
            ));
        }

        let mut payload = vec![0; len];
        reader.read_exact(&mut payload)?;

        Ok(Some(Self {
            id,
            is_error,
            payload,
        }))
    }
}

// =========================================================================================
// Server Implementation
// =========================================================================================

/// An RPC Server that dispatches incoming requests to registered handlers.
pub struct RpcServer {
    // RUST INSIGHT:
    // We use `Arc<RwLock>` because the handlers are shared across all connection threads.
    // An `RwLock` allows concurrent reads (method lookups) while permitting runtime updates if needed.
    handlers: Arc<RwLock<HashMap<String, Box<dyn RpcHandler>>>>,
}

impl Default for RpcServer {
    fn default() -> Self {
        Self::new()
    }
}

impl RpcServer {
    #[must_use]
    pub fn new() -> Self {
        Self {
            handlers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Registers a new handler for a specific method name.
    ///
    /// # Panics
    /// Panics if the internal handler lock is poisoned by another thread.
    pub fn register<H: RpcHandler>(&self, method: &str, handler: H) {
        self.handlers
            .write()
            .unwrap()
            .insert(method.to_string(), Box::new(handler));
    }

    /// Starts the server on the given address, blocking the current thread.
    ///
    /// # Errors
    /// Returns an error if the listener cannot bind to the given address.
    pub fn run<A: ToSocketAddrs>(&self, addr: A) -> io::Result<()> {
        let listener = TcpListener::bind(addr)?;

        // Ignore connections that fail to accept; keep serving the rest.
        for stream in listener.incoming().flatten() {
            let handlers = Arc::clone(&self.handlers);

            // GOTCHA: Unbounded thread spawning can lead to exhaustion.
            // PRODUCTION NOTE: A production RPC server uses a bounded thread pool or async tasks.
            thread::spawn(move || {
                if let Err(e) = Self::handle_client(stream, &handlers)
                    && e.kind() != io::ErrorKind::UnexpectedEof
                {
                    // Ignore standard disconnects, log others
                }
            });
        }
        Ok(())
    }

    fn handle_client(
        mut stream: TcpStream,
        handlers: &Arc<RwLock<HashMap<String, Box<dyn RpcHandler>>>>,
    ) -> io::Result<()> {
        let mut reader = BufReader::new(stream.try_clone()?);

        loop {
            let request = match RpcRequest::parse(&mut reader) {
                Ok(Some(req)) => req,
                Ok(None) => return Ok(()), // Clean disconnect
                Err(e) => return Err(e),
            };

            // Dispatch
            let response_payload = {
                let handlers_read = handlers.read().unwrap();
                if let Some(handler) = handlers_read.get(&request.method) {
                    handler.handle(&request.payload)
                } else {
                    Err(format!("Method '{}' not found", request.method))
                }
            };

            // Construct response
            let response = match response_payload {
                Ok(payload) => RpcResponse {
                    id: request.id,
                    is_error: false,
                    payload,
                },
                Err(err_msg) => RpcResponse {
                    id: request.id,
                    is_error: true,
                    payload: err_msg.into_bytes(),
                },
            };

            stream.write_all(&response.serialize())?;
        }
    }
}

// =========================================================================================
// Client Implementation
// =========================================================================================

impl Drop for RpcClient {
    fn drop(&mut self) {
        let stream = self.write_stream.lock().unwrap();
        let _ = stream.shutdown(std::net::Shutdown::Both);
    }
}

type CallbackMap = Arc<Mutex<HashMap<u64, mpsc::Sender<RpcResponse>>>>;

/// An RPC Client that multiplexes requests over a single connection.
pub struct RpcClient {
    next_id: AtomicU64,
    write_stream: Mutex<TcpStream>,
    callbacks: CallbackMap,
    /// Set to true by the background reader thread when it exits (EOF or error),
    /// so `call()` can fail fast instead of blocking forever on a dead connection.
    closed: Arc<AtomicBool>,
}

impl RpcClient {
    /// Connects to an RPC server at the given address.
    ///
    /// # Errors
    /// Returns an error if the TCP connection cannot be established or the
    /// stream cannot be cloned for the background reader thread.
    ///
    /// # Panics
    /// The background reader thread panics if the internal callbacks lock is poisoned.
    pub fn connect<A: ToSocketAddrs>(addr: A) -> io::Result<Self> {
        let stream = TcpStream::connect(addr)?;
        let read_stream = stream.try_clone()?;
        let write_stream = Mutex::new(stream);
        let callbacks = Arc::new(Mutex::new(HashMap::<u64, mpsc::Sender<RpcResponse>>::new()));

        let closed = Arc::new(AtomicBool::new(false));

        // Spawn background reader thread to demultiplex responses
        let callbacks_clone = Arc::clone(&callbacks);
        let closed_clone = Arc::clone(&closed);
        thread::spawn(move || {
            let mut reader = BufReader::new(read_stream);
            // Loop until EOF (`Ok(None)`) or a connection error stops the parse.
            while let Ok(Some(response)) = RpcResponse::parse(&mut reader) {
                let mut map = callbacks_clone.lock().unwrap();
                if let Some(sender) = map.remove(&response.id) {
                    let _ = sender.send(response); // Ignore if caller hung up
                }
            }

            // BUGFIX: The reader thread is exiting (connection closed or errored).
            // Mark the client closed and drop every pending sender by draining the
            // callbacks map. Dropping the `Sender`s makes any blocked `rx.recv()`
            // in `call()` return `Err` immediately instead of hanging forever, and
            // the `closed` flag lets subsequent calls fail fast.
            closed_clone.store(true, Ordering::Release);
            let mut map = callbacks_clone.lock().unwrap();
            map.clear(); // drops all Senders -> wakes all blocked receivers with Err
        });

        Ok(Self {
            next_id: AtomicU64::new(1),
            write_stream,
            callbacks,
            closed,
        })
    }

    /// Calls a remote method synchronously.
    ///
    /// # Errors
    /// Returns an error if the connection is closed, the request cannot be
    /// written, the connection drops before a response arrives, or the remote
    /// handler returns an error.
    ///
    /// # Panics
    /// Panics if the internal callbacks or write-stream lock is poisoned.
    pub fn call(&self, method: &str, payload: &[u8]) -> Result<Vec<u8>, String> {
        // Fail fast if the reader thread has already observed a closed connection.
        if self.closed.load(Ordering::Acquire) {
            return Err("Connection closed".to_string());
        }

        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let request = RpcRequest {
            id,
            method: method.to_string(),
            payload: payload.to_vec(),
        };

        // Create a channel for this specific request
        let (tx, rx) = mpsc::channel();

        {
            let mut map = self.callbacks.lock().unwrap();
            map.insert(id, tx);
        }

        // Re-check after inserting: the reader thread may have drained the map
        // (dropping our sender) in the window between our first check and the
        // insert. Without this, such a call would block on `recv()` forever.
        if self.closed.load(Ordering::Acquire) {
            self.callbacks.lock().unwrap().remove(&id);
            return Err("Connection closed".to_string());
        }

        // Send request over the wire
        {
            let mut stream = self.write_stream.lock().unwrap();
            if stream.write_all(&request.serialize()).is_err() {
                self.callbacks.lock().unwrap().remove(&id);
                return Err("Failed to write to connection".to_string());
            }
        }

        // Wait for response from background thread
        match rx.recv() {
            Ok(response) => {
                if response.is_error {
                    Err(String::from_utf8_lossy(&response.payload).to_string())
                } else {
                    Ok(response.payload)
                }
            }
            Err(_) => Err("Connection closed before response received".to_string()),
        }
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `tonic`: A fully featured gRPC framework over HTTP/2 with async/await support, utilizing
//   Protobuf for strict typing and code generation.
// - `tarpc`: A Rust-specific RPC framework focusing on macros for generating traits and async handling.
// - `jsonrpsee`: Focused specifically on the JSON-RPC 2.0 standard, widely used in blockchain (Substrate).
//
// Missing vs. Production:
// - **Async I/O**: Our implementation blocks OS threads. Production uses `tokio` for scalability.
// - **Connection Resilience**: No automatic retries, keep-alives, or heartbeat mechanisms.
// - **Timeouts**: The client `recv()` will block forever if the server drops the request. Production uses `recv_timeout` or async timeouts.
// - **Serialization Integration**: We pass raw `&[u8]`. Production frameworks integrate directly with Serde or Protobuf to pass strongly typed structs.
//
// Next Steps:
// 1. Add `Duration` timeouts to the client `call` method to prevent infinite hangs.
// 2. Port the internal `TcpStream` handlers to use `tokio::net::TcpStream` for non-blocking IO.
// 3. Integrate with the `serde_framework.rs` module to allow passing Rust types directly.

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_rpc_request_serialization() {
        let req = RpcRequest {
            id: 42,
            method: "echo".to_string(),
            payload: b"hello".to_vec(),
        };
        let bytes = req.serialize();

        let mut reader = BufReader::new(bytes.as_slice());
        let parsed = RpcRequest::parse(&mut reader).unwrap().unwrap();

        assert_eq!(req, parsed);
    }

    #[test]
    fn test_rpc_response_serialization() {
        let resp = RpcResponse {
            id: 99,
            is_error: true,
            payload: b"error message".to_vec(),
        };
        let bytes = resp.serialize();

        let mut reader = BufReader::new(bytes.as_slice());
        let parsed = RpcResponse::parse(&mut reader).unwrap().unwrap();

        assert_eq!(resp, parsed);
    }

    #[test]
    fn test_parse_rejects_huge_length_prefix() {
        // A malicious peer claims a payload of usize::MAX bytes. Before the fix
        // this hit `vec![0; usize::MAX]` -> capacity-overflow panic. Now both
        // request and response parsers must reject it without allocating.
        let req_frame = b"1\nfoo\n18446744073709551615\n";
        let mut reader = BufReader::new(req_frame.as_slice());
        let err = RpcRequest::parse(&mut reader).expect_err("huge len must error");
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);

        let resp_frame = b"1\n0\n18446744073709551615\n";
        let mut reader = BufReader::new(resp_frame.as_slice());
        let err = RpcResponse::parse(&mut reader).expect_err("huge len must error");
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn test_call_errors_when_connection_closed() {
        // Regression: when the background reader thread exits (server closed the
        // connection), a blocked/subsequent `call()` must return Err promptly
        // instead of hanging forever on `rx.recv()`.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        // Server accepts one connection then immediately closes it (EOF to client).
        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            drop(stream);
        });

        let client = RpcClient::connect(addr).expect("Failed to connect");
        server.join().unwrap();

        // Run call() on a worker thread and use recv_timeout as a safety net so a
        // regression (unbounded hang) fails the test rather than hanging the suite.
        let (done_tx, done_rx) = mpsc::channel();
        thread::spawn(move || {
            let result = client.call("echo", b"hi");
            let _ = done_tx.send(result);
        });

        match done_rx.recv_timeout(Duration::from_secs(5)) {
            Ok(result) => assert!(result.is_err(), "call() must error on a closed connection"),
            Err(_) => panic!("call() hung: reader exit did not wake the blocked caller"),
        }
    }

    #[test]
    fn test_rpc_client_server_integration() {
        // Setup Server
        let server = Arc::new(RpcServer::new());

        server.register("ping", |payload: &[u8]| {
            let req_str = std::str::from_utf8(payload).unwrap();
            if req_str == "ping" {
                Ok(b"pong".to_vec())
            } else {
                Err("Invalid payload".to_string())
            }
        });

        server.register("uppercase", |payload: &[u8]| {
            let req_str = std::str::from_utf8(payload).unwrap();
            Ok(req_str.to_uppercase().into_bytes())
        });

        // Find an open port
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let server_clone = Arc::clone(&server);
        thread::spawn(move || {
            // We use the already bound listener to ensure port is known
            for stream in listener.incoming() {
                let stream = stream.unwrap();
                let handlers = Arc::clone(&server_clone.handlers);
                thread::spawn(move || {
                    let _ = RpcServer::handle_client(stream, &handlers);
                });
            }
        });

        // Give server a tiny moment to start listening
        thread::sleep(Duration::from_millis(50));

        // Setup Client
        let client = RpcClient::connect(addr).expect("Failed to connect");

        // Test Success case
        let res = client.call("ping", b"ping").unwrap();
        assert_eq!(res, b"pong");

        let res = client.call("uppercase", b"hello world").unwrap();
        assert_eq!(res, b"HELLO WORLD");

        // Test Application Error case
        let err = client.call("ping", b"wrong").unwrap_err();
        assert_eq!(err, "Invalid payload");

        // Test Unknown Method case
        let err = client.call("unknown", b"").unwrap_err();
        assert_eq!(err, "Method 'unknown' not found");

        // Test multiplexing: send multiple requests from different threads using same client
        let client = Arc::new(client);
        let mut handles = vec![];

        for i in 0..10 {
            let c = Arc::clone(&client);
            handles.push(thread::spawn(move || {
                let payload = format!("val:{}", i);
                let res = c.call("uppercase", payload.as_bytes()).unwrap();
                let expected = format!("VAL:{}", i);
                assert_eq!(String::from_utf8(res).unwrap(), expected);
            }));
        }

        for h in handles {
            h.join().unwrap();
        }
    }
}
