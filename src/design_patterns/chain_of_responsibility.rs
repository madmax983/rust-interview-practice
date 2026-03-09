// Suppress pedantic and nursery lints for design pattern examples.
#![allow(clippy::pedantic, clippy::nursery, unused)]
//! # Chain of Responsibility Pattern
//!
//! Replaces: **Chain of Responsibility** (OOP), **Nested If-Else Statements**
//!
//! Real Rust usage: `tower::Service` (Middleware), Request router patterns, `Option::or_else` / `Result::or_else`
//!
//! ## Why this pattern exists in Rust
//! In OOP, the Chain of Responsibility passes a request along a chain of handlers. Each handler decides either to
//! process the request or pass it to the next handler in the chain. This is often implemented with abstract base classes
//! and `next: Option<Box<Handler>>`.
//!
//! In Rust, we achieve this more elegantly using:
//! 1. **Option/Result Combinators:** The functional approach, chaining `or_else` or `and_then` directly.
//! 2. **Iteration over Handlers:** Storing a `Vec<Box<dyn Handler>>` (or a slice of function pointers) and
//!    iterating until one successfully processes the request.
//! 3. **Middleware (Tower-style):** Statically composing handlers at compile time (see `middleware.rs`).
//!
//! ## Architecture
//!
//! **Approach 1: Functional Chaining (Idiomatic)**
//! ```text
//! [ Request ] --> (handler1) -.
//!                             |-- (or_else) --> (handler2) -.
//!                                                           |-- (or_else) --> (handler3)
//! ```
//!
//! **Approach 2: Trait Objects (Dynamic Chain)**
//! ```text
//! [ Request ] --> [ Handler 1, Handler 2, Handler 3 ] (Vec<Box<dyn Handler>>)
//!                      |          |          |
//!                  (Pass)     (Handle)       x
//! ```
//!
//! **Invariants:**
//! - A handler takes a request and returns an `Option` or `Result` indicating success, failure, or "pass".
//! - The chain stops processing as soon as a handler successfully handles the request (or explicitly errors out).
//!
//! ## When to use
//! - When multiple objects can handle a request, but the specific handler isn't known a priori.
//! - To decouple the sender of a request from its receiver.

// ============================================================================
// The Request Object
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    pub id: u32,
    pub payload: String,
    pub user_role: Role,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Role {
    Guest,
    User,
    Admin,
}

// ============================================================================
// Approach 1: Functional Combinators (Idiomatic Rust)
// ============================================================================

// OWNERSHIP INSIGHT: Handlers take references to the request and return an Option.
// This allows zero-cost chaining without taking ownership of the request until necessary.

fn handle_guest(req: &Request) -> Option<String> {
    if req.user_role == Role::Guest {
        Some(format!("Guest handled request {}", req.id))
    } else {
        None // Pass to next
    }
}

fn handle_user(req: &Request) -> Option<String> {
    if req.user_role == Role::User {
        Some(format!("User handled request {}", req.id))
    } else {
        None // Pass to next
    }
}

fn handle_admin(req: &Request) -> Option<String> {
    if req.user_role == Role::Admin {
        Some(format!("Admin handled request {}", req.id))
    } else {
        None // Pass to next
    }
}

/// A functional pipeline using Option combinators.
///
/// **COMPILE-TIME WIN:** `or_else` is lazily evaluated. Handlers are only called
/// if the previous handler returned `None`.
#[must_use]
pub fn process_request_functional(req: &Request) -> String {
    handle_guest(req)
        .or_else(|| handle_user(req))
        .or_else(|| handle_admin(req))
        .unwrap_or_else(|| "Unhandled request".to_string())
}

// ============================================================================
// Approach 2: Dynamic Trait Objects (List of Handlers)
// ============================================================================

// When you need to configure the chain at runtime (e.g., loading plugins),
// you can use a trait and a collection of dynamic handlers.

pub trait RequestHandler {
    /// Returns `Some(Response)` if handled, or `None` to pass to the next handler.
    fn handle(&self, req: &Request) -> Option<String>;
}

// Concrete Handlers

pub struct CacheHandler {
    cached_id: u32,
    response: String,
}

impl RequestHandler for CacheHandler {
    fn handle(&self, req: &Request) -> Option<String> {
        if req.id == self.cached_id {
            Some(format!("Cache hit: {}", self.response))
        } else {
            None
        }
    }
}

pub struct ValidationHandler;

impl RequestHandler for ValidationHandler {
    fn handle(&self, req: &Request) -> Option<String> {
        if req.payload.is_empty() {
            // TRADEOFF: Here we hijack the chain by returning a definitive response (error),
            // stopping further processing.
            Some("Validation failed: empty payload".to_string())
        } else {
            None // Payload is valid, let someone else handle the *actual* work
        }
    }
}

pub struct DatabaseHandler;

impl RequestHandler for DatabaseHandler {
    fn handle(&self, req: &Request) -> Option<String> {
        // Fallback handler
        Some(format!("DB processed request {}", req.id))
    }
}

// The Chain itself

pub struct HandlerChain {
    // TRADEOFF: Requires dynamic dispatch and heap allocation.
    handlers: Vec<Box<dyn RequestHandler>>,
}

impl HandlerChain {
    pub const fn new() -> Self {
        Self {
            handlers: Vec::new(),
        }
    }

    pub fn add_handler(&mut self, handler: Box<dyn RequestHandler>) {
        self.handlers.push(handler);
    }

    pub fn process(&self, req: &Request) -> String {
        for handler in &self.handlers {
            if let Some(response) = handler.handle(req) {
                // Stop iterating as soon as a handler returns Some
                return response;
            }
        }
        "Unhandled request".to_string()
    }
}

impl Default for HandlerChain {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_functional_chain() {
        let req_user = Request {
            id: 1,
            payload: "data".to_string(),
            user_role: Role::User,
        };
        assert_eq!(
            process_request_functional(&req_user),
            "User handled request 1"
        );

        let req_admin = Request {
            id: 2,
            payload: "secret".to_string(),
            user_role: Role::Admin,
        };
        assert_eq!(
            process_request_functional(&req_admin),
            "Admin handled request 2"
        );
    }

    #[test]
    fn test_dynamic_chain() {
        let mut chain = HandlerChain::new();
        chain.add_handler(Box::new(CacheHandler {
            cached_id: 42,
            response: "cached_data".to_string(),
        }));
        chain.add_handler(Box::new(ValidationHandler));
        chain.add_handler(Box::new(DatabaseHandler));

        // 1. Hit Cache
        let req1 = Request {
            id: 42,
            payload: "query".to_string(),
            user_role: Role::Guest,
        };
        assert_eq!(chain.process(&req1), "Cache hit: cached_data");

        // 2. Miss Cache -> Fail Validation (Short-circuit)
        let req2 = Request {
            id: 99,
            payload: "".to_string(),
            user_role: Role::User,
        };
        assert_eq!(chain.process(&req2), "Validation failed: empty payload");

        // 3. Miss Cache -> Pass Validation -> Hit DB
        let req3 = Request {
            id: 100,
            payload: "insert into db".to_string(),
            user_role: Role::Admin,
        };
        assert_eq!(chain.process(&req3), "DB processed request 100");
    }
}

// ============================================================================
// Footer
// ============================================================================
//
// How this pattern appears in std/crates:
// - `Option::or_else` and `Result::or_else` are the foundational building blocks of functional chains in Rust.
// - HTTP frameworks like `axum` or `actix-web` use router trees and middleware, which act exactly as a Chain of Responsibility.
//
// What the GoF/OOP equivalent is and why it doesn't translate directly:
// In OOP, the chain is often linked via an explicit `next` pointer inside each handler (`handler.next.handle()`).
// In Rust, holding a mutable reference to the `next` handler inside a chain makes ownership complex.
// Therefore, we either compose the chain at compile time (Functional `or_else`, Middleware)
// or iterate over a flat collection of handlers (`Vec<Box<dyn Handler>>`).
//
// When to reach for this vs. simpler alternatives:
// For simple, static workflows, `match` statements or `if-else` blocks are clearer and faster.
// Reach for the dynamic Chain of Responsibility ONLY when the set of handlers needs to be extensible
// at runtime (e.g., dynamically loaded plugins).
//
// Suggested combinations with other patterns in this collection:
// - **Command Pattern**: The request object passed through the chain is often a Command.
// - **Decorator / Middleware**: Middleware is effectively a statically composed Chain of Responsibility.
//
// GOTCHA:
// If you implement a handler that *consumes* the request (takes ownership), it cannot easily pass it to the next handler
// if it decides not to process it. Always design handlers to take shared references `&Request` if possible,
// or return `Result<Response, Request>` to give ownership back to the caller on failure.
//
// ANTI-PATTERN:
// Translating OOP literally into Rust:
// ```rust
// struct Handler { next: Option<Box<Handler>> }
// ```
// Creating a linked list of objects just to represent a processing pipeline is inefficient and painful in Rust.
// Use a `Vec` or functional combinators instead.
