//! # Drop Bomb Pattern
//!
//! **Replaces:** No direct GoF equivalent. This is a genuinely emergent Rust pattern.
//! **Real-world usage:** `std::thread::JoinGuard` (pre-1.0), `rocket` (routing constraints), `diesel` (transaction management).
//!
//! **Why this pattern exists in Rust:**
//! Rust's typestate pattern is excellent for moving invariants to compile time.
//! However, sometimes typestate is too restrictive or cumbersome (e.g., when a value must
//! conditionally perform an action before being destroyed, but tracking that state at compile time
//! across complex control flow is impossible).
//!
//! The Drop Bomb pattern enforces *deterministic runtime invariants*. It works by asserting
//! that a required action occurred via a `defused` flag, and *panicking* in the `Drop` implementation
//! if the struct is dropped without the action being taken.
//!
//! ## Architecture
//!
//! ```text
//! Struct (The Bomb)
//! ├── data
//! ├── defused: bool = false
//! ├── fn commit(mut self) -> Self.defused = true
//! └── Drop -> if !self.defused { panic!("Bomb exploded!") }
//! ```
//!
//! **Invariants Enforced:**
//! - **Guaranteed Action:** A specific method *must* be called before the object is destroyed.
//! - Prevents silently ignoring critical operations like committing a transaction, replying to a network request, or acknowledging an event.
//!
//! **When to use:** When typestate (consuming `self` and returning a new type) is too ergonomically painful,
//! but you still must guarantee an API contract is fulfilled before an object goes out of scope.
//!
//! **When it's overkill:** Whenever Typestate (consuming `self` to enforce state transitions) works.
//! A compile-time error is always better than a runtime panic. Drop bombs are a fallback.
//!
//! ## Anti-Pattern
//!
//! The anti-pattern is relying purely on documentation or `#[must_use]` attributes,
//! which only warn if the *return value* is ignored, but do nothing if the user stores the
//! value and then forgets to call `.commit()` on it.
//!
//! ```rust
//! // ANTI-PATTERN: Relying only on must_use
//! #[must_use] // Only warns if the Transaction struct itself is instantly dropped
//! struct Transaction;
//!
//! impl Transaction {
//!     fn commit(&self) {} // No guarantee this is ever called!
//! }
//!
//! fn handle() {
//!     let tx = Transaction;
//!     // tx is dropped here. commit() was never called. Bug silently ignored.
//! }
//! ```
//!
//! ## Idiomatic Rust Implementation

/// A database transaction that MUST be explicitly committed or rolled back.
/// If it goes out of scope without either happening, it panics (drops a bomb)
/// to alert the developer of the bug.
pub struct Transaction {
    name: String,
    // The fuse.
    defused: bool,
}

impl Transaction {
    #[must_use]
    pub fn begin(name: impl Into<String>) -> Self {
        println!("Transaction started");
        Self {
            name: name.into(),
            defused: false, // The bomb is armed upon creation
        }
    }

    /// Commits the transaction and defuses the bomb.
    pub fn commit(mut self) {
        println!("Transaction '{}' committed", self.name);
        // Defuse the bomb so Drop doesn't panic.
        self.defused = true;
        // The value is dropped here, but `defused` is true, so it's safe.
    }

    /// Explicitly rolls back the transaction and defuses the bomb.
    pub fn rollback(mut self) {
        println!("Transaction '{}' rolled back", self.name);
        self.defused = true;
    }
}

impl Drop for Transaction {
    fn drop(&mut self) {
        // COMPILE-TIME WIN: None. This is explicitly a RUNTIME assertion.
        // TRADEOFF: We trade a compile-time guarantee (Typestate) for ergonomic runtime checking.

        // If we are already panicking, we don't want to panic again,
        // as a double-panic in Rust aborts the process immediately.
        if !self.defused && !std::thread::panicking() {
            // The bomb goes off!
            panic!(
                "Transaction '{}' was dropped without being explicitly committed or rolled back! \
                 This is a severe bug.",
                self.name
            );
        }
    }
}

// PRODUCTION NOTE: The check `!std::thread::panicking()` is crucial.
// If an error occurs and a panic starts unwinding the stack, dropping armed bombs
// along the way will cause a double-panic and an immediate abort, obscuring the original error.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transaction_commit_defuses_bomb() {
        let tx = Transaction::begin("test_commit");
        tx.commit(); // Bomb defused, no panic at end of scope.
    }

    #[test]
    fn test_transaction_rollback_defuses_bomb() {
        let tx = Transaction::begin("test_rollback");
        tx.rollback(); // Bomb defused, no panic at end of scope.
    }

    #[test]
    #[should_panic(expected = "Transaction 'test_panic' was dropped without being explicitly committed")]
    fn test_transaction_drop_bomb_explodes() {
        // We create a transaction but don't commit or rollback.
        let _tx = Transaction::begin("test_panic");

        // When _tx goes out of scope here, the Drop impl will panic.
    }
}

// **Standard Library Equivalents:**
// The standard library avoids this pattern in favor of `Result` and explicit typestates,
// though it was heavily debated for `std::thread::JoinGuard` before Rust 1.0.
//
// **GoF Equivalent:**
// None. This leverages RAII (Resource Acquisition Is Initialization) semantics unique
// to systems languages like C++ and Rust, combined with Rust's strict move semantics.
//
// **When to reach for this vs. simpler alternatives:**
// Use Typestate (consuming `self`) whenever possible. Only reach for a Drop Bomb when:
// 1. The object must be stored in a structure that cannot easily change type.
// 2. The required action happens conditionally across highly complex control flow where
//    proving completion to the compiler via Typestate is unergonomic or impossible.
//
// **Suggested combinations:**
// - Combine with the **Builder Pattern** to ensure `build()` is finally called.
// - Use within **RAII Guards** (like the Object Pool) to guarantee critical release operations happen or loudly fail.