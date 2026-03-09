// Suppress pedantic and nursery lints for design pattern examples.
#![allow(clippy::pedantic, clippy::nursery, unused)]
//! # Extension Trait Pattern
//!
//! Replaces: **Extension Methods** (C#), **Monkey Patching** (Ruby/Python)
//!
//! Real Rust usage: `itertools::Itertools`, `tokio::io::AsyncReadExt`, `rand::Rng`
//!
//! ## Why this pattern exists in Rust
//! Rust adheres to strict coherence rules (the "Orphan Rule"): you can only implement a trait for a type
//! if you define the trait OR the type in your crate. This prevents you from adding methods directly to
//! foreign types (like `String` or `Vec<T>`).
//!
//! The Extension Trait pattern circumvents this by defining a new trait with the desired methods,
//! implementing it for the foreign type, and then bringing the trait into scope.
//!
//! ## Architecture
//!
//! ```text
//! [ Foreign Type (String) ]  <--  [ My Trait (StringExt) ]
//!         |                               ^
//!         |                               |
//!         +---- (impl StringExt for String)
//! ```
//!
//! **Invariants:**
//! - The extension methods are only available when the trait is imported (`use my_crate::StringExt;`).
//! - No runtime overhead; methods are statically dispatched.
//! - Methods are scoped; different crates can define extension methods with the same name without conflict (disambiguated by trait import).
//!
//! ## When to use
//! - To add convenience methods to standard library types or types from other crates.
//! - To group utility functions under a fluent interface (`x.my_method()`) rather than free functions (`my_func(x)`).

// ============================================================================
// The Pattern: Adding methods to a specific type
// ============================================================================

pub trait StringExt {
    /// Truncates the string to a maximum length, ensuring we don't split words
    /// if possible, and definitely don't split UTF-8 characters.
    fn truncate_at_whitespace(&self, max_len: usize) -> &str;

    /// Checks if the string contains only numeric characters.
    fn is_numeric_only(&self) -> bool;
}

// COMPILE-TIME WIN: We implement this trait for `str` (the unsized slice),
// which means it automatically applies to `&str`, `String`, `Cow<str>`, etc.
// via Deref coercion when calling methods.
impl StringExt for str {
    fn truncate_at_whitespace(&self, max_len: usize) -> &str {
        if self.len() <= max_len {
            return self;
        }

        // 1. Find a safe split point (char boundary) <= max_len
        let mut split_idx = max_len;
        while !self.is_char_boundary(split_idx) {
            split_idx -= 1;
        }

        let slice = &self[..split_idx];

        // 2. Try to find the last whitespace to break cleanly
        // OWNERSHIP INSIGHT: We return &str, which is a slice of the original string (self).
        // No allocation happens here. We just return a narrower view (a "borrowed slice").
        match slice.rfind(char::is_whitespace) {
            Some(idx) => &self[..idx],
            None => slice, // Fallback to hard cut if no whitespace found
        }
    }

    fn is_numeric_only(&self) -> bool {
        !self.is_empty() && self.chars().all(|c| c.is_numeric())
    }
}

// ============================================================================
// Advanced: Blanket Implementations (Adding methods to ALL types matching a bound)
// ============================================================================

pub trait IteratorExt: Iterator {
    /// Counts items that match a predicate.
    ///
    /// This demonstrates adding methods to *any* type that implements Iterator.
    fn count_where<F>(self, predicate: F) -> usize
    where
        Self: Sized,
        F: FnMut(&Self::Item) -> bool,
    {
        self.filter(predicate).count()
    }
}

// OWNERSHIP INSIGHT: The blanket implementation covers ANY type T that implements Iterator.
// This is how `itertools` works. `?Sized` allows it to apply to unsized iterators if methods don't require Sized.
impl<T: ?Sized> IteratorExt for T where T: Iterator {}

// ============================================================================
// Footer
// ============================================================================
//
// How this pattern appears in std/crates:
// - `std::io::Read` / `Write` are traits, but `byteorder` crate adds `ReadBytesExt`.
// - `tokio` separates core traits (`AsyncRead`) from utility methods (`AsyncReadExt`) to reduce compile times.
//
// OOP Equivalent:
// - C# Extension Methods (`public static void MyMethod(this String s)`).
// - Kotlin Extension Functions.
// - JavaScript prototype modification (but safe and scoped).

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_logic() {
        let s = "Hello beautiful world";

        // Truncate at whitespace
        assert_eq!(s.truncate_at_whitespace(10), "Hello");

        // Exact length
        assert_eq!(s.truncate_at_whitespace(5), "Hello");

        // Hard cut (no whitespace in range)
        assert_eq!("Supercalifragilistic".truncate_at_whitespace(5), "Super");

        // Unicode safety
        let s_uni = "Héllo world";
        // 'é' is 2 bytes. "Héllo" is 1+2+1+1+1 = 6 bytes.
        // Truncating at 2 should give "H" (index 1), because index 2 is mid-'é'.
        // Wait, 'é' in UTF-8 is 0xC3 0xA9.
        // Index 0: H
        // Index 1: \xC3 (start of é)
        // Index 2: \xA9 (continuation of é)
        // Index 3: l
        // If max_len = 2. split_idx starts at 2. is_char_boundary(2)? No.
        // split_idx -> 1. is_char_boundary(1)? Yes.
        // slice = &self[..1] -> "H".
        assert_eq!(s_uni.truncate_at_whitespace(2), "H");

        // If max_len = 3. split_idx = 3. Yes. slice = "Hé" (1 byte 'H' + 2 bytes 'é').
        assert_eq!(s_uni.truncate_at_whitespace(3), "Hé");

        // If max_len = 4. split_idx = 4. Yes. slice = "Hél" (1+2+1).
        assert_eq!(s_uni.truncate_at_whitespace(4), "Hél");
    }

    #[test]
    fn test_iterator_ext() {
        let nums = vec![1, 2, 3, 4, 5, 6];
        // We can call count_where directly on the iterator because of the blanket impl
        let evens = nums.iter().count_where(|&x| x % 2 == 0);
        assert_eq!(evens, 3);
    }

    #[test]
    fn test_numeric_only() {
        assert!("12345".is_numeric_only());
        assert!(!"123a5".is_numeric_only());
        assert!(!"".is_numeric_only()); // Specification says !empty
    }
}
