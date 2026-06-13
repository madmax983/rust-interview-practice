//! # 572. Subtree of Another Tree
//!
//! Difficulty: Easy
//! Link: <https://leetcode.com/problems/subtree-of-another-tree/>
//!
//! Given the roots of two binary trees `root` and `subRoot`, return `true` if there
//! is a subtree of `root` with the same structure and node values of `subRoot` and `false` otherwise.
//!
//! A subtree of a binary tree `tree` is a tree that consists of a node in `tree` and all of
//! this node's descendants. The tree `tree` could also be considered as a subtree of itself.
//!
//! This problem demonstrates pattern matching with `Option<Box<TreeNode>>` and recursion to compare
//! nested structures. It naturally extends the "Same Tree" problem by applying it recursively across
//! all nodes of the main tree. We provide both a recursive brute-force search and an optimized
//! string serialization approach.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::trees::subtree_of_another_tree::{is_subtree, TreeNode};
//!
//! // root: [3,4,5,1,2]
//! let mut root = TreeNode::new(3);
//! let mut left = TreeNode::new(4);
//! left.left = Some(Box::new(TreeNode::new(1)));
//! left.right = Some(Box::new(TreeNode::new(2)));
//! root.left = Some(Box::new(left));
//! root.right = Some(Box::new(TreeNode::new(5)));
//!
//! // subRoot: [4,1,2]
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

/// Helper to check if two trees are completely identical (from "Same Tree" problem)
fn is_same_tree(p: &Option<Box<TreeNode>>, q: &Option<Box<TreeNode>>) -> bool {
    // RUST INSIGHT: We borrow the Options to avoid cloning entire subtrees for comparison.
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

/// Brute Force / Recursive Search Approach
/// Time: O(M * N) - Where M is nodes in root, N is nodes in subRoot.
/// Space: O(H) - Where H is the height of the root tree (recursion stack).
///
/// We traverse the `root` tree. For every node we visit, we check if the tree starting
/// at that node is identical to `subRoot` using our `is_same_tree` helper.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_brute_force(
    root: Option<Box<TreeNode>>,
    sub_root: Option<Box<TreeNode>>,
) -> bool {
    fn dfs(node: &Option<Box<TreeNode>>, sub: &Option<Box<TreeNode>>) -> bool {
        if node.is_none() {
            return false;
        }

        if is_same_tree(node, sub) {
            return true;
        }

        // RUST INSIGHT: We unwrap safely because we checked `node.is_none()` above.
        // `as_ref().unwrap()` gives us a reference to the Box without consuming it.
        let inner = node.as_ref().unwrap();
        dfs(&inner.left, sub) || dfs(&inner.right, sub)
    }

    dfs(&root, &sub_root)
}

/// Optimized Approach: Pre-order String Serialization
/// Time: O(M + N) - One pass to serialize each tree, plus substring search.
/// Space: O(M + N) - To store the serialized strings.
///
/// If we serialize both trees using pre-order traversal (being careful to include
/// specific markers for `None` to preserve structural uniqueness), the problem reduces
/// to checking if the serialized `subRoot` is a substring of the serialized `root`.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_optimized(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    // Helper to serialize the tree
    fn serialize(node: &Option<Box<TreeNode>>, out: &mut String) {
        match node {
            Some(n) => {
                // GOTCHA: We must prepend a marker (like '#') before the value.
                // Otherwise, a value of "12" could incorrectly match a node value of "2"
                // nested within another node.
                let _ = write!(out, "#{val}", val = n.val);
                serialize(&n.left, out);
                serialize(&n.right, out);
            }
            None => {
                // Important to mark nulls uniquely to preserve structure
                out.push_str("#None");
            }
        }
    }

    let mut root_str = String::new();
    let mut sub_str = String::new();

    serialize(&root, &mut root_str);
    serialize(&sub_root, &mut sub_str);

    // RUST INSIGHT: String matching (`contains`) in Rust uses highly optimized algorithms
    // under the hood (like Two-Way string matching), effectively giving us O(M+N) time complexity here.
    root_str.contains(&sub_str)
}

/// Main entry point - uses optimized solution
#[must_use]
pub fn is_subtree(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    is_subtree_optimized(root, sub_root)
}

// Alternative Approaches:
// 1. **KMP or Rabin-Karp on Tree arrays**: Serialize to arrays of nodes and perform a rigorous
//    O(M+N) substring search if allocating strings is problematic.
// 2. **Merkle Tree / Hashing**: Compute a hash for each subtree bottom-up in O(M). If the hash
//    matches the hash of `subRoot`, verify with `is_same_tree` to avoid collisions.

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(val: i32) -> Option<Box<TreeNode>> {
        Some(Box::new(TreeNode::new(val)))
    }

    #[test]
    fn test_brute_force_example_1() {
        let mut root = TreeNode::new(3);
        let mut left = TreeNode::new(4);
        left.left = leaf(1);
        left.right = leaf(2);
        root.left = Some(Box::new(left));
        root.right = leaf(5);

        let mut sub = TreeNode::new(4);
        sub.left = leaf(1);
        sub.right = leaf(2);

        assert!(is_subtree_brute_force(
            Some(Box::new(root)),
            Some(Box::new(sub))
        ));
    }

    #[test]
    fn test_optimized_example_1() {
        let mut root = TreeNode::new(3);
        let mut left = TreeNode::new(4);
        left.left = leaf(1);
        left.right = leaf(2);
        root.left = Some(Box::new(left));
        root.right = leaf(5);

        let mut sub = TreeNode::new(4);
        sub.left = leaf(1);
        sub.right = leaf(2);

        assert!(is_subtree_optimized(
            Some(Box::new(root)),
            Some(Box::new(sub))
        ));
    }

    #[test]
    fn test_example_2_not_subtree() {
        let mut root = TreeNode::new(3);
        let mut left = TreeNode::new(4);
        left.left = leaf(1);

        let mut right_child_of_left = TreeNode::new(2);
        right_child_of_left.left = leaf(0); // Extra child!
        left.right = Some(Box::new(right_child_of_left));

        root.left = Some(Box::new(left));
        root.right = leaf(5);

        let mut sub = TreeNode::new(4);
        sub.left = leaf(1);
        sub.right = leaf(2);

        assert!(!is_subtree(Some(Box::new(root)), Some(Box::new(sub))));
    }

    #[test]
    fn test_same_trees() {
        let root = leaf(1);
        let sub = leaf(1);
        assert!(is_subtree(root, sub));
    }

    #[test]
    fn test_value_substring_trick() {
        // If we didn't use markers like '#', value '12' might match '2'.
        let root = leaf(12);
        let sub = leaf(2);
        assert!(!is_subtree(root, sub));
    }
}
