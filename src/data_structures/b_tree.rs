//! # B-Tree Implementation
//!
//! Implements a B-Tree (standard Knuth definition) from scratch.
//! This implementation handles node splitting, insertion, search, and deletion.
//!
//! **Replaces Crates:** `btree_map` (std), `im` (persistent B-Trees)
//!
//! **Real-world Usage:**
//! - Databases (`PostgreSQL`, `MySQL`, `SQLite`) for indexing.
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

/// A node in the B-Tree.
#[derive(Debug, Clone)]
struct Node<K, V> {
    // RUST INSIGHT:
    // We use parallel vectors (`keys` and `vals`) instead of a single vector of tuples `Vec<(K, V)>`.
    // This improves cache locality when searching through keys via binary search, as the values
    // aren't polluting the cache line if we only need to compare keys.
    keys: Vec<K>,
    vals: Vec<V>,
    // PRODUCTION NOTE:
    // We use `Vec<Box<Node<K, V>>>` for children. A real production B-Tree (like standard library `BTreeMap`)
    // often uses raw pointers and allocates nodes manually or via an arena to guarantee nodes are stored
    // contiguously or strictly manage lifetimes without the overhead of `Box`.
    children: Vec<Box<Self>>,
}

impl<K: Ord + Clone, V: Clone> Node<K, V> {
    const fn new(is_leaf: bool) -> Self {
        Self {
            keys: Vec::new(),
            vals: Vec::new(),
            children: if is_leaf { Vec::new() } else { Vec::new() },
        }
    }

    const fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    // --- Deletion Helpers ---

    fn delete_key(&mut self, t: usize, key: &K) -> Option<V> {
        let idx = match self.keys.binary_search(key) {
            Ok(i) => i,
            Err(i) => i,
        };

        if idx < self.keys.len() && &self.keys[idx] == key {
            if self.is_leaf() {
                self.keys.remove(idx);
                Some(self.vals.remove(idx))
            } else {
                self.remove_from_non_leaf(t, idx)
            }
        } else {
            // GOTCHA:
            // If the key is not in this node and this is a leaf, the key does not exist.
            // But if it's an internal node, we must descend. Before descending, we MUST ensure
            // the child we are descending into has at least `t` keys, so that if we delete
            // from it, it won't underflow. This proactive filling is what makes deletion O(log N)
            // in a single downward pass without needing to backtrack.
            if self.is_leaf() {
                return None; // Key not found
            }

            let is_last_child = idx == self.keys.len();

            if self.children[idx].keys.len() < t {
                self.fill(t, idx);
            }

            // After fill, if we merged with prev child, the index of interest might have shifted.
            let mut target_idx = idx;
            if is_last_child && idx > self.children.len() - 1 {
                target_idx -= 1;
            } else if idx >= self.children.len() {
                target_idx = self.children.len() - 1;
            }

            self.children[target_idx].delete_key(t, key)
        }
    }

    fn remove_from_non_leaf(&mut self, t: usize, idx: usize) -> Option<V> {
        // Key k is present in this node at index idx.

        // 1. If child that precedes k (children[idx]) has at least t keys...
        if self.children[idx].keys.len() >= t {
            let (pred_key, pred_val) = self.get_pred(idx);
            // Replace k with pred
            self.keys[idx] = pred_key.clone();
            let old_val = std::mem::replace(&mut self.vals[idx], pred_val);

            // Recursively delete pred from the child
            self.children[idx].delete_key(t, &pred_key);
            return Some(old_val);
        }

        // 2. If child that succeeds k (children[idx+1]) has at least t keys...
        if self.children[idx + 1].keys.len() >= t {
            let (succ_key, succ_val) = self.get_succ(idx);
            self.keys[idx] = succ_key.clone();
            let old_val = std::mem::replace(&mut self.vals[idx], succ_val);

            self.children[idx + 1].delete_key(t, &succ_key);
            return Some(old_val);
        }

        // 3. Both have t-1 keys. Merge k and children[idx+1] into children[idx].
        let old_val = self.vals[idx].clone();
        let key = self.keys[idx].clone(); // Need key for recursive delete call
        self.merge(t, idx);
        self.children[idx].delete_key(t, &key);
        Some(old_val)
    }

    fn get_pred(&self, idx: usize) -> (K, V) {
        let mut cur = &self.children[idx];
        while !cur.is_leaf() {
            cur = cur.children.last().unwrap();
        }
        (
            cur.keys.last().unwrap().clone(),
            cur.vals.last().unwrap().clone(),
        )
    }

    fn get_succ(&self, idx: usize) -> (K, V) {
        let mut cur = &self.children[idx + 1];
        while !cur.is_leaf() {
            cur = cur.children.first().unwrap();
        }
        (
            cur.keys.first().unwrap().clone(),
            cur.vals.first().unwrap().clone(),
        )
    }

    fn fill(&mut self, t: usize, idx: usize) {
        if idx > 0 && self.children[idx - 1].keys.len() >= t {
            self.borrow_from_prev(idx);
        } else if idx < self.children.len() - 1 && self.children[idx + 1].keys.len() >= t {
            self.borrow_from_next(idx);
        } else if idx < self.children.len() - 1 {
            self.merge(t, idx);
        } else {
            self.merge(t, idx - 1);
        }
    }

    fn borrow_from_prev(&mut self, idx: usize) {
        let (head, tail) = self.children.split_at_mut(idx);
        let sibling = &mut head[idx - 1];
        let child = &mut tail[0];

        // 1. Move last key from sibling up to parent
        let sibling_key = sibling.keys.pop().unwrap();
        let sibling_val = sibling.vals.pop().unwrap();

        // 2. Insert parent's key/val (at idx-1) into child (at 0)
        let parent_key = std::mem::replace(&mut self.keys[idx - 1], sibling_key);
        let parent_val = std::mem::replace(&mut self.vals[idx - 1], sibling_val);

        child.keys.insert(0, parent_key);
        child.vals.insert(0, parent_val);

        // 3. If not leaf, move sibling's last child to child's first child
        if !child.is_leaf() {
            let sibling_child = sibling.children.pop().unwrap();
            child.children.insert(0, sibling_child);
        }
    }

    fn borrow_from_next(&mut self, idx: usize) {
        let (head, tail) = self.children.split_at_mut(idx + 1);
        let child = &mut head[idx];
        let sibling = &mut tail[0];

        // 1. Move first key from sibling up to parent
        let sibling_key = sibling.keys.remove(0);
        let sibling_val = sibling.vals.remove(0);

        // 2. Insert parent's key/val (at idx) into child (at end)
        let parent_key = std::mem::replace(&mut self.keys[idx], sibling_key);
        let parent_val = std::mem::replace(&mut self.vals[idx], sibling_val);

        child.keys.push(parent_key);
        child.vals.push(parent_val);

        // 3. If not leaf, move sibling's first child to child's last child
        if !child.is_leaf() {
            let sibling_child = sibling.children.remove(0);
            child.children.push(sibling_child);
        }
    }

    fn merge(&mut self, _t: usize, idx: usize) {
        // Merge children[idx] and children[idx+1]
        // The key at self.keys[idx] is moved down into the merged node.

        // We need to take children out to avoid multiple mutable borrows
        let child = self.children.remove(idx); // Removes child at idx
        let sibling = self.children.remove(idx); // Removes child at idx+1 (now at idx)

        // Let's construct the new merged child.
        // We'll reuse `child` node (left one).
        let mut new_child = child; // Box<Node>
        let mut right_child = sibling; // Box<Node>

        // Move key from parent to new_child
        let parent_key = self.keys.remove(idx);
        let parent_val = self.vals.remove(idx);

        new_child.keys.push(parent_key);
        new_child.vals.push(parent_val);

        // Move keys/vals from right_child
        // RUST INSIGHT: We use `append` instead of `extend` to transfer ownership of elements efficiently
        // from one vector to another without reallocating or cloning individually.
        new_child.keys.append(&mut right_child.keys);
        new_child.vals.append(&mut right_child.vals);

        // Move children from right_child
        if !new_child.is_leaf() {
            new_child.children.append(&mut right_child.children);
        }

        // Insert new_child back to children
        self.children.insert(idx, new_child);
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
    #[must_use] 
    pub fn new(t: usize) -> Self {
        assert!(t >= 2, "Degree must be at least 2");
        Self {
            root: Box::new(Node::new(true)),
            t,
            len: 0,
        }
    }

    /// Returns the number of elements in the B-Tree.
    #[must_use] 
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns true if the B-Tree is empty.
    #[must_use] 
    pub const fn is_empty(&self) -> bool {
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
        let t = self.t;
        if Self::is_full(t, &self.root) {
            let new_root = Box::new(Node::new(false));
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

    /// Deletes a key from the B-Tree.
    /// Returns the value if the key existed.
    pub fn delete(&mut self, key: K) -> Option<V> {
        let result = self.root.delete_key(self.t, &key);

        // If root has 0 keys (and is not a leaf), make its first child the new root.
        if self.root.keys.is_empty() && !self.root.is_leaf() {
            let child = self.root.children.remove(0);
            self.root = child;
        }
        // If leaf and empty, the tree is now empty but we keep the empty root node (as per `new`).

        if result.is_some() {
            self.len -= 1;
        }
        result
    }

    const fn is_full(t: usize, node: &Node<K, V>) -> bool {
        node.keys.len() == 2 * t - 1
    }

    fn split_child(t: usize, parent: &mut Node<K, V>, i: usize) {
        let mut child = parent.children.remove(i);
        let mut new_child = Box::new(Node::new(child.is_leaf()));

        let split_idx = t - 1;
        let median_key = child.keys.remove(split_idx);
        let median_val = child.vals.remove(split_idx);

        new_child.keys.extend(child.keys.drain((t - 1)..));
        new_child.vals.extend(child.vals.drain((t - 1)..));

        if !child.is_leaf() {
            new_child.children.extend(child.children.drain(t..));
        }

        parent.keys.insert(i, median_key);
        parent.vals.insert(i, median_val);

        parent.children.insert(i, child);
        parent.children.insert(i + 1, new_child);
    }

    fn insert_non_full(
        t: usize,
        len: &mut usize,
        node: &mut Node<K, V>,
        key: K,
        val: V,
    ) -> Option<V> {
        let mut i = node.keys.len();

        if node.is_leaf() {
            if let Ok(idx) = node.keys.binary_search(&key) {
                let old = mem::replace(&mut node.vals[idx], val);
                return Some(old);
            }
            while i > 0 && key < node.keys[i - 1] {
                i -= 1;
            }
            node.keys.insert(i, key);
            node.vals.insert(i, val);
            *len += 1;
            None
        } else {
            while i > 0 && key < node.keys[i - 1] {
                i -= 1;
            }

            if i > 0 && node.keys[i - 1] == key {
                let old = mem::replace(&mut node.vals[i - 1], val);
                return Some(old);
            }
            match node.keys.binary_search(&key) {
                Ok(idx) => {
                    let old = mem::replace(&mut node.vals[idx], val);
                    return Some(old);
                }
                Err(idx) => {
                    i = idx;
                }
            }

            if Self::is_full(t, &node.children[i]) {
                Self::split_child(t, node, i);
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
        let mut btree = BTree::new(2);
        for i in 1..=4 {
            btree.insert(i, i * 10);
        }
        assert_eq!(btree.len(), 4);
        for i in 1..=4 {
            assert_eq!(btree.get(&i), Some(&(i * 10)));
        }
        assert_eq!(btree.root.keys.len(), 1);
        assert_eq!(btree.root.keys[0], 2);
        assert_eq!(btree.root.children.len(), 2);
    }

    #[test]
    fn test_btree_large_insertion() {
        let mut btree = BTree::new(3);
        let n = 100;
        for i in 0..n {
            btree.insert(i, i);
        }
        assert_eq!(btree.len(), n);
        for i in 0..n {
            assert_eq!(btree.get(&i), Some(&i));
        }
    }

    #[test]
    fn test_btree_delete_leaf() {
        let mut btree = BTree::new(2);
        btree.insert(1, 10);
        btree.insert(2, 20);

        assert_eq!(btree.delete(1), Some(10));
        assert_eq!(btree.get(&1), None);
        assert_eq!(btree.len(), 1);
        assert_eq!(btree.get(&2), Some(&20));
    }

    #[test]
    fn test_btree_delete_root_shrink() {
        let mut btree = BTree::new(2);
        // Insert 1, 2, 3 -> root has [2], children [1] and [3]
        btree.insert(1, 10);
        btree.insert(2, 20);
        btree.insert(3, 30);

        // Delete 2 (root key)
        assert_eq!(btree.delete(2), Some(20));
        assert_eq!(btree.len(), 2);
        assert_eq!(btree.get(&1), Some(&10));
        assert_eq!(btree.get(&3), Some(&30));
        assert_eq!(btree.root.keys.len(), 2); // [1, 3]
    }

    #[test]
    fn test_btree_delete_internal_node_pred() {
        let mut btree = BTree::new(2);
        // [10, 20, 30, 40, 50]
        btree.insert(1, 10);
        btree.insert(2, 20);
        btree.insert(3, 30);
        btree.insert(4, 40);
        btree.insert(5, 50);

        // Tree structure for t=2:
        // Root: [3]
        // Left: [1, 2]
        // Right: [4, 5]

        // Delete 3 (root).
        // Left child [1, 2] has 2 keys (>=t).
        // Should find pred (2), replace 3 with 2.
        // Recurse delete 2 from left.

        assert_eq!(btree.delete(3), Some(30));
        assert_eq!(btree.get(&3), None);
        assert_eq!(btree.get(&2), Some(&20));
        assert_eq!(btree.root.keys[0], 2);
    }

    #[test]
    fn test_btree_delete_not_found() {
        let mut btree = BTree::new(2);
        btree.insert(1, 10);
        assert_eq!(btree.delete(2), None);
        assert_eq!(btree.len(), 1);
    }

    #[test]
    fn test_btree_delete_borrow_from_sibling() {
        // t=3. Min keys=2. Max keys=5.
        let mut btree = BTree::new(3);

        // Insert to fill Left child and min Right child
        // Root: [100]
        // Left: [10, 20, 30] (3 keys)
        // Right: [200, 300] (2 keys - min)

        btree.insert(100, 100);
        btree.insert(10, 10);
        btree.insert(20, 20);
        btree.insert(30, 30);
        btree.insert(200, 200);
        btree.insert(300, 300);

        // Delete 200 from Right. Right has 2 keys.
        // It will underflow (1 key < 2).
        // Left sibling has 3 keys (>=3).
        // Should borrow from Left.
        // Left gives 30 to Root. Root gives 100 to Right.
        // New Left: [10, 20].
        // New Root: [30].
        // New Right: [100, 300] (after deleting 200? No wait).

        // Steps:
        // 1. Delete 200. 200 is in leaf.
        // 2. Before descending to Right, check if it has t keys.
        //    Right has 2 keys (t-1=2? No t=3, t-1=2).
        //    Wait, `t=3`. `keys.len() < t`. 2 < 3. Yes.
        // 3. Fill Right.
        //    Check Left sibling. Has 3 keys. 3 >= 3. Yes.
        //    Borrow from Left.
        //    - Move 30 (last of Left) to Root.
        //    - Move 100 (from Root) to Right (at 0).
        //    - Right becomes [100, 200, 300].
        //    - Left becomes [10, 20].
        //    - Root becomes [30].
        // 4. Descend to Right [100, 200, 300].
        // 5. Delete 200. -> [100, 300].

        assert_eq!(btree.delete(200), Some(200));
        assert_eq!(btree.root.keys, vec![30]);
        // Verify children via public API implicitly
        assert_eq!(btree.get(&10), Some(&10));
        assert_eq!(btree.get(&20), Some(&20));
        assert_eq!(btree.get(&30), Some(&30)); // Root
        assert_eq!(btree.get(&100), Some(&100));
        assert_eq!(btree.get(&300), Some(&300));
    }

    #[test]
    fn test_btree_delete_merge_siblings() {
        // t=3. Min 2.
        let mut btree = BTree::new(3);

        // Root: [100]
        // Left: [10, 20] (Min)
        // Right: [200, 300] (Min)

        btree.insert(100, 100);
        btree.insert(10, 10);
        btree.insert(20, 20);
        btree.insert(200, 200);
        btree.insert(300, 300);

        // Delete 200. Right needs fill.
        // Left has 2 keys. Cannot borrow.
        // Merge Left, Root key, Right.
        // New Node: [10, 20, 100, 200, 300].
        // Root becomes empty -> Height reduced.
        // Then delete 200 -> [10, 20, 100, 300].

        assert_eq!(btree.delete(200), Some(200));
        assert_eq!(btree.len(), 4);
        assert_eq!(btree.root.keys, vec![10, 20, 100, 300]);
    }

    #[test]
    fn test_btree_stress() {
        let mut btree = BTree::new(3);
        let n = 1000;

        // Insert
        for i in 0..n {
            btree.insert(i, i);
        }
        assert_eq!(btree.len(), n);

        // Delete even numbers
        for i in (0..n).step_by(2) {
            assert_eq!(btree.delete(i), Some(i));
        }
        assert_eq!(btree.len(), n / 2);

        // Verify odd numbers remain
        for i in (1..n).step_by(2) {
            assert_eq!(btree.get(&i), Some(&i));
        }
        // Verify even numbers gone
        for i in (0..n).step_by(2) {
            assert_eq!(btree.get(&i), None);
        }
    }
}
