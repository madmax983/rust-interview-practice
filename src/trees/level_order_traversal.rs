//! # 102. Binary Tree Level Order Traversal
//!
//! Link: <https://leetcode.com/problems/binary-tree-level-order-traversal/>
//!
//! Given the root of a binary tree, return the level order traversal of its nodes' values.
//! (i.e., from left to right, level by level).
//!
//! This problem is a foundational exercise in tree traversal algorithms.
//! In Rust, it serves as an excellent case study for `VecDeque`, ownership transfer,
//! and iterator design patterns. Implementing a custom iterator for this traversal
//! transforms a standard algorithm into a reusable, idiomatic tool.
//!
//! Note: BFS with a queue is the single canonical solution for level-order traversal
//! (O(n) time, O(w) space), so the brute-force/optimized/optimal progression does not
//! meaningfully apply here. The alternative approaches are noted at the bottom of the file.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::trees::level_order_traversal::{level_order, TreeNode};
//!
//! let mut root = TreeNode::new(3);
//! root.left = Some(Box::new(TreeNode::new(9)));
//! root.right = Some(Box::new(TreeNode::new(20)));
//! if let Some(ref mut r) = root.right {
//!     r.left = Some(Box::new(TreeNode::new(15)));
//!     r.right = Some(Box::new(TreeNode::new(7)));
//! }
//!
//! let result = level_order(Some(Box::new(root)));
//! assert_eq!(result, vec![vec![3], vec![9, 20], vec![15, 7]]);
//! ```
//!
//! ## Constraints
//!
//! - The number of nodes in the tree is in the range `[0, 2000]`.
//! - `-1000 <= Node.val <= 1000`

use std::collections::VecDeque;

// Definition for a binary tree node.
#[derive(Debug, PartialEq, Eq)]
pub struct TreeNode {
    pub val: i32,
    pub left: Option<Box<TreeNode>>,
    pub right: Option<Box<TreeNode>>,
}

impl TreeNode {
    #[inline]
    #[must_use]
    pub const fn new(val: i32) -> Self {
        Self {
            val,
            left: None,
            right: None,
        }
    }
}

/// Helper struct for level order traversal iterator.
///
/// Instead of a monolithic loop inside the function, we encapsulate the
/// traversal state (the queue) in this struct. This allows us to yield
/// results lazily or in batches, adhering to the "Iterator Pattern".
///
/// We hold `Box<TreeNode>` directly in the queue, meaning this iterator
/// *consumes* the tree as it traverses.
pub struct LevelOrderIterator {
    queue: VecDeque<Box<TreeNode>>,
}

impl Iterator for LevelOrderIterator {
    // We yield a Vector of integers for each level
    type Item = Vec<i32>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.queue.is_empty() {
            return None;
        }

        // RUST INSIGHT: We capture the *current* length of the queue.
        // This snapshot is crucial because we will be pushing children
        // to the back of the queue during this loop, but those belong to the *next* level.
        let level_size = self.queue.len();

        // BOLT OPTIMIZATION: Pre-allocate capacity using `Vec::with_capacity`
        // since the level size is known upfront, preventing unnecessary heap reallocations.
        let mut level_values = Vec::with_capacity(level_size);

        for _ in 0..level_size {
            // unwrap is safe here because we checked is_empty() and loop is bounded by initial len
            let node = self.queue.pop_front().unwrap();
            level_values.push(node.val);

            // Push children to queue for next level
            // Note: Ownership of children is moved from `node` to `queue`
            if let Some(left) = node.left {
                self.queue.push_back(left);
            }
            if let Some(right) = node.right {
                self.queue.push_back(right);
            }
        }

        Some(level_values)
    }
}

/// Standard approach: Breadth-First Search (BFS) using a Queue.
///
/// Time: O(n) - We visit every node exactly once.
/// Space: O(w) - Where `w` is the maximum width of the tree (number of nodes in the widest level).
///               In the worst case (perfect binary tree), this is roughly n/2.
///
/// # Idiomatic Rust
/// Rather than writing a `while let` loop directly, we implement `Iterator`
/// for a custom struct. This allows the consumer to use iterator adapters
/// like `take`, `map`, or `collect`.
///
/// # Gotcha
/// A common mistake is using `queue.len()` directly in the loop condition:
/// `for _ in 0..queue.len()`. This is buggy because `queue.len()` changes
/// as we push children! We must capture `level_size` before the loop.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn level_order(root: Option<Box<TreeNode>>) -> Vec<Vec<i32>> {
    // Handle the empty case explicitly or let the iterator handle it.
    // Here, if root is None, we return an empty vec immediately or use the iterator.
    if let Some(node) = root {
        let mut queue = VecDeque::new();
        queue.push_back(node);
        LevelOrderIterator { queue }.collect()
    } else {
        Vec::new()
    }
}

/// # Alternative Approaches
///
/// 1. **Recursive DFS**: Pass the level index as an argument to a recursive function.
///    Push values into `result[level]`. This is often shorter but uses stack space O(h)
///    instead of queue space O(w).
/// 2. **Two Vectors**: Use two vectors `current_level` and `next_level` instead of a queue.
///    Swap them at the end of each level. This avoids `VecDeque` but is functionally equivalent.
///    It can be slightly more cache-friendly for very large levels.

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create a leaf node
    fn leaf(val: i32) -> Option<Box<TreeNode>> {
        Some(Box::new(TreeNode::new(val)))
    }

    #[test]
    fn test_happy_path() {
        // Tree:
        //      3
        //     / \
        //    9  20
        //      /  \
        //     15   7
        let mut root = TreeNode::new(3);
        root.left = leaf(9);
        let mut right = TreeNode::new(20);
        right.left = leaf(15);
        right.right = leaf(7);
        root.right = Some(Box::new(right));

        let result = level_order(Some(Box::new(root)));
        assert_eq!(result, vec![vec![3], vec![9, 20], vec![15, 7]]);
    }

    #[test]
    fn test_empty_tree() {
        let result = level_order(None);
        let expected: Vec<Vec<i32>> = Vec::new();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_single_node() {
        let result = level_order(leaf(1));
        assert_eq!(result, vec![vec![1]]);
    }

    #[test]
    fn test_unbalanced_left() {
        //      1
        //     /
        //    2
        //   /
        //  3
        let mut root = TreeNode::new(1);
        let mut node2 = TreeNode::new(2);
        node2.left = leaf(3);
        root.left = Some(Box::new(node2));

        let result = level_order(Some(Box::new(root)));
        assert_eq!(result, vec![vec![1], vec![2], vec![3]]);
    }

    #[test]
    fn test_iterator_behavior() {
        // Verify we can use iterator methods
        // Tree: 1 -> 2 -> 3 (right children)
        let mut root = TreeNode::new(1);
        let mut node2 = TreeNode::new(2);
        node2.right = leaf(3);
        root.right = Some(Box::new(node2));

        let mut queue = VecDeque::new();
        queue.push_back(Box::new(root));
        let iter = LevelOrderIterator { queue };

        // Take only the first 2 levels
        let levels: Vec<Vec<i32>> = iter.take(2).collect();
        assert_eq!(levels, vec![vec![1], vec![2]]);
    }
}
