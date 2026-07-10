//! # 295. Find Median from Data Stream
//!
//! The **median** is the middle value in an ordered integer list. If the size of the list is even, there is no middle value, and the median is the mean of the two middle values.
//!
//! - For example, for `arr = [2,3,4]`, the median is `3`.
//! - For example, for `arr = [2,3]`, the median is `(2 + 3) / 2 = 2.5`.
//!
//! Implement the `MedianFinder` class:
//! - `MedianFinder()` initializes the `MedianFinder` object.
//! - `void addNum(int num)` adds the integer `num` from the data stream to the data structure.
//! - `double findMedian()` returns the median of all elements so far. Answers within `10^-5` of the actual answer will be accepted.
//!
//! Difficulty: Hard
//!
//! [LeetCode Problem 295](https://leetcode.com/problems/find-median-from-data-stream/)
//!
//! ## Why this matters in Rust
//! This problem perfectly illustrates the power of `std::collections::BinaryHeap` and Rust's strict typing system. Specifically, it demonstrates:
//! - **Min-Heap implementation**: `BinaryHeap` in Rust is a max-heap by default. We use `std::cmp::Reverse` as a zero-cost abstraction wrapper to turn it into a min-heap without having to write custom comparator logic.
//! - **Invariant preservation**: Maintaining balancing invariants across two independent collections (`max_heap` and `min_heap`) and safely updating them.
//! - **Casting safety**: Carefully casting `i32` to `f64` using `as f64` only when we have calculated the sum, or directly on `i32` elements, showcasing Rust's lack of implicit casting.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::heaps::find_median_from_data_stream::MedianFinder;
//!
//! let mut median_finder = MedianFinder::new();
//! median_finder.add_num(1);
//! median_finder.add_num(2);
//! assert_eq!(median_finder.find_median(), 1.5); // (1 + 2) / 2
//! median_finder.add_num(3);
//! assert_eq!(median_finder.find_median(), 2.0);
//! ```
//!
//! ## Constraints
//!
//! - `-10^5 <= num <= 10^5`
//! - There will be at least one element in the data structure before calling `findMedian`.
//! - At most `5 * 10^4` calls will be made to `addNum` and `findMedian`.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// Brute force approach: Store all numbers in a Vector, then sort upon request.
///
/// Time: O(1) for `addNum`, O(N log N) for `findMedian`
/// Space: O(N) to store all numbers
#[derive(Default)]
pub struct MedianFinderBruteForce {
    nums: Vec<i32>,
}

impl MedianFinderBruteForce {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_num(&mut self, num: i32) {
        self.nums.push(num);
    }

    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn find_median(&self) -> f64 {
        if self.nums.is_empty() {
            return 0.0;
        }

        // RUST INSIGHT: We don't want to mutate self in a getter, so we clone to sort.
        // This makes find_median very expensive!
        let mut sorted = self.nums.clone();
        sorted.sort_unstable();

        let n = sorted.len();
        if n % 2 == 1 {
            f64::from(sorted[n / 2])
        } else {
            // RUST INSIGHT: Ensure we convert to f64 first to avoid integer division truncation
            f64::midpoint(f64::from(sorted[n / 2 - 1]), f64::from(sorted[n / 2]))
        }
    }
}

/// Optimized approach: Insert numbers into a Vector in sorted order using binary search.
///
/// Time: O(N) for `addNum` due to `Vec::insert` shifting elements (O(log N) to find index). O(1) for `findMedian`.
/// Space: O(N) to store all numbers
#[derive(Default)]
pub struct MedianFinderOptimized {
    nums: Vec<i32>,
}

impl MedianFinderOptimized {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_num(&mut self, num: i32) {
        // RUST INSIGHT: binary_search returns Ok(idx) if found, or Err(idx) where it should be inserted.
        let pos = self.nums.binary_search(&num).unwrap_or_else(|e| e);

        // GOTCHA: insert() is O(N) because it shifts all subsequent elements to the right.
        self.nums.insert(pos, num);
    }

    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn find_median(&self) -> f64 {
        if self.nums.is_empty() {
            return 0.0;
        }

        let n = self.nums.len();
        if n % 2 == 1 {
            f64::from(self.nums[n / 2])
        } else {
            f64::midpoint(f64::from(self.nums[n / 2 - 1]), f64::from(self.nums[n / 2]))
        }
    }
}

/// Optimal approach: Use two Heaps. A Max-Heap for the lower half, and a Min-Heap for the upper half.
///
/// We maintain the invariant that `low` (max-heap) has either the same number of elements
/// as `high` (min-heap), or exactly one more element.
///
/// Time: O(log N) for `addNum`. O(1) for `findMedian`.
/// Space: O(N) to store elements in the two heaps.
#[derive(Default)]
pub struct MedianFinderOptimal {
    // Max-heap stores the smaller half of the numbers
    low: BinaryHeap<i32>,
    // Min-heap stores the larger half of the numbers
    // RUST INSIGHT: Reverse<T> implements Ord in reverse order, turning Max-Heap into Min-Heap
    high: BinaryHeap<Reverse<i32>>,
}

impl MedianFinderOptimal {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_num(&mut self, num: i32) {
        // Push to max-heap
        self.low.push(num);

        // Balance the heaps: max element of `low` must move to `high`
        // RUST INSIGHT: if let is idiomatic for handling Options when we only care about the Some case
        if let Some(max_from_low) = self.low.pop() {
            self.high.push(Reverse(max_from_low));
        }

        // Maintain the invariant: `low` can have at most 1 more element than `high`
        if self.low.len() < self.high.len()
            && let Some(Reverse(min_from_high)) = self.high.pop()
        {
            self.low.push(min_from_high);
        }
    }

    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn find_median(&self) -> f64 {
        if self.low.len() > self.high.len() {
            // We know low has at least one element here
            // GOTCHA: peek() returns Option<&T>, we need to dereference it.
            f64::from(*self.low.peek().unwrap_or(&0))
        } else {
            // Heaps are of equal size
            let l = f64::from(*self.low.peek().unwrap_or(&0));
            // RUST INSIGHT: Destructuring the Reverse tuple struct when peeking
            let h = match self.high.peek() {
                Some(Reverse(val)) => f64::from(*val),
                None => 0.0,
            };
            f64::midpoint(l, h)
        }
    }
}

/// Main entry point - uses optimal solution
pub type MedianFinder = MedianFinderOptimal;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force() {
        let mut mf = MedianFinderBruteForce::new();
        mf.add_num(1);
        mf.add_num(2);
        assert_eq!(mf.find_median(), 1.5);
        mf.add_num(3);
        assert_eq!(mf.find_median(), 2.0);
    }

    #[test]
    fn test_optimized() {
        let mut mf = MedianFinderOptimized::new();
        mf.add_num(1);
        mf.add_num(2);
        assert_eq!(mf.find_median(), 1.5);
        mf.add_num(3);
        assert_eq!(mf.find_median(), 2.0);
    }

    #[test]
    fn test_optimal() {
        let mut mf = MedianFinderOptimal::new();
        mf.add_num(1);
        mf.add_num(2);
        assert_eq!(mf.find_median(), 1.5);
        mf.add_num(3);
        assert_eq!(mf.find_median(), 2.0);
    }

    #[test]
    fn test_edge_cases() {
        let mut mf = MedianFinderOptimal::new();

        // Negative numbers and zeroes
        mf.add_num(-1);
        assert_eq!(mf.find_median(), -1.0);
        mf.add_num(-2);
        assert_eq!(mf.find_median(), -1.5);
        mf.add_num(-3);
        assert_eq!(mf.find_median(), -2.0);
        mf.add_num(-4);
        assert_eq!(mf.find_median(), -2.5);
        mf.add_num(5);
        assert_eq!(mf.find_median(), -2.0);
    }

    #[test]
    fn test_all_approaches_random_stress() {
        let mut brute = MedianFinderBruteForce::new();
        let mut optimized = MedianFinderOptimized::new();
        let mut optimal = MedianFinderOptimal::new();

        let inputs = [41, 35, 62, 5, 97, 108, 0, -50, 42, 42];

        for num in inputs {
            brute.add_num(num);
            optimized.add_num(num);
            optimal.add_num(num);

            assert_eq!(brute.find_median(), optimized.find_median());
            assert_eq!(optimized.find_median(), optimal.find_median());
        }
    }
}
