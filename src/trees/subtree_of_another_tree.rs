//! # 572. Subtree of Another Tree
//!
//! Difficulty: Easy
//! Link: <https://leetcode.com/problems/subtree-of-another-tree/>
//!
//! Given the roots of two binary trees `root` and `subRoot`, return `true` if there is a subtree of `root`
//! with the same structure and node values of `subRoot` and `false` otherwise.
//!
//! This problem perfectly demonstrates Rust's pattern matching with `Option<Box<TreeNode>>` and recursion,
//! as well as showcasing advanced optimization techniques using string serialization, pre-allocation, and
//! the zero-allocation `write!` macro.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::trees::subtree_of_another_tree::{is_subtree, TreeNode};
//!
//! let mut sub_root = TreeNode::new(4);
//! sub_root.left = Some(Box::new(TreeNode::new(1)));
//! sub_root.right = Some(Box::new(TreeNode::new(2)));
//!
//! let mut root = TreeNode::new(3);
//! root.left = Some(Box::new(sub_root.clone()));
//! root.right = Some(Box::new(TreeNode::new(5)));
//!
//! assert_eq!(is_subtree(Some(Box::new(root)), Some(Box::new(sub_root))), true);
//! ```
//!
//! ## Benchmarking Note
//! We demonstrate multiple approaches:
//! 1. Brute Force (Recursive)
//! 2. Naive String Serialization
//! 3. Zero-Allocation Optimal Serialization

use std::fmt::Write; // Needed for `write!` macro

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

/// Brute Force approach: Recursive search
///
/// Time: O(M * N) - M is number of nodes in root, N is number of nodes in subRoot
/// Space: O(H) - Max recursion depth is H (height of tree)
///
/// We traverse `root` and for each node, we check if the tree starting from that node
/// is identical to `subRoot`.
///
/// Note: We pass `Option<&TreeNode>` to helper functions using `.as_deref()` to
/// borrow the trees without consuming them, idiomatic in Rust!
#[must_use]
pub fn is_subtree_brute_force(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    // Helper to check if two trees are identical
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

    // DFS to find potential matching subroots
    fn dfs(node: Option<&TreeNode>, sub_root: Option<&TreeNode>) -> bool {
        if let Some(n) = node {
            if is_same_tree(Some(n), sub_root) {
                return true;
            }
            dfs(n.left.as_deref(), sub_root) || dfs(n.right.as_deref(), sub_root)
        } else {
            false
        }
    }

    // RUST INSIGHT: `.as_deref()` converts `&Option<Box<T>>` into `Option<&T>`
    dfs(root.as_deref(), sub_root.as_deref())
}


/// Naive String Serialization Approach
///
/// Time: O(M + N)
/// Space: O(M + N) for the strings
///
/// We serialize both trees into strings and use substring search.
///
/// GOTCHA: We must use proper node boundary markers (like `,VAL,`) to avoid false positive
/// substring matches (e.g. subtree "2" matching inside node "12"). We also need to represent
/// `None` nodes (e.g. `#`) to preserve tree structure.
#[must_use]
pub fn is_subtree_naive_serialize(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    fn serialize(node: Option<&TreeNode>) -> String {
        match node {
            Some(n) => {
                let left_str = serialize(n.left.as_deref());
                let right_str = serialize(n.right.as_deref());
                // Inefficient allocation here!
                format!(",{},{}{}", n.val, left_str, right_str)
            }
            None => ",#,".to_string(),
        }
    }

    let root_str = serialize(root.as_deref());
    let sub_str = serialize(sub_root.as_deref());

    root_str.contains(&sub_str)
}


/// Optimal Serialization: Zero-Allocation and Pre-allocation
///
/// Time: O(M + N)
/// Space: O(M + N) - But with minimal re-allocations
///
/// We serialize both trees but we avoid the O(N^2) allocations of `format!` by
/// pre-allocating a `String` buffer and iteratively writing into it.
#[must_use]
pub fn is_subtree_optimal(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    // Helper to serialize into an existing buffer using `write!` macro.
    fn serialize(node: Option<&TreeNode>, buf: &mut String) {
        match node {
            Some(n) => {
                // RUST INSIGHT: Using `write!` directly to the buffer avoids allocations
                // associated with intermediate strings or `format!`. We ignore the Result
                // as writing to a String buffer won't fail unless out of memory.
                let _ = write!(buf, ",{},", n.val);
                serialize(n.left.as_deref(), buf);
                serialize(n.right.as_deref(), buf);
            }
            None => {
                let _ = write!(buf, ",#,");
            }
        }
    }

    // Allocate slightly larger buffers to minimize reallocation.
    let mut root_str = String::with_capacity(2048);
    let mut sub_str = String::with_capacity(512);

    serialize(root.as_deref(), &mut root_str);
    serialize(sub_root.as_deref(), &mut sub_str);

    root_str.contains(&sub_str)
}

/// Main entry point
#[must_use]
pub fn is_subtree(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    is_subtree_optimal(root, sub_root)
}

// Alternative Approaches:
// 1. **Merkle Hashing**: We could compute a hash for every subtree bottom-up.
//    If hashes match, we compare the subtrees to avoid hash collisions. This gives
//    O(M+N) time and avoids string manipulations.

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(val: i32) -> Option<Box<TreeNode>> {
        Some(Box::new(TreeNode::new(val)))
    }

    #[test]
    fn test_happy_path() {
        // subRoot:
        //   4
        //  / \
        // 1   2
        let mut sub_root = TreeNode::new(4);
        sub_root.left = leaf(1);
        sub_root.right = leaf(2);

        // root:
        //     3
        //    / \
        //   4   5
        //  / \
        // 1   2
        let mut root = TreeNode::new(3);
        root.left = Some(Box::new(sub_root.clone()));
        root.right = leaf(5);

        assert!(is_subtree_brute_force(Some(Box::new(root.clone())), Some(Box::new(sub_root.clone()))));
        assert!(is_subtree_naive_serialize(Some(Box::new(root.clone())), Some(Box::new(sub_root.clone()))));
        assert!(is_subtree_optimal(Some(Box::new(root)), Some(Box::new(sub_root))));
    }

    #[test]
    fn test_edge_case_not_subtree_due_to_extra_leaf() {
        // subRoot:
        //   4
        //  / \
        // 1   2
        let mut sub_root = TreeNode::new(4);
        sub_root.left = leaf(1);
        sub_root.right = leaf(2);

        // root:
        //     3
        //    / \
        //   4   5
        //  / \
        // 1   2
        //    /
        //   0
        let mut root_left = TreeNode::new(4);
        root_left.left = leaf(1);

        let mut root_left_right = TreeNode::new(2);
        root_left_right.left = leaf(0);
        root_left.right = Some(Box::new(root_left_right));

        let mut root = TreeNode::new(3);
        root.left = Some(Box::new(root_left));
        root.right = leaf(5);

        assert!(!is_subtree_brute_force(Some(Box::new(root.clone())), Some(Box::new(sub_root.clone()))));
        assert!(!is_subtree_naive_serialize(Some(Box::new(root.clone())), Some(Box::new(sub_root.clone()))));
        assert!(!is_subtree_optimal(Some(Box::new(root)), Some(Box::new(sub_root))));
    }

    #[test]
    fn test_stress_substring_boundary_gotcha() {
        // If we naive serialized "12" and "2" without boundaries, "12" contains "2"
        // subRoot: 2
        let sub_root = leaf(2);

        // root: 12
        let root = leaf(12);

        assert!(!is_subtree_brute_force(root.clone(), sub_root.clone()));
        assert!(!is_subtree_naive_serialize(root.clone(), sub_root.clone()));
        assert!(!is_subtree_optimal(root, sub_root));
    }
}
