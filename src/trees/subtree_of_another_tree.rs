//! # 572. Subtree of Another Tree
//!
//! Difficulty: Easy
//! Link: <https://leetcode.com/problems/subtree-of-another-tree/>
//!
//! Given the roots of two binary trees `root` and `subRoot`, return `true` if there is a subtree of `root` with the same structure and node values of `subRoot` and `false` otherwise.
//!
//! A subtree of a binary tree `tree` is a tree that consists of a node in `tree` and all of this node's descendants. The tree `tree` could also be considered as a subtree of itself.
//!
//! This problem perfectly demonstrates pattern matching with `Option<Box<TreeNode>>` and recursion in Rust to compare nested structures. We provide both a recursive brute-force search and an optimized string serialization approach.
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
//!
//! ## Constraints
//!
//! - The number of nodes in the `root` tree is in the range `[1, 2000]`.
//! - The number of nodes in the `subRoot` tree is in the range `[1, 1000]`.
//! - `-10^4 <= root.val <= 10^4`
//! - `-10^4 <= subRoot.val <= 10^4`

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

/// Helper function to check if two trees are identical.
fn is_same_tree(p: &Option<Box<TreeNode>>, q: &Option<Box<TreeNode>>) -> bool {
    // RUST INSIGHT: This match is exhaustive. The compiler guarantees we handle every possible combination
    match (p, q) {
        (Some(n_p), Some(n_q)) => {
            n_p.val == n_q.val
                && is_same_tree(&n_p.left, &n_q.left)
                && is_same_tree(&n_p.right, &n_q.right)
        }
        (None, None) => true,
        _ => false,
    }
}

/// Brute Force approach: Recursive search
///
/// Time: O(M * N) - Where M is the number of nodes in `root` and N is the number of nodes in `subRoot`.
/// Space: O(M) - For the recursion stack in the worst-case scenario.
///
/// We recursively check if the current `root` node forms a tree identical to `subRoot`.
/// If not, we recursively check its left and right children.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_brute_force(
    root: Option<Box<TreeNode>>,
    sub_root: Option<Box<TreeNode>>,
) -> bool {
    fn dfs(node: &Option<Box<TreeNode>>, sub_root: &Option<Box<TreeNode>>) -> bool {
        if node.is_none() {
            return false;
        }

        if is_same_tree(node, sub_root) {
            return true;
        }

        let n = node.as_ref().unwrap();
        dfs(&n.left, sub_root) || dfs(&n.right, sub_root)
    }

    dfs(&root, &sub_root)
}

/// Optimized approach: String Serialization (Pre-order Traversal)
///
/// Time: O(M + N) - Where M is the number of nodes in `root` and N is the number of nodes in `subRoot`.
/// Space: O(M + N) - For storing the serialized string representation of both trees.
///
/// This approach serializes both trees into strings, taking care to mark nulls and wrap values
/// so that false positives (like "12" matching within "123") are avoided.
/// Then it simply checks if the string of `subRoot` is a substring of the string of `root`.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_optimized(
    root: Option<Box<TreeNode>>,
    sub_root: Option<Box<TreeNode>>,
) -> bool {
    // Helper to serialize tree into a String via pre-order traversal
    fn serialize(node: &Option<Box<TreeNode>>, acc: &mut String) {
        match node {
            Some(n) => {
                // GOTCHA: We must wrap values uniquely (e.g., in brackets) to prevent
                // "12" from matching inside "123".
                let _ = write!(acc, "[{}]", n.val);
                serialize(&n.left, acc);
                serialize(&n.right, acc);
            }
            None => {
                acc.push_str("[#]");
            }
        }
    }

    let mut root_str = String::with_capacity(4000); // Pre-allocate based on constraint
    let mut sub_root_str = String::with_capacity(2000);

    serialize(&root, &mut root_str);
    serialize(&sub_root, &mut sub_root_str);

    // Check if sub_root serialization is a substring of root serialization
    root_str.contains(&sub_root_str)
}

/// Main entry point
#[must_use]
pub fn is_subtree(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    is_subtree_brute_force(root, sub_root)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create a leaf node
    fn leaf(val: i32) -> Option<Box<TreeNode>> {
        Some(Box::new(TreeNode::new(val)))
    }

    #[test]
    fn test_happy_path() {
        // root:
        //      3
        //     / \
        //    4   5
        //   / \
        //  1   2
        let mut root = TreeNode::new(3);
        root.left = Some(Box::new(TreeNode::new(4)));
        root.right = Some(Box::new(TreeNode::new(5)));
        root.left.as_mut().unwrap().left = leaf(1);
        root.left.as_mut().unwrap().right = leaf(2);

        // subRoot:
        //    4
        //   / \
        //  1   2
        let mut sub_root = TreeNode::new(4);
        sub_root.left = leaf(1);
        sub_root.right = leaf(2);

        let r = Some(Box::new(root));
        let sr = Some(Box::new(sub_root));

        assert!(is_subtree_brute_force(r.clone(), sr.clone()));
        assert!(is_subtree_optimized(r, sr));
    }

    #[test]
    fn test_edge_case_not_subtree() {
        // root:
        //      3
        //     / \
        //    4   5
        //   / \
        //  1   2
        //     /
        //    0
        let mut root = TreeNode::new(3);
        root.left = Some(Box::new(TreeNode::new(4)));
        root.right = Some(Box::new(TreeNode::new(5)));
        root.left.as_mut().unwrap().left = leaf(1);
        let mut n2 = TreeNode::new(2);
        n2.left = leaf(0);
        root.left.as_mut().unwrap().right = Some(Box::new(n2));

        // subRoot:
        //    4
        //   / \
        //  1   2
        let mut sub_root = TreeNode::new(4);
        sub_root.left = leaf(1);
        sub_root.right = leaf(2);

        let r = Some(Box::new(root));
        let sr = Some(Box::new(sub_root));

        assert!(!is_subtree_brute_force(r.clone(), sr.clone()));
        assert!(!is_subtree_optimized(r, sr));
    }

    #[test]
    fn test_stress_boundary_values() {
        // Ensuring no substring matching tricks fail, e.g., value 12 inside 123
        let mut root = TreeNode::new(12);
        root.left = leaf(3);

        let sub_root = TreeNode::new(2);

        let r = Some(Box::new(root));
        let sr = Some(Box::new(sub_root));

        assert!(!is_subtree_brute_force(r.clone(), sr.clone()));
        assert!(!is_subtree_optimized(r, sr));
    }
}
