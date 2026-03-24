// Suppress pedantic and nursery lints for design pattern examples.
#![allow(clippy::pedantic, clippy::nursery, unused)]

//! # OOP Correction Factor Patterns
//!
//! Replaces: **Singleton**, **Observer**, **State/Runtime Validations**
//!
//! Real Rust usage: `std::sync::OnceLock`, `tokio::sync::broadcast`, `hyper::client::Builder`
//!
//! ## Why this pattern exists in Rust specifically
//! Half of the classic OOP patterns exist to work around limitations in C++ and Java
//! (lack of sum types, shared mutable state by default, no affine types). Rust's strict
//! ownership model, affine types (move semantics), and powerful enums fundamentally
//! shift the landscape.
//!
//! Here we explore the three categories of OOP patterns in Rust:
//! 1. **Dissolves Entirely:** Patterns that are replaced by language features or simpler paradigms (e.g., Singleton).
//! 2. **Transforms into Something Better:** Patterns that map to Rust's ownership model (e.g., Observer -> Channels).
//! 3. **Genuinely New:** Patterns born from Rust's unique affine type system (e.g., Typestate).
//!
//! ## Meta-Pattern: Make Illegal States Unrepresentable
//! Rust's deepest design pattern. If a pattern doesn't move invariants from runtime to
//! compile time, question whether it's earning its complexity.
//!
//! ## Architecture
//!
//! ### Type Relationship Diagrams
//!
//! **Category 1: Dissolved (Singleton)**
//! ```text
//! [ OOP Singleton ] --> Global mutable object with locks everywhere.
//! [ Rust Approach ] --> fn() + std::sync::OnceLock (Immutable global data).
//! ```
//!
//! **Category 2: Transformed (Observer)**
//! ```text
//! [ OOP Observer ] --> Subject holds `Vec<Box<dyn Observer>>`. Re-entrancy hell, dangling pointers.
//! [ Rust Approach ] --> Sender --> [ Channel ] --> Receiver. Decoupled ownership.
//! ```
//!
//! **Category 3: Genuinely New (Typestate)**
//! ```text
//! [ OOP State ] --> object.connect(); if !object.is_connected() throw Exception;
//! [ Rust Typestate ] --> UnconnectedState --connect()--> ConnectedState (Returns a new type).
//! ```
//!
//! **Invariants Enforced:**
//! - **Typestate:** Compile-time guarantee that you cannot call `.send_data()` on an unconnected socket.
//! - **Channels:** Compile-time guarantee against data races during event notification.
//!
//! **Anti-patterns:**
//! - Forcing `Arc<Mutex<T>>` to build a Singleton object instead of passing state down.
//! - Storing a list of `Rc<RefCell<dyn Observer>>` to simulate OOP event listeners.
//! - Using boolean flags (`is_initialized`) inside structs instead of consuming the struct to produce a new type.

use std::sync::{OnceLock, mpsc};
use std::thread;

// ============================================================================
// Category 1: Dissolves Entirely — The Singleton Anti-Pattern
// ============================================================================

// ANTI-PATTERN: `class Config { private static Config instance; ... }`
// The OOP Singleton forces global mutable state, leading to hidden dependencies and testing nightmares.
// In Rust, global mutable state requires `unsafe` or `Arc<Mutex<T>>` overhead.

// PRODUCTION NOTE: If you truly need global immutable initialization, use `std::sync::OnceLock`.
// If you need global mutable state, you are likely designing it wrong. Pass a context struct instead.

static GLOBAL_CONFIG: OnceLock<String> = OnceLock::new();

pub fn initialize_system(config_value: String) {
    // GOTCHA: OnceLock can only be set once. Subsequent sets are ignored/return an Err.
    let _ = GLOBAL_CONFIG.set(config_value);
}

pub fn get_config() -> &'static str {
    // OWNERSHIP INSIGHT: We return a static string slice. The memory is tied to the lifetime
    // of the program, requiring no locks for reads after initialization.
    GLOBAL_CONFIG.get().map(|s| s.as_str()).unwrap_or("default")
}

// ============================================================================
// Category 2: Transforms into Something Better — Observer vs. Channels
// ============================================================================

// ANTI-PATTERN:
// ```java
// class Subject {
//     List<Observer> listeners;
//     void notify() { for (var l : listeners) l.update(); }
// }
// ```
// In Rust, storing a list of mutable trait objects (`Vec<Rc<RefCell<dyn Observer>>>`) fights the
// borrow checker, causes runtime panics on re-entrant calls, and leaks memory if cycles form.

// TRADEOFF: Channels decouple the sender and receiver. The sender doesn't know who is listening,
// but you lose the synchronous, blocking nature of OOP observers (which is usually a good thing).

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    UserLoggedIn(String),
    ServerShutdown,
}

pub struct EventBus {
    sender: mpsc::Sender<Event>,
    receiver: mpsc::Receiver<Event>,
}

impl EventBus {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self { sender, receiver }
    }

    pub fn sender(&self) -> mpsc::Sender<Event> {
        self.sender.clone()
    }

    // OWNERSHIP INSIGHT: The listener takes ownership of the Receiver.
    // Rust's channels enforce that only one entity can receive from a standard `mpsc` channel.
    // (Use `tokio::sync::broadcast` for multi-consumer scenarios in production).
    pub fn into_receiver(self) -> mpsc::Receiver<Event> {
        self.receiver
    }
}

// ============================================================================
// Category 3: Genuinely New — Typestate (Make Illegal States Unrepresentable)
// ============================================================================

// ANTI-PATTERN: The OOP way:
// ```cpp
// class Connection {
//     bool is_connected = false;
//     void connect() { is_connected = true; }
//     void send() { if (!is_connected) throw Error(); ... }
// }
// ```
// This relies on RUNTIME validation. If the programmer forgets the `if`, the program crashes.

// COMPILE-TIME WIN: By using Rust's affine types (move semantics), we consume the old state
// and return a completely new type. It is statically impossible to call `send` on an `Unconnected` state.

pub struct Unconnected;
pub struct Connected;

/// A network connection that explicitly models its state in the type system.
pub struct Connection<State> {
    url: String,
    // Zero-sized type marker. Compiles away completely.
    _state: std::marker::PhantomData<State>,
}

impl Connection<Unconnected> {
    pub fn new(url: &str) -> Self {
        Self {
            url: url.to_string(),
            _state: std::marker::PhantomData,
        }
    }

    // OWNERSHIP INSIGHT: `self` is consumed by value. The `Unconnected` instance ceases to exist.
    pub fn connect(self) -> Result<Connection<Connected>, String> {
        if self.url.is_empty() {
            return Err("Invalid URL".to_string());
        }
        // Return the new state type
        Ok(Connection {
            url: self.url,
            _state: std::marker::PhantomData,
        })
    }
}

impl Connection<Connected> {
    // This method ONLY exists on `Connection<Connected>`.
    pub fn send(&self, data: &str) -> String {
        format!("Sending '{}' to {}", data, self.url)
    }

    // OWNERSHIP INSIGHT: Consumes `self` again, reverting to the Unconnected type.
    pub fn disconnect(self) -> Connection<Unconnected> {
        Connection {
            url: self.url,
            _state: std::marker::PhantomData,
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dissolved_singleton() {
        // Uninitialized state
        // assert_eq!(get_config(), "default"); // Assuming tests run isolated

        initialize_system("production_db".to_string());

        // State is now locked in globally
        assert_eq!(get_config(), "production_db");

        // Subsequent initializations are ignored
        initialize_system("test_db".to_string());
        assert_eq!(get_config(), "production_db");
    }

    #[test]
    fn test_transformed_observer() {
        let bus = EventBus::new();
        let sender1 = bus.sender();
        let sender2 = bus.sender();
        let receiver = bus.into_receiver();

        // Simulate multiple subjects emitting events
        sender1
            .send(Event::UserLoggedIn("Alice".to_string()))
            .unwrap();
        sender2.send(Event::ServerShutdown).unwrap();

        // Listener processes events sequentially
        assert_eq!(
            receiver.recv().unwrap(),
            Event::UserLoggedIn("Alice".to_string())
        );
        assert_eq!(receiver.recv().unwrap(), Event::ServerShutdown);
    }

    #[test]
    fn test_genuinely_new_typestate() {
        let conn = Connection::<Unconnected>::new("tcp://localhost:8080");

        // COMPILE ERROR if uncommented:
        // conn.send("Hello"); // Error: no method named `send` found for struct `Connection<Unconnected>`

        let connected = conn.connect().unwrap();

        // Valid because the type is now Connection<Connected>
        let response = connected.send("PING");
        assert_eq!(response, "Sending 'PING' to tcp://localhost:8080");

        let disconnected = connected.disconnect();

        // COMPILE ERROR if uncommented:
        // disconnected.send("PING"); // Error: no method named `send` found for struct `Connection<Unconnected>`

        // Re-connect
        let _reconnected = disconnected.connect().unwrap();
    }
}

// ============================================================================
// Footer
// ============================================================================
//
// How this pattern appears in std/major crates:
// - **Typestate:** `std::fs::File` (OpenOptions builder), `hyper` HTTP builders.
// - **Channels:** `std::sync::mpsc`, `tokio::sync::{mpsc, broadcast, watch}`.
// - **OnceLock:** `std::sync::OnceLock`, `lazy_static`, `once_cell`.
//
// When to reach for this vs simpler alternatives:
// - Use Typestate when invalid state transitions are catastrophic (e.g., launching a rocket without fuel).
// - Use Channels when you need pub/sub decoupled from lifetimes.
// - Avoid singletons unless absolutely necessary for external C-FFI or truly static configuration.
