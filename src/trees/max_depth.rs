//! # 104. Maximum Depth of Binary Tree
//!
//! Link: <https://leetcode.com/problems/maximum-depth-of-binary-tree/>
//!
//! Given the root of a binary tree, return its maximum depth.
//!
//! A binary tree's maximum depth is the number of nodes along the longest path from the
//! root node down to the farthest leaf node.
//!
//! This problem is fundamental for understanding tree traversal (DFS vs BFS) and recursion.
//! In Rust, it highlights the difference between stack-allocated recursion and heap-allocated
//! iterative solutions, and how `Option` combinators can express tree logic succinctly.
//!
//! ## Examples
//!
//! ```
//! use leetcode::trees::max_depth::{max_depth, TreeNode};
//!
//! let mut root = TreeNode::new(3);
//! root.left = Some(Box::new(TreeNode::new(9)));
//! let mut right = TreeNode::new(20);
//! right.left = Some(Box::new(TreeNode::new(15)));
//! right.right = Some(Box::new(TreeNode::new(7)));
//! root.right = Some(Box::new(right));
//!
//! assert_eq!(max_depth(Some(Box::new(root))), 3);
//! ```
//!
//! ## Constraints
//!
//! - The number of nodes in the tree is in the range `[0, 10^4]`.
//! - `-100 <= Node.val <= 100`

use std::cmp;
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

/// Recursive Approach: Depth-First Search (DFS).
///
/// This is the most intuitive solution. The depth of a node is 1 plus the maximum depth
/// of its subtrees.
///
/// Time: O(n) - We visit every node exactly once.
/// Space: O(h) - The recursion stack depth corresponds to the height of the tree.
///               Worst case O(n) for a skewed tree, Best case O(log n) for balanced.
///
/// # Rust Insight
/// Rust's pattern matching makes handling the `None` case (base case) explicit and safe.
/// Unlike C/C++ where null pointer checks are easy to forget, `Option` forces you to handle it.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn max_depth_recursive(root: Option<Box<TreeNode>>) -> i32 {
    match root {
        Some(node) => {
            let left_depth = max_depth_recursive(node.left);
            let right_depth = max_depth_recursive(node.right);
            1 + cmp::max(left_depth, right_depth)
        }
        None => 0,
    }
}

/// Iterative Approach: Breadth-First Search (BFS).
///
/// We use a queue to traverse the tree level by level. Each iteration processes one full
/// level, incrementing the depth counter.
///
/// Time: O(n) - Visits every node.
/// Space: O(w) - Stores the current level in the queue. `w` is the maximum width of the tree.
///
/// # Why this matters
/// While recursion is elegant, it uses the call stack, which is limited (though generous in Rust).
/// An iterative approach moves the storage to the heap (via `VecDeque`), allowing us to handle
/// much deeper trees without a stack overflow.
///
/// # Gotcha
/// When implementing BFS for depth, you must capture the `level_size` *before* iterating
/// through the queue to distinguish between nodes of the current level and their children
/// (which belong to the next level).
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn max_depth_iterative(root: Option<Box<TreeNode>>) -> i32 {
    let mut depth = 0;
    if let Some(node) = root {
        let mut queue = VecDeque::new();
        queue.push_back(node);

        while !queue.is_empty() {
            depth += 1;
            let level_size = queue.len();

            for _ in 0..level_size {
                let node = queue.pop_front().unwrap();
                if let Some(left) = node.left {
                    queue.push_back(left);
                }
                if let Some(right) = node.right {
                    queue.push_back(right);
                }
            }
        }
    }
    depth
}

/// Functional Approach: Fold/Map Combinators.
///
/// This approach tries to express the recursive logic using functional combinators.
/// While succinct, it can be harder to read for those unfamiliar with `Option` combinators.
///
/// Time: O(n)
/// Space: O(h)
///
/// # Rust Insight
/// `Option::map_or` is a powerful method that applies a function if the option is `Some`,
/// or returns a default value if it is `None`. It replaces the `match` statement in the
/// recursive solution.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn max_depth_fold(root: Option<Box<TreeNode>>) -> i32 {
    root.map_or(0, |node| {
        1 + cmp::max(max_depth_fold(node.left), max_depth_fold(node.right))
    })
}

/// Main entry point.
///
/// We default to the recursive solution as it is the most idiomatic and clear for this specific problem.
#[must_use]
pub fn max_depth(root: Option<Box<TreeNode>>) -> i32 {
    max_depth_recursive(root)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create a leaf node
    fn leaf(val: i32) -> Option<Box<TreeNode>> {
        Some(Box::new(TreeNode::new(val)))
    }

    #[test]
    fn test_recursive_simple() {
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

        assert_eq!(max_depth_recursive(Some(Box::new(root))), 3);
    }

    #[test]
    fn test_iterative_simple() {
        // Same tree as above
        let mut root = TreeNode::new(3);
        root.left = leaf(9);
        let mut right = TreeNode::new(20);
        right.left = leaf(15);
        right.right = leaf(7);
        root.right = Some(Box::new(right));

        assert_eq!(max_depth_iterative(Some(Box::new(root))), 3);
    }

    #[test]
    fn test_fold_simple() {
        // Same tree as above
        let mut root = TreeNode::new(3);
        root.left = leaf(9);
        let mut right = TreeNode::new(20);
        right.left = leaf(15);
        right.right = leaf(7);
        root.right = Some(Box::new(right));

        assert_eq!(max_depth_fold(Some(Box::new(root))), 3);
    }

    #[test]
    fn test_empty_tree() {
        assert_eq!(max_depth(None), 0);
        assert_eq!(max_depth_iterative(None), 0);
        assert_eq!(max_depth_fold(None), 0);
    }

    #[test]
    fn test_skewed_tree() {
        // 1 -> 2 -> 3
        let mut root = TreeNode::new(1);
        let mut node2 = TreeNode::new(2);
        node2.right = leaf(3);
        root.right = Some(Box::new(node2));

        assert_eq!(max_depth(Some(Box::new(root))), 3);
    }

    #[test]
    fn test_single_node() {
        assert_eq!(max_depth(leaf(1)), 1);
    }
}
