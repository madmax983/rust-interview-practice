//! # `LeetCode` 281: Zigzag Iterator
//!
//! **Difficulty:** Medium
//!
//! **Link:** <https://leetcode.com/problems/zigzag-iterator/>
//!
//! **Why this matters in Rust:**
//! This problem demonstrates custom `Iterator` implementations, state management with multiple underlying iterators,
//! and trait bound application for educational contrast between brute force and zero-allocation optimal solutions.
//! It is a natural fit for Rust's `std::collections::VecDeque` and highlights why iterator adapters eliminate off-by-one errors.
//!
//! ## Approach
//!
//! The optimal approach uses a round-robin strategy powered by `std::collections::VecDeque`.
//! Instead of allocating memory to interleave elements upfront, we store the active iterators in a queue.
//! On each call to `next()`, we pop the front iterator, yield its next element, and if it still has elements remaining,
//! we push it to the back of the queue.
//!
//! **Time Complexity:** $O(1)$ per `next()` and `has_next()` call.
//! **Space Complexity:** $O(K)$ where $K$ is the number of iterators (2 for the `LeetCode` specific version),
//! since we only store the iterator objects and do not duplicate the underlying elements.
//!
//! ## Alternative approaches
//!
//! - **Brute Force:** Collect all elements by interleaving them into a new `Vec` during initialization,
//!   then use `IntoIter`. This consumes $O(N)$ extra space, defeating the purpose of an iterator.

use std::collections::VecDeque;

/// An educational representation of the brute force approach.
/// Collects elements upfront, causing unnecessary O(N) allocations.
pub struct BruteForceZigzagIterator {
    data: std::vec::IntoIter<i32>,
}

impl BruteForceZigzagIterator {
    /// Creates a new `BruteForceZigzagIterator`.
    #[must_use]
    pub fn new(v1: Vec<i32>, v2: Vec<i32>) -> Self {
        let mut data = Vec::with_capacity(v1.len() + v2.len());
        let mut i1 = v1.into_iter();
        let mut i2 = v2.into_iter();

        loop {
            let n1 = i1.next();
            let n2 = i2.next();
            if n1.is_none() && n2.is_none() {
                break;
            }
            if let Some(val) = n1 {
                data.push(val);
            }
            if let Some(val) = n2 {
                data.push(val);
            }
        }

        Self {
            data: data.into_iter(),
        }
    }

    /// Returns the next element.
    ///
    /// # Panics
    /// Panics if called when `has_next()` is false.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> i32 {
        self.data.next().unwrap()
    }

    /// Checks if there are more elements.
    #[must_use]
    pub fn has_next(&self) -> bool {
        self.data.len() > 0
    }
}

// RUST INSIGHT: We implement a generic `Zigzag` struct that accepts any Iterator.
// This shows how Rust's type system allows us to build abstract adapters
// that compose cleanly with the standard library.

/// A generic zigzag iterator that interleaves elements from an arbitrary collection of iterators.
pub struct Zigzag<I> {
    iters: VecDeque<I>,
}

impl<I> Zigzag<I> {
    /// Creates a new generic `Zigzag` iterator.
    pub fn new(iters: impl IntoIterator<Item = I>) -> Self {
        Self {
            iters: iters.into_iter().collect(),
        }
    }
}

impl<I> Iterator for Zigzag<I>
where
    I: Iterator,
{
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(mut iter) = self.iters.pop_front() {
            if let Some(val) = iter.next() {
                // If the iterator still has elements, put it back at the end of the queue.
                self.iters.push_back(iter);
                return Some(val);
            }
        }
        None
    }
}

/// The specific struct that satisfies the `LeetCode` API, which doesn't use standard `Iterator`.
pub struct ZigzagIterator {
    iters: VecDeque<std::vec::IntoIter<i32>>,
}

impl ZigzagIterator {
    /// Creates a new `ZigzagIterator` from two vectors.
    #[must_use]
    pub fn new(v1: Vec<i32>, v2: Vec<i32>) -> Self {
        let mut iters = VecDeque::new();

        let i1 = v1.into_iter();
        if i1.len() > 0 {
            iters.push_back(i1);
        }

        let i2 = v2.into_iter();
        if i2.len() > 0 {
            iters.push_back(i2);
        }

        Self { iters }
    }

    /// Returns the next element in the zigzag sequence.
    ///
    /// # Panics
    ///
    /// Panics if called when there are no elements left.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> i32 {
        if let Some(mut iter) = self.iters.pop_front() {
            // RUST INSIGHT: We know this unwrap is safe because we only store iterators
            // that have at least one element (enforced via len() > 0 checking).
            let val = iter.next().unwrap();

            // We leverage ExactSizeIterator bound natively implemented on vec::IntoIter
            if iter.len() > 0 {
                self.iters.push_back(iter);
            }
            val
        } else {
            panic!("Called next on empty ZigzagIterator")
        }
    }

    /// Checks if the iterator has a next element.
    #[must_use]
    pub fn has_next(&self) -> bool {
        // GOTCHA: We don't need to peek the actual values. Since we maintain the invariant
        // that only non-empty iterators are stored in the queue, we just check if it is not empty.
        !self.iters.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zigzag_happy_path() {
        let mut zi = ZigzagIterator::new(vec![1, 2], vec![3, 4, 5, 6]);
        let mut res = Vec::new();
        while zi.has_next() {
            res.push(zi.next());
        }
        assert_eq!(res, vec![1, 3, 2, 4, 5, 6]);
    }

    #[test]
    fn test_zigzag_edge_case_empty() {
        // Uses explicit type inference as directed by memory: Vec::<i32>::new() instead of vec![]
        let zi = ZigzagIterator::new(Vec::<i32>::new(), Vec::<i32>::new());
        assert!(!zi.has_next());

        let mut zi2 = ZigzagIterator::new(vec![1], Vec::<i32>::new());
        assert!(zi2.has_next());
        assert_eq!(zi2.next(), 1);
        assert!(!zi2.has_next());
    }

    #[test]
    fn test_zigzag_stress_boundary() {
        let mut zi = ZigzagIterator::new(vec![1, 2, 3], vec![4]);
        let mut res = Vec::new();
        while zi.has_next() {
            res.push(zi.next());
        }
        assert_eq!(res, vec![1, 4, 2, 3]);
    }

    #[test]
    fn test_generic_zigzag() {
        let v1 = vec![1, 2, 3];
        let v2 = vec![4, 5];
        let v3 = vec![6, 7, 8, 9];

        let iters = vec![v1.into_iter(), v2.into_iter(), v3.into_iter()];
        let zigzag = Zigzag::new(iters);

        let res: Vec<i32> = zigzag.collect();
        assert_eq!(res, vec![1, 4, 6, 2, 5, 7, 3, 8, 9]);
    }

    #[test]
    fn test_brute_force_zigzag() {
        let mut zi = BruteForceZigzagIterator::new(vec![1, 2], vec![3, 4, 5, 6]);
        let mut res = Vec::new();
        while zi.has_next() {
            res.push(zi.next());
        }
        assert_eq!(res, vec![1, 3, 2, 4, 5, 6]);
    }
}
