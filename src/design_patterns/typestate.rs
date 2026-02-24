//! # Typestate Pattern
//!
//! Replaces: **State Pattern** (OOP)
//!
//! Real Rust usage: `std::fs::File` (OpenOptions), `hyper::Request` (Builder), `embedded-hal`
//!
//! ## Why this pattern exists in Rust
//! Rust's ownership system and affine types (types that can be moved and consumed) allow us to encode state transitions
//! in the type system itself. Unlike OOP where an object changes its internal state but keeps the same type,
//! in Rust we can consume the old state and return a new type, making invalid states unrepresentable.
//!
//! ## Architecture
//!
//! ```text
//! [ HttpRequest<Unvalidated> ] --(validate())--> [ HttpRequest<Validated> ]
//!           |                                          |
//!           +-- (cannot send)                          +--> (send())
//! ```
//!
//! **Invariants:**
//! - A request cannot be sent unless it has been validated.
//! - Validation consumes the unvalidated request, preventing double-validation or modification after validation.
//!
//! ## When to use
//! - When order of operations must be enforced (e.g., Init -> Configure -> Run).
//! - When certain operations are only valid in specific states.
//! - To eliminate runtime checks for state validity.

use std::marker::PhantomData;

// ============================================================================
// State Markers
// ============================================================================

// OWNERSHIP INSIGHT: Zero-sized types (ZSTs) are used as state markers.
// They disappear at runtime but guide the compiler's type checking.
pub struct Unvalidated;
pub struct Validated;

// ============================================================================
// The Context (The State Machine)
// ============================================================================

/// A generic HTTP request wrapper that tracks its validation state.
pub struct HttpRequest<State = Unvalidated> {
    url: String,
    method: String,
    body: String,
    // COMPILE-TIME WIN: PhantomData holds the state type without consuming space.
    _state: PhantomData<State>,
}

impl HttpRequest<Unvalidated> {
    /// Creates a new unvalidated request.
    pub fn new(url: impl Into<String>) -> Self {
        HttpRequest {
            url: url.into(),
            method: "GET".to_string(),
            body: "".to_string(),
            _state: PhantomData,
        }
    }

    /// Updates the method. Only allowed in Unvalidated state.
    pub fn method(mut self, method: &str) -> Self {
        self.method = method.to_string();
        self
    }

    /// Transitions to the Validated state.
    ///
    /// Consumes `self` so the old `Unvalidated` request is gone.
    pub fn validate(self) -> Result<HttpRequest<Validated>, String> {
        if self.url.is_empty() {
            return Err("URL cannot be empty".to_string());
        }
        if !self.url.starts_with("http") {
            return Err("URL must start with http".to_string());
        }

        // TRADEOFF: We destructure and reconstruct to change the type.
        // This is zero-cost at runtime due to compiler optimizations.
        Ok(HttpRequest {
            url: self.url,
            method: self.method,
            body: self.body,
            _state: PhantomData,
        })
    }
}

impl HttpRequest<Validated> {
    /// Sends the request. Only available on Validated requests.
    pub fn send(self) {
        println!("Sending {} request to {}", self.method, self.url);
        // In a real implementation, this would perform network I/O.
    }
}

// ============================================================================
// Alternatives / Anti-Patterns
// ============================================================================

// ANTI-PATTERN: Runtime flags
#[allow(dead_code)]
struct BadHttpRequest {
    url: String,
    is_validated: bool,
}
// This requires checking `if !self.is_validated` in every method,
// and risks panicking at runtime if the check is forgotten.

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_flow() {
        let req = HttpRequest::new("https://rust-lang.org").method("POST");

        // COMPILE-TIME WIN: The following line would fail to compile:
        // req.send(); // Error: no method named `send` found for type `HttpRequest<Unvalidated>`

        let validated_req = req.validate().expect("Validation failed");

        // COMPILE-TIME WIN: The following line would fail to compile:
        // validated_req.method("GET"); // Error: no method named `method`

        validated_req.send();
    }

    #[test]
    fn test_invalid_flow() {
        let req = HttpRequest::new("ftp://bad-url");
        assert!(req.validate().is_err());
    }
}
