//! # 281. Zigzag Iterator
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/zigzag-iterator/>
//!
//! Given two vectors of integers `v1` and `v2`, implement an iterator to return their elements alternately.
//!
//! This problem matters in Rust because it perfectly illustrates custom state management over collections.
//! It demonstrates how to combine multiple underlying iterators into a single stream, showcasing Rust's
//! zero-cost abstractions and the contrast between brute-force allocations and optimal in-place iteration.
//!
//! ## Approach
//!
//! We provide two solutions:
//! 1. `ZigzagIteratorBruteForce`: Flattens the vectors into a single queue during initialization.
//!    - **Time Complexity:** O(N + M) for initialization, O(1) for `next()`.
//!    - **Space Complexity:** O(N + M) to store the flattened elements.
//!    - **Why it's less idiomatic:** Pre-computing and allocating a new vector wastes memory and defeats
//!      the purpose of an iterator, which should ideally be lazy.
//!
//! 2. `ZigzagIterator`: The optimal, zero-allocation approach. It keeps the iterators of `v1` and `v2`
//!    and a pointer to track whose turn it is.
//!    - **Time Complexity:** O(1) for initialization, O(1) for `next()`.
//!    - **Space Complexity:** O(1) beyond the input vectors (which are consumed).
//!    - **Why it's idiomatic:** It uses `IntoIter` to consume the input vectors without additional allocations.
//!      It leverages `ExactSizeIterator` (via `.len()`) for an immutable `has_next` check, avoiding the need
//!      for a mutable `Peekable` adapter.

use std::vec::IntoIter;

/// Brute-force approach: Pre-computes the zigzag pattern and stores it in a single vector.
/// This approach is easy to write but sub-optimal in space complexity.
pub struct ZigzagIteratorBruteForce {
    data: IntoIter<i32>,
}

impl ZigzagIteratorBruteForce {
    #[must_use]
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(v1: Vec<i32>, v2: Vec<i32>) -> Self {
        let mut data = Vec::with_capacity(v1.len() + v2.len());
        let mut iter1 = v1.into_iter();
        let mut iter2 = v2.into_iter();

        // RUST INSIGHT: Here we manually drain both iterators into a new buffer.
        // While straightforward, this completely defeats the "lazy evaluation"
        // paradigm that iterators are meant to provide.
        loop {
            let val1 = iter1.next();
            let val2 = iter2.next();
            if val1.is_none() && val2.is_none() {
                break;
            }
            if let Some(v) = val1 {
                data.push(v);
            }
            if let Some(v) = val2 {
                data.push(v);
            }
        }

        Self {
            data: data.into_iter(),
        }
    }

    /// # Panics
    /// Panics if called when `has_next()` is false, consistent with LeetCode's constraints.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> i32 {
        self.data.next().unwrap()
    }

    #[must_use]
    pub fn has_next(&self) -> bool {
        self.data.len() > 0
    }
}

/// Optimal approach: Maintains iterators over the original vectors and tracks turns.
/// This avoids allocating O(N + M) memory for the combined output.
pub struct ZigzagIterator {
    // Array of iterators to allow dynamic indexing based on `current_turn`.
    iters: [IntoIter<i32>; 2],
    current_turn: usize,
}

impl ZigzagIterator {
    #[must_use]
    #[allow(clippy::needless_pass_by_value)] // Required by LeetCode signature
    pub fn new(v1: Vec<i32>, v2: Vec<i32>) -> Self {
        Self {
            // RUST INSIGHT: Consuming the vectors via `into_iter` transfers ownership.
            // This is zero-cost and avoids any deep cloning of the underlying data.
            iters: [v1.into_iter(), v2.into_iter()],
            current_turn: 0,
        }
    }

    /// # Panics
    /// Panics if called when the iterator is empty.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> i32 {
        // If the iterator for the current turn is exhausted, skip to the other one.
        if self.iters[self.current_turn].len() == 0 {
            self.current_turn = 1 - self.current_turn;
        }

        // GOTCHA: We must `unwrap()` here because LeetCode's signature returns `i32`
        // instead of `Option<i32>`. The platform guarantees `next` is only called
        // if `has_next` is true.
        let val = self.iters[self.current_turn].next().unwrap();

        // Alternate turns for the next call.
        self.current_turn = 1 - self.current_turn;
        val
    }

    #[must_use]
    pub fn has_next(&self) -> bool {
        // RUST INSIGHT: `std::vec::IntoIter` implements `ExactSizeIterator`, so `.len()`
        // is available and takes an immutable reference `&self`.
        // If we used `Peekable`, checking if the next item exists would require `&mut self`,
        // which breaks LeetCode's immutable `has_next(&self)` signature.
        self.iters[0].len() > 0 || self.iters[1].len() > 0
    }
}

// Optional standard library iterator implementation to show Rust idiomatic usage.
impl Iterator for ZigzagIterator {
    type Item = i32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.has_next() {
            Some(ZigzagIterator::next(self))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_happy_path() {
        let v1 = vec![1, 2];
        let v2 = vec![3, 4, 5, 6];

        // Brute force test
        let mut brute = ZigzagIteratorBruteForce::new(v1.clone(), v2.clone());
        let mut res_brute = Vec::<i32>::new();
        while brute.has_next() {
            res_brute.push(brute.next());
        }
        assert_eq!(res_brute, vec![1, 3, 2, 4, 5, 6]);

        // Optimal test
        let mut optimal = ZigzagIterator::new(v1, v2);
        let mut res_optimal = Vec::<i32>::new();
        while optimal.has_next() {
            res_optimal.push(optimal.next());
        }
        assert_eq!(res_optimal, vec![1, 3, 2, 4, 5, 6]);
    }

    #[test]
    fn test_edge_case_one_empty() {
        let v1 = vec![1];
        let v2 = Vec::<i32>::new();

        // Optimal test
        let mut optimal = ZigzagIterator::new(v1, v2);
        assert!(optimal.has_next());
        assert_eq!(optimal.next(), 1);
        assert!(!optimal.has_next());
    }

    #[test]
    fn test_edge_case_both_empty() {
        let v1: Vec<i32> = Vec::<i32>::new();
        let v2: Vec<i32> = Vec::<i32>::new();

        let optimal = ZigzagIterator::new(v1, v2);
        assert!(!optimal.has_next());
    }

    #[test]
    fn test_stress_case_large_difference() {
        let v1 = vec![1, 2, 3];
        let v2 = vec![4, 5, 6, 7, 8, 9, 10];

        let mut optimal = ZigzagIterator::new(v1, v2);
        let mut res = Vec::<i32>::new();
        while optimal.has_next() {
            res.push(optimal.next());
        }
        assert_eq!(res, vec![1, 4, 2, 5, 3, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn test_idiomatic_iterator_trait() {
        let v1 = vec![1, 2];
        let v2 = vec![3, 4, 5, 6];
        let optimal = ZigzagIterator::new(v1, v2);

        // Test standard Iterator trait implementation
        let res: Vec<i32> = optimal.collect();
        assert_eq!(res, vec![1, 3, 2, 4, 5, 6]);
    }
}

// Alternative approaches footer:
// - Generalization to K vectors: Instead of an array of 2 iterators, use a `VecDeque<IntoIter<i32>>`.
//   Pop from the front, take the next element, and if the iterator is not empty, push it back to the rear.
