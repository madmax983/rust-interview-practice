//! # Red-Black Tree Implementation
//!
//! # Header
//!
//! *   **Problem Name**: Red-Black Tree
//! *   **Difficulty**: Hard (Self-Balancing BST)
//! *   **Link**: <https://en.wikipedia.org/wiki/Red%E2%80%93black_tree>
//! *   **Why this matters in Rust**: This is the data structure powering `std::collections::BTreeMap` (historically; Rust now uses B-Trees, but C++ `std::map` and Java `TreeMap` use RB-Trees). It provides guaranteed O(log N) operations.
//!
//! # Architecture
//!
//! A Red-Black Tree is a binary search tree with one extra bit of storage per node: its **color** (Red or Black).
//! By constraining the node colors on any simple path from the root to a leaf, red-black trees ensure that no such path is more than twice as long as any other, so that the tree is approximately balanced.
//!
//! **Invariants:**
//! 1.  Every node is either red or black.
//! 2.  The root is black.
//! 3.  Every leaf (NIL) is black.
//! 4.  If a node is red, then both its children are black. (No two consecutive red nodes).
//! 5.  For each node, all simple paths from the node to descendant leaves contain the same number of black nodes.
//!
//! **Complexity:**
//!
//! | Operation | Time | Space |
//! | :--- | :--- | :--- |
//! | Search | O(log N) | O(1) |
//! | Insert | O(log N) | O(1) |
//! | Delete | O(log N) | O(1) |
//!
//! **Implementation Note:**
//! To avoid `Option<Box<Node>>` complexity in parent pointers (needed for rotations), we use a recursive approach or an iterative approach with a stack.
//! A fully iterative implementation with parent pointers requires `Rc<RefCell<Node>>` or `unsafe` pointers.
//! For educational clarity, we implement **Left-Leaning Red-Black Trees (LLRB)** (Sedgewick variant), which simplifies the invariants by enforcing that red links are always left-leaning.
//! This maps 1-1 to 2-3 Trees.

use std::cmp::Ordering;
use std::fmt::Debug;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Node<K, V> {
    key: K,
    val: V,
    color: Color,
    left: Option<Box<Node<K, V>>>,
    right: Option<Box<Node<K, V>>>,
}

impl<K, V> Node<K, V> {
    fn new(key: K, val: V, color: Color) -> Self {
        Self {
            key,
            val,
            color,
            left: None,
            right: None,
        }
    }
}

pub struct RedBlackTree<K, V> {
    root: Option<Box<Node<K, V>>>,
    len: usize,
}

impl<K: Ord, V> Default for RedBlackTree<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Ord, V> RedBlackTree<K, V> {
    pub fn new() -> Self {
        Self { root: None, len: 0 }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        let mut current = self.root.as_ref();
        while let Some(node) = current {
            match key.cmp(&node.key) {
                Ordering::Less => current = node.left.as_ref(),
                Ordering::Greater => current = node.right.as_ref(),
                Ordering::Equal => return Some(&node.val),
            }
        }
        None
    }

    pub fn insert(&mut self, key: K, val: V) {
        // LLRB Insertion
        // The root is always black.
        self.root = Some(Self::insert_rec(self.root.take(), key, val, &mut self.len));
        if let Some(ref mut node) = self.root {
            node.color = Color::Black;
        }
    }

    // Helper: is_red
    fn is_red(node: Option<&Box<Node<K, V>>>) -> bool {
        match node {
            Some(n) => n.color == Color::Red,
            None => false,
        }
    }

    // Rotations
    //      x              y
    //     / \            / \
    //    A   y    =>    x   C
    //       / \        / \
    //      B   C      A   B
    fn rotate_left(mut h: Box<Node<K, V>>) -> Box<Node<K, V>> {
        let mut x = h.right.take().expect("Right child must exist for rotate_left");
        h.right = x.left.take();
        x.color = h.color;
        h.color = Color::Red;
        x.left = Some(h);
        x
    }

    //      x              y
    //     / \            / \
    //    y   C    =>    A   x
    //   / \                / \
    //  A   B              B   C
    fn rotate_right(mut h: Box<Node<K, V>>) -> Box<Node<K, V>> {
        let mut x = h.left.take().expect("Left child must exist for rotate_right");
        h.left = x.right.take();
        x.color = h.color;
        h.color = Color::Red;
        x.right = Some(h);
        x
    }

    // Flip Colors
    fn flip_colors(h: &mut Box<Node<K, V>>) {
        h.color = Color::Red;
        if let Some(ref mut l) = h.left {
            l.color = Color::Black;
        }
        if let Some(ref mut r) = h.right {
            r.color = Color::Black;
        }
    }

    fn insert_rec(
        mut h: Option<Box<Node<K, V>>>,
        key: K,
        val: V,
        len: &mut usize,
    ) -> Box<Node<K, V>> {
        if h.is_none() {
            *len += 1;
            // New nodes are always RED at bottom (standard RB invariant)
            return Box::new(Node::new(key, val, Color::Red));
        }

        let mut node = h.take().unwrap();

        match key.cmp(&node.key) {
            Ordering::Less => {
                node.left = Some(Self::insert_rec(node.left.take(), key, val, len));
            }
            Ordering::Greater => {
                node.right = Some(Self::insert_rec(node.right.take(), key, val, len));
            }
            Ordering::Equal => {
                node.val = val; // Update value
            }
        }

        // LLRB Fixes
        // 1. Right child red, left black -> Rotate Left
        if Self::is_red(node.right.as_ref()) && !Self::is_red(node.left.as_ref()) {
            node = Self::rotate_left(node);
        }

        // 2. Left child red, left-left grandchild red -> Rotate Right
        // To check left-left, we must check node.left then node.left.left.
        // We cannot borrow `node.left` then mutate `node`.
        let mut needs_rotate_right = false;
        if let Some(ref l) = node.left {
            if l.color == Color::Red && Self::is_red(l.left.as_ref()) {
                needs_rotate_right = true;
            }
        }
        if needs_rotate_right {
            node = Self::rotate_right(node);
        }

        // 3. Both children red -> Flip Colors
        if Self::is_red(node.left.as_ref()) && Self::is_red(node.right.as_ref()) {
            Self::flip_colors(&mut node);
        }

        node
    }

    /// Checks invariants for testing
    #[cfg(test)]
    fn check_invariants(&self) -> bool {
        if let Some(ref root) = self.root {
            if root.color != Color::Black {
                return false;
            }
        }
        // Check standard RB properties
        // 1. No red node has a red child
        // 2. Black height is consistent
        let (consistent_bh, _) = self.check_black_height(self.root.as_ref());
        let no_consecutive_red = self.check_no_red_red(self.root.as_ref());

        consistent_bh && no_consecutive_red
    }

    #[cfg(test)]
    fn check_black_height(&self, node: Option<&Box<Node<K, V>>>) -> (bool, usize) {
        match node {
            None => (true, 1), // Null is black
            Some(n) => {
                let (lok, lh) = self.check_black_height(n.left.as_ref());
                let (rok, rh) = self.check_black_height(n.right.as_ref());
                let bh = lh + if n.color == Color::Black { 1 } else { 0 };
                (lok && rok && lh == rh, bh)
            }
        }
    }

    #[cfg(test)]
    fn check_no_red_red(&self, node: Option<&Box<Node<K, V>>>) -> bool {
        match node {
            None => true,
            Some(n) => {
                if n.color == Color::Red {
                    if Self::is_red(n.left.as_ref()) || Self::is_red(n.right.as_ref()) {
                        return false;
                    }
                }
                self.check_no_red_red(n.left.as_ref()) && self.check_no_red_red(n.right.as_ref())
            }
        }
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `std::collections::BTreeMap`: Uses a B-Tree, which is cache-friendlier than RB-Tree.
//   RB-Trees are better when node stability (pointer validity) is required in unsafe languages,
//   but in Rust `BTreeMap` is preferred for almost all use cases.
// - `rbtree`: Crate implementing standard RB-Tree.
//
// Missing vs. Production:
// - **Deletions**: Implementing deletion in LLRB is complex (requires pushing red down). Not implemented here for brevity.
// - **Iterators**: `iter()`, `iter_mut()`, `into_iter()` are missing.
// - **Parent Pointers**: Standard RB-Trees often use parent pointers for O(1) iterators.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_insert() {
        let mut tree = RedBlackTree::new();
        tree.insert(10, "ten");
        tree.insert(20, "twenty");
        tree.insert(5, "five");

        assert_eq!(tree.get(&10), Some(&"ten"));
        assert_eq!(tree.get(&20), Some(&"twenty"));
        assert_eq!(tree.get(&5), Some(&"five"));
        assert_eq!(tree.len(), 3);
    }

    #[test]
    fn test_update() {
        let mut tree = RedBlackTree::new();
        tree.insert(1, "one");
        tree.insert(1, "ONE");
        assert_eq!(tree.get(&1), Some(&"ONE"));
        assert_eq!(tree.len(), 1);
    }

    #[test]
    fn test_balance_invariants() {
        let mut tree = RedBlackTree::new();
        // Insert in sorted order - worst case for BST, but RB Tree should balance
        for i in 0..100 {
            tree.insert(i, i);
        }

        assert_eq!(tree.len(), 100);
        assert!(tree.check_invariants());

        // Check search works
        for i in 0..100 {
            assert_eq!(tree.get(&i), Some(&i));
        }
    }

    #[test]
    fn test_random_inserts() {
        let mut tree = RedBlackTree::new();
        let data = vec![50, 20, 80, 10, 30, 60, 90, 5, 15, 25, 35, 55, 65, 85, 95];
        for &x in &data {
            tree.insert(x, x);
        }

        assert!(tree.check_invariants());
        for &x in &data {
            assert_eq!(tree.get(&x), Some(&x));
        }
    }
}
