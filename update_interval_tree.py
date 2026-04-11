import re

with open("src/data_structures/interval_tree.rs", "r") as f:
    content = f.read()

# Add trait definition
trait_def = """/// A trait defining the interface for an Interval Mapping structure.
/// Shows how Rust traits enable swappable strategies (e.g., Tree-based vs Array-backed).
pub trait IntervalMap<T, V> {
    /// Inserts an interval and its associated value into the tree.
    fn insert(&mut self, interval: Interval<T>, value: V);
    /// Finds a single interval that overlaps with the given query interval.
    fn find_overlapping(&self, query: &Interval<T>) -> Option<(&Interval<T>, &V)>;
    /// Finds all intervals that overlap with the given query interval.
    fn find_all_overlapping(&self, query: &Interval<T>) -> Vec<(&Interval<T>, &V)>;
}

/// Represents a closed interval `[low, high]`."""

content = content.replace("/// Represents a closed interval `[low, high]`.", trait_def)

# Add GOTCHA comment
gotcha = """        // RUST INSIGHT: BST ordering by `low`
        // We use the `low` bound as the primary key for the Binary Search Tree.
        // GOTCHA: If many intervals have the exact same `low` value, a standard BST
        // might degrade. Production trees often handle duplicates via lists at each node
        // or by using `high` as a secondary tie-breaker.
        if new_node.interval.low < current.interval.low {"""
content = content.replace("""        // RUST INSIGHT: BST ordering by `low`
        if new_node.interval.low < current.interval.low {""", gotcha)

# Split impl block to impl trait
impl_tree = """impl<T: Copy + Ord, V> IntervalMap<T, V> for IntervalTree<T, V> {
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
            if let Some(ref left) = node.left {
                if left.max >= query.low {
                    current = node.left.as_ref();
                    continue;
                }
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
    pub fn new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Returns the number of intervals in the tree.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns true if the tree contains no intervals.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Recursively inserts a new node into the tree, maintaining the BST property"""

old_impl = """impl<T: Copy + Ord, V> IntervalTree<T, V> {
    /// Creates a new, empty Interval Tree.
    #[must_use]
    pub fn new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Returns the number of intervals in the tree.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns true if the tree contains no intervals.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Inserts an interval and its associated value into the tree.
    pub fn insert(&mut self, interval: Interval<T>, value: V) {
        let new_node = Box::new(Node::new(interval, value));
        if let Some(mut root) = self.root.take() {
            Self::insert_node(&mut root, new_node);
            self.root = Some(root);
        } else {
            self.root = Some(new_node);
        }
        self.len += 1;
    }

    /// Recursively inserts a new node into the tree, maintaining the BST property"""

content = content.replace(old_impl, impl_tree)

# Remove the trait methods from the impl block
remove1 = """
    /// Finds a single interval that overlaps with the given query interval.
    /// Returns a reference to the `(Interval, Value)` pair if an overlap is found.
    pub fn find_overlapping(&self, query: &Interval<T>) -> Option<(&Interval<T>, &V)> {
        let mut current = self.root.as_ref();

        while let Some(node) = current {
            if node.interval.overlaps(query) {
                return Some((&node.interval, &node.value));
            }

            // RUST INSIGHT: The core optimization of the Interval Tree.
            // If the left child exists and its `max` is >= the query's `low`,
            // then there *might* be an overlapping interval in the left subtree.
            // Otherwise, we can safely skip the entire left subtree and search the right.
            if let Some(ref left) = node.left {
                if left.max >= query.low {
                    current = node.left.as_ref();
                    continue;
                }
            }

            // If we didn't go left, go right.
            current = node.right.as_ref();
        }

        None
    }

    /// Finds all intervals that overlap with the given query interval.
    pub fn find_all_overlapping(&self, query: &Interval<T>) -> Vec<(&Interval<T>, &V)> {
        let mut results = Vec::new();
        if let Some(ref root) = self.root {
            Self::find_all_overlapping_recursive(root, query, &mut results);
        }
        results
    }
"""
content = content.replace(remove1, "")


footer = """// Missing vs. Production:
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

#[cfg(test)]"""

old_footer = """// Missing vs. Production:
// - **Self-Balancing**: This implementation is a standard BST. It can degrade to O(N) if intervals are inserted in sorted order.
//   Production versions use AVL or Red-Black logic to rotate nodes (updating `max` appropriately during rotations).
// - **Deletion**: We omit the `remove` operation for simplicity, as deleting from a BST while updating the augmented `max` values recursively is complex.

#[cfg(test)]"""

content = content.replace(old_footer, footer)

with open("src/data_structures/interval_tree.rs", "w") as f:
    f.write(content)
