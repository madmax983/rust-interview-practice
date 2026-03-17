// Suppress pedantic and nursery lints for design pattern examples.
#![allow(clippy::pedantic, clippy::nursery, unused)]

//! # Session Types Pattern
//!
//! Replaces: **Runtime State Machines** (OOP), **Boolean/Enum State Flags** (C/C++), **Invalid State Exceptions** (Java)
//!
//! Real Rust usage: `hyper` (HTTP upgrade protocols), `postgres` (connection state), `embedded-hal`
//!
//! ## Why this pattern exists in Rust specifically
//! In many languages, ensuring that a protocol (e.g., handshake -> send data -> close) is followed
//! exactly requires runtime assertions (e.g., `if state != Connected { throw Error }`). Rust's
//! affine type system (ownership) allows us to consume a state and return a entirely new type
//! representing the next state. If you try to use the old state, the compiler throws a "use after move" error.
//! This completely eliminates an entire class of runtime bugs.
//!
//! ## Architecture
//!
//! ```text
//! [ Init ] --(connect)--> [ Connected ]
//!                              |
//!                        (send_data)
//!                              |
//!                              V
//!                        [ Connected ]
//!                              |
//!                           (close)
//!                              |
//!                              V
//!                         [ Closed ]
//! ```
//!
//! **Invariants:**
//! - You cannot send data before connecting.
//! - You cannot close a connection twice.
//! - You cannot send data after closing.
//! - The protocol sequence is strictly enforced at compile time.
//!
//! ## When to use vs. when this pattern is overkill
//! - **Use:** When implementing complex network protocols, stateful APIs (like builders), or
//!   when the cost of an invalid state transition is high (e.g., embedded hardware sequences).
//! - **Overkill:** For simple objects where state doesn't dictate available operations, or
//!   where state transitions are dynamic and not known at compile time.
//!
//! ## Anti-patterns
//! - Using `struct Connection { state: Enum }` and checking the enum variant inside every method,
//!   which pushes errors to runtime and requires `Result` returns for logic that could be static.

// ============================================================================
// State Markers (Zero-Sized Types)
// ============================================================================

/// Represents an initial, unconnected state.
pub struct Init;

/// Represents an active, connected state ready for data transfer.
pub struct Connected;

/// Represents a safely closed state.
pub struct Closed;

// ============================================================================
// The Session Type Structure
// ============================================================================

/// A protocol handler whose capabilities depend on its generic state `S`.
pub struct Protocol<S> {
    // Shared data across all states (e.g., an underlying socket or buffer)
    url: String,

    // We use a zero-sized type marker to track the state at compile time without overhead.
    // OWNERSHIP INSIGHT: The marker consumes no memory at runtime.
    state: std::marker::PhantomData<S>,
}

// ============================================================================
// State Transitions
// ============================================================================

// 1. Initial State Implementation
impl Protocol<Init> {
    /// Creates a new protocol instance in the `Init` state.
    #[must_use]
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            state: std::marker::PhantomData,
        }
    }

    /// Transitions from `Init` to `Connected`.
    ///
    /// OWNERSHIP INSIGHT: We consume `self` (taking ownership) and return a new
    /// type `Protocol<Connected>`. The old `Protocol<Init>` is destroyed and can
    /// never be used again.
    #[must_use]
    pub fn connect(self) -> Protocol<Connected> {
        // PRODUCTION NOTE: In reality, this might return a Result<Protocol<Connected>, Error>
        // if the connection can fail.
        println!("Connecting to {}", self.url);
        Protocol {
            url: self.url,
            state: std::marker::PhantomData,
        }
    }
}

// 2. Connected State Implementation
impl Protocol<Connected> {
    /// Sends data over the active connection.
    ///
    /// COMPILE-TIME WIN: This method simply doesn't exist on `Protocol<Init>` or
    /// `Protocol<Closed>`. You cannot accidentally call it in the wrong state.
    pub fn send_data(&self, data: &str) {
        println!("Sending data to {}: {}", self.url, data);
    }

    /// Transitions from `Connected` to `Closed`.
    #[must_use]
    pub fn close(self) -> Protocol<Closed> {
        println!("Closing connection to {}", self.url);
        Protocol {
            url: self.url,
            state: std::marker::PhantomData,
        }
    }
}

// 3. Closed State Implementation
impl Protocol<Closed> {
    // TRADEOFF: We must write explicit code for the `Closed` state, even if it has no
    // methods. It increases boilerplate compared to a simple boolean flag.

    /// Reconnects, returning to the `Connected` state.
    #[must_use]
    pub fn reconnect(self) -> Protocol<Connected> {
        println!("Reconnecting to {}", self.url);
        Protocol {
            url: self.url,
            state: std::marker::PhantomData,
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
    fn test_valid_session_sequence() {
        // META-PATTERN: Make illegal states unrepresentable.
        // We start in the Init state.
        let session = Protocol::<Init>::new("https://rust-lang.org");

        // session.send_data("hello"); // ERROR: method not found in `Protocol<Init>`

        // Transition to Connected
        let session = session.connect();

        // Now we can send data
        session.send_data("hello");
        session.send_data("world");

        // Transition to Closed
        let session = session.close();

        // session.send_data("after close"); // ERROR: method not found in `Protocol<Closed>`
        // session.close(); // ERROR: method not found (already closed)

        // Transition back to Connected
        let session = session.reconnect();
        session.send_data("hello again");
    }

    // COMPILE-TIME WIN:
    // fn test_use_after_move() {
    //     let session = Protocol::<Init>::new("https://test.com");
    //     let connected = session.connect();
    //
    //     // GOTCHA: Attempting to use the old state variable
    //     // let err = session.connect(); // ERROR: use of moved value: `session`
    // }
}

// ============================================================================
// Footer
// ============================================================================
//
// How this pattern appears in std/crates:
// - `hyper`: Uses typestates extensively to ensure headers aren't modified after
//   the body has started streaming.
// - `typestate` crate: Provides macros to generate session types automatically.
//
// What the GoF/OOP equivalent is and why it doesn't translate directly:
// The OOP equivalent is the State Pattern, where an object holds a reference to a
// State interface. This requires heap allocation, dynamic dispatch (vtable), and
// runtime error handling for invalid operations. Rust's approach is a zero-cost
// abstraction resolved entirely by the compiler.
//
// When to reach for this vs. simpler alternatives:
// Reach for this when the sequence of operations is strictly defined and violating
// it represents a critical bug. For simple objects (like a UI toggle button), a
// standard `enum` with a `match` statement is far simpler and perfectly adequate.
//
// Suggested combinations with other patterns in this collection:
// - **Builder Pattern**: Typestate is heavily used in advanced builders to enforce
//   that required fields are set before `build()` can be called.
// - **RAII Guards**: A typestate could return a custom Drop guard when closing to
//   ensure external resources are cleaned up.
