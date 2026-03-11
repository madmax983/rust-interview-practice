// Suppress pedantic and nursery lints for design pattern examples.
#![allow(clippy::pedantic, clippy::nursery, unused)]

//! # Drop Bomb Pattern
//!
//! Replaces: **Runtime Assertions**, **`finally` blocks**
//!
//! Real Rust usage: `drop_bomb` crate, `tracing::span::Entered` (panics if dropped out of order), `crossbeam::epoch::Guard`
//!
//! ## Why this pattern exists in Rust
//! Rust relies heavily on RAII (Resource Acquisition Is Initialization) and the `Drop` trait.
//! A "Drop Bomb" is an emergent pattern used when you must ensure an object is explicitly handled
//! before it goes out of scope, but you cannot enforce this via the type system (Typestate) because
//! of dynamic control flow or complex borrowing rules.
//!
//! The Drop Bomb sets a `defused = false` flag on creation. If the object is dropped before the flag
//! is set to `true` via an explicit method call (like `commit()` or `abort()`), the `Drop` implementation
//! panics, loudly alerting the developer to the logic error.
//!
//! ## Architecture
//!
//! **Approach: Defusable Guard**
//! ```text
//! let bomb = Transaction::begin();
//! // ... do work ...
//! bomb.commit(); // Sets `defused = true`
//! // If we return early before commit(), the bomb panics on drop.
//! ```
//!
//! **Invariants:**
//! - The object must be explicitly consumed or modified before dropping.
//! - The `Drop` implementation acts as a mandatory runtime assertion.
//!
//! ## When to use
//! - Ensuring that a user explicitly handles an error or commits a transaction.
//! - Validating that scopes or guards are entered and exited in the correct order (e.g., in testing frameworks or logging tracing).
//!
//! ## Anti-patterns
//! - Using a Drop Bomb when a static Typestate pattern (consuming `self` and returning a different type) would suffice and catch the error at compile time.

// ============================================================================
// Approach: The Drop Bomb
// ============================================================================

/// Represents a critical operation that must be either committed or rolled back explicitly.
///
/// **OWNERSHIP INSIGHT:**
/// We don't want the user to just let this struct drop. They must explicitly acknowledge
/// the outcome. We track this using the `defused` boolean.
pub struct Transaction {
    name: String,
    defused: bool,
}

impl Transaction {
    #[must_use]
    pub fn begin(name: impl Into<String>) -> Self {
        println!("Transaction started.");
        Self {
            name: name.into(),
            defused: false,
        }
    }

    /// Explicitly commit the transaction, defusing the bomb.
    pub fn commit(mut self) {
        println!("Transaction '{}' committed.", self.name);
        self.defused = true;
        // `self` will drop at the end of this scope, but `defused` is true, so it's safe.
    }

    /// Explicitly rollback the transaction, defusing the bomb.
    pub fn rollback(mut self) {
        println!("Transaction '{}' rolled back.", self.name);
        self.defused = true;
    }
}

// COMPILE-TIME WIN vs RUNTIME: We'd prefer to use Typestate to force `commit` to be called
// by making `Transaction` unusable otherwise. But what if the user just ignores the `Transaction`
// entirely? (`let _ = Transaction::begin();`). Typestate can't force them to call a method.
// The `#[must_use]` attribute helps, but it only issues a warning.
// The Drop Bomb guarantees a loud failure.
impl Drop for Transaction {
    fn drop(&mut self) {
        if !self.defused && !std::thread::panicking() {
            // GOTCHA: We check `!std::thread::panicking()` to avoid double-panicking.
            // If the thread is already panicking due to another error, we don't want to
            // abort the entire process by panicking again during the unwinding phase.
            panic!(
                "DROP BOMB EXPLODED: Transaction '{}' was dropped without being committed or rolled back!",
                self.name
            );
        } else if !self.defused {
            // Silently swallow the error because we are already panicking.
            println!("Transaction '{}' dropped during unwind.", self.name);
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
    fn test_defused_commit() {
        let tx = Transaction::begin("DbUpdate");
        tx.commit();
        // Drop is called, but defused is true. Test passes.
    }

    #[test]
    fn test_defused_rollback() {
        let tx = Transaction::begin("DbUpdate");
        tx.rollback();
        // Drop is called, but defused is true. Test passes.
    }

    #[test]
    #[should_panic(expected = "DROP BOMB EXPLODED")]
    fn test_explosion_on_early_return() {
        let _tx = Transaction::begin("ExplosiveOperation");

        // Simulating an early return or forgotten commit
        // _tx goes out of scope here. The Drop Bomb will explode!
    }

    // PRODUCTION NOTE: How to test the `!std::thread::panicking()` branch?
    // We can spawn a thread that deliberately panics *before* the transaction is defused.
    // The transaction should drop silently during the unwind, without causing a double panic (abort).
    #[test]
    #[should_panic(expected = "Intentional Panic")]
    fn test_silent_drop_during_panic() {
        let _tx = Transaction::begin("WillPanic");

        panic!("Intentional Panic");
        // During unwinding, `_tx` is dropped. It notices the thread is panicking
        // and skips its own panic. The test catches the "Intentional Panic".
    }
}

// ============================================================================
// Footer
// ============================================================================
//
// How this pattern appears in std/crates:
// - `tracing`: Uses Drop Bombs to ensure that `Entered` spans are exited in the reverse order they were entered.
//
// What the GoF/OOP equivalent is and why it doesn't translate directly:
// This is a uniquely Rust/C++ pattern arising from deterministic destruction (RAII). Garbage-collected
// languages cannot rely on finalizers running predictably, so they rely on `try-with-resources` or `using` blocks.
