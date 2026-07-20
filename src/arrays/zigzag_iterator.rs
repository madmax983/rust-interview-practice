//! # 281. Zigzag Iterator
//!
//! Difficulty: Medium
//! Link: https://leetcode.com/problems/zigzag-iterator/
//!
//! Why this matters in Rust: This problem is a fantastic exercise in implementing the
//! `Iterator` trait and managing multiple internal state streams. It contrasts the naive
//! approach of allocating a combined collection against a zero-allocation, purely
//! state-driven optimal approach that yields elements lazily.
//!
//! ## Approach
//!
//! We need to iterate through two 1D vectors in a zigzag manner.
//!
//! The **brute force approach** is to consume both input vectors and construct a completely
//! new combined vector containing the elements in the correct zigzag order, then just
//! return an iterator over that combined vector. This is O(N) time and O(N) extra space,
//! where N is the total number of elements.
//!
//! The **optimal approach** implements a custom iterator that only holds iterators (or
//! indices) into the original vectors. It maintains state indicating which vector to pull
//! from next. This approach takes O(1) extra space and is entirely lazy, allocating nothing.
//!
//! Idiomatic reasoning: Rust's `Iterator` trait is perfectly suited for this. By holding
//! `std::vec::IntoIter` inside our struct, we consume the original vectors without extra
//! allocation, and then lazily pull elements from them on demand.
//!
//! ## Alternative Approaches
//!
//! 1.  **Queue of Iterators**: For a generalization to `K` vectors, a `VecDeque` of iterators
//!     is standard. You pop an iterator, yield its next element, and push it to the back if
//!     it still has elements. This scales cleanly.

/// Brute Force implementation: Pre-computes the entire zigzag sequence into a `Vec`.
///
/// This approach allocates memory proportional to the total number of elements.
/// It is easy to write but sub-optimal in terms of space complexity.
#[derive(Debug)]
pub struct ZigzagIteratorBruteForce {
    data: std::vec::IntoIter<i32>,
}

impl ZigzagIteratorBruteForce {
    /// Initializes the `ZigzagIteratorBruteForce` object.
    #[must_use]
    #[allow(clippy::needless_pass_by_value)] // LeetCode signature
    pub fn new(v1: Vec<i32>, v2: Vec<i32>) -> Self {
        let mut combined = Vec::with_capacity(v1.len() + v2.len());
        let mut iter1 = v1.into_iter();
        let mut iter2 = v2.into_iter();

        // Loop until both are exhausted.
        loop {
            let next1 = iter1.next();
            let next2 = iter2.next();

            if next1.is_none() && next2.is_none() {
                break;
            }

            if let Some(val) = next1 {
                combined.push(val);
            }
            if let Some(val) = next2 {
                combined.push(val);
            }
        }

        Self {
            data: combined.into_iter(),
        }
    }
}

impl Iterator for ZigzagIteratorBruteForce {
    type Item = i32;

    fn next(&mut self) -> Option<Self::Item> {
        self.data.next()
    }
}

// RUST INSIGHT: We implement `Iterator` directly, which gives us `next()`, `has_next()`
// implicitly via `.peekable()` or simply `.is_some()` on the return of `next()`.
// For LeetCode, we sometimes need to provide explicit `next()` and `has_next()` methods
// outside of the trait to match the platform signature, but implementing `Iterator` is idiomatic.

/// Optimal implementation: A zero-allocation custom iterator.
///
/// Maintains two internal iterators and a boolean flag to track whose turn it is.
///
/// Note: The LeetCode problem signature typically demands `next(&mut self) -> i32`
/// and `has_next(&self) -> bool`. Here, we provide idiomatic Rust.
#[derive(Debug)]
pub struct ZigzagIterator {
    iter1: std::vec::IntoIter<i32>,
    iter2: std::vec::IntoIter<i32>,
    /// true if we should pull from iter1 next, false for iter2.
    turn_one: bool,
}

impl ZigzagIterator {
    /// Initializes the `ZigzagIterator` object.
    #[must_use]
    #[allow(clippy::needless_pass_by_value)] // LeetCode signature
    pub fn new(v1: Vec<i32>, v2: Vec<i32>) -> Self {
        Self {
            iter1: v1.into_iter(),
            iter2: v2.into_iter(),
            turn_one: true, // Start with v1
        }
    }
}

impl Iterator for ZigzagIterator {
    type Item = i32;

    fn next(&mut self) -> Option<Self::Item> {
        // We try the preferred iterator first. If it's exhausted, we fall back to the other.
        if self.turn_one {
            if let Some(val) = self.iter1.next() {
                // If iter2 still has elements, switch turns. Otherwise, stay on iter1 (which will just return None next time).
                self.turn_one = false;
                Some(val)
            } else {
                // iter1 is exhausted, permanently switch to iter2 and pull from it.
                self.turn_one = false;
                self.iter2.next()
            }
        } else if let Some(val) = self.iter2.next() {
            self.turn_one = true;
            Some(val)
        } else {
            // iter2 is exhausted, permanently switch to iter1.
            self.turn_one = true;
            self.iter1.next()
        }
    }
}

// To perfectly match LeetCode's expected API if required:
impl ZigzagIterator {
    /// LeetCode specific `next` method.
    /// Panics if called when `has_next` is false.
    ///
    /// # Panics
    /// Panics if called when there are no more elements.
    #[allow(clippy::should_implement_trait)] // Intentionally matching LeetCode signature
    pub fn next(&mut self) -> i32 {
        Iterator::next(self).unwrap()
    }

    /// LeetCode specific `has_next` method.
    /// Requires creating a peekable iterator internally if implemented purely.
    /// Since `std::vec::IntoIter` implements `ExactSizeIterator`, we can cheat here
    /// by checking lengths without consuming.
    #[must_use]
    pub fn has_next(&self) -> bool {
        self.iter1.len() > 0 || self.iter2.len() > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zigzag_iterator_happy_path() {
        let v1 = vec![1, 2];
        let v2 = vec![3, 4, 5, 6];
        let mut iter = ZigzagIterator::new(v1, v2);

        assert!(iter.has_next());
        assert_eq!(iter.next(), 1);
        assert_eq!(iter.next(), 3);
        assert_eq!(iter.next(), 2);
        assert_eq!(iter.next(), 4);
        assert_eq!(iter.next(), 5);
        assert_eq!(iter.next(), 6);
        assert!(!iter.has_next());
    }

    #[test]
    fn test_zigzag_iterator_empty_first() {
        let v1 = vec![];
        let v2 = vec![1, 2, 3];
        let mut iter = ZigzagIterator::new(v1, v2);

        assert!(iter.has_next());
        assert_eq!(iter.next(), 1);
        assert_eq!(iter.next(), 2);
        assert_eq!(iter.next(), 3);
        assert!(!iter.has_next());
    }

    #[test]
    fn test_zigzag_iterator_both_empty() {
        let v1 = vec![];
        let v2 = vec![];
        let iter = ZigzagIterator::new(v1, v2);

        assert!(!iter.has_next());
    }

    #[test]
    fn test_zigzag_iterator_brute_force_happy_path() {
        let v1 = vec![1, 2];
        let v2 = vec![3, 4, 5, 6];
        let mut iter = ZigzagIteratorBruteForce::new(v1, v2);

        assert_eq!(iter.next(), Some(1));
        assert_eq!(iter.next(), Some(3));
        assert_eq!(iter.next(), Some(2));
        assert_eq!(iter.next(), Some(4));
        assert_eq!(iter.next(), Some(5));
        assert_eq!(iter.next(), Some(6));
        assert_eq!(iter.next(), None);
    }
}
