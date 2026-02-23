//! # Builder Pattern (Typestate Variant)
//!
//! Replaces: **Telescoping Constructor**, **Builder Pattern** (GoF, runtime checks)
//!
//! Real Rust usage: `std::process::Command`, `reqwest::ClientBuilder`, `typed-builder` crate
//!
//! ## Why this pattern exists in Rust
//! The classic Builder pattern often postpones validation until `.build()` is called, resulting in runtime errors (`Result`).
//! Rust's Typestate pattern allows us to enforce required fields at compile time. If you forget a required field,
//! the code simply doesn't compile.
//!
//! ## Architecture
//!
//! ```text
//! ServerBuilder::new()          -> ServerBuilder<NoPort>
//!      .port(8080)              -> ServerBuilder<HasPort>
//!      .build()                 -> Server
//! ```
//!
//! **Invariants:**
//! - `build()` cannot be called until all required fields are set.
//! - Optional fields (like `host`) don't change the type state.
//!
//! ## When to use
//! - When constructing complex objects with many parameters.
//! - When some parameters are mandatory and others optional.
//! - To guarantee a valid object upon construction (Parse, don't validate).

use std::marker::PhantomData;

// ============================================================================
// State Markers
// ============================================================================

pub struct NoPort;
pub struct HasPort;

// ============================================================================
// The Builder
// ============================================================================

pub struct ServerBuilder<P> {
    host: String,
    port: Option<u16>,
    // Marker for port state
    _marker: PhantomData<P>,
}

impl ServerBuilder<NoPort> {
    pub fn new() -> Self {
        ServerBuilder {
            host: "127.0.0.1".to_string(),
            port: None,
            _marker: PhantomData,
        }
    }

    /// Sets the port. Transitions state from NoPort to HasPort.
    pub fn port(self, port: u16) -> ServerBuilder<HasPort> {
        ServerBuilder {
            host: self.host,
            port: Some(port),
            _marker: PhantomData,
        }
    }
}

// Methods available in any state
impl<P> ServerBuilder<P> {
    /// Sets the host (optional field).
    pub fn host(mut self, host: impl Into<String>) -> Self {
        self.host = host.into();
        self
    }
}

// Methods available only when fully configured
impl ServerBuilder<HasPort> {
    pub fn build(self) -> Server {
        // OWNERSHIP INSIGHT: We consume the builder to produce the Server.
        // No runtime validation needed for 'port' because the type guarantees it exists.
        Server {
            host: self.host,
            port: self.port.unwrap(), // Safe unwrap
        }
    }
}

// ============================================================================
// The Product
// ============================================================================

#[derive(Debug)]
pub struct Server {
    host: String,
    port: u16,
}

impl Server {
    pub fn start(&self) {
        println!("Server listening on {}:{}", self.host, self.port);
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_success() {
        let server = ServerBuilder::new().host("localhost").port(8080).build();

        assert_eq!(server.port, 8080);
        assert_eq!(server.host, "localhost");
    }

    #[test]
    fn test_builder_default_host() {
        let server = ServerBuilder::new().port(3000).build();

        assert_eq!(server.host, "127.0.0.1");
        assert_eq!(server.port, 3000);
    }

    // COMPILE-TIME WIN:
    // fn test_missing_port() {
    //     let server = ServerBuilder::new()
    //         .host("localhost")
    //         .build(); // Error: method `build` not found for `ServerBuilder<NoPort>`
    // }
}
