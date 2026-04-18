//! # Dynamic Error Handling Framework
//!
//! Implements a dynamic error handling framework akin to `anyhow` or `eyre`.
//! It provides type erasure for errors, context attachment, and downcasting.
//!
//! **Replaces Crates:** `anyhow`, `eyre`
//!
//! **Real-world Usage:**
//! - Application-level error propagation where exact error types don't need to be strictly matched.
//! - Adding domain-specific context strings to lower-level I/O or parsing errors.
//! - Capturing stack traces for unrecoverable errors (simulated here via chaining).
//!
//! **Why build it yourself?**
//! Engineers use `anyhow::Result` everywhere but often don't understand how it erases types
//! using `Box<dyn std::error::Error>`, how downcasting works via `std::any::Any`,
//! or how extension traits are used to add `.context()` to standard `Result`s.
//! Building it demystifies trait objects and Rust's orphan rules.

use std::error::Error as StdError;
use std::fmt;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure:
//
//      Error (Opaque struct)
//      └── Box<dyn StdError> (Trait object)
//          ├── Option 1: Concrete Error (e.g., io::Error)
//          ├── Option 2: MessageError ("something went wrong")
//          └── Option 3: DynContextError ("failed to read file" -> Next Error)
//
// Invariants:
// 1. Any type that implements `std::error::Error + Send + Sync + 'static` can be converted into `Error`.
// 2. Context can be layered indefinitely by wrapping the existing `Box<dyn StdError>`.
// 3. Downcasting traverses the chain of errors (via `.source()`) to find a matching concrete type.
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ Create        │ O(1)        │ O(1) alloc  │
// │ Add Context   │ O(1)        │ O(1) alloc  │
// │ Downcast      │ O(N) chain  │ O(1)        │
// │ Display/Debug │ O(N) chain  │ O(1)        │
// └───────────────┴─────────────┴─────────────┘
//
// Design Decisions:
// - **Type Erasure**: `Box<dyn StdError + Send + Sync + 'static>`.
//   - *Tradeoff*: Requires heap allocation on the error path. The fast path (Ok) is unimpacted.
// - **Context**: Extension trait `ContextExt`.
//   - *Tradeoff*: Elegantly extends `Result`, but requires importing the trait into scope to use.
// - **Custom vs StdError**: `anyhow::Error` actually *doesn't* implement `std::error::Error` so that
//   it can provide a blanket `From<E>` without hitting conflicting trait implementations in `std`.

/// The core error type, analogous to `anyhow::Error`.
pub struct Error {
    inner: Box<dyn StdError + Send + Sync + 'static>,
}

impl Error {
    /// Creates a new error from a concrete type.
    #[must_use]
    pub fn new<E>(error: E) -> Self
    where
        E: StdError + Send + Sync + 'static,
    {
        Self {
            inner: Box::new(error),
        }
    }

    /// Creates a new error from an arbitrary message.
    #[must_use]
    pub fn msg<M>(message: M) -> Self
    where
        M: fmt::Display + fmt::Debug + Send + Sync + 'static,
    {
        Self {
            inner: Box::new(MessageError { message }),
        }
    }

    /// Adds context to the current error, wrapping it.
    #[must_use]
    pub fn context<C>(mut self, context: C) -> Self
    where
        C: fmt::Display + Send + Sync + 'static,
    {
        let error = self.inner;
        self.inner = Box::new(DynContextError { context, error });
        self
    }

    /// Attempts to downcast to a specific concrete type by traversing the error chain.
    #[must_use]
    pub fn downcast_ref<E>(&self) -> Option<&E>
    where
        E: StdError + 'static,
    {
        // RUST INSIGHT: To downcast a generic `dyn Error`, we rely on `std::error::Error::downcast_ref`,
        // which internally uses `std::any::Any::type_id`. By iterating through `.source()`, we can
        // find deeply nested root causes.
        for error in self.chain() {
            if let Some(downcasted) = error.downcast_ref::<E>() {
                return Some(downcasted);
            }
        }
        None
    }

    /// Returns an iterator over the chain of errors.
    #[must_use]
    pub fn chain(&self) -> Chain<'_> {
        Chain {
            current: Some(self.inner.as_ref()),
        }
    }
}

/// Iterator over the error chain.
pub struct Chain<'a> {
    current: Option<&'a (dyn StdError + 'static)>,
}

impl<'a> Iterator for Chain<'a> {
    type Item = &'a (dyn StdError + 'static);

    fn next(&mut self) -> Option<Self::Item> {
        let current = self.current?;
        self.current = current.source();
        Some(current)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.inner)
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // PRODUCTION NOTE: `anyhow` captures an actual stack backtrace via `std::backtrace::Backtrace`
        // if enabled. Here, we simply print the logical chain of errors.
        write!(f, "{}", self.inner)?;

        let mut chain = self.chain().skip(1).peekable();
        if chain.peek().is_some() {
            write!(f, "\n\nCaused by:")?;
            for (i, error) in chain.enumerate() {
                write!(f, "\n    {}: {}", i, error)?;
            }
        }
        Ok(())
    }
}

// Blanket implementation to convert any StdError into our Error.
// GOTCHA: If `Error` itself implemented `StdError`, this `From` impl would conflict with
// the standard library's `impl<T> From<T> for T`. Thus, `Error` must remain an opaque struct.
impl<E> From<E> for Error
where
    E: StdError + Send + Sync + 'static,
{
    fn from(error: E) -> Self {
        Self::new(error)
    }
}

// =========================================================================================
// Internal Error Wrappers
// =========================================================================================

struct MessageError<M> {
    message: M,
}

impl<M: fmt::Display> fmt::Display for MessageError<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.message, f)
    }
}

impl<M: fmt::Debug> fmt::Debug for MessageError<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.message, f)
    }
}

impl<M: fmt::Display + fmt::Debug> StdError for MessageError<M> {}

struct DynContextError<C> {
    context: C,
    error: Box<dyn StdError + Send + Sync + 'static>,
}

impl<C: fmt::Display> fmt::Display for DynContextError<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.context, f)
    }
}

impl<C: fmt::Display> fmt::Debug for DynContextError<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}\n\nCaused by:\n    {:?}", self.context, self.error)
    }
}

impl<C: fmt::Display> StdError for DynContextError<C> {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        Some(self.error.as_ref())
    }
}

// =========================================================================================
// Context Extension Trait
// =========================================================================================

/// Extension trait to provide `.context()` on `Result` and `Option`.
pub trait ContextExt<T> {
    /// Adds context to the error.
    fn context<C>(self, context: C) -> Result<T, Error>
    where
        C: fmt::Display + Send + Sync + 'static;

    /// Lazily adds context to the error.
    fn with_context<C, F>(self, f: F) -> Result<T, Error>
    where
        C: fmt::Display + Send + Sync + 'static,
        F: FnOnce() -> C;
}

impl<T, E> ContextExt<T> for std::result::Result<T, E>
where
    E: Into<Error>,
{
    fn context<C>(self, context: C) -> Result<T, Error>
    where
        C: fmt::Display + Send + Sync + 'static,
    {
        self.map_err(|e| e.into().context(context))
    }

    fn with_context<C, F>(self, f: F) -> Result<T, Error>
    where
        C: fmt::Display + Send + Sync + 'static,
        F: FnOnce() -> C,
    {
        self.map_err(|e| e.into().context(f()))
    }
}

impl<T> ContextExt<T> for Option<T> {
    fn context<C>(self, context: C) -> Result<T, Error>
    where
        C: fmt::Display + Send + Sync + 'static,
    {
        self.ok_or_else(|| Error::msg(MessageWrapper { inner: context }))
    }

    fn with_context<C, F>(self, f: F) -> Result<T, Error>
    where
        C: fmt::Display + Send + Sync + 'static,
        F: FnOnce() -> C,
    {
        self.ok_or_else(|| Error::msg(MessageWrapper { inner: f() }))
    }
}

struct MessageWrapper<T> {
    inner: T,
}

impl<T: fmt::Display> fmt::Display for MessageWrapper<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.inner, f)
    }
}

impl<T: fmt::Display> fmt::Debug for MessageWrapper<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.inner, f) // We don't require C to be Debug, so just use Display for Debug output too.
    }
}

// Convenience Result alias
pub type Result<T, E = Error> = std::result::Result<T, E>;

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `anyhow`: Production-ready, relies on unsafe code to provide a thin 1-word pointer (instead of a fat pointer Box<dyn Error>),
//   which keeps `Result` size small. Also captures backtraces.
// - `eyre`: A fork of `anyhow` designed for customizable error reporting (e.g. `color-eyre`).
//
// Missing vs. Production:
// - **Thin Pointers**: We use standard `Box<dyn ...>`, which is 2 words (fat pointer). `anyhow` hand-rolls vtables.
// - **Backtraces**: We don't integrate with `std::backtrace`.
// - **Macro**: No `anyhow!("...")` or `bail!("...")` macros, though easily added.
//
// Suggested next steps / extensions:
// 1. Add `#[track_caller]` to capture source locations (file/line) where errors are created.
// 2. Add macros `ensure!`, `bail!`, and `error!` to ergonomically create errors.
// 3. Integrate with the nightly `std::backtrace::Backtrace` to capture call stacks.
//
// Benchmarking Note:
// Benchmarking error creation vs. the happy path should verify that the `Result` size matches standard `Result`,
// and that using `?` without errors incurs zero overhead. Using `criterion`, measure `Error::new` and
// `ContextExt::context` taking care to use `std::hint::black_box`.

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[derive(Debug)]
    struct CustomError;
    impl fmt::Display for CustomError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "CustomError")
        }
    }
    impl StdError for CustomError {}

    #[test]
    fn test_error_creation_and_display() {
        let err = Error::new(CustomError);
        assert_eq!(err.to_string(), "CustomError");
    }

    #[test]
    fn test_error_msg() {
        let err = Error::msg("something failed");
        assert_eq!(err.to_string(), "something failed");
    }

    #[test]
    fn test_context_chaining() {
        let err = Error::new(CustomError).context("Failed at step 1");
        assert_eq!(err.to_string(), "Failed at step 1");

        let chain: Vec<_> = err.chain().collect();
        assert_eq!(chain.len(), 2);
        assert_eq!(chain[0].to_string(), "Failed at step 1");
        assert_eq!(chain[1].to_string(), "CustomError");
    }

    #[test]
    fn test_downcast_ref() {
        let err = Error::new(CustomError)
            .context("Failed at step 1")
            .context("Outer step");

        // Find deep error
        assert!(err.downcast_ref::<CustomError>().is_some());

        // Test standard non-match
        assert!(err.downcast_ref::<io::Error>().is_none());
    }

    #[test]
    fn test_result_context() {
        fn fail() -> Result<(), io::Error> {
            Err(io::Error::new(io::ErrorKind::NotFound, "file missing"))
        }

        let res = fail().context("Failed to open config");
        assert!(res.is_err());

        let err = res.unwrap_err();
        assert_eq!(err.to_string(), "Failed to open config");
        assert!(err.downcast_ref::<io::Error>().is_some());
    }

    #[test]
    fn test_option_context() {
        let opt: Option<i32> = None;
        let res = opt.context("Value was none");
        assert!(res.is_err());

        let err = res.unwrap_err();
        assert_eq!(err.to_string(), "Value was none");
    }
}
