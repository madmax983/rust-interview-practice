// Suppress pedantic and nursery lints for design pattern examples.
#![allow(clippy::pedantic, clippy::nursery, unused)]

//! # Parse, Don't Validate Pattern
//!
//! Replaces: **Input Validation Checks** scattered throughout business logic,
//! **Stringly-Typed** programming.
//!
//! Real Rust usage: `std::str::FromStr`, `serde`, `url::Url`
//!
//! ## Why this pattern exists in Rust
//! In Java/C++/Python, it's common to accept primitive types (like `String` or `int`)
//! and perform validation checks (e.g., `if (email.contains("@"))`) at the beginning of
//! functions. This leads to redundant checks and the possibility of passing invalid
//! data to a function that forgets to validate it.
//!
//! Rust's strong type system and "make illegal states unrepresentable" philosophy
//! encourage parsing raw data into structured, strictly-typed data *once* at the
//! boundary of the system. Once the data is in the structured type, the type system
//! guarantees its validity.
//!
//! ## Architecture
//!
//! **Approach: Parse at the Boundary**
//! ```text
//! Raw Input (String) --> | Parse (Result<T, E>) | --> Validated Type (T) --> Business Logic
//! ```
//!
//! **Invariants:**
//! - Business logic only accepts validated types (e.g., `EmailAddress` instead of `String`).
//! - It is physically impossible to construct an invalid `EmailAddress` type.
//!
//! ## When to use
//! - When accepting data from external sources (user input, network, files).
//! - When validation rules are complex and you want to ensure they are enforced universally.
//!
//! ## Anti-patterns
//! - Accepting `String` for an email address and validating it inside every function.
//! - Returning a boolean `is_valid()` instead of returning a parsed type.

use std::convert::TryFrom;
use std::fmt;

// ============================================================================
// Approach: Parse, Don't Validate
// ============================================================================

// ANTI-PATTERN: Stringly-typed.
// fn send_email(email: String) -> Result<(), Error> {
//     if !email.contains('@') { return Err(...); }
//     // ... send email
// }

// PRODUCTION NOTE: Use Newtypes to wrap primitives and enforce invariants.
// COMPILE-TIME WIN: Once you have an `EmailAddress`, you know 100% it is valid.

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct EmailAddress(String);

#[derive(Debug, PartialEq, Eq)]
pub enum EmailError {
    MissingAtSymbol,
    Empty,
}

impl fmt::Display for EmailError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingAtSymbol => write!(f, "Email must contain an '@' symbol"),
            Self::Empty => write!(f, "Email cannot be empty"),
        }
    }
}

impl std::error::Error for EmailError {}

// OWNERSHIP INSIGHT: We consume the String during parsing. If it fails, the caller
// gets an error. If it succeeds, the String is owned by the newtype.
impl TryFrom<String> for EmailAddress {
    type Error = EmailError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.trim().is_empty() {
            return Err(EmailError::Empty);
        }
        if !value.contains('@') {
            return Err(EmailError::MissingAtSymbol);
        }
        Ok(Self(value))
    }
}

// GOTCHA: Do NOT implement `DerefMut` for newtypes if mutating the inner value
// could break the invariant! Only provide immutable access.
impl AsRef<str> for EmailAddress {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

// ============================================================================
// Using the Parsed Type
// ============================================================================

/// Business logic that ONLY accepts a validated `EmailAddress`.
/// TRADEOFF: Callers must do the parsing up-front before calling this function,
/// moving the error handling to the boundaries of the application.
pub fn send_welcome_email(email: &EmailAddress) -> String {
    format!("Sending welcome email to {}", email.as_ref())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_email() {
        let raw = "user@example.com".to_string();
        let email = EmailAddress::try_from(raw).unwrap();

        assert_eq!(email.as_ref(), "user@example.com");
        assert_eq!(
            send_welcome_email(&email),
            "Sending welcome email to user@example.com"
        );
    }

    #[test]
    fn test_missing_at_symbol() {
        let raw = "userexample.com".to_string();
        let result = EmailAddress::try_from(raw);

        assert_eq!(result, Err(EmailError::MissingAtSymbol));
    }

    #[test]
    fn test_empty_email() {
        let raw = "   ".to_string();
        let result = EmailAddress::try_from(raw);

        assert_eq!(result, Err(EmailError::Empty));
    }
}
