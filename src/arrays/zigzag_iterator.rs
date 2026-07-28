//! # 281. Zigzag Iterator
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/zigzag-iterator/>
//!
//! Given two vectors of integers `v1` and `v2`, implement an iterator to return their elements alternately.
//!
//! This problem perfectly demonstrates Rust's powerful standard library `Iterator` trait, ownership model,
//! and the use of state machines. It highlights the difference between eager allocation (brute force),
//! manual state tracking (optimal zero-allocation), and idiomatic `Iterator` implementation that seamlessly
//! integrates with the language ecosystem.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::arrays::zigzag_iterator::ZigzagIterator;
//!
//! let v1 = vec![1, 2];
//! let v2 = vec![3, 4, 5, 6];
//! let mut i = ZigzagIterator::new(v1, v2);
//!
//! let mut result = Vec::new();
//! while i.has_next() {
//!     result.push(i.next());
//! }
//! assert_eq!(result, vec![1, 3, 2, 4, 5, 6]);
//! ```
//!
//! ## Constraints
//!
//! - `0 <= v1.length, v2.length <= 1000`
//! - `1 <= v1.length + v2.length <= 2000`
//! - `-10^9 <= v1[i], v2[i] <= 10^9`

use std::collections::VecDeque;

/// Brute Force approach: Eager allocation.
///
/// Time: O(n) where n is the total number of elements, to pre-compute the merged array.
/// Space: O(n) to store the merged array.
///
/// This approach simply merges the two vectors upfront during initialization.
/// It works, but it defeats the purpose of an iterator, which should ideally
/// be lazy and evaluate elements on demand, minimizing memory overhead.
pub struct ZigzagIteratorBruteForce {
    queue: VecDeque<i32>,
}

impl ZigzagIteratorBruteForce {
    #[must_use]
    #[allow(clippy::needless_pass_by_value)] // LeetCode signature
    pub fn new(v1: Vec<i32>, v2: Vec<i32>) -> Self {
        let mut queue = VecDeque::new();
        let mut it1 = v1.into_iter();
        let mut it2 = v2.into_iter();

        loop {
            match (it1.next(), it2.next()) {
                (Some(val1), Some(val2)) => {
                    queue.push_back(val1);
                    queue.push_back(val2);
                }
                (Some(val1), None) => {
                    queue.push_back(val1);
                }
                (None, Some(val2)) => {
                    queue.push_back(val2);
                }
                (None, None) => break,
            }
        }

        Self { queue }
    }

    #[must_use]
    #[allow(clippy::should_implement_trait)]
    /// Returns the next element in the zigzag sequence.
    ///
    /// # Panics
    /// Panics if there is no next element.
    pub fn next(&mut self) -> i32 {
        // GOTCHA: `.unwrap()` could panic, but LeetCode's `has_next()` check prevents it.
        // In a production codebase, `Iterator::next` returning an `Option<T>` is safer.
        self.queue.pop_front().unwrap()
    }

    #[must_use]
    pub fn has_next(&self) -> bool {
        !self.queue.is_empty()
    }
}

/// Optimal approach: Lazy Evaluation State Machine.
///
/// Time: O(1) for `new`, `next`, and `has_next`.
/// Space: O(1) beyond the input iterators themselves.
///
/// This approach models the zigzag behavior lazily. It stores the iterators
/// directly and keeps track of which iterator's turn it is. This achieves
/// zero-allocation during traversal (no extra `Vec` or `VecDeque`), truly
/// acting as an iterator.
pub struct ZigzagIterator {
    it1: std::vec::IntoIter<i32>,
    it2: std::vec::IntoIter<i32>,
    turn: bool, // true for it1, false for it2
}

impl ZigzagIterator {
    #[must_use]
    #[allow(clippy::needless_pass_by_value)] // LeetCode signature
    pub fn new(v1: Vec<i32>, v2: Vec<i32>) -> Self {
        Self {
            it1: v1.into_iter(),
            it2: v2.into_iter(),
            turn: true, // Start with v1
        }
    }

    #[must_use]
    #[allow(clippy::should_implement_trait)]
    /// Returns the next element in the zigzag sequence.
    ///
    /// # Panics
    /// Panics if there is no next element.
    pub fn next(&mut self) -> i32 {
        if self.turn {
            if self.it1.len() > 0 {
                self.turn = false; // Next is it2
                self.it1.next().unwrap()
            } else {
                // it1 is empty, must be in it2. We don't flip `turn` since it1 is exhausted.
                self.it2.next().unwrap()
            }
        } else if self.it2.len() > 0 {
            self.turn = true; // Next is it1
            self.it2.next().unwrap()
        } else {
            // it2 is empty, must be in it1.
            self.it1.next().unwrap()
        }
    }

    #[must_use]
    pub fn has_next(&self) -> bool {
        // RUST INSIGHT: We use `ExactSizeIterator::len()` here (`iter.len() > 0`)
        // because `has_next` takes an immutable reference `&self`.
        // If we used `std::iter::Peekable`, `peek()` would require `&mut self`.
        self.it1.len() > 0 || self.it2.len() > 0
    }
}

/// Idiomatic approach: Leveraging Rust's standard `Iterator` trait.
///
/// Time: O(1) for traversal methods.
/// Space: O(1) beyond inputs.
///
/// Instead of a custom struct with `has_next` and `next`, we can implement
/// standard `std::iter::Iterator`. This allows seamless integration with
/// Rust's `for` loops, `.map()`, `.collect()`, and other combinators.
pub struct ZigzagIteratorIdiomatic {
    it1: std::vec::IntoIter<i32>,
    it2: std::vec::IntoIter<i32>,
    turn: bool,
}

impl ZigzagIteratorIdiomatic {
    #[must_use]
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(v1: Vec<i32>, v2: Vec<i32>) -> Self {
        Self {
            it1: v1.into_iter(),
            it2: v2.into_iter(),
            turn: true,
        }
    }
}

// PRODUCTION NOTE: By implementing `Iterator`, `ZigzagIteratorIdiomatic` can now
// be used elegantly within Rust's extensive functional iteration ecosystem.
impl Iterator for ZigzagIteratorIdiomatic {
    type Item = i32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.turn {
            if self.it1.len() > 0 {
                self.turn = false;
                self.it1.next()
            } else {
                self.it2.next()
            }
        } else if self.it2.len() > 0 {
            self.turn = true;
            self.it2.next()
        } else {
            self.it1.next()
        }
    }
}

// Alternative approaches footer:
// - If the problem generalizes to `k` lists, we can store iterators in a `VecDeque`
//   and cycle through them, popping iterators that are exhausted. This cleanly
//   manages `k`-way zigzag patterns.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force_happy_path() {
        let v1 = vec![1, 2];
        let v2 = vec![3, 4, 5, 6];
        let mut i = ZigzagIteratorBruteForce::new(v1, v2);
        let mut result = Vec::<i32>::new();
        while i.has_next() {
            result.push(i.next());
        }
        assert_eq!(result, vec![1, 3, 2, 4, 5, 6]);
    }

    #[test]
    fn test_optimal_happy_path() {
        let v1 = vec![1, 2];
        let v2 = vec![3, 4, 5, 6];
        let mut i = ZigzagIterator::new(v1, v2);
        let mut result = Vec::<i32>::new();
        while i.has_next() {
            result.push(i.next());
        }
        assert_eq!(result, vec![1, 3, 2, 4, 5, 6]);
    }

    #[test]
    fn test_idiomatic_happy_path() {
        let v1 = vec![1, 2];
        let v2 = vec![3, 4, 5, 6];
        let i = ZigzagIteratorIdiomatic::new(v1, v2);
        let result: Vec<i32> = i.collect();
        assert_eq!(result, vec![1, 3, 2, 4, 5, 6]);
    }

    #[test]
    fn test_empty_vectors() {
        // v1 empty
        let v1 = Vec::<i32>::new();
        let v2 = vec![1];
        let mut i = ZigzagIterator::new(v1, v2.clone());
        let mut result = Vec::<i32>::new();
        while i.has_next() {
            result.push(i.next());
        }
        assert_eq!(result, vec![1]);

        let id = ZigzagIteratorIdiomatic::new(Vec::<i32>::new(), v2);
        let res_id: Vec<i32> = id.collect();
        assert_eq!(res_id, vec![1]);

        // v2 empty
        let v1 = vec![1];
        let v2 = Vec::<i32>::new();
        let mut i = ZigzagIterator::new(v1.clone(), v2);
        let mut result = Vec::<i32>::new();
        while i.has_next() {
            result.push(i.next());
        }
        assert_eq!(result, vec![1]);

        let id2 = ZigzagIteratorIdiomatic::new(v1, Vec::<i32>::new());
        let res_id2: Vec<i32> = id2.collect();
        assert_eq!(res_id2, vec![1]);

        // both empty
        let i = ZigzagIterator::new(Vec::<i32>::new(), Vec::<i32>::new());
        assert!(!i.has_next());

        let id3 = ZigzagIteratorIdiomatic::new(Vec::<i32>::new(), Vec::<i32>::new());
        let res_id3: Vec<i32> = id3.collect();
        assert_eq!(res_id3, Vec::<i32>::new());
    }

    #[test]
    fn test_v1_longer() {
        let v1 = vec![1, 2, 3];
        let v2 = vec![4];
        let mut i = ZigzagIterator::new(v1, v2);
        let mut result = Vec::<i32>::new();
        while i.has_next() {
            result.push(i.next());
        }
        assert_eq!(result, vec![1, 4, 2, 3]);
    }
}
