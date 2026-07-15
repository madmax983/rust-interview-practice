//! # 572. Subtree of Another Tree
//!
//! Difficulty: Easy
//! Link: <https://leetcode.com/problems/subtree-of-another-tree/>
//!
//! Given the roots of two binary trees `root` and `subRoot`, return `true` if there is a subtree of `root` with the same structure and node values of `subRoot` and `false` otherwise.
//!
//! This problem perfectly demonstrates Rust's powerful recursive pattern matching combined with its trait system. By using `std::fmt::Write`, we can also serialize the tree to string with zero allocations directly into an existing buffer.
//!
//! ## Approach
//!
//! There are a few approaches:
//! 1. Brute Force recursion: At every node, check if `is_same_tree(current, subRoot)`.
//! 2. String Serialization: Serialize both trees and do substring match.
//!
//! Here we implement the Recursive approach as the primary idiomatic pattern, and a zero-allocation string serialization as the optimized trick.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::trees::subtree_of_another_tree::{is_subtree, TreeNode};
//!
//! let mut root = TreeNode::new(3);
//! root.left = Some(Box::new(TreeNode::new(4)));
//! root.right = Some(Box::new(TreeNode::new(5)));
//! root.left.as_mut().unwrap().left = Some(Box::new(TreeNode::new(1)));
//! root.left.as_mut().unwrap().right = Some(Box::new(TreeNode::new(2)));
//!
//! let mut sub_root = TreeNode::new(4);
//! sub_root.left = Some(Box::new(TreeNode::new(1)));
//! sub_root.right = Some(Box::new(TreeNode::new(2)));
//!
//! assert_eq!(is_subtree(Some(Box::new(root)), Some(Box::new(sub_root))), true);
//! ```

use std::fmt::Write;

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

/// Helper function to check if two trees are structurally identical
fn is_same_tree(p: Option<&TreeNode>, q: Option<&TreeNode>) -> bool {
    match (p, q) {
        (Some(n_p), Some(n_q)) => {
            n_p.val == n_q.val
                && is_same_tree(n_p.left.as_deref(), n_q.left.as_deref())
                && is_same_tree(n_p.right.as_deref(), n_q.right.as_deref())
        }
        (None, None) => true,
        _ => false,
    }
}

/// Recursive Brute-Force approach
///
/// Time: O(M * N) - Where M is the number of nodes in root and N is number in subRoot
/// Space: O(M) - For recursion depth
///
/// We traverse the root tree, and at each node, we check if the tree starting at that node matches `subRoot`.
/// We use `.as_deref()` heavily to avoid taking ownership of the `Box<TreeNode>` while passing `Option<&TreeNode>`.
#[must_use]
pub fn is_subtree_recursive(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    // RUST INSIGHT: We only want to borrow the trees for the check, not consume them,
    // so we pass `Option<&TreeNode>` to our recursive helper.
    fn check_subtree(r: Option<&TreeNode>, s: Option<&TreeNode>) -> bool {
        match r {
            Some(node) => {
                is_same_tree(Some(node), s)
                    || check_subtree(node.left.as_deref(), s)
                    || check_subtree(node.right.as_deref(), s)
            }
            None => s.is_none(), // If root is empty, subRoot must also be empty
        }
    }

    check_subtree(root.as_deref(), sub_root.as_deref())
}

/// Optimized approach: Zero-allocation Serialization
///
/// Time: O(M + N) - Serialize both trees and perform substring search
/// Space: O(M + N) - For the serialized strings
///
/// This approach serializes both trees into strings using a pre-order traversal with distinct
/// node boundary markers. We then check if the subRoot string is a substring of the root string.
/// We use `String::with_capacity` and `write!` macro to avoid intermediate allocations.
#[must_use]
pub fn is_subtree_optimal(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    fn serialize(node: Option<&TreeNode>, out: &mut String) {
        match node {
            Some(n) => {
                // GOTCHA: We must wrap the value in boundary markers (like #val#)
                // so that "12" is not mistaken as a subtree of "112".
                let _ = write!(out, "#{val}#", val = n.val);
                serialize(n.left.as_deref(), out);
                serialize(n.right.as_deref(), out);
            }
            None => {
                out.push_str("null");
            }
        }
    }

    let mut root_str = String::with_capacity(1000); // Pre-allocate based on expected max constraints
    let mut sub_root_str = String::with_capacity(1000);

    serialize(root.as_deref(), &mut root_str);
    serialize(sub_root.as_deref(), &mut sub_root_str);

    root_str.contains(&sub_root_str)
}

/// Main entry point (defaults to the recursive brute-force as it's cleaner without constraints)
#[must_use]
pub fn is_subtree(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    is_subtree_recursive(root, sub_root)
}

// Alternative Approaches:
// 1. **KMP (Knuth-Morris-Pratt)**: The substring match in `is_subtree_optimal` uses Rust's built-in `contains` which is highly optimized, but a rigorous algorithmic interview might require a manual KMP implementation for guaranteed O(M+N) time.
// 2. **Merkle Hashing**: Hash each subtree bottom-up. Matches can be verified in O(1) time once hashed, reducing time to O(M+N).

#[cfg(test)]
mod tests {
    #![allow(clippy::unnecessary_wraps)]

    use super::*;

    fn leaf(val: i32) -> Option<Box<TreeNode>> {
        Some(Box::new(TreeNode::new(val)))
    }

    #[test]
    fn test_happy_path() {
        let mut root = TreeNode::new(3);
        root.left = Some(Box::new(TreeNode::new(4)));
        root.right = Some(Box::new(TreeNode::new(5)));
        root.left.as_mut().unwrap().left = leaf(1);
        root.left.as_mut().unwrap().right = leaf(2);

        let mut sub_root = TreeNode::new(4);
        sub_root.left = leaf(1);
        sub_root.right = leaf(2);

        assert!(is_subtree_recursive(
            Some(Box::new(root.clone())),
            Some(Box::new(sub_root.clone()))
        ));
        assert!(is_subtree_optimal(
            Some(Box::new(root)),
            Some(Box::new(sub_root))
        ));
    }

    #[test]
    fn test_edge_case_not_subtree() {
        let mut root = TreeNode::new(3);
        root.left = Some(Box::new(TreeNode::new(4)));
        root.right = Some(Box::new(TreeNode::new(5)));
        root.left.as_mut().unwrap().left = leaf(1);

        let mut sub_root = TreeNode::new(4);
        sub_root.left = leaf(1);
        sub_root.right = leaf(2);

        assert!(!is_subtree_recursive(
            Some(Box::new(root.clone())),
            Some(Box::new(sub_root.clone()))
        ));
        assert!(!is_subtree_optimal(
            Some(Box::new(root)),
            Some(Box::new(sub_root))
        ));
    }

    #[test]
    fn test_stress_substring_trick() {
        let root = TreeNode::new(12);

        let sub_root = TreeNode::new(2);

        assert!(!is_subtree_recursive(
            Some(Box::new(root.clone())),
            Some(Box::new(sub_root.clone()))
        ));
        assert!(!is_subtree_optimal(
            Some(Box::new(root)),
            Some(Box::new(sub_root))
        ));
    }
}
