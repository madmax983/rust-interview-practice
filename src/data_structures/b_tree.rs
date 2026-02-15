//! # B-Tree Implementation
//!
//! Implements a B-Tree (standard Knuth definition) from scratch.
//! This implementation handles node splitting, insertion, and search.
//!
//! **Replaces Crates:** `btree_map` (std), `im` (persistent B-Trees)
//!
//! **Real-world Usage:**
//! - Databases (PostgreSQL, MySQL, SQLite) for indexing.
//! - File systems (NTFS, HFS+, Btrfs, XFS).
//! - In-memory ordered maps where cache locality is critical (Rust's `BTreeMap`).
//!
//! **Why build it yourself?**
//! Implementing a B-Tree teaches you about balancing data structures that are optimized for block-based storage (or cache lines).
//! You'll learn how to manage the "split" operation that propagates up the tree, and how to maintain invariants like minimum occupancy.
//! It's a step up from binary trees because you deal with arrays of keys and children, making the logic for indices critical.

use std::fmt::Debug;
use std::mem;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure:
//
//      Node
//      ┌───────────────────────┐
//      │ Keys: [10, 20]        │
//      │ Vals: ["A", "B"]      │
//      │ Children: [*, *, *]   │
//      └───────────┬──┬──┬─────┘
//                  │  │  │
//        ┌─────────┘  │  └─────────┐
//        ▼            ▼            ▼
//      Child 0      Child 1      Child 2
//      [< 10]       [10-20]      [> 20]
//
// Invariants (Degree t):
// 1. Every node has at most 2t - 1 keys.
// 2. Every non-root node has at least t - 1 keys.
// 3. Every node has at most 2t children.
// 4. All leaves appear at the same depth.
// 5. Keys in a node are sorted in increasing order.
//
// Complexity:
// ┌───────────┬──────────────┬──────────────┐
// │ Operation │ Time         │ Space        │
// ├───────────┼──────────────┼──────────────┤
// │ Search    │ O(log_t N)   │ O(1)         │
// │ Insert    │ O(t log_t N) │ O(1)         │
// │ Delete    │ O(t log_t N) │ O(1)         │
// └───────────┴──────────────┴──────────────┘
// Note: The factor `t` comes from shifting elements in the arrays (Vec::insert/remove).
//
// Design Decisions:
// - **Degree**: Configurable `t`. Standard `BTreeMap` usually picks a large B (e.g., 6) to fit a cache line.
// - **Storage**: `Vec` for keys, values, and children.
//   - *Tradeoff*: `Vec` operations are O(t), but since t is small (constant), it's fast.
//   - *Alternative*: Fixed-size arrays. Harder in Rust without const generics (until recently).
// - **Node Type**: We use a single `Node` struct.
//   - *Alternative*: Separate `InternalNode` and `LeafNode` types to save space (no children pointer in leaves).
//     This complicates the type system (enum wrapping). For simplicity, we use one struct and check `children.is_empty()`.

/// A node in the B-Tree.
#[derive(Debug, Clone)]
struct Node<K, V> {
    keys: Vec<K>,
    vals: Vec<V>,
    children: Vec<Box<Node<K, V>>>,
}

impl<K, V> Node<K, V> {
    fn new(is_leaf: bool) -> Self {
        Self {
            keys: Vec::new(),
            vals: Vec::new(),
            children: if is_leaf { Vec::new() } else { Vec::new() },
        }
    }

    fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }
}

/// A B-Tree implementation.
pub struct BTree<K, V> {
    root: Box<Node<K, V>>,
    t: usize, // Minimum degree
    len: usize,
}

impl<K: Ord + Clone + Debug, V: Clone + Debug> BTree<K, V> {
    /// Creates a new B-Tree with minimum degree `t`.
    /// `t` must be >= 2.
    pub fn new(t: usize) -> Self {
        assert!(t >= 2, "Degree must be at least 2");
        Self {
            root: Box::new(Node::new(true)),
            t,
            len: 0,
        }
    }

    /// Returns the number of elements in the B-Tree.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns true if the B-Tree is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Searches for a key in the B-Tree.
    pub fn get(&self, key: &K) -> Option<&V> {
        self.search_node(&self.root, key)
    }

    fn search_node<'a>(&'a self, node: &'a Node<K, V>, key: &K) -> Option<&'a V> {
        let mut i = 0;
        // Find the first key greater than or equal to k
        while i < node.keys.len() && key > &node.keys[i] {
            i += 1;
        }

        // If the found key is equal to k, return this value
        if i < node.keys.len() && key == &node.keys[i] {
            return Some(&node.vals[i]);
        }

        // If leaf, key is not present
        if node.is_leaf() {
            return None;
        }

        // Recurse to the appropriate child
        self.search_node(&node.children[i], key)
    }

    /// Inserts a key-value pair into the B-Tree.
    /// If the key already exists, updates the value and returns the old value.
    pub fn insert(&mut self, key: K, val: V) -> Option<V> {
        // RUST INSIGHT: We handle the root splitting as a special case.
        // If the root is full, we create a new empty root and split the old root into it.
        let t = self.t;
        if Self::is_full(t, &self.root) {
            let mut new_root = Box::new(Node::new(false));
            // Move old root to be a child of new root
            let old_root = mem::replace(&mut self.root, new_root);
            self.root.children.push(old_root);

            // Split the old root (now child 0)
            Self::split_child(t, &mut self.root, 0);

            // Now insert into the non-full root
            Self::insert_non_full(t, &mut self.len, &mut self.root, key, val)
        } else {
            Self::insert_non_full(t, &mut self.len, &mut self.root, key, val)
        }
    }

    fn is_full(t: usize, node: &Node<K, V>) -> bool {
        node.keys.len() == 2 * t - 1
    }

    /// Splits the child `i` of `parent`.
    /// `parent` must be non-full.
    /// `parent.children[i]` must be full.
    fn split_child(t: usize, parent: &mut Node<K, V>, i: usize) {
        // Need to temporarily take the child out to avoid borrowing issues
        // We can do this because we know children[i] exists.
        // GOTCHA: Splitting involves moving half the keys/children to a new node.
        // Off-by-one errors are very common here.

        let mut child = parent.children.remove(i);
        let mut new_child = Box::new(Node::new(child.is_leaf()));

        // `new_child` gets the last t-1 keys of `child`
        // `child` keeps the first t-1 keys
        // The middle key (index t-1) moves up to `parent`

        // Move keys
        // child keys: [0..t-1] (keep), [t-1] (up), [t..2t-1] (move)
        let split_idx = t - 1;
        let median_key = child.keys.remove(split_idx);
        let median_val = child.vals.remove(split_idx);

        // Move remaining t-1 keys to new_child
        new_child.keys.extend(child.keys.drain((t - 1)..));
        new_child.vals.extend(child.vals.drain((t - 1)..));

        // Move children if not leaf
        if !child.is_leaf() {
            // child children: [0..t] (keep), [t..2t] (move)
            new_child.children.extend(child.children.drain(t..));
        }

        // Insert median key/val into parent
        parent.keys.insert(i, median_key);
        parent.vals.insert(i, median_val);

        // Insert children back
        parent.children.insert(i, child); // Put the modified child back
        parent.children.insert(i + 1, new_child); // Insert after the split child
    }

    /// Insert into a non-full node.
    fn insert_non_full(t: usize, len: &mut usize, node: &mut Node<K, V>, key: K, val: V) -> Option<V> {
        let mut i = node.keys.len();

        if node.is_leaf() {
            // Check for duplicate in the leaf
            if let Ok(idx) = node.keys.binary_search(&key) {
                 let old = mem::replace(&mut node.vals[idx], val);
                 return Some(old);
            }

            // Insert sorted
            while i > 0 && key < node.keys[i - 1] {
                i -= 1;
            }
            node.keys.insert(i, key);
            node.vals.insert(i, val);
            *len += 1;
            None
        } else {
            // Internal node
            // Find child to recurse into
            while i > 0 && key < node.keys[i - 1] {
                i -= 1;
            }

            // Check if key exists in this internal node
            if i > 0 && node.keys[i - 1] == key {
                let old = mem::replace(&mut node.vals[i - 1], val);
                return Some(old);
            }

            // Let's use binary search for clarity and correctness.
            match node.keys.binary_search(&key) {
                Ok(idx) => {
                    let old = mem::replace(&mut node.vals[idx], val);
                    return Some(old);
                }
                Err(idx) => {
                    i = idx; // This is the child index to go down to
                }
            }

            // Detect if child is full
            if Self::is_full(t, &node.children[i]) {
                Self::split_child(t, node, i);
                // After split, the middle key moves up to `node` at `i`.
                // The child `i` is split into `i` and `i+1`.
                // We need to decide which one to go into.
                if key > node.keys[i] {
                    i += 1;
                } else if key == node.keys[i] {
                     let old = mem::replace(&mut node.vals[i], val);
                     return Some(old);
                }
            }

            Self::insert_non_full(t, len, &mut node.children[i], key, val)
        }
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `std::collections::BTreeMap`: The standard library implementation is highly optimized,
//   using unsafe code for raw pointer manipulation to avoid the overhead of `Box` and `Vec` in nodes.
//   It also separates leaf and internal nodes more strictly.
// - `im`: Provides immutable/persistent B-Trees, where modifying returns a new tree sharing structure.
//
// Missing vs. Production:
// - **Deletion**: We only implemented insert/search. Deletion in B-Trees is complex (merging, borrowing).
// - **Memory Layout**: We use `Box<Node>`, scattering nodes in heap. Production B-Trees often use a custom allocator
//   or arena to keep nodes contiguous for cache performance.
// - **Concurrency**: This is single-threaded. `RwLock<BTree>` works but scales poorly.
//   Concurrent B-Trees (B-Link Trees) allow simultaneous readers/writers.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_btree_basic_insert_get() {
        let mut btree = BTree::new(2); // t=2, max keys = 3

        assert!(btree.insert(1, "one").is_none());
        assert!(btree.insert(2, "two").is_none());
        assert!(btree.insert(3, "three").is_none());

        assert_eq!(btree.get(&1), Some(&"one"));
        assert_eq!(btree.get(&2), Some(&"two"));
        assert_eq!(btree.get(&3), Some(&"three"));
        assert_eq!(btree.get(&4), None);
        assert_eq!(btree.len(), 3);
    }

    #[test]
    fn test_btree_update() {
        let mut btree = BTree::new(3);
        btree.insert(1, 10);
        assert_eq!(btree.insert(1, 20), Some(10));
        assert_eq!(btree.get(&1), Some(&20));
        assert_eq!(btree.len(), 1); // Length shouldn't increase
    }

    #[test]
    fn test_btree_splitting_root() {
        // t=2. Max keys = 3.
        // Insert 1, 2, 3. Root is full [1, 2, 3].
        // Insert 4. Root splits.
        // New root: [2].
        // Left child: [1].
        // Right child: [3, 4].

        let mut btree = BTree::new(2);
        for i in 1..=4 {
            btree.insert(i, i * 10);
        }

        assert_eq!(btree.len(), 4);
        for i in 1..=4 {
            assert_eq!(btree.get(&i), Some(&(i * 10)));
        }

        // Check internal structure (implementation details)
        // Root should have 1 key (2).
        assert_eq!(btree.root.keys.len(), 1);
        assert_eq!(btree.root.keys[0], 2);
        assert_eq!(btree.root.children.len(), 2);
    }

    #[test]
    fn test_btree_large_insertion() {
        let mut btree = BTree::new(3); // t=3, min=2, max=5 keys.
        let n = 100;

        for i in 0..n {
            btree.insert(i, i);
        }

        assert_eq!(btree.len(), n);
        for i in 0..n {
            assert_eq!(btree.get(&i), Some(&i));
        }
    }
}
