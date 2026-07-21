//! # 281. Zigzag Iterator
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/zigzag-iterator/>
//!
//! Why this matters in Rust: This problem illustrates custom `Iterator` implementations and managing
//! state across multiple underlying iterators. It provides a great contrast between brute force
//! approaches that allocate a new collection and optimal approaches that compose iterators lazily
//! with zero allocation.
//!
//! ## Approach
//!
//! We need to alternate yielding elements from two different lists.
//!
//! **Brute Force:**
//! Read all elements from both lists, alternate them, and store the result in a new vector.
//! Then just iterate over that vector.
//! - Time Complexity: O(n + m) for construction, O(1) per `next()` call.
//! - Space Complexity: O(n + m) auxiliary space to store the merged vector.
//!
//! **Optimal (Zero-Allocation Iterator Composition):**
//! Keep references (or iterators) to the original collections. Keep track of which iterator's turn
//! it is. We can store `std::vec::IntoIter<i32>` or similar. When `next()` is called, we fetch from
//! the active iterator and swap the turn. If one is exhausted, we yield from the other.
//! - Time Complexity: O(1) per `next()` call.
//! - Space Complexity: O(1) auxiliary space beyond the provided iterators.
//!
//! Why idiomatic Rust: We implement the standard `std::iter::Iterator` trait for our `ZigzagIterator`
//! when possible, but for `LeetCode` compatibility, we also provide a custom struct exposing `next(&mut self) -> i32`
//! and `has_next(&self) -> bool` methods. We use `Option` heavily to handle exhausted iterators safely
//! without index bounds checking.
//!
//! ## Alternative approaches
//!
//! We can generalize this to `k` iterators by storing a `VecDeque` of active iterators. In `next()`,
//! we pop an iterator from the front, yield an item, and if it still has items, push it to the back.

use std::collections::VecDeque;

/// Brute force approach: Merge all elements upfront into a new allocated vector.
pub struct ZigzagIteratorBruteForce {
    data: std::vec::IntoIter<i32>,
}

impl ZigzagIteratorBruteForce {
    #[must_use]
    #[allow(clippy::needless_pass_by_value)] // LeetCode signature
    pub fn new(v1: Vec<i32>, v2: Vec<i32>) -> Self {
        let mut merged = Vec::with_capacity(v1.len() + v2.len());
        let mut iter1 = v1.into_iter();
        let mut iter2 = v2.into_iter();

        loop {
            match (iter1.next(), iter2.next()) {
                (Some(val1), Some(val2)) => {
                    merged.push(val1);
                    merged.push(val2);
                }
                (Some(val1), None) => merged.push(val1),
                (None, Some(val2)) => merged.push(val2),
                (None, None) => break,
            }
        }

        Self {
            data: merged.into_iter(),
        }
    }

    /// # Panics
    ///
    /// Panics if called when `has_next` is false.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> i32 {
        // LeetCode's interface assumes `next` is only called if `has_next` is true.
        // In a real Rust application, we'd implement `Iterator` and return `Option<i32>`.
        self.data.next().unwrap()
    }

    #[must_use]
    pub fn has_next(&self) -> bool {
        self.data.len() > 0
    }
}

/// Optimal approach: Generalization to k-iterators using a deque of iterators.
/// This uses zero additional allocations beyond the deque of iterators itself.
pub struct ZigzagIteratorOptimal {
    // Store iterators in a deque to handle the round-robin yielding pattern easily.
    iters: VecDeque<std::vec::IntoIter<i32>>,
}

impl ZigzagIteratorOptimal {
    #[must_use]
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(v1: Vec<i32>, v2: Vec<i32>) -> Self {
        let mut iters = VecDeque::new();
        if !v1.is_empty() {
            iters.push_back(v1.into_iter());
        }
        if !v2.is_empty() {
            iters.push_back(v2.into_iter());
        }
        Self { iters }
    }

    // GOTCHA: LeetCode requires a `next(&mut self) -> i32` method, not returning an `Option`.
    // This deviates from idiomatic Rust. We use `#[allow(clippy::should_implement_trait)]` to
    // silence the pedantic clippy warning.
    /// # Panics
    ///
    /// Panics if called when `has_next` is false.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> i32 {
        // Pop the iterator whose turn it is
        if let Some(mut current_iter) = self.iters.pop_front() {
            // RUST INSIGHT: We safely fetch the next item. If the iterator isn't empty after yielding,
            // we push it back to the end of the queue for the next round.
            if let Some(val) = current_iter.next() {
                // If it has more elements, push it to the back
                if current_iter.len() > 0 {
                    self.iters.push_back(current_iter);
                }
                return val;
            }
        }
        // Panics if `next` is called when `has_next` is false.
        unreachable!("next called on empty iterator")
    }

    #[must_use]
    pub fn has_next(&self) -> bool {
        !self.iters.is_empty()
    }
}

pub type ZigzagIterator = ZigzagIteratorOptimal;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force_happy_path() {
        let mut iter = ZigzagIteratorBruteForce::new(vec![1, 2], vec![3, 4, 5, 6]);
        let mut result = Vec::new();
        while iter.has_next() {
            result.push(iter.next());
        }
        assert_eq!(result, vec![1, 3, 2, 4, 5, 6]);
    }

    #[test]
    fn test_optimal_happy_path() {
        let mut iter = ZigzagIteratorOptimal::new(vec![1, 2], vec![3, 4, 5, 6]);
        let mut result = Vec::new();
        while iter.has_next() {
            result.push(iter.next());
        }
        assert_eq!(result, vec![1, 3, 2, 4, 5, 6]);
    }

    #[test]
    fn test_edge_case_one_empty() {
        let mut iter = ZigzagIteratorOptimal::new(vec![], vec![1]);
        assert!(iter.has_next());
        assert_eq!(iter.next(), 1);
        assert!(!iter.has_next());
    }

    #[test]
    fn test_edge_case_both_empty() {
        let iter = ZigzagIteratorOptimal::new(vec![], vec![]);
        assert!(!iter.has_next());
    }

    #[test]
    fn test_stress_boundary() {
        let v1 = (0..1000).collect();
        let v2 = (1000..2000).collect();
        let mut iter = ZigzagIteratorOptimal::new(v1, v2);

        let mut count = 0;
        while iter.has_next() {
            let val = iter.next();
            if count % 2 == 0 {
                assert_eq!(val, count / 2); // from v1
            } else {
                assert_eq!(val, 1000 + count / 2); // from v2
            }
            count += 1;
        }
        assert_eq!(count, 2000);
    }
}
