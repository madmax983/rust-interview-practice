// Suppress pedantic and nursery lints for design pattern examples.
#![allow(clippy::pedantic, clippy::nursery, unused)]

//! # OOP Correction Factor Patterns
//!
//! Replaces: **Shared Mutable State** (Locks), **Inheritance** (Classes), **Exceptions** (Throws)
//!
//! Real Rust usage: `tokio::sync::mpsc`, `std::iter::Iterator`, `std::result::Result`
//!
//! ## Why these patterns exist in Rust
//! Many developers approach Rust with Java, Python, or C++ mental models. This file focuses on the highest
//! "correction factor" patterns—the areas where OOP thinking causes the most friction in Rust.
//!
//! ## Meta-Pattern: Make Illegal States Unrepresentable
//! Throughout these examples, notice how Rust uses its type system (Ownership, Traits, Enums, Lifetimes)
//! to move runtime errors (NullPointerExceptions, ConcurrentModificationExceptions) into compile-time guarantees.
//!
//! ## Architecture
//!
//! **Approach 1: Message Passing over Shared Mutable State**
//! - Instead of `Arc<Mutex<HashMap>>`, we use an Actor model with `mpsc` channels.
//!
//! **Approach 2: Composition and Traits over Inheritance**
//! - Instead of an Abstract Base Class `Animal`, we use `Trait` for behavior and `Enum` for data.
//!
//! **Approach 3: Result/Option over Exceptions**
//! - Instead of throwing exceptions, we return `Result` and force the caller to handle them.

use std::collections::HashMap;
use std::sync::mpsc;
use std::thread;

// ============================================================================
// Approach 1: Message Passing over Shared Mutable State
// ============================================================================

// ANTI-PATTERN: `Arc<Mutex<State>>` everywhere, leading to deadlocks and contention.
// PRODUCTION NOTE: In Rust, "Share XOR Mutate" is the rule. We transfer ownership of messages instead.

/// Messages that can be sent to the State Manager.
pub enum StateMessage {
    Set(String, u32),
    Get(String, mpsc::Sender<Option<u32>>),
}

/// An Actor that manages state without locks.
pub struct StateManager {
    receiver: mpsc::Receiver<StateMessage>,
    state: HashMap<String, u32>,
}

impl StateManager {
    #[must_use]
    pub fn new(receiver: mpsc::Receiver<StateMessage>) -> Self {
        Self {
            receiver,
            state: HashMap::new(),
        }
    }

    /// Run the actor loop. It exclusively owns `state`, so no locks are needed.
    pub fn run(mut self) {
        for msg in self.receiver {
            match msg {
                // OWNERSHIP INSIGHT: The actor takes ownership of the key and value.
                StateMessage::Set(key, value) => {
                    self.state.insert(key, value);
                }
                StateMessage::Get(key, respond_to) => {
                    let val = self.state.get(&key).copied();
                    // Ignore errors if the requester hung up
                    let _ = respond_to.send(val);
                }
            }
        }
    }
}

// ============================================================================
// Approach 2: Composition and Enums over Inheritance
// ============================================================================

// ANTI-PATTERN: Abstract Base Classes like `class Animal { virtual void speak() = 0; }`
// TRADEOFF: You cannot easily add new data variants without modifying the enum (closed set),
// but you gain exhaustive pattern matching.

/// The behavior of a sound-making entity.
pub trait MakesSound {
    fn speak(&self) -> String;
}

/// The data representation.
/// COMPILE-TIME WIN: Enums are closed sets. The compiler ensures we handle all variants in `match`.
pub enum Animal {
    Dog { name: String },
    Cat { lives_left: u8 },
    // META-PATTERN: Making illegal states unrepresentable. A Bird without wings is impossible here.
    Bird { wingspan_cm: u32 },
}

impl MakesSound for Animal {
    fn speak(&self) -> String {
        match self {
            Self::Dog { name } => format!("{name} says Woof!"),
            Self::Cat { lives_left } => format!("Meow! I have {lives_left} lives."),
            Self::Bird { wingspan_cm } => format!("Chirp! My wings are {wingspan_cm}cm long."),
        }
    }
}

// ============================================================================
// Approach 3: Result/Option over Exceptions
// ============================================================================

// ANTI-PATTERN: `throw new InvalidDataException();`
// COMPILE-TIME WIN: The caller is forced to acknowledge that this operation can fail.

#[derive(Debug, PartialEq)]
pub enum ParseError {
    EmptyString,
    InvalidNumber,
    NegativeValue,
}

/// Parses a string into a positive integer.
///
/// GOTCHA: Using `.unwrap()` in production code. Always handle the `Result` or propagate it with `?`.
///
/// # Errors
/// Returns `ParseError` if the string is empty, not a number, or negative.
pub fn parse_positive_int(input: &str) -> Result<u32, ParseError> {
    if input.trim().is_empty() {
        return Err(ParseError::EmptyString);
    }

    let num: i32 = input.parse().map_err(|_| ParseError::InvalidNumber)?;

    // META-PATTERN: Make illegal states unrepresentable. We return a `u32`, so a negative
    // result is impossible once this function succeeds.
    if num < 0 {
        return Err(ParseError::NegativeValue);
    }

    Ok(num as u32)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_passing() {
        let (tx, rx) = mpsc::channel();

        // Spawn the actor thread
        thread::spawn(move || {
            let manager = StateManager::new(rx);
            manager.run();
        });

        // Send a message to set state
        tx.send(StateMessage::Set("score".to_string(), 100))
            .unwrap();

        // Send a message to get state
        let (reply_tx, reply_rx) = mpsc::channel();
        tx.send(StateMessage::Get("score".to_string(), reply_tx))
            .unwrap();

        let response = reply_rx.recv().unwrap();
        assert_eq!(response, Some(100));
    }

    #[test]
    fn test_composition_over_inheritance() {
        let dog = Animal::Dog {
            name: "Rex".to_string(),
        };
        let cat = Animal::Cat { lives_left: 9 };

        assert_eq!(dog.speak(), "Rex says Woof!");
        assert_eq!(cat.speak(), "Meow! I have 9 lives.");
    }

    #[test]
    fn test_result_over_exceptions() {
        assert_eq!(parse_positive_int("42"), Ok(42));
        assert_eq!(parse_positive_int(""), Err(ParseError::EmptyString));
        assert_eq!(parse_positive_int("abc"), Err(ParseError::InvalidNumber));
        assert_eq!(parse_positive_int("-5"), Err(ParseError::NegativeValue));
    }
}
