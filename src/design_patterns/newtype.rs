//! # Newtype Pattern
//!
//! Replaces: **Primitive Obsession** (OOP)
//!
//! Real Rust usage: `std::time::Duration`, `std::num::Wrapping`, `actix_web::web::Json`
//!
//! ## Why this pattern exists in Rust
//! Rust allows defining a tuple struct with a single field that is distinct from its underlying type at compile time
//! but has identical memory layout at runtime (`#[repr(transparent)]`). This allows attaching traits and behavior
//! to primitives or foreign types without overhead.
//!
//! ## Architecture
//!
//! ```text
//! struct Miles(f64);
//! struct Kilometers(f64);
//!
//! // Mismatch at compile time:
//! // fn travel(dist: Kilometers)
//! // travel(Miles(5.0)) // Compile Error!
//! ```
//!
//! **Invariants:**
//! - Semantic separation of identical underlying types (e.g., ID types, units).
//! - Strictly controlled interface (you only expose what you implement).
//!
//! ## When to use
//! - To enforce units of measure (prevent Mars Climate Orbiter bugs).
//! - To implement external traits on external types (Newtype Wrapper).
//! - To hide internal implementation details while keeping performance.

use std::fmt;
use std::ops::{Add, Deref};

// ============================================================================
// The Pattern
// ============================================================================

/// Represents a distance in miles.
///
/// **Ownership Insight:** `#[repr(transparent)]` guarantees this struct has the
/// exact same ABI and memory layout as `f64`. It can be safely transmuted
/// across FFI boundaries if needed.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Miles(pub f64);

/// Represents a distance in kilometers.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Kilometers(pub f64);

// ============================================================================
// Trait Implementations (The "Methods")
// ============================================================================

impl Miles {
    pub fn new(val: f64) -> Self {
        Miles(val)
    }

    pub fn to_kilometers(self) -> Kilometers {
        Kilometers(self.0 * 1.60934)
    }
}

impl Kilometers {
    pub fn new(val: f64) -> Self {
        Kilometers(val)
    }

    pub fn to_miles(self) -> Miles {
        Miles(self.0 * 0.621371)
    }
}

// COMPILE-TIME WIN: implementing Add for Miles means we can add Miles+Miles,
// but not Miles+Kilometers or Miles+f64 without explicit conversion.
impl Add for Miles {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Miles(self.0 + other.0)
    }
}

impl fmt::Display for Miles {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} mi", self.0)
    }
}

// ============================================================================
// Deref Coercion: A Double-Edged Sword
// ============================================================================

/// A wrapper around a String that provides strict validation but allows reading as str.
pub struct Username(String);

impl Username {
    pub fn new(s: impl Into<String>) -> Result<Self, &'static str> {
        let s = s.into();
        if s.len() < 3 {
            return Err("Username too short");
        }
        Ok(Username(s))
    }
}

// TRADEOFF: Implementing Deref allows `&Username` to be used anywhere `&str` is expected.
// This is ergonomic (methods like .len(), .contains() work automatically),
// but it exposes the underlying type's entire read interface.
// DO NOT implement DerefMut unless the inner type's invariants cannot be violated.
impl Deref for Username {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unit_safety() {
        let distance_home = Miles(5.0);
        let distance_gym = Miles(2.0);

        let total = distance_home + distance_gym;
        assert_eq!(total.0, 7.0);

        // COMPILE-TIME WIN:
        // let k = Kilometers(10.0);
        // let mix = distance_home + k; // Error: mismatched types
    }

    #[test]
    fn test_deref_coercion() {
        let user = Username::new("jules").unwrap();

        // We can use str methods directly on Username because of Deref
        assert!(user.contains("ul"));
        assert_eq!(user.len(), 5);

        // We can pass &Username to functions expecting &str
        fn print_name(n: &str) {
            println!("User: {}", n);
        }
        print_name(&user);
    }
}
