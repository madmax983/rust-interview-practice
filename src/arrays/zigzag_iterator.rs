//! # 281. Zigzag Iterator
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/zigzag-iterator/>
//!
//! Why this matters in Rust: This problem is a natural fit for Rust's `Iterator` trait,
//! demonstrating how to manage internal state (multiple iterators) and implement custom
//! iterator logic. It highlights Rust's zero-cost abstractions by avoiding intermediate
//! allocations when iterating.
//!
//! ## Approach
//!
//! We will provide two implementations:
//! 1. `ZigzagIteratorBruteForce`: A straightforward approach that pre-computes the entire zigzag
//!    pattern into a single `VecDeque` during initialization. This uses extra memory but is
//!    conceptually simple.
//! 2. `ZigzagIterator`: An optimal, zero-allocation approach that holds iterators to the original
//!    vectors and dynamically yields elements in a zigzag fashion by maintaining state of whose turn
//!    it is. This is the true "idiomatic Rust" way, as it creates an adapter without copying data.
//!
//! ## Alternative approaches
//!
//! - Using `std::iter::Iterator::zip`: You could `zip` the two iterators, but `zip` stops when the
//!   shortest iterator is exhausted. For this problem, we need to continue yielding the remaining
//!   elements of the longer iterator, which makes `zip` unsuitable without additional `chain`ing logic.

use std::collections::VecDeque;
use std::iter::Peekable;
use std::vec::IntoIter;

/// Brute force approach: Pre-compute the zigzag pattern
/// Time: O(N) for initialization, O(1) for `next` and `has_next`
/// Space: O(N) where N is the total number of elements in v1 and v2
pub struct ZigzagIteratorBruteForce {
    queue: VecDeque<i32>,
}

impl ZigzagIteratorBruteForce {
    #[must_use]
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(v1: Vec<i32>, v2: Vec<i32>) -> Self {
        let mut queue = VecDeque::with_capacity(v1.len() + v2.len());
        let mut iter1 = v1.into_iter();
        let mut iter2 = v2.into_iter();

        // Alternating loop to pre-populate the queue
        loop {
            let val1 = iter1.next();
            let val2 = iter2.next();

            if val1.is_none() && val2.is_none() {
                break;
            }

            if let Some(v) = val1 {
                queue.push_back(v);
            }
            if let Some(v) = val2 {
                queue.push_back(v);
            }
        }

        Self { queue }
    }

    /// # Panics
    /// Panics if the queue is empty.
    #[allow(clippy::should_implement_trait)] // Required by LeetCode signature
    pub fn next(&mut self) -> i32 {
        // GOTCHA: For LeetCode, we assume `next` is only called if `has_next` is true.
        // In idiomatic Rust, `Iterator::next` returns `Option<T>` to handle emptiness safely.
        self.queue.pop_front().unwrap()
    }

    #[must_use]
    pub fn has_next(&self) -> bool {
        !self.queue.is_empty()
    }
}

/// Optimal approach: State management with underlying iterators
/// Time: O(1) for initialization, `next`, and `has_next`
/// Space: O(1) (excluding the iterators themselves, which take minimal space)
pub struct ZigzagIterator {
    // We store iterators rather than the original Vecs to avoid indexing and bounds checks.
    iter1: Peekable<IntoIter<i32>>,
    iter2: Peekable<IntoIter<i32>>,
    // Keeps track of whose turn it is. True for iter1, False for iter2.
    turn_first: bool,
}

impl ZigzagIterator {
    #[must_use]
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(v1: Vec<i32>, v2: Vec<i32>) -> Self {
        Self {
            // RUST INSIGHT: `into_iter` consumes the `Vec` and gives ownership of elements
            // to the iterator. `peekable` allows us to look ahead without consuming.
            iter1: v1.into_iter().peekable(),
            iter2: v2.into_iter().peekable(),
            turn_first: true,
        }
    }

    /// # Panics
    /// Panics if the iterator is empty.
    #[allow(clippy::should_implement_trait)] // Required by LeetCode signature
    pub fn next(&mut self) -> i32 {
        let has_first = self.iter1.peek().is_some();
        let has_second = self.iter2.peek().is_some();

        // Determine which iterator to pull from based on the current turn and availability
        if (self.turn_first && has_first) || !has_second {
            self.turn_first = false; // Toggle turn
            // Safe because we checked `has_first` or `has_second` is false (meaning first has elements given constraints)
            self.iter1.next().unwrap()
        } else {
            self.turn_first = true; // Toggle turn
            // Safe because if we reach here, we know `iter2` has elements.
            self.iter2.next().unwrap()
        }
    }

    pub fn has_next(&mut self) -> bool {
        // RUST INSIGHT: `peek` takes `&mut self` on `Peekable`, so this method requires `&mut self`.
        // This differs from some LeetCode platforms which might expect `&self` for `has_next`.
        self.iter1.peek().is_some() || self.iter2.peek().is_some()
    }
}

/// Idiomatic Rust standard library approach: Implementing the `Iterator` trait
/// This isn't the `LeetCode` signature, but it is how you would actually do this in a real Rust crate.
pub struct IdiomaticZigzagIterator<I1, I2> {
    iter1: I1,
    iter2: I2,
    turn_first: bool,
}

impl<I1, I2> IdiomaticZigzagIterator<I1, I2>
where
    I1: Iterator,
    I2: Iterator<Item = I1::Item>,
{
    #[must_use]
    pub const fn new(iter1: I1, iter2: I2) -> Self {
        Self {
            iter1,
            iter2,
            turn_first: true,
        }
    }
}

// RUST INSIGHT: By implementing `Iterator`, our struct automatically gets all iterator adapters
// like `map`, `filter`, `fold`, `collect`, etc., completely for free!
impl<I1, I2> Iterator for IdiomaticZigzagIterator<I1, I2>
where
    I1: Iterator,
    I2: Iterator<Item = I1::Item>,
{
    type Item = I1::Item;

    fn next(&mut self) -> Option<Self::Item> {
        if self.turn_first {
            self.turn_first = false;
            // Try getting from iter1 first. If it's exhausted, fallback to iter2.
            self.iter1.next().or_else(|| self.iter2.next())
        } else {
            self.turn_first = true;
            // Try getting from iter2 first. If it's exhausted, fallback to iter1.
            self.iter2.next().or_else(|| self.iter1.next())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force_happy_path() {
        let v1 = vec![1, 2];
        let v2 = vec![3, 4, 5, 6];
        let mut i = ZigzagIteratorBruteForce::new(v1, v2);

        let mut result = Vec::new();
        while i.has_next() {
            result.push(i.next());
        }
        assert_eq!(result, vec![1, 3, 2, 4, 5, 6]);
    }

    #[test]
    fn test_brute_force_empty() {
        let v1 = vec![];
        let v2 = vec![];
        let i = ZigzagIteratorBruteForce::new(v1, v2);
        assert!(!i.has_next());
    }

    #[test]
    fn test_optimal_happy_path() {
        let v1 = vec![1, 2];
        let v2 = vec![3, 4, 5, 6];
        let mut i = ZigzagIterator::new(v1, v2);

        let mut result = Vec::new();
        while i.has_next() {
            result.push(i.next());
        }
        assert_eq!(result, vec![1, 3, 2, 4, 5, 6]);
    }

    #[test]
    fn test_optimal_first_longer() {
        let v1 = vec![1, 2, 3, 4];
        let v2 = vec![5, 6];
        let mut i = ZigzagIterator::new(v1, v2);

        let mut result = Vec::new();
        while i.has_next() {
            result.push(i.next());
        }
        assert_eq!(result, vec![1, 5, 2, 6, 3, 4]);
    }

    #[test]
    fn test_optimal_empty() {
        let v1 = vec![];
        let v2 = vec![];
        let mut i = ZigzagIterator::new(v1, v2);
        assert!(!i.has_next());
    }

    #[test]
    fn test_idiomatic_happy_path() {
        let v1 = vec![1, 2];
        let v2 = vec![3, 4, 5, 6];

        // RUST INSIGHT: Because `IdiomaticZigzagIterator` implements `Iterator`,
        // we can just `.collect()` it into a Vec!
        let iter = IdiomaticZigzagIterator::new(v1.into_iter(), v2.into_iter());
        let result: Vec<i32> = iter.collect();

        assert_eq!(result, vec![1, 3, 2, 4, 5, 6]);
    }

    #[test]
    fn test_idiomatic_edge_cases() {
        // One empty, one not
        let v1: Vec<i32> = vec![];
        let v2 = vec![1];
        let iter = IdiomaticZigzagIterator::new(v1.into_iter(), v2.into_iter());
        assert_eq!(iter.collect::<Vec<i32>>(), vec![1]);

        // Both empty
        let v1: Vec<i32> = vec![];
        let v2: Vec<i32> = vec![];
        let iter = IdiomaticZigzagIterator::new(v1.into_iter(), v2.into_iter());
        assert_eq!(iter.collect::<Vec<i32>>(), Vec::<i32>::new());
    }
}
