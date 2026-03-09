// Suppress pedantic and nursery lints for design pattern examples.
#![allow(clippy::pedantic, clippy::nursery, unused)]
//! # The `From`/`Into` Conversion Web
//!
//! Replaces: **Constructor Overloading** (C++, Java), **Implicit Type Conversions** (C)
//!
//! Real Rust usage: `std::convert::From`, `std::convert::Into`, `?` operator (try operator)
//!
//! ## Why this pattern exists in Rust
//! Rust lacks constructor overloading and implicit type coercions (mostly). Instead,
//! the `From` and `Into` traits define a standardized, explicit web of safe conversions.
//! When you implement `From<T> for U`, the compiler automatically provides a blanket
//! implementation of `Into<U> for T`. This powers the ergonomic `?` operator for error
//! propagation.
//!
//! ## Architecture
//!
//! ```text
//! [ std::io::Error ] --> From --> [ DatabaseError ] --> From --> [ AppError ]
//! ```
//!
//! **Invariants:**
//! - Implement `From`, never `Into` directly (unless the conversion requires types outside your crate).
//! - Conversions should be infallible and typically cheap (or clearly documented if not).
//! - The `?` operator implicitly calls `.into()` on the error type to match the function's return signature.
//!
//! ## When to use
//! - Creating ergonomic APIs that accept multiple types (`fn build<T: Into<String>>(s: T)`).
//! - Building a clean error hierarchy where low-level errors propagate into high-level ones.

use std::fs::File;
use std::io;
use std::num::ParseIntError;

// ============================================================================
// The Error Hierarchy Conversion Web
// ============================================================================

#[derive(Debug)]
pub enum DatabaseError {
    Io(io::Error),
    Parse(ParseIntError),
    NotFound(String),
}

// COMPILE-TIME WIN: By implementing `From`, we get `Into` for free,
// and the `?` operator can automatically upcast `io::Error` to `DatabaseError`.
impl From<io::Error> for DatabaseError {
    fn from(err: io::Error) -> Self {
        DatabaseError::Io(err)
    }
}

impl From<ParseIntError> for DatabaseError {
    fn from(err: ParseIntError) -> Self {
        DatabaseError::Parse(err)
    }
}

// ============================================================================
// Ergonomic API Inputs
// ============================================================================

pub struct User {
    username: String,
}

impl User {
    // OWNERSHIP INSIGHT: Accepting `Into<String>` allows callers to pass either
    // a `&str` (which allocates a new String) or a `String` (which moves without allocation).
    // This provides the ergonomics of overloading without the complexity.
    pub fn new<S: Into<String>>(username: S) -> Self {
        User {
            username: username.into(),
        }
    }
}

// ============================================================================
// Logic Simulation
// ============================================================================

pub fn read_and_parse_id(path: &str) -> Result<u32, DatabaseError> {
    // The `?` operator here translates to:
    // match File::open(path) {
    //     Ok(file) => file,
    //     Err(e) => return Err(From::from(e)), // io::Error -> DatabaseError
    // }
    let _file = File::open(path)?;

    // Simulate reading a string and parsing
    let content = "123";
    let id: u32 = content.parse()?; // ParseIntError -> DatabaseError

    Ok(id)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_into_string() {
        let u1 = User::new("alice"); // &str -> String
        let u2 = User::new(String::from("bob")); // String -> String (zero-cost move)

        assert_eq!(u1.username, "alice");
        assert_eq!(u2.username, "bob");
    }

    #[test]
    fn test_error_conversion() {
        // Trigger an IO error
        let result = read_and_parse_id("non_existent_file.txt");
        assert!(matches!(result, Err(DatabaseError::Io(_))));
    }

    #[test]
    fn test_explicit_into() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file missing");

        // Explicitly using the blanket Into implementation provided by From
        let db_err: DatabaseError = io_err.into();
        assert!(matches!(db_err, DatabaseError::Io(_)));
    }
}

// ============================================================================
// Footer
// ============================================================================
//
// Production Note:
// In production, crate `thiserror` automates the generation of `From` implementations
// for error enums using `#[from]`.
//
// TRADEOFF:
// Using `impl Into<T>` in function signatures causes monomorphization (code bloat)
// because a separate copy of the function is generated for every input type.
// If the function is large, consider taking `&str` or moving the `into()` conversion
// to a non-generic inner function.
