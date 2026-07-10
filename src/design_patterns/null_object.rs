// Suppress pedantic and nursery lints for design pattern examples.
#![allow(clippy::pedantic, clippy::nursery, unused)]

//! # Null Object Pattern
//!
//! Replaces: **Null Object Pattern** (OOP / GoF)
//!
//! Real Rust usage: Everywhere, dissolved entirely into `Option<T>`.
//!
//! ## Why this pattern exists in Rust
//! In OOP, the Null Object pattern is used to avoid null reference exceptions by providing a default
//! object that implements an interface but does nothing. This prevents constant `if obj != null` checks.
//!
//! In Rust, **this pattern dissolves entirely**. Rust does not have `null`. The concept of absence
//! is handled strictly through the `Option<T>` enum. The compiler forces you to handle the `None` case,
//! rendering Null Object implementations redundant and unidiomatic.
//!
//! ## Architecture
//!
//! **Approach 1: Idiomatic Rust (`Option<T>`)**
//! ```text
//! [ Client ] --calls--> [ Option<Logger>::map(|l| l.log()) ]
//! ```
//!
//! **Approach 2: Trait Objects (The OOP translation - Anti-Pattern)**
//! ```text
//! [ trait Logger ]
//!        |
//!        +---> [ ConsoleLogger ]
//!        +---> [ NullLogger ]
//! ```
//!
//! **Invariants:**
//! - Absence of a value is encoded in the type system as `Option::None`.
//! - Methods cannot be called on a "null" object without explicit matching or combinators (`map`, `and_then`).
//!
//! ## When to use
//! - Always use `Option<T>` when a value might be absent.
//! - Use the Null Object trait implementation ONLY if you are forced to provide a concrete type to an external library that does not accept `Option`.
//!
//! ## Anti-patterns
//! - Creating a `NullObject` struct that implements a trait with empty methods, just to avoid using `Option<T>`.

// ============================================================================
// Approach 1: Idiomatic Rust (Option<T>)
// ============================================================================

pub struct ConsoleLogger;

impl ConsoleLogger {
    pub fn log(&self, msg: &str) {
        println!("LOG: {msg}");
    }
}

pub struct Application {
    // OWNERSHIP INSIGHT: The logger is owned, but might not exist.
    // We don't need a heap-allocated `Box<dyn Logger>` just to allow a null state.
    logger: Option<ConsoleLogger>,
}

impl Application {
    #[must_use]
    pub const fn new(logger: Option<ConsoleLogger>) -> Self {
        Self { logger }
    }

    pub fn do_work(&self) {
        // COMPILE-TIME WIN: We cannot call `.log()` on `self.logger` directly.
        // We must use `if let` or a combinator, making the null-check explicit and enforced.
        if let Some(logger) = &self.logger {
            logger.log("Work started");
        }
    }
}

// ============================================================================
// Approach 2: The OOP Translation (Anti-Pattern in Rust)
// ============================================================================

// ANTI-PATTERN: This forces dynamic dispatch (`Box<dyn Trait>`) and heap allocation
// just to avoid an `Option`.

pub trait Logger {
    fn log(&self, msg: &str);
}

pub struct StdoutLogger;
impl Logger for StdoutLogger {
    fn log(&self, msg: &str) {
        println!("{msg}");
    }
}

pub struct NullLogger;
impl Logger for NullLogger {
    // Empty implementation
    fn log(&self, _msg: &str) {}
}

pub struct LegacyApplication {
    logger: Box<dyn Logger>,
}

impl LegacyApplication {
    #[must_use]
    pub fn new(logger: Box<dyn Logger>) -> Self {
        Self { logger }
    }

    pub fn do_work(&self) {
        // No check needed, but we paid for dynamic dispatch.
        self.logger.log("Legacy work started");
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    // test-code: these tests assert "no panic occurred"; the `assert!(true)` markers are illustrative.
    #![allow(clippy::assertions_on_constants)]

    use super::*;

    #[test]
    fn test_idiomatic_null_object() {
        let app_with_logger = Application::new(Some(ConsoleLogger));
        // Prints "LOG: Work started"
        app_with_logger.do_work();

        let app_without_logger = Application::new(None);
        // Does nothing, safely handled by Option
        app_without_logger.do_work();

        // Assertions are implicit by the lack of panics
        assert!(true);
    }

    #[test]
    fn test_anti_pattern_null_object() {
        let app = LegacyApplication::new(Box::new(NullLogger));
        app.do_work(); // Does nothing

        assert!(true);
    }
}

// ============================================================================
// Footer
// ============================================================================
//
// How this pattern appears in std/crates:
// - Everywhere! `Option` is one of the foundational enums in Rust.
//
// What the GoF/OOP equivalent is and why it doesn't translate directly:
// OOP lacks an ergonomic `Option` type that forces exhaustiveness checking, leading to the Null Object pattern.
// Rust's `Option` provides zero-cost abstractions to handle absence safely.
