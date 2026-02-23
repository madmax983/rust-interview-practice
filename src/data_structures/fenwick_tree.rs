//! # Fenwick Tree (Binary Indexed Tree) Implementation
//!
//! A data structure that provides efficient methods for calculation and manipulation of the prefix sums of a table of values.
//!
//! **Replaces Crates:** `fenwick-tree`, `bit_tree`
//!
//! **Real-world Usage:**
//! - Arithmetic Coding (compression algorithms).
//! - Order Statistic Trees (finding the k-th smallest element).
//! - Computational Geometry (range sum queries in multiple dimensions).
//! - Database indices for cumulative statistics.
//!
//! **Why build it yourself?**
//! It's a masterpiece of bit manipulation. Understanding how `i & -i` extracts the least significant bit
//! and how that relates to tree traversal in an array is a "Eureka!" moment in computer science.
//!
//! # Architecture
//!
//! **Concept:**
//! Each index `i` in the array stores the sum of a range of length `low_bit(i)`.
//!
//! **Diagram:**
//!
//! ```text
//! Index:  1   2   3   4   5   6   7   8
//! Value: [1] [2] [3] [4] [5] [6] [7] [8]
//! Tree:  [1] [3] [3] [10][5] [11][7] [36]
//!         │   │   │   │   │   │   │   │
//!         └──►│   │   │   │   │   │   │
//!             └──►│   │   │   │   │   │
//!                 └──►│   │   │   │   │
//!                     └──►│   │   │   │
//!                         │   │   │   │
//!                         └──►│   │   │
//!                             └──►│   │
//!                                 └──►│
//! ```
//!
//! **Invariants:**
//! *   `tree[i]` stores sum of `[i - (i&-i) + 1, i]`.
//! *   1-based indexing is used internally for bit magic, but API is 0-based.
//!
//! **Complexity:**
//! ┌───────────────┬─────────────┬─────────────┐
//! │ Operation     │ Time        │ Space       │
//! ├───────────────┼─────────────┼─────────────┤
//! │ Update        │ O(log N)    │ O(1)        │
//! │ Prefix Sum    │ O(log N)    │ O(1)        │
//! │ Range Sum     │ O(log N)    │ O(1)        │
//! │ Build         │ O(N)        │ O(N)        │
//! └───────────────┴─────────────┴─────────────┘

use std::ops::{AddAssign, Sub};

/// A Binary Indexed Tree (Fenwick Tree).
///
/// Supports point updates and range sum queries.
/// The stored type `T` must support addition, subtraction, copying, and have a default (zero) value.
#[derive(Clone, Debug)]
pub struct FenwickTree<T> {
    tree: Vec<T>,
}

impl<T> FenwickTree<T>
where
    T: Copy + Default + AddAssign + Sub<Output = T>,
{
    /// Creates a new Fenwick Tree of size `n` with all zeros.
    pub fn new(n: usize) -> Self {
        // Internal size is n + 1 because we use 1-based indexing.
        Self {
            tree: vec![T::default(); n + 1],
        }
    }

    /// Creates a Fenwick Tree from an existing slice (O(N) build).
    pub fn from_slice(data: &[T]) -> Self {
        let n = data.len();
        let mut tree = vec![T::default(); n + 1];

        // 1. Copy data to 1-based positions
        for (i, &val) in data.iter().enumerate() {
            tree[i + 1] = val;
        }

        // 2. Propagate sums
        // Each node `i` contributes to `i + (i & -i)`
        for i in 1..=n {
            let parent = i + (i & i.wrapping_neg()); // i + LSB(i)
            if parent <= n {
                let child_val = tree[i];
                tree[parent] += child_val;
            }
        }

        Self { tree }
    }

    /// Adds `delta` to the element at `index` (0-based).
    /// Time: O(log N)
    pub fn add(&mut self, index: usize, delta: T) {
        assert!(index < self.len(), "index out of bounds");
        let mut i = index + 1; // Convert to 1-based
        while i < self.tree.len() {
            self.tree[i] += delta;
            // i += LSB(i)
            i += i & i.wrapping_neg();
        }
    }

    /// Computes the prefix sum up to `index` (inclusive, 0-based).
    /// Returns sum of `[0, index]`.
    /// Time: O(log N)
    pub fn prefix_sum(&self, index: usize) -> T {
        assert!(index < self.len(), "index out of bounds");
        let mut sum = T::default();
        let mut i = index + 1; // Convert to 1-based

        while i > 0 {
            sum += self.tree[i];
            // i -= LSB(i)
            i -= i & i.wrapping_neg();
        }
        sum
    }

    /// Computes the sum of the range `[left, right]` (inclusive, 0-based).
    /// Time: O(log N)
    pub fn range_sum(&self, left: usize, right: usize) -> T {
        if left > right {
            return T::default();
        }
        let right_sum = self.prefix_sum(right);
        if left == 0 {
            right_sum
        } else {
            right_sum - self.prefix_sum(left - 1)
        }
    }

    /// Returns the capacity of the tree.
    pub fn len(&self) -> usize {
        self.tree.len() - 1
    }

    /// Returns true if empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

// RUST INSIGHT:
// Using `wrapping_neg()` is crucial because `-i` in two's complement is `(!i) + 1`.
// In Rust, unary `-` on unsigned types is not supported directly (except via wrapping_neg).
// Since `usize` is unsigned, we use `i.wrapping_neg()` to get the two's complement.
// `i & i.wrapping_neg()` gives the isolate lowest set bit (LSB).

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `fenwick-tree`: Provides similar functionality but often lacks the O(N) linear build constructor.
//
// Missing vs. Production:
// - **Multi-dimensional**: Production geometric systems use 2D/3D Fenwick trees.
// - **Binary Search**: One can find the smallest index with prefix_sum >= target in O(log N) by traversing the bit tree directly (binary lifting), instead of binary searching the `prefix_sum` function O(log^2 N).

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_sum_and_update() {
        let mut ft = FenwickTree::new(5);
        // [0, 0, 0, 0, 0]

        ft.add(0, 1);
        // [1, 0, 0, 0, 0]
        assert_eq!(ft.prefix_sum(0), 1);
        assert_eq!(ft.prefix_sum(4), 1);

        ft.add(2, 5);
        // [1, 0, 5, 0, 0]
        assert_eq!(ft.prefix_sum(1), 1); // sum[0..1] = 1
        assert_eq!(ft.prefix_sum(2), 6); // sum[0..2] = 1 + 0 + 5 = 6
        assert_eq!(ft.range_sum(1, 3), 5); // sum[1..3] = 0 + 5 + 0 = 5
    }

    #[test]
    fn test_from_slice_linear_build() {
        let data = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let ft = FenwickTree::from_slice(&data);

        assert_eq!(ft.prefix_sum(0), 1);
        assert_eq!(ft.prefix_sum(7), 36); // sum(1..8) = 36
        assert_eq!(ft.range_sum(2, 5), 3 + 4 + 5 + 6); // 18
    }

    #[test]
    fn test_updates_accumulate() {
        let mut ft = FenwickTree::new(10);
        ft.add(5, 10);
        ft.add(5, 20);
        assert_eq!(ft.range_sum(5, 5), 30);
    }

    #[test]
    fn test_empty() {
        let ft: FenwickTree<i32> = FenwickTree::new(0);
        assert!(ft.is_empty());
        assert_eq!(ft.len(), 0);
    }

    #[test]
    #[should_panic]
    fn test_out_of_bounds_access() {
        let ft: FenwickTree<i32> = FenwickTree::new(5);
        ft.prefix_sum(5); // Valid indices 0..4
    }

    #[test]
    fn test_i64_and_negative() {
        let mut ft = FenwickTree::new(4);
        ft.add(0, 10);
        ft.add(1, -5);
        assert_eq!(ft.prefix_sum(1), 5);
    }
}
