//! # Null Object Pattern
//!
//! **Replaces:** OOP Null Object Pattern
//! **Real-world usage:** `std::option::Option`, virtually every Rust crate.
//!
//! **Why this pattern exists in Rust:**
//! In classical OOP (like Java or C++), null references are a billion-dollar mistake.
//! To avoid `NullPointerException`s without littering code with `if (obj != null)` checks,
//! OOP developers use the Null Object Pattern: implementing an interface with a "do nothing" class.
//!
//! In Rust, the Null Object pattern dissolves entirely. Rust doesn't have null references;
//! instead, it has the `Option<T>` enum. The compiler *forces* you to handle the `None` case,
//! rendering the traditional Null Object pattern obsolete and an anti-pattern in Rust.
//!
//! ## Architecture
//!
//! ```text
//! OOP Null Object:
//! Interface -> RealObject
//!           -> NullObject (No-op methods)
//!
//! Rust Idiomatic:
//! Option<T> -> Some(T) (Real Object)
//!           -> None    (Compile-time enforced "nothing")
//! ```
//!
//! **Invariants Enforced:**
//! - **Compile-time Null Safety:** You cannot accidentally use a null value. The compiler enforces exhaustive pattern matching or explicit unwrapping.
//! - **No "Ghost" Objects:** You don't have objects in memory that exist solely to do nothing.
//!
//! **When to use:** Always, when a value might be absent. Use `Option<T>`.
//! **When it's overkill:** Never. It's the standard library way.
//!
//! ## Anti-Pattern
//!
//! A naive translation of the OOP Null Object pattern into Rust involves creating a trait and a "No-op" struct.
//! This defeats the purpose of Rust's explicit null safety (`Option`) and adds unnecessary boilerplate and dynamic dispatch overhead (if using trait objects).
//!
//! ```rust
//! // ANTI-PATTERN: Doing it the OOP way in Rust
//! trait Logger {
//!     fn log(&self, msg: &str);
//! }
//!
//! struct ConsoleLogger;
//! impl Logger for ConsoleLogger {
//!     fn log(&self, msg: &str) {
//!         println!("Log: {}", msg);
//!     }
//! }
//!
//! // The Null Object
//! struct NullLogger;
//! impl Logger for NullLogger {
//!     fn log(&self, _msg: &str) {
//!         // Do nothing
//!     }
//! }
//! ```
//!
//! ## Idiomatic Rust Implementation
//!
//! In Rust, we just use `Option<T>` and let combinators or `if let` handle the "do nothing" branch naturally.

/// A simple struct representing a real object that does work.
pub struct RealWorker {
    name: String,
}

impl RealWorker {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }

    /// Performs the work.
    pub fn do_work(&self) -> String {
        format!("Worker {} is doing work", self.name)
    }
}

/// A service that might or might not have a worker.
/// Instead of a `Box<dyn Worker>` where one implementation is a `NullWorker`,
/// we use `Option<RealWorker>`.
// COMPILE-TIME WIN: If the worker is absent, it's explicitly typed as `None`.
// You cannot accidentally call `.do_work()` on it without handling the `Option`.
pub struct Service {
    worker: Option<RealWorker>,
}

impl Service {
    #[must_use]
    pub const fn new(worker: Option<RealWorker>) -> Self {
        Self { worker }
    }

    /// Executes the worker if it exists, otherwise does nothing (or handles the absence).
    pub fn execute(&self) -> Option<String> {
        // OWNERSHIP INSIGHT: We borrow the worker inside the Option using `as_ref()`.
        // Then we use `map` which is a functional combinator that only runs the closure if it's `Some`.
        // This effectively replaces the polymorphic dispatch of the Null Object pattern
        // with a compile-time checked, statically dispatched conditional execution.
        self.worker.as_ref().map(RealWorker::do_work)
    }

    /// Alternative way using `if let` which is very common when side-effects are needed
    /// instead of returning a value.
    pub fn execute_with_side_effect(&self, output: &mut Vec<String>) {
        if let Some(worker) = &self.worker {
            output.push(worker.do_work());
        }
        // If it's `None`, we naturally do nothing. No need for a Null Object to "swallow" the call.
    }
}

// GOTCHA: Sometimes, an API *requires* a type to implement a trait (e.g., `std::io::Write`).
// If you genuinely need a "do nothing" implementation of a trait to pass into a generic function
// that expects that trait, `std` sometimes provides it (e.g., `std::io::sink()`).
// But this is for satisfying trait bounds, not for avoiding null checks.

// PRODUCTION NOTE: For function arguments, instead of passing an `Option`,
// consider using the `Default` trait if a meaningful default exists,
// or just require the concrete type and let the caller decide whether to call the function at all.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_with_real_worker() {
        let worker = RealWorker::new("Alice");
        let service = Service::new(Some(worker));

        assert_eq!(service.execute(), Some("Worker Alice is doing work".to_string()));

        let mut out = Vec::new();
        service.execute_with_side_effect(&mut out);
        assert_eq!(out, vec!["Worker Alice is doing work".to_string()]);
    }

    #[test]
    fn test_with_no_worker() {
        // This is the Rust equivalent of passing the "Null Object".
        // It's just `None`.
        let service = Service::new(None);

        // The method naturally returns `None` instead of doing a "no-op" and returning some default.
        assert_eq!(service.execute(), None);

        let mut out = Vec::new();
        service.execute_with_side_effect(&mut out);
        // The output remains untouched.
        assert!(out.is_empty());
    }
}

// RUST INSIGHT: Notice there is no "Footer" module documentation block required structurally
// at the bottom of the file in Rust. We can just use regular comments to finish the thought.
//
// **Standard Library Equivalents:**
// - `std::option::Option` is the ultimate replacement for the Null Object pattern.
// - `std::io::sink()` provides a true "null object" for the `Write` trait when you literally need to discard writes.
//
// **GoF Equivalent:**
// The Null Object Pattern (sometimes considered a special case of the State or Strategy pattern).
// It doesn't translate directly because Rust's type system handles absence (`Option`) natively and safely,
// whereas Java/C++ rely on references that can secretly be null.
//
// **When to reach for this vs. simpler alternatives:**
// Always use `Option`. It is the simpler alternative. Never implement a custom "Null Object" struct
// unless you are fulfilling a specific, external trait bound requirement where an `Option` cannot be used
// (like passing a no-op writer to a function expecting `impl std::io::Write`).
//
// **Suggested combinations:**
// - Use with the **State Pattern** (Enum Dispatch) to model "No State" as an explicit variant if `Option` alone isn't expressive enough.