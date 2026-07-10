//! # Interval Tree Implementation
//!
//! An augmented binary search tree designed for efficient overlapping interval queries.
//!
//! **Replaces Crates:** `intervaltree`, `rust-lapper`
//!
//! **Real-world Usage:**
//! - Calendar applications to find overlapping events/meetings.
//! - Network routing protocols to find overlapping IP subnets.
//! - Window management in GUI systems.
//! - Bioinformatics (genomic feature overlap queries).
//!
//! **Why build it yourself?**
//! An Interval Tree is a classic example of an "augmented" data structure. It teaches you
//! how to extend a standard Binary Search Tree (BST) by maintaining additional metadata
//! (in this case, the `max` upper bound in the subtree) to speed up complex queries from O(N) to O(log N).

use std::cmp;
use std::fmt::Debug;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure:
// An Interval Tree is a Binary Search Tree (BST) where:
// 1. Each node stores an `Interval` (low, high).
// 2. The tree is ordered by the `low` value of the intervals.
// 3. Each node maintains a `max` value, which is the maximum `high` value of any interval
//    in the subtree rooted at that node.
//
//      Node
//      ┌───────────────────────┐
//      │ Interval: [15, 20]    │
//      │ Max: 30               │
//      └───────┬───────┬───────┘
//              │       │
//          ┌───┘       └───┐
//          ▼               ▼
//      [10, 30]          [17, 19]
//      Max: 30           Max: 19
//
// Invariants:
// 1. BST property: For any node N, all nodes in N's left subtree have `low` <= N.low,
//    and all nodes in N's right subtree have `low` >= N.low.
// 2. Max property: For any node N, N.max = max(N.interval.high, N.left.max, N.right.max).
// 3. Intervals are closed `[low, high]`, meaning `low <= high`.
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ Insert        │ O(log N)*   │ O(1)        │
// │ Find Overlap  │ O(log N)    │ O(1)        │
// │ Find All      │ O(R log N)**│ O(R)        │
// └───────────────┴─────────────┴─────────────┘
// * Assumes a balanced tree. This simple implementation does not self-balance, so worst-case is O(N).
// ** R is the number of overlapping intervals returned.
//
// Design Decisions:
// - **Memory Layout**: We use `Box<Node>` for tree links. A production tree might use an Arena allocator
//   or a flat `Vec` (array-backed tree) for better cache locality, similar to how ECS or standard `BTreeMap` is implemented.
// - **Balancing**: This is a naïve unbalanced BST for educational simplicity. A production Interval Tree
//   usually augments a Red-Black Tree or an AVL Tree to guarantee O(log N) depth.

/// A trait defining the interface for an Interval Mapping structure.
/// Shows how Rust traits enable swappable strategies (e.g., Tree-based vs Array-backed).
pub trait IntervalMap<T, V> {
    /// Inserts an interval and its associated value into the tree.
    fn insert(&mut self, interval: Interval<T>, value: V);
    /// Finds a single interval that overlaps with the given query interval.
    fn find_overlapping(&self, query: &Interval<T>) -> Option<(&Interval<T>, &V)>;
    /// Finds all intervals that overlap with the given query interval.
    fn find_all_overlapping(&self, query: &Interval<T>) -> Vec<(&Interval<T>, &V)>;
}

/// Represents a closed interval `[low, high]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interval<T> {
    pub low: T,
    pub high: T,
}

impl<T: PartialOrd> Interval<T> {
    /// Creates a new Interval.
    ///
    /// # Panics
    /// Panics if `low > high`.
    pub fn new(low: T, high: T) -> Self {
        assert!(low <= high, "Interval low must be <= high");
        Self { low, high }
    }

    /// Returns true if this interval overlaps with another interval.
    pub fn overlaps(&self, other: &Self) -> bool {
        self.low <= other.high && self.high >= other.low
    }
}

/// A node in the Interval Tree.
#[derive(Debug, Clone)]
struct Node<T, V> {
    interval: Interval<T>,
    value: V,
    max: T,
    left: Option<Box<Self>>,
    right: Option<Box<Self>>,
}

impl<T: Copy + Ord, V> Node<T, V> {
    const fn new(interval: Interval<T>, value: V) -> Self {
        let max = interval.high;
        Self {
            interval,
            value,
            max,
            left: None,
            right: None,
        }
    }

    /// Updates the `max` field based on the interval and children's max values.
    fn update_max(&mut self) {
        let mut max = self.interval.high;
        if let Some(ref l) = self.left {
            max = cmp::max(max, l.max);
        }
        if let Some(ref r) = self.right {
            max = cmp::max(max, r.max);
        }
        self.max = max;
    }
}

/// An Interval Tree mapping intervals to values.
#[derive(Debug, Clone)]
pub struct IntervalTree<T, V> {
    root: Option<Box<Node<T, V>>>,
    len: usize,
}

impl<T: Copy + Ord, V> Default for IntervalTree<T, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Copy + Ord, V> IntervalMap<T, V> for IntervalTree<T, V> {
    fn insert(&mut self, interval: Interval<T>, value: V) {
        let new_node = Box::new(Node::new(interval, value));
        if let Some(mut root) = self.root.take() {
            Self::insert_node(&mut root, new_node);
            self.root = Some(root);
        } else {
            self.root = Some(new_node);
        }
        self.len += 1;
    }

    fn find_overlapping(&self, query: &Interval<T>) -> Option<(&Interval<T>, &V)> {
        let mut current = self.root.as_ref();

        while let Some(node) = current {
            if node.interval.overlaps(query) {
                return Some((&node.interval, &node.value));
            }

            // RUST INSIGHT: The core optimization of the Interval Tree.
            // If the left child exists and its `max` is >= the query's `low`,
            // then there *might* be an overlapping interval in the left subtree.
            // Otherwise, we can safely skip the entire left subtree and search the right.
            if let Some(ref left) = node.left
                && left.max >= query.low
            {
                current = node.left.as_ref();
                continue;
            }

            // If we didn't go left, go right.
            current = node.right.as_ref();
        }

        None
    }

    fn find_all_overlapping(&self, query: &Interval<T>) -> Vec<(&Interval<T>, &V)> {
        let mut results = Vec::new();
        if let Some(ref root) = self.root {
            Self::find_all_overlapping_recursive(root, query, &mut results);
        }
        results
    }
}

impl<T: Copy + Ord, V> IntervalTree<T, V> {
    /// Creates a new, empty Interval Tree.
    #[must_use]
    pub const fn new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Returns the number of intervals in the tree.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns true if the tree contains no intervals.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Recursively inserts a new node into the tree, maintaining the BST property
    /// by `low` value and updating the augmented `max` values.
    fn insert_node(current: &mut Box<Node<T, V>>, new_node: Box<Node<T, V>>) {
        // RUST INSIGHT: BST ordering by `low`
        // We use the `low` bound as the primary key for the Binary Search Tree.
        // GOTCHA: If many intervals have the exact same `low` value, a standard BST
        // might degrade. Production trees often handle duplicates via lists at each node
        // or by using `high` as a secondary tie-breaker.
        if new_node.interval.low < current.interval.low {
            if let Some(ref mut left) = current.left {
                Self::insert_node(left, new_node);
            } else {
                current.left = Some(new_node);
            }
        } else {
            if let Some(ref mut right) = current.right {
                Self::insert_node(right, new_node);
            } else {
                current.right = Some(new_node);
            }
        }

        // Update the max value for the current node after insertion
        current.update_max();
    }

    fn find_all_overlapping_recursive<'a>(
        node: &'a Node<T, V>,
        query: &Interval<T>,
        results: &mut Vec<(&'a Interval<T>, &'a V)>,
    ) {
        // Early exit: if the max of this subtree is less than the query's low,
        // there can be no overlaps in this subtree.
        if node.max < query.low {
            return;
        }

        // Check if left subtree might contain overlaps
        if let Some(ref left) = node.left
            && left.max >= query.low
        {
            Self::find_all_overlapping_recursive(left, query, results);
        }

        // Check the current node
        if node.interval.overlaps(query) {
            results.push((&node.interval, &node.value));
        }

        // Check right subtree. We only need to visit the right subtree
        // if the query's high is >= the current node's low, because nodes in
        // the right subtree all have `low` >= `node.interval.low`.
        if query.high >= node.interval.low
            && let Some(ref right) = node.right
            && right.max >= query.low
        {
            Self::find_all_overlapping_recursive(right, query, results);
        }
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `intervaltree`: Implements a generic Interval Tree, usually augmented over a balanced tree like Red-Black to ensure O(log N) guarantees.
// - `rust-lapper`: An array-backed interval tree (or rather, a sorted array of intervals) optimized for genomic data. It's often faster for read-heavy workloads because of cache locality and binary search.
//
// Missing vs. Production:
// - **Self-Balancing**: This implementation is a standard BST. It can degrade to O(N) if intervals are inserted in sorted order.
//   Production versions use AVL or Red-Black logic to rotate nodes (updating `max` appropriately during rotations).
//   // PRODUCTION NOTE: Keeping the `max` field correctly updated during tree rotations is tricky
//   // but strictly necessary to maintain O(log N) query time in an augmented Red-Black tree.
// - **Deletion**: We omit the `remove` operation for simplicity, as deleting from a BST while updating the augmented `max` values recursively is complex.
//
// Suggested next steps / extensions:
// 1. Implement self-balancing rotations (e.g., Red-Black tree rules) while correctly maintaining `max`.
// 2. Implement the `remove` operation.
// 3. Build a flat array-backed Interval Tree layout (like `rust-lapper`) for cache locality comparisons.
//
// Benchmarking Note:
// Use `criterion` to benchmark this implementation. Measure insertion throughput
// and query latency using `std::hint::black_box()` to prevent compiler optimizations.
// A typical test would be inserting 100,000 intervals and querying 1,000 random
// points or intervals against them.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interval_overlaps() {
        let i1 = Interval::new(15, 20);
        let i2 = Interval::new(10, 30);
        let i3 = Interval::new(17, 19);
        let i4 = Interval::new(5, 10);
        let i5 = Interval::new(25, 30);

        assert!(i1.overlaps(&i2));
        assert!(i1.overlaps(&i3));
        assert!(!i1.overlaps(&i4)); // 20 >= 5, but 15 <= 10 is false
        assert!(!i1.overlaps(&i5));

        // Edge cases (touching boundaries)
        let i6 = Interval::new(20, 25);
        assert!(i1.overlaps(&i6)); // 15 <= 25 and 20 >= 20
    }

    #[test]
    #[should_panic(expected = "Interval low must be <= high")]
    fn test_invalid_interval() {
        Interval::new(20, 15);
    }

    #[test]
    fn test_insert_and_find_single() {
        let mut tree = IntervalTree::new();
        tree.insert(Interval::new(15, 20), "A");
        tree.insert(Interval::new(10, 30), "B");
        tree.insert(Interval::new(17, 19), "C");
        tree.insert(Interval::new(5, 20), "D");
        tree.insert(Interval::new(12, 15), "E");
        tree.insert(Interval::new(30, 40), "F");

        assert_eq!(tree.len(), 6);

        // Query overlapping [14, 16]
        // Could be [15, 20], [10, 30], [5, 20], [12, 15]
        let query = Interval::new(14, 16);
        let overlap = tree.find_overlapping(&query);
        assert!(overlap.is_some());

        // Verify the overlap is correct
        let (interval, _) = overlap.unwrap();
        assert!(interval.overlaps(&query));
    }

    #[test]
    fn test_find_all_overlapping() {
        let mut tree = IntervalTree::new();
        tree.insert(Interval::new(15, 20), "A");
        tree.insert(Interval::new(10, 30), "B");
        tree.insert(Interval::new(17, 19), "C");
        tree.insert(Interval::new(5, 8), "D");
        tree.insert(Interval::new(25, 35), "E");

        let query = Interval::new(16, 18);
        let mut overlaps = tree.find_all_overlapping(&query);

        // Sort results by value to make assertion stable
        overlaps.sort_by_key(|(_, val)| **val);

        assert_eq!(overlaps.len(), 3);
        assert_eq!(*overlaps[0].1, "A");
        assert_eq!(*overlaps[1].1, "B");
        assert_eq!(*overlaps[2].1, "C");
    }

    #[test]
    fn test_no_overlaps() {
        let mut tree = IntervalTree::new();
        tree.insert(Interval::new(1, 5), "A");
        tree.insert(Interval::new(10, 15), "B");

        let query = Interval::new(6, 9);
        assert!(tree.find_overlapping(&query).is_none());
        assert!(tree.find_all_overlapping(&query).is_empty());
    }

    #[test]
    fn test_max_augmentation() {
        let mut tree = IntervalTree::new();
        tree.insert(Interval::new(10, 20), "A");
        tree.insert(Interval::new(5, 30), "B"); // Left child, higher max
        tree.insert(Interval::new(15, 25), "C"); // Right child

        // Root max should be 30
        let root = tree.root.as_ref().unwrap();
        assert_eq!(root.max, 30);
    }
}
