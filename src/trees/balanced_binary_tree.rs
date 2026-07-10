//! # 110. Balanced Binary Tree
//!
//! Difficulty: Easy
//! Link: <https://leetcode.com/problems/balanced-binary-tree/>
//!
//! Given a binary tree, determine if it is height-balanced. A height-balanced
//! binary tree is defined as a binary tree in which the left and right subtrees
//! of every node differ in height by no more than 1.
//!
//! ## Why This Matters in Rust
//!
//! In languages like C++ or Java, it is common to use sentinel values (like `-1`)
//! returned from a `height` function to indicate an error state (e.g., that the tree
//! is unbalanced). This blends the domain of successful calculation and failure states.
//!
//! In Rust, this problem perfectly demonstrates how to use the type system (`Result<T, E>`)
//! to explicitly separate success states (`Ok(height)`) from failure states (`Err(Unbalanced)`).
//! Combining this with the `?` operator provides zero-cost short-circuiting: we stop evaluating
//! the tree immediately as soon as an unbalanced subtree is found, without clunky `if height == -1` checks.
//!
//! ## Approach
//!
//! We provide two implementations:
//!
//! 1. **Top-Down (Brute Force)**: Calculates the height of the left and right subtrees for every node,
//!    then recursively checks if the left and right subtrees are themselves balanced.
//!    - **Time**: `O(N^2)` in the worst case (skewed tree) because height is recalculated.
//!    - **Space**: `O(N)` for the call stack.
//!
//! 2. **Bottom-Up (Optimal)**: Calculates the height while simultaneously checking for balance.
//!    If any subtree is unbalanced, the error propagates up immediately.
//!    - **Time**: `O(N)` since each node is visited only once.
//!    - **Space**: `O(H)` where `H` is the tree height, for the call stack.

use std::cmp;

// Definition for a binary tree node.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct TreeNode {
    pub val: i32,
    pub left: Option<Box<Self>>,
    pub right: Option<Box<Self>>,
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

/// Helper function to calculate the height of a tree for the brute force approach.
///
/// Time: O(N)
/// Space: O(H) for recursion stack
#[must_use]
fn height(node: Option<&TreeNode>) -> i32 {
    node.map_or(0, |n| {
        1 + cmp::max(height(n.left.as_deref()), height(n.right.as_deref()))
    })
}

/// Brute Force Approach: Top-Down Recursion
///
/// For each node, it calculates the height of the left and right subtrees
/// to verify the balance condition. It then recursively applies the same
/// logic to the left and right children.
///
/// Time: O(N^2) in worst case (skewed tree), O(N log N) in balanced tree.
/// Space: O(H) for the call stack.
///
/// # Gotcha
/// Recalculating the height for every node causes a lot of duplicated work.
/// `height` is called repeatedly for the same nodes as we traverse down.
#[must_use]
pub fn is_balanced_brute_force(root: Option<Box<TreeNode>>) -> bool {
    root.is_none_or(|node| {
        let left_height = height(node.left.as_deref());
        let right_height = height(node.right.as_deref());

        let diff = (left_height - right_height).abs();

        diff <= 1 && is_balanced_brute_force(node.left) && is_balanced_brute_force(node.right)
    })
}

/// We define a custom Zero-Sized Type (ZST) for our error.
/// This costs absolutely nothing at runtime but gives semantic meaning to `Err`.
struct Unbalanced;

/// Optimal Approach: Bottom-Up Recursion
///
/// We define a helper that returns `Result<i32, Unbalanced>`.
/// This explicitly separates the concept of "the tree is balanced with height H"
/// from "the tree is unbalanced".
///
/// Time: O(N) - visits every node at most once.
/// Space: O(H) - call stack height.
///
/// # Rust Insight
/// The `?` operator gives us elegant short-circuiting. If `check_height(node.left)`
/// returns `Err(Unbalanced)`, the `?` immediately bubbles that error up the call stack.
/// We don't even evaluate `node.right`! This is much cleaner than checking for a
/// magic number like `-1`.
// LeetCode signature: `root` matches the by-value node type shared across all three implementations.
#[allow(clippy::needless_pass_by_value)]
#[must_use]
pub fn is_balanced_optimal(root: Option<Box<TreeNode>>) -> bool {
    fn check_height(node: Option<&TreeNode>) -> Result<i32, Unbalanced> {
        match node {
            None => Ok(0),
            Some(n) => {
                // Short-circuit: if left is unbalanced, propagate Err immediately
                let left_height = check_height(n.left.as_deref())?;
                // Short-circuit: if right is unbalanced, propagate Err immediately
                let right_height = check_height(n.right.as_deref())?;

                if (left_height - right_height).abs() > 1 {
                    Err(Unbalanced)
                } else {
                    Ok(1 + cmp::max(left_height, right_height))
                }
            }
        }
    }

    // If it returns Ok(height), it's balanced. If Err, it's unbalanced.
    check_height(root.as_deref()).is_ok()
}

/// Main entry point - aliases to the optimal approach.
#[must_use]
pub fn is_balanced(root: Option<Box<TreeNode>>) -> bool {
    is_balanced_optimal(root)
}

/// ## Alternative Approaches
///
/// - **Iterative DFS**: You can implement this iteratively using a custom stack and a Hash Map
///   to store heights of subtrees. This avoids the recursion stack limit, but is highly verbose
///   and requires extra O(N) heap allocations for the Hash Map. In Rust, the elegant `Result`
///   based recursion is overwhelmingly preferred unless absolute stack safety is required
///   for extremely deep trees.
#[cfg(test)]
mod tests {
    // test-code: helpers return Option<Box<TreeNode>> to match the tree's child field type.
    #![allow(clippy::unnecessary_wraps)]

    use super::*;

    // Helper to create a leaf node
    fn leaf(val: i32) -> Option<Box<TreeNode>> {
        Some(Box::new(TreeNode::new(val)))
    }

    #[test]
    fn test_balanced_tree_happy_path() {
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

        let boxed_root = Some(Box::new(root));
        assert!(is_balanced_brute_force(boxed_root.clone()));
        assert!(is_balanced_optimal(boxed_root));
    }

    #[test]
    fn test_unbalanced_tree_edge_case() {
        // Tree:
        //         1
        //        / \
        //       2   2
        //      / \
        //     3   3
        //    / \
        //   4   4
        let mut root = TreeNode::new(1);
        let mut left = TreeNode::new(2);
        let mut left_left = TreeNode::new(3);
        left_left.left = leaf(4);
        left_left.right = leaf(4);

        left.left = Some(Box::new(left_left));
        left.right = leaf(3);

        root.left = Some(Box::new(left));
        root.right = leaf(2);

        let boxed_root = Some(Box::new(root));
        assert!(!is_balanced_brute_force(boxed_root.clone()));
        assert!(!is_balanced_optimal(boxed_root));
    }

    #[test]
    fn test_empty_tree_boundary() {
        assert!(is_balanced_brute_force(None));
        assert!(is_balanced_optimal(None));
    }

    #[test]
    fn test_single_node() {
        let root = leaf(1);
        assert!(is_balanced_brute_force(root.clone()));
        assert!(is_balanced_optimal(root));
    }
}
