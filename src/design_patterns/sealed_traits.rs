//! # Sealed Trait Pattern
//!
//! Replaces: **Final Classes** (Java), **Sealed Classes** (C#)
//!
//! Real Rust usage: `serde_json::Value` (impls), `std::slice::SliceIndex` (sealed)
//!
//! ## Why this pattern exists in Rust
//! Rust traits are open by default: any crate can implement a public trait for their own types.
//! Sometimes, a library wants to expose a trait for *polymorphism* (accepting `T: MyTrait`)
//! but prevent downstream users from *implementing* it. This allows the library to:
//! 1. Add new methods to the trait without breaking changes (because no user impls exist to break).
//! 2. Exhaustively match on the known implementations internally.
//! 3. Enforce safety invariants that rely on the implementation details.
//!
//! ## Architecture
//!
//! ```text
//! [ Public Trait (Color) ]  <-- (inherits) -- [ Private Trait (Sealed) ]
//!         |
//!         +-- impl for Red
//!         +-- impl for Blue
//!
//! // User code:
//! struct Green;
//! impl Color for Green {} // ERROR: Cannot implement Sealed (it's private)
//! ```
//!
//! **Invariants:**
//! - The trait is public and usable in bounds (`fn foo<T: Color>`).
//! - The trait cannot be implemented by types outside the defining crate.
//!
//! ## When to use
//! - When your trait is a "closed set" of behaviors.
//! - When you need to future-proof a trait against adding new methods.
//! - When unsafe code relies on the correctness of the implementation.

// ============================================================================
// The "Sealed" Supertrait
// ============================================================================

/// This module is private, so `Sealed` is not accessible outside this crate.
mod private {
    pub trait Sealed {}
}

// OWNERSHIP INSIGHT:
// By requiring `private::Sealed` as a supertrait, we ensure that
// only types that implement `Sealed` can implement `Color`.
// Since `Sealed` is in a private module, external crates cannot name it,
// and thus cannot implement it.
pub trait Color: private::Sealed {
    fn to_hex(&self) -> String;

    // TRADEOFF: We can add new methods here in the future without a major version bump,
    // because we know exactly who implements this trait (us).
    fn describe(&self) -> String {
        format!("Color({})", self.to_hex())
    }
}

// ============================================================================
// The Allowed Implementations
// ============================================================================

pub struct Red;
pub struct Green;
pub struct Blue;

// We must implement Sealed for our types
impl private::Sealed for Red {}
impl private::Sealed for Green {}
impl private::Sealed for Blue {}

// Now we can implement the public trait
impl Color for Red {
    fn to_hex(&self) -> String {
        "#FF0000".to_string()
    }
}

impl Color for Green {
    fn to_hex(&self) -> String {
        "#00FF00".to_string()
    }
}

impl Color for Blue {
    fn to_hex(&self) -> String {
        "#0000FF".to_string()
    }
}

// ============================================================================
// Usage
// ============================================================================

pub fn draw<C: Color>(color: C) {
    println!("Drawing {}", color.to_hex());
}

// ============================================================================
// Footer
// ============================================================================
//
// How this pattern appears in std/crates:
// - `std::slice::SliceIndex`: Implemented only for types that can index a slice (usize, ranges).
// - `serde_json`: Prevents users from implementing internal traits that control serialization logic.
//
// GOTCHA:
// - The error message for users trying to implement `Color` will say "private trait `Sealed` is not accessible".
// - This is effectively "final" for traits.

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_usage() {
        draw(Red);
        draw(Green);
        draw(Blue);

        assert_eq!(Red.describe(), "Color(#FF0000)");
        assert_eq!(Green.describe(), "Color(#00FF00)");
        assert_eq!(Blue.describe(), "Color(#0000FF)");
    }

    // Note: We cannot easily test that external crates *cannot* implement `Color`
    // within a unit test because unit tests have access to private modules of the parent.
    // However, if we were to create a separate module that couldn't see `private`:

    mod external_crate_simulation {
        // use super::Color;
        // struct MyColor;
        // impl Color for MyColor {} // This would fail if uncommented because `Sealed` is missing
    }
}
