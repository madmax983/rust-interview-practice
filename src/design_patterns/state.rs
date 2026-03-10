// Suppress pedantic and nursery lints for design pattern examples.
#![allow(clippy::pedantic, clippy::nursery, unused)]
//! # State Pattern (Dynamic State)
//!
//! Replaces: **State Pattern** (OOP / GoF)
//!
//! Real Rust usage: Implementing networking protocols, game entity AI, parsers.
//!
//! ## Why this pattern exists in Rust
//! In OOP, the State pattern allows an object to alter its behavior when its internal state changes,
//! usually by delegating to a `State` interface. The object appears to change its class.
//!
//! In Rust, we have two distinct ways to handle state:
//! 1. **Typestate (Static State):** Encoding state in the type system (`struct Order<Pending>`).
//!    This is covered in `typestate.rs`. It's safer but strictly checked at compile time.
//! 2. **Enum Dispatch (Dynamic State):** Using an `enum` to represent all possible states.
//!    This is the idiomatic translation of the dynamic GoF State pattern. It is closed
//!    (all states are known), highly optimized (no heap allocation or dynamic dispatch),
//!    and allows easy transitions.
//!
//! ## Architecture
//!
//! ```text
//! [ Context (Document) ] --owns--> [ State Enum (Draft | Moderation | Published) ]
//! ```
//!
//! **Invariants:**
//! - Transitions consume the current state and return the next state (or mutate in place).
//! - All possible states are known at compile time via the Enum.
//! - Exhaustive pattern matching (`match`) ensures no state is accidentally ignored.
//!
//! ## When to use
//! - When the state changes dynamically based on runtime input (e.g., parsers, game AI).
//! - When you have a fixed number of states and need to switch between them.
//!
//! ## Anti-patterns
//! - Using `Box<dyn State>` instead of an `enum`. In Rust, `enum` is almost always preferred
//!   over trait objects for State because the number of states is typically finite and closed.
//!   Trait objects require heap allocation and scatter the state transition logic.

// ============================================================================
// Approach: Enum Dispatch for Dynamic State
// ============================================================================

// COMPILE-TIME WIN: An enum ensures exhaustive checking. If we add an `Archived` state later,
// the compiler will force us to handle it in every match statement.
#[derive(Debug, PartialEq, Clone)]
pub enum State {
    Draft,
    Moderation,
    Published,
}

pub struct Post {
    state: State,
    content: String,
}

impl Post {
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: State::Draft,
            content: String::new(),
        }
    }

    /// Add text to the post. Only allowed in Draft state.
    pub fn add_text(&mut self, text: &str) {
        // TRADEOFF: Unlike Typestate, this requires a runtime check.
        // But it allows the Post struct to remain a single, unified type.
        if let State::Draft = self.state {
            self.content.push_str(text);
        }
    }

    /// Request a review, moving from Draft to Moderation.
    pub fn request_review(&mut self) {
        // State transition happens here.
        if let State::Draft = self.state {
            self.state = State::Moderation;
        }
    }

    /// Approve the post, moving from Moderation to Published.
    pub fn approve(&mut self) {
        if let State::Moderation = self.state {
            self.state = State::Published;
        }
    }

    /// Get the content of the post. Only visible if Published.
    #[must_use]
    pub fn content(&self) -> &str {
        // OWNERSHIP INSIGHT: We match on the enum to determine behavior.
        // GOTCHA: If we used Trait Objects here, we'd have to downcast or have the trait method return an Option,
        // which gets messy. Enum matching is clean and exhaustive.
        match self.state {
            State::Published => &self.content,
            _ => "", // Empty if not published
        }
    }

    #[must_use]
    pub const fn current_state(&self) -> &State {
        &self.state
    }
}

impl Default for Post {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Alternative Approach: Advanced Enum with State Data
// ============================================================================

// Sometimes, states carry their own specific data that shouldn't exist in other states.
// Rust enums handle this brilliantly without needing separate classes.

// META-PATTERN: "Make illegal states unrepresentable"
// Notice how `token` only exists in the `Connected` state, and `retries` only exists in the
// `Connecting` state. It is physically impossible to access a `token` while `Disconnected`,
// preventing a whole class of runtime logic errors.
#[derive(Debug, PartialEq)]
pub enum ConnectionState {
    Disconnected,
    Connecting { ip: String, retries: u8 },
    Connected { ip: String, token: String },
}

pub struct Connection {
    state: ConnectionState,
}

impl Connection {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: ConnectionState::Disconnected,
        }
    }

    pub fn connect(&mut self, ip: &str) {
        self.state = ConnectionState::Connecting {
            ip: ip.to_string(),
            retries: 0,
        };
    }

    pub fn handle_response(&mut self, success: bool, token: &str) {
        // We use `std::mem::replace` to take ownership of the inner state data
        // while leaving a dummy state in its place temporarily, avoiding a borrow checker error.
        let current_state = std::mem::replace(&mut self.state, ConnectionState::Disconnected);

        self.state = match (current_state, success) {
            (ConnectionState::Connecting { ip, .. }, true) => ConnectionState::Connected {
                ip,
                token: token.to_string(),
            },
            (ConnectionState::Connecting { ip, retries }, false) if retries < 3 => {
                ConnectionState::Connecting {
                    ip,
                    retries: retries + 1,
                }
            }
            // Fallback for max retries or invalid transitions
            _ => ConnectionState::Disconnected,
        };
    }
}

impl Default for Connection {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Footer
// ============================================================================
//
// How this pattern appears in std/crates:
// - Everywhere! Enums are the primary way to manage finite state in Rust.
// - `std::task::Poll`: Represents the state of a Future (`Ready` or `Pending`).
//
// What the GoF/OOP equivalent is and why it doesn't translate directly:
// In OOP, State is typically implemented as an Interface (`State`) with classes for each state
// (`DraftState`, `PublishedState`) and the Context holding a reference to the Interface.
// Rust avoids this overhead (dynamic dispatch, heap allocation, scattered logic) by using Enums.
//
// When to reach for this vs. simpler alternatives:
// Reach for dynamic enum-based state when state transitions happen at runtime and the context needs
// to remain a single, unified type (e.g., storing all connections in a single `Vec<Connection>`).
// If the state is known at compile time and you want the highest level of safety, use the Typestate pattern instead.
//
// Suggested combinations with other patterns in this collection:
// - **Typestate Pattern**: The static, compile-time equivalent of this pattern.
//
// PRODUCTION NOTE:
// In more complex state machines (e.g., game AI or network protocols), the transition logic
// can become very large. In those cases, you might still use an enum, but delegate the *logic*
// for each state to separate structs or functions to keep the `match` block manageable.

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_post_workflow() {
        let mut post = Post::new();
        assert_eq!(post.current_state(), &State::Draft);

        post.add_text("I ate a salad for lunch today");
        assert_eq!(post.content(), ""); // Not published yet

        post.request_review();
        assert_eq!(post.current_state(), &State::Moderation);

        // Cannot add text in moderation
        post.add_text(" and it was good");

        post.approve();
        assert_eq!(post.current_state(), &State::Published);
        assert_eq!(post.content(), "I ate a salad for lunch today");
    }

    #[test]
    fn test_connection_state_machine() {
        let mut conn = Connection::new();
        assert_eq!(conn.state, ConnectionState::Disconnected);

        conn.connect("192.168.1.1");
        assert_eq!(
            conn.state,
            ConnectionState::Connecting {
                ip: "192.168.1.1".to_string(),
                retries: 0
            }
        );

        // Fail once
        conn.handle_response(false, "");
        assert_eq!(
            conn.state,
            ConnectionState::Connecting {
                ip: "192.168.1.1".to_string(),
                retries: 1
            }
        );

        // Succeed
        conn.handle_response(true, "abcdef");
        assert_eq!(
            conn.state,
            ConnectionState::Connected {
                ip: "192.168.1.1".to_string(),
                token: "abcdef".to_string()
            }
        );
    }
}
