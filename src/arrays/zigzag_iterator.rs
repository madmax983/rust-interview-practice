//! # 281. Zigzag Iterator
//!
//! **Difficulty:** Medium
//!
//! Given two vectors of integers `v1` and `v2`, implement an iterator to return their elements alternately.
//!
//! [LeetCode Problem 281](https://leetcode.com/problems/zigzag-iterator/)
//!
//! ## Why This Matters in Rust
//!
//! This problem perfectly demonstrates Rust's powerful iterator system and ownership semantics.
//! While other languages might just use index pointers, Rust allows us to model this via
//! custom iterators holding underlying iterators (`Vec::into_iter`). This problem highlights
//! the difference between:
//! 1. Allocating intermediate results (Brute force).
//! 2. Mutating iterators via a state machine without allocations (Optimal).
//!
//! ## Approach
//!
//! We explore three implementations:
//! - **Brute Force:** Pre-calculate the entire zigzag sequence into a new `Vec`, then just
//!   yield from its iterator. Simple, but O(N) space.
//! - **Optimized:** Hold the two iterators as options or index-pointers into references.
//! - **Optimal (Idiomatic Rust):** Store a queue/deque of active iterators. This naturally
//!   scales from 2 vectors to `K` vectors without modifying the core logic.
//!   It avoids extra allocations by consuming `IntoIter`s.

// RUST INSIGHT: We implement these as distinct structs to demonstrate different architectural patterns.
// In a real Rust codebase, we'd usually build a single struct implementing `Iterator`.

// ============================================================================
// Brute Force Approach: Pre-allocate the entire sequence
// ============================================================================

/// Time: O(N + M) during initialization, O(1) per `next()` call.
/// Space: O(N + M) to store the merged elements.
pub struct ZigzagIteratorBruteForce {
    data: std::vec::IntoIter<i32>,
}

impl ZigzagIteratorBruteForce {
    /// Initialize by eagerly interleaving elements into a single vector.
    #[must_use]
    pub fn new(v1: Vec<i32>, v2: Vec<i32>) -> Self {
        let mut merged = Vec::with_capacity(v1.len() + v2.len());
        let mut iter1 = v1.into_iter();
        let mut iter2 = v2.into_iter();

        loop {
            match (iter1.next(), iter2.next()) {
                (Some(a), Some(b)) => {
                    merged.push(a);
                    merged.push(b);
                }
                (Some(a), None) => merged.push(a),
                (None, Some(b)) => merged.push(b),
                (None, None) => break,
            }
        }

        Self {
            data: merged.into_iter(),
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> i32 {
        self.data.next().unwrap_or(0) // Problem typically assumes valid calls or we handle via Option.
    }

    #[must_use]
    pub fn has_next(&self) -> bool {
        // RUST INSIGHT: Since std::vec::IntoIter implements ExactSizeIterator,
        // we can easily check length without consuming elements.
        self.data.len() > 0
    }
}

// ============================================================================
// Optimized Approach: Toggle State Machine
// ============================================================================

/// Time: O(1) per `next()` call.
/// Space: O(1) beyond the input iterators.
pub struct ZigzagIteratorOptimized {
    iter1: std::vec::IntoIter<i32>,
    iter2: std::vec::IntoIter<i32>,
    turn: bool, // true = iter1's turn, false = iter2's turn
}

impl ZigzagIteratorOptimized {
    #[must_use]
    pub fn new(v1: Vec<i32>, v2: Vec<i32>) -> Self {
        Self {
            iter1: v1.into_iter(),
            iter2: v2.into_iter(),
            turn: true,
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> i32 {
        if self.turn {
            if let Some(val) = self.iter1.next() {
                self.turn = false;
                return val;
            }
            // iter1 is exhausted, fall back to iter2 (no turn change)
            self.iter2.next().unwrap_or(0)
        } else {
            if let Some(val) = self.iter2.next() {
                self.turn = true;
                return val;
            }
            // iter2 is exhausted, fall back to iter1
            self.iter1.next().unwrap_or(0)
        }
    }

    #[must_use]
    pub fn has_next(&self) -> bool {
        self.iter1.len() > 0 || self.iter2.len() > 0
    }
}

// ============================================================================
// Optimal Approach: Queue of Iterators (Scalable to K vectors)
// ============================================================================

use std::collections::VecDeque;

/// Time: O(1) per `next()` call.
/// Space: O(K) where K is the number of input vectors (here K=2).
pub struct ZigzagIteratorOptimal {
    // GOTCHA: We use VecDeque here so we can cycle through active iterators
    // by popping from the front and pushing to the back.
    iters: VecDeque<std::vec::IntoIter<i32>>,
}

impl ZigzagIteratorOptimal {
    #[must_use]
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

    /// Get the next element.
    ///
    /// # Panics
    ///
    /// Panics if the popped iterator is unexpectedly empty.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> i32 {
        if let Some(mut iter) = self.iters.pop_front() {
            // Unwrapping is safe because we only store non-empty iterators
            let val = iter.next().unwrap();

            // If the iterator still has elements, put it at the back of the queue
            if iter.len() > 0 {
                self.iters.push_back(iter);
            }
            val
        } else {
            0 // Or panic/Option depending on exact problem signature
        }
    }

    #[must_use]
    pub fn has_next(&self) -> bool {
        !self.iters.is_empty()
    }
}

// RUST INSIGHT: Idiomatic Rust would implement `Iterator` trait instead of custom `next` and `has_next`.
// We just enqueue all, and lazily filter exhausted ones during `next()`.

pub struct IdiomaticZigzag<I> {
    iters: VecDeque<I>,
}

impl<I: Iterator> IdiomaticZigzag<I> {
    pub fn new(iters: impl IntoIterator<Item = I>) -> Self {
        Self {
            iters: iters.into_iter().collect(),
        }
    }
}

impl<I: Iterator> Iterator for IdiomaticZigzag<I> {
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(mut iter) = self.iters.pop_front() {
            if let Some(val) = iter.next() {
                self.iters.push_back(iter);
                return Some(val);
            }
        }
        None
    }
}

// ============================================================================
// Alternative Approaches
// ============================================================================
// 1. **Index-based Tracking:** Maintain pointers `i` and `j` and alternate reading from
//    slices `&[i32]`. This works if we only borrow, but requires the original vectors
//    to stay alive (lifetime bounds). Consuming `IntoIter` avoids lifetimes entirely.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force() {
        let mut iter = ZigzagIteratorBruteForce::new(vec![1, 2], vec![3, 4, 5, 6]);
        let mut result = vec![];
        while iter.has_next() {
            result.push(iter.next());
        }
        assert_eq!(result, vec![1, 3, 2, 4, 5, 6]);
    }

    #[test]
    fn test_optimized() {
        let mut iter = ZigzagIteratorOptimized::new(vec![1, 2], vec![3, 4, 5, 6]);
        let mut result = vec![];
        while iter.has_next() {
            result.push(iter.next());
        }
        assert_eq!(result, vec![1, 3, 2, 4, 5, 6]);
    }

    #[test]
    fn test_optimal() {
        let mut iter = ZigzagIteratorOptimal::new(vec![1, 2], vec![3, 4, 5, 6]);
        let mut result = vec![];
        while iter.has_next() {
            result.push(iter.next());
        }
        assert_eq!(result, vec![1, 3, 2, 4, 5, 6]);
    }

    #[test]
    fn test_idiomatic() {
        let v1 = vec![1, 2];
        let v2 = vec![3, 4, 5, 6];
        let iter = IdiomaticZigzag::new(vec![v1.into_iter(), v2.into_iter()]);
        let result: Vec<i32> = iter.collect();
        assert_eq!(result, vec![1, 3, 2, 4, 5, 6]);
    }

    #[test]
    fn test_empty_first() {
        let mut iter = ZigzagIteratorOptimal::new(vec![], vec![1]);
        assert!(iter.has_next());
        assert_eq!(iter.next(), 1);
        assert!(!iter.has_next());
    }

    #[test]
    fn test_both_empty() {
        let iter = ZigzagIteratorOptimal::new(vec![], vec![]);
        assert!(!iter.has_next());
    }

    #[test]
    fn test_idiomatic_k_vectors() {
        let v1 = vec![1, 2, 3];
        let v2 = vec![4, 5, 6, 7];
        let v3 = vec![8, 9];
        let iter = IdiomaticZigzag::new(vec![v1.into_iter(), v2.into_iter(), v3.into_iter()]);
        let result: Vec<i32> = iter.collect();
        assert_eq!(result, vec![1, 4, 8, 2, 5, 9, 3, 6, 7]);
    }
}
