//! # 284. Peeking Iterator
//!
//! - Difficulty: Medium
//! - `LeetCode`: <https://leetcode.com/problems/peeking-iterator/>
//!
//! This problem perfectly illustrates the tension between object-oriented API requirements
//! and Rust's strict ownership model, specifically regarding interior mutability and iterator design.
//!
//! ## Why this matters in Rust
//! This problem demonstrates how to handle APIs that demand immutable references (`&self`)
//! while requiring state mutations (advancing an iterator) internally. It teaches interior mutability
//! via `RefCell` and leveraging concrete type guarantees like `std::vec::IntoIter`'s slice capabilities.
//!
//! ## Approach
//!
//! The challenge arises because `LeetCode`'s Java-style API demands that `peek()` and `has_next()`
//! take an immutable reference (`&self`). However, standard iteration inherently mutates state.
//! `std::iter::Peekable::peek()` requires `&mut self` because it may need to advance the underlying
//! iterator to fetch and cache the next element.
//!
//! We provide three implementations:
//! 1. **Brute Force (`PeekingIteratorBruteForce`)**: Uses `RefCell` to achieve interior mutability,
//!    wrapping a standard `Peekable`. This circumvents the borrow checker at runtime but adds overhead
//!    and is generally not idiomatic if it can be avoided. Time: O(1), Space: O(1).
//! 2. **Optimal (`PeekingIteratorOptimal`)**: Leverages the specific concrete type `LeetCode` provides:
//!    `std::vec::IntoIter<i32>`. Because `IntoIter` implements `ExactSizeIterator` and has an `as_slice()`
//!    method, we can peek and check the length completely immutably, avoiding runtime borrow checking.
//!    Time: O(1), Space: O(1).
//! 3. **Idiomatic Generic (`PeekingIteratorGeneric`)**: Demonstrates how this *should* be designed in
//!    pure Rust. It accepts any `Iterator`, but correctly requires `&mut self` for both `peek()` and `next()`.

use std::cell::RefCell;
use std::iter::Peekable;

/// Brute Force: Interior mutability with `RefCell`
///
/// This allows us to use `&self` while internally mutating the iterator state to advance and peek.
///
/// # GOTCHA
/// `RefCell` adds a small runtime overhead for borrow checking. If `borrow_mut()` is called while
/// another borrow is active, it will panic.
pub struct PeekingIteratorBruteForce {
    iter: RefCell<Peekable<std::vec::IntoIter<i32>>>,
}

impl PeekingIteratorBruteForce {
    #[must_use]
    pub fn new(iter: std::vec::IntoIter<i32>) -> Self {
        Self {
            iter: RefCell::new(iter.peekable()),
        }
    }

    /// Returns the next element in the array without moving the pointer.
    ///
    /// # Panics
    /// Panics if called when there are no more elements (`has_next()` returns `false`).
    #[must_use]
    pub fn peek(&self) -> i32 {
        // RUST INSIGHT: We use `borrow_mut()` even though the method takes `&self`
        // because `Peekable::peek()` requires `&mut self`.
        *self.iter.borrow_mut().peek().unwrap()
    }

    /// Returns the next element in the array and moves the pointer to the next element.
    ///
    /// # Panics
    /// Panics if the iterator is already exhausted.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> i32 {
        self.iter.borrow_mut().next().unwrap()
    }

    /// Returns `true` if there are still elements in the array.
    #[must_use]
    pub fn has_next(&self) -> bool {
        self.iter.borrow_mut().peek().is_some()
    }
}

/// Optimal: Leveraging `ExactSizeIterator` and slice capabilities
///
/// Since we know the exact type is `std::vec::IntoIter<i32>`, we can use its unique methods.
pub struct PeekingIteratorOptimal {
    iter: std::vec::IntoIter<i32>,
}

impl PeekingIteratorOptimal {
    #[must_use]
    pub const fn new(iter: std::vec::IntoIter<i32>) -> Self {
        Self { iter }
    }

    /// Returns the next element in the array without moving the pointer.
    ///
    /// # Panics
    /// Panics if called when the iterator is empty.
    #[must_use]
    pub fn peek(&self) -> i32 {
        // RUST INSIGHT: `as_slice()` returns the remaining elements as a slice.
        // This lets us peek at the first element immutably without advancing the iterator.
        *self.iter.as_slice().first().unwrap()
    }

    /// Returns the next element in the array and moves the pointer to the next element.
    ///
    /// # Panics
    /// Panics if the iterator is already exhausted.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> i32 {
        self.iter.next().unwrap()
    }

    /// Returns `true` if there are still elements in the array.
    #[must_use]
    pub fn has_next(&self) -> bool {
        // RUST INSIGHT: `IntoIter` implements `ExactSizeIterator`, so we can check its length
        // without consuming or mutating it.
        self.iter.len() > 0
    }
}

/// Idiomatic Generic: The Rust way
///
/// This is how a peeking iterator should be implemented in idiomatic Rust, which naturally
/// matches `std::iter::Peekable`.
pub struct PeekingIteratorGeneric<I: Iterator> {
    iter: Peekable<I>,
}

impl<I: Iterator> PeekingIteratorGeneric<I> {
    #[must_use]
    pub fn new(iter: I) -> Self {
        Self {
            iter: iter.peekable(),
        }
    }

    /// Peeks at the next element without consuming it.
    pub fn peek(&mut self) -> Option<&I::Item> {
        // RUST INSIGHT: We require `&mut self` here, correctly representing that
        // the underlying state may change when peeking.
        self.iter.peek()
    }

    /// Consumes and returns the next element.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<I::Item> {
        self.iter.next()
    }

    /// Returns `true` if the iterator has more elements.
    pub fn has_next(&mut self) -> bool {
        self.iter.peek().is_some()
    }
}

/// Main entry point (`LeetCode` equivalent)
pub type PeekingIterator = PeekingIteratorOptimal;

// ## Alternative approaches
//
// Another approach for a strictly immutable `peek` without `as_slice()` would be to manually cache the
// next element in an `Option<T>` within a `Cell` or `RefCell`, simulating what `Peekable` does but with
// interior mutability natively built into the wrapper.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force_happy_path() {
        let iter = vec![1, 2, 3].into_iter();
        let mut peeker = PeekingIteratorBruteForce::new(iter);

        assert_eq!(peeker.next(), 1);
        assert_eq!(peeker.peek(), 2);
        assert_eq!(peeker.next(), 2);
        assert!(peeker.has_next());
        assert_eq!(peeker.peek(), 3);
        assert_eq!(peeker.next(), 3);
        assert!(!peeker.has_next());
    }

    #[test]
    fn test_brute_force_empty() {
        let iter = vec![].into_iter();
        let peeker = PeekingIteratorBruteForce::new(iter);
        assert!(!peeker.has_next());
    }

    #[test]
    fn test_brute_force_stress() {
        let data: Vec<i32> = (0..10_000).collect();
        let iter = data.clone().into_iter();
        let mut peeker = PeekingIteratorBruteForce::new(iter);

        for &val in &data {
            assert!(peeker.has_next());
            assert_eq!(peeker.peek(), val);
            assert_eq!(peeker.next(), val);
        }
        assert!(!peeker.has_next());
    }

    #[test]
    fn test_optimal_happy_path() {
        let iter = vec![1, 2, 3].into_iter();
        let mut peeker = PeekingIteratorOptimal::new(iter);

        assert_eq!(peeker.next(), 1);
        assert_eq!(peeker.peek(), 2);
        assert_eq!(peeker.next(), 2);
        assert!(peeker.has_next());
        assert_eq!(peeker.peek(), 3);
        assert_eq!(peeker.next(), 3);
        assert!(!peeker.has_next());
    }

    #[test]
    fn test_optimal_empty() {
        let iter = vec![].into_iter();
        let peeker = PeekingIteratorOptimal::new(iter);
        assert!(!peeker.has_next());
    }

    #[test]
    fn test_optimal_stress() {
        let data: Vec<i32> = (0..10_000).collect();
        let iter = data.clone().into_iter();
        let mut peeker = PeekingIteratorOptimal::new(iter);

        for &val in &data {
            assert!(peeker.has_next());
            assert_eq!(peeker.peek(), val);
            assert_eq!(peeker.next(), val);
        }
        assert!(!peeker.has_next());
    }

    #[test]
    fn test_generic_happy_path() {
        let iter = vec!["apple", "banana", "cherry"].into_iter();
        let mut peeker = PeekingIteratorGeneric::new(iter);

        assert_eq!(peeker.next(), Some("apple"));
        assert_eq!(peeker.peek(), Some(&"banana"));
        assert_eq!(peeker.next(), Some("banana"));
        assert!(peeker.has_next());
        assert_eq!(peeker.next(), Some("cherry"));
        assert!(!peeker.has_next());
    }

    #[test]
    fn test_generic_empty() {
        let iter = std::iter::empty::<i32>();
        let mut peeker = PeekingIteratorGeneric::new(iter);
        assert!(!peeker.has_next());
        assert_eq!(peeker.peek(), None);
        assert_eq!(peeker.next(), None);
    }
}
