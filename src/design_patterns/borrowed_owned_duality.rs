//! # Borrowed vs. Owned Duality (`Cow`)
//!
//! Replaces: **Defensive Copying** (C++/Java), **Immutable/Mutable Split**
//!
//! Real Rust usage: `std::borrow::Cow`, `std::path::PathBuf` (via `Into`), `String::from_utf8_lossy`
//!
//! ## Why this pattern exists in Rust
//! In many cases, you want to write a function that accepts data, reads it, and *occasionally* modifies it.
//! If you always take `&T`, you can't modify. If you always take `T` (owned), the caller must clone,
//! which is wasteful if no modification happens.
//!
//! The "Clone-on-Write" (Cow) pattern allows you to hold either a reference or an owned value.
//! You can read from both. If you need to write, you upgrade the reference to an owned value (clone)
//! only when necessary.
//!
//! ## Architecture
//!
//! ```text
//! enum Cow<'a, B> where B: ToOwned + ?Sized {
//!     Borrowed(&'a B),
//!     Owned(<B as ToOwned>::Owned),
//! }
//! ```
//!
//! **Invariants:**
//! - `Borrowed` variant holds a reference.
//! - `Owned` variant holds the owned data.
//! - `to_mut()` ensures you have a mutable reference to owned data, cloning if currently borrowed.
//!
//! ## When to use
//! - When most operations are read-only, but some paths require mutation.
//! - To avoid allocation in hot paths (e.g., string processing).

use std::borrow::Borrow;
use std::fmt;
use std::ops::Deref;

// ============================================================================
// The Pattern: Simplified Cow
// ============================================================================

// OWNERSHIP INSIGHT:
// B: 'a means the type B must outlive the lifetime 'a.
// ?Sized allows B to be a slice (str, [T]).
// ToOwned is a trait that defines how to go from &B to B::Owned (e.g., &str -> String).
pub enum MyCow<'a, B>
where
    B: 'a + ToOwned + ?Sized,
{
    Borrowed(&'a B),
    Owned(<B as ToOwned>::Owned),
}

impl<'a, B> MyCow<'a, B>
where
    B: ToOwned + ?Sized,
{
    /// returns true if the data is owned
    pub fn is_owned(&self) -> bool {
        matches!(self, MyCow::Owned(_))
    }

    /// Acquires a mutable reference to the owned form of the data.
    /// Clones the data if it is not already owned.
    pub fn to_mut(&mut self) -> &mut <B as ToOwned>::Owned {
        match *self {
            MyCow::Borrowed(borrowed) => {
                // COMPILE-TIME WIN: usage of ToOwned::to_owned allows generic cloning logic
                let owned = borrowed.to_owned();
                *self = MyCow::Owned(owned);
                match *self {
                    MyCow::Owned(ref mut owned) => owned,
                    _ => unreachable!(),
                }
            }
            MyCow::Owned(ref mut owned) => owned,
        }
    }
}

// Allow dereferencing to &B regardless of variant
impl<'a, B> Deref for MyCow<'a, B>
where
    B: ToOwned + ?Sized,
{
    type Target = B;

    fn deref(&self) -> &B {
        match *self {
            MyCow::Borrowed(b) => b,
            MyCow::Owned(ref o) => o.borrow(),
        }
    }
}

// We implement Clone manually to avoid requiring B to be Clone,
// relying on ToOwned to produce the owned value.
impl<'a, B> Clone for MyCow<'a, B>
where
    B: ToOwned + ?Sized,
{
    fn clone(&self) -> Self {
        match *self {
            MyCow::Borrowed(b) => MyCow::Borrowed(b),
            MyCow::Owned(ref o) => {
                // We create a new owned value from the existing owned value.
                // Since `o` is `B::Owned`, and `B::Owned` implies `Borrow<B>`,
                // we can borrow it back to `&B` and call `to_owned()`.
                let cloned = o.borrow().to_owned();
                MyCow::Owned(cloned)
            }
        }
    }
}

impl<'a, B> fmt::Display for MyCow<'a, B>
where
    B: ToOwned + ?Sized + fmt::Display,
    <B as ToOwned>::Owned: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            MyCow::Borrowed(b) => fmt::Display::fmt(b, f),
            MyCow::Owned(ref o) => fmt::Display::fmt(o, f),
        }
    }
}

// ============================================================================
// The "Into<Cow>" Pattern
// ============================================================================

impl<'a> From<&'a str> for MyCow<'a, str> {
    fn from(s: &'a str) -> Self {
        MyCow::Borrowed(s)
    }
}

impl<'a> From<String> for MyCow<'a, str> {
    fn from(s: String) -> Self {
        MyCow::Owned(s)
    }
}

// Example function showing the benefit
pub fn sanitize_input<'a, S>(input: S) -> MyCow<'a, str>
where
    S: Into<MyCow<'a, str>>,
{
    let mut cow = input.into();
    if cow.contains("bad_word") {
        // COW IN ACTION: Only allocates (clones) if we hit this path
        cow.to_mut().replace_range(.., "[REDACTED]");
    }
    cow
}

// ============================================================================
// Footer
// ============================================================================
//
// How this pattern appears in std:
// - `std::borrow::Cow`: The standard implementation.
// - `String::from_utf8_lossy`: Returns `Cow<str>`. If valid UTF-8, borrows. If invalid, replaces chars and owns result.
//
// GOTCHA:
// - `Cow` requires the `ToOwned` trait, which can be complex for custom types.
// - `to_mut()` can be expensive if called unexpectedly.

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cow_behavior() {
        let s = "hello world";
        let mut cow: MyCow<str> = MyCow::Borrowed(s);

        assert!(!cow.is_owned());
        assert_eq!(&*cow, "hello world"); // Deref works

        // Mutation triggers clone
        let string_ref = cow.to_mut();
        string_ref.push_str("!");

        assert!(cow.is_owned());
        assert_eq!(&*cow, "hello world!");
    }

    #[test]
    fn test_into_cow() {
        // Case 1: Pass &str (Borrowed) - Clean path
        let input = "good input";
        let result = sanitize_input(input);
        assert!(!result.is_owned()); // Should stay borrowed
        assert_eq!(&*result, "good input");

        // Case 2: Pass &str (Borrowed) - Dirty path
        let input = "this has a bad_word";
        let result = sanitize_input(input);
        assert!(result.is_owned()); // Should become owned because we modified it
        assert_eq!(&*result, "[REDACTED]");

        // Case 3: Pass String (Owned)
        let input = String::from("already owned bad_word");
        let result = sanitize_input(input);
        assert!(result.is_owned()); // Was already owned
        assert_eq!(&*result, "[REDACTED]");
    }

    #[test]
    fn test_clone() {
        let cow1: MyCow<str> = MyCow::Borrowed("foo");
        let cow2 = cow1.clone();
        assert!(!cow2.is_owned());

        let mut cow3: MyCow<str> = MyCow::Borrowed("bar");
        cow3.to_mut(); // Make owned
        let cow4 = cow3.clone();
        assert!(cow4.is_owned());
    }
}
