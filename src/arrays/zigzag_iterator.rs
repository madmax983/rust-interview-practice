//! # 281. Zigzag Iterator
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/zigzag-iterator/>
//!
//! This problem demonstrates how to model stateful iterators over multiple collections in Rust.
//! It highlights the difference between eager evaluation (allocating a new combined vector)
//! and lazy evaluation (managing iterator state) which is crucial for zero-allocation performance.
//!
//! ## Approach
//!
//! The problem asks us to iterate through two lists in a zigzag fashion.
//!
//! 1. **Brute Force**: Eagerly combine both lists into a single `Vec` during initialization,
//!    then iterate over the combined list. Time: O(N+M) upfront, Space: O(N+M) additional allocation.
//! 2. **Optimized**: Use a queue (like `VecDeque`) of iterators. This naturally extends to `k` lists.
//!    Since we pop an iterator, take its next element, and push it back if it has more, we evaluate lazily.
//!    Time: O(1) per `next()`, Space: O(K) where K is the number of lists.
//! 3. **Optimal / Generic**: Since there are exactly two lists, we can store their `IntoIter` instances directly
//!    along with a boolean flag for the turn. This avoids the overhead of a queue.
//!    Time: O(1) per `next()`, Space: O(1) additional space.
//!    We also demonstrate trait bounds (`ExactSizeIterator`) for an educational generic version.
//!
//! In Python or Java, one might use a generator or store the lists and indices. In Rust,
//! passing ownership (`Vec<i32>`) allows us to use `IntoIter` directly, which consumes the list
//! without additional allocations, perfectly matching the zero-cost abstraction philosophy.
//!
//! // RUST INSIGHT:
//! // Leveraging `ExactSizeIterator::len()` (like on `std::vec::IntoIter`) allows us to check
//! // if an iterator has more elements without needing a mutable reference `&mut self`, which
//! // would be required if we used `std::iter::Peekable::peek()`.
//!
//! // GOTCHA:
//! // The `LeetCode` method signatures use `next(&mut self) -> i32`. This differs from Rust's
//! // standard `Iterator::next(&mut self) -> Option<Item>`, forcing us to explicitly bypass
//! // standard trait implementation for the exact signature match.

use std::collections::VecDeque;

/// Brute Force Approach: Eagerly allocate and combine.
pub struct ZigzagIteratorBruteForce {
    // We store an IntoIter of the pre-computed combined elements.
    merged: std::vec::IntoIter<i32>,
}

impl ZigzagIteratorBruteForce {
    #[must_use]
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(v1: Vec<i32>, v2: Vec<i32>) -> Self {
        let mut merged = Vec::with_capacity(v1.len() + v2.len());
        let mut iter1 = v1.into_iter();
        let mut iter2 = v2.into_iter();

        loop {
            let val1 = iter1.next();
            let val2 = iter2.next();
            if val1.is_none() && val2.is_none() {
                break;
            }
            if let Some(v) = val1 {
                merged.push(v);
            }
            if let Some(v) = val2 {
                merged.push(v);
            }
        }

        Self {
            merged: merged.into_iter(),
        }
    }

    #[allow(clippy::should_implement_trait)]
    /// # Panics
    /// Panics if called when `has_next` is false.
    pub fn next(&mut self) -> i32 {
        self.merged.next().unwrap()
    }

    #[must_use]
    pub fn has_next(&self) -> bool {
        self.merged.len() > 0
    }
}

/// Optimized Approach: Generalized for K lists using a queue of iterators.
pub struct ZigzagIteratorOptimized {
    queue: VecDeque<std::vec::IntoIter<i32>>,
}

impl ZigzagIteratorOptimized {
    #[must_use]
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(v1: Vec<i32>, v2: Vec<i32>) -> Self {
        let mut queue = VecDeque::with_capacity(2);

        // Only add iterators that actually have elements
        let i1 = v1.into_iter();
        if i1.len() > 0 {
            queue.push_back(i1);
        }
        let i2 = v2.into_iter();
        if i2.len() > 0 {
            queue.push_back(i2);
        }

        Self { queue }
    }

    #[allow(clippy::should_implement_trait)]
    /// # Panics
    /// Panics if called when `has_next` is false.
    pub fn next(&mut self) -> i32 {
        // Pop the front iterator
        let mut iter = self.queue.pop_front().unwrap();
        // Extract the next value
        let val = iter.next().unwrap();

        // RUST INSIGHT: ExactSizeIterator lets us check remaining elements
        // without mutating the iterator, unlike Peekable.
        if iter.len() > 0 {
            self.queue.push_back(iter);
        }

        val
    }

    #[must_use]
    pub fn has_next(&self) -> bool {
        !self.queue.is_empty()
    }
}

/// Educational Insight: A Generic Zigzag Iterator
/// Demonstrates trait bound application. By requiring `ExactSizeIterator`,
/// we ensure we can check `len()` without mutating the iterator.
pub struct ZigzagIteratorGeneric<I1, I2> {
    i1: I1,
    i2: I2,
    turn: bool,
}

impl<I1, I2, T> ZigzagIteratorGeneric<I1, I2>
where
    I1: ExactSizeIterator<Item = T>,
    I2: ExactSizeIterator<Item = T>,
{
    #[must_use]
    pub const fn new(i1: I1, i2: I2) -> Self {
        Self { i1, i2, turn: true }
    }

    #[allow(clippy::should_implement_trait)]
    /// # Panics
    /// Panics if called when `has_next` is false.
    pub fn next(&mut self) -> T {
        if self.turn {
            if self.i1.len() > 0 {
                self.turn = false;
                self.i1.next().unwrap()
            } else {
                self.i2.next().unwrap()
            }
        } else {
            if self.i2.len() > 0 {
                self.turn = true;
                self.i2.next().unwrap()
            } else {
                self.i1.next().unwrap()
            }
        }
    }

    #[must_use]
    pub fn has_next(&self) -> bool {
        self.i1.len() > 0 || self.i2.len() > 0
    }
}

/// Optimal Approach: Specialized for exactly 2 lists, zero-allocation.
/// Wraps our generic educational implementation to meet the `LeetCode` API.
pub struct ZigzagIteratorOptimal {
    inner: ZigzagIteratorGeneric<std::vec::IntoIter<i32>, std::vec::IntoIter<i32>>,
}

impl ZigzagIteratorOptimal {
    #[must_use]
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(v1: Vec<i32>, v2: Vec<i32>) -> Self {
        Self {
            inner: ZigzagIteratorGeneric::new(v1.into_iter(), v2.into_iter()),
        }
    }

    #[allow(clippy::should_implement_trait)]
    /// # Panics
    /// Panics if called when `has_next` is false.
    pub fn next(&mut self) -> i32 {
        self.inner.next()
    }

    #[must_use]
    pub fn has_next(&self) -> bool {
        self.inner.has_next()
    }
}

// -----------------------------------------------------------------------------
// Alternative Approaches
// -----------------------------------------------------------------------------
// 1. **Index-based State**: Store `v1: Vec<i32>` and `v2: Vec<i32>` and keep track of two indices `idx1` and `idx2`.
//    This works well if you need to retain ownership of the data (e.g. they are passed as references)
//    but since LeetCode passes them by value, `IntoIter` is more idiomatic and uses zero extra space.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force_happy_path() {
        let mut iter = ZigzagIteratorBruteForce::new(vec![1, 2], vec![3, 4, 5, 6]);
        let mut res = Vec::new();
        while iter.has_next() {
            res.push(iter.next());
        }
        assert_eq!(res, vec![1, 3, 2, 4, 5, 6]);
    }

    #[test]
    fn test_optimized_happy_path() {
        let mut iter = ZigzagIteratorOptimized::new(vec![1, 2], vec![3, 4, 5, 6]);
        let mut res = Vec::new();
        while iter.has_next() {
            res.push(iter.next());
        }
        assert_eq!(res, vec![1, 3, 2, 4, 5, 6]);
    }

    #[test]
    fn test_optimal_happy_path() {
        let mut iter = ZigzagIteratorOptimal::new(vec![1, 2], vec![3, 4, 5, 6]);
        let mut res = Vec::new();
        while iter.has_next() {
            res.push(iter.next());
        }
        assert_eq!(res, vec![1, 3, 2, 4, 5, 6]);
    }

    #[test]
    fn test_all_approaches_edge_case_empty_first() {
        let v1 = Vec::<i32>::new();
        let v2 = vec![1, 2, 3];

        let mut brute = ZigzagIteratorBruteForce::new(v1.clone(), v2.clone());
        let mut optz = ZigzagIteratorOptimized::new(v1.clone(), v2.clone());
        let mut optimal = ZigzagIteratorOptimal::new(v1, v2);

        let mut res_brute = Vec::new();
        while brute.has_next() {
            res_brute.push(brute.next());
        }

        let mut res_optz = Vec::new();
        while optz.has_next() {
            res_optz.push(optz.next());
        }

        let mut res_optimal = Vec::new();
        while optimal.has_next() {
            res_optimal.push(optimal.next());
        }

        assert_eq!(res_brute, vec![1, 2, 3]);
        assert_eq!(res_optz, vec![1, 2, 3]);
        assert_eq!(res_optimal, vec![1, 2, 3]);
    }

    #[test]
    fn test_all_approaches_edge_case_both_empty() {
        let v1 = Vec::<i32>::new();
        let v2 = Vec::<i32>::new();

        let brute = ZigzagIteratorBruteForce::new(v1.clone(), v2.clone());
        let optz = ZigzagIteratorOptimized::new(v1.clone(), v2.clone());
        let optimal = ZigzagIteratorOptimal::new(v1, v2);

        assert!(!brute.has_next());
        assert!(!optz.has_next());
        assert!(!optimal.has_next());
    }

    #[test]
    fn test_all_approaches_stress_uneven() {
        let v1 = vec![1, 2, 3, 4, 5];
        let v2 = vec![6];

        let mut brute = ZigzagIteratorBruteForce::new(v1.clone(), v2.clone());
        let mut optz = ZigzagIteratorOptimized::new(v1.clone(), v2.clone());
        let mut optimal = ZigzagIteratorOptimal::new(v1, v2);

        let mut res_brute = Vec::new();
        while brute.has_next() {
            res_brute.push(brute.next());
        }

        let mut res_optz = Vec::new();
        while optz.has_next() {
            res_optz.push(optz.next());
        }

        let mut res_optimal = Vec::new();
        while optimal.has_next() {
            res_optimal.push(optimal.next());
        }

        assert_eq!(res_brute, vec![1, 6, 2, 3, 4, 5]);
        assert_eq!(res_optz, vec![1, 6, 2, 3, 4, 5]);
        assert_eq!(res_optimal, vec![1, 6, 2, 3, 4, 5]);
    }
}
