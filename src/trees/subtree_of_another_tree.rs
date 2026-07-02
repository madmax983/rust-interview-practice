//! # 572. Subtree of Another Tree
//!
//! Difficulty: Easy
//! Link: <https://leetcode.com/problems/subtree-of-another-tree/>
//!
//! Given the roots of two binary trees `root` and `subRoot`, return `true` if there is a subtree of `root` with the same structure and node values of `subRoot` and `false` otherwise.
//!
//! This problem perfectly demonstrates Rust's strict ownership and borrowing rules when dealing with recursive data structures. Because traversing a tree shouldn't consume it, we must carefully pass borrowed references (like `Option<&TreeNode>`) rather than owned values, utilizing `.as_deref()` to convert `&Option<Box<TreeNode>>` to `Option<&TreeNode>` without moving the underlying data.
//!
//! ## Approaches
//!
//! 1. **Brute Force Search**: We perform a traversal on `root`, and for every node we visit, we check if the tree starting from there matches `subRoot` identically (using a helper similar to `is_same_tree`).
//!    - Time: O(M * N) where M is nodes in `root` and N is nodes in `subRoot`.
//!    - Space: O(M) for recursion stack.
//! 2. **Naive String Serialization**: We serialize both trees into string representations (using preorder traversal with null markers) and then check if the `subRoot` string is a substring of the `root` string.
//!    - Time: O(M + N + M*N) for substring match (can be O(M+N) with KMP).
//!    - Space: O(M + N) to hold the string representations.
//! 3. **Optimized Zero-Allocation Serialization**: Similar to serialization, but we pre-allocate the output buffer and use `write!` to append directly, avoiding intermediate `format!` heap allocations. We also carefully use boundary markers (like `[val]`) to avoid false positive substring matches.
//!    - Time: O(M + N + M*N) for string search.
//!    - Space: O(M + N) for the optimized string buffer, but with zero intermediate allocations.
//!

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

/// Brute Force Search Approach
///
/// This approach explores the tree and explicitly checks for a matching structure at every node.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_brute_force(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    // Helper to check if two borrowed trees are identical
    fn is_same_tree(p: Option<&TreeNode>, q: Option<&TreeNode>) -> bool {
        match (p, q) {
            (Some(n_p), Some(n_q)) => {
                n_p.val == n_q.val
                    // RUST INSIGHT: .as_deref() safely converts &Option<Box<TreeNode>>
                    // into Option<&TreeNode> allowing us to inspect children without consuming them.
                    && is_same_tree(n_p.left.as_deref(), n_q.left.as_deref())
                    && is_same_tree(n_p.right.as_deref(), n_q.right.as_deref())
            }
            (None, None) => true,
            _ => false,
        }
    }

    // Helper to traverse the main tree
    fn dfs(node: Option<&TreeNode>, sub: Option<&TreeNode>) -> bool {
        match node {
            Some(n) => {
                is_same_tree(Some(n), sub)
                    || dfs(n.left.as_deref(), sub)
                    || dfs(n.right.as_deref(), sub)
            }
            None => false,
        }
    }

    dfs(root.as_deref(), sub_root.as_deref())
}

/// Naive Serialization Approach
///
/// We serialize both trees using a preorder traversal, then check if the `subRoot` representation
/// is a substring of the `root` representation.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_naive_serialization(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    fn serialize(node: Option<&TreeNode>) -> String {
        match node {
            Some(n) => {
                // GOTCHA: We must wrap the value in delimiters (e.g. `[val]`) to avoid false matches
                // where "12" would match as a substring of "112".
                // Also, using `format!` here creates many intermediate heap allocations per depth.
                format!("[{}]{}{}", n.val, serialize(n.left.as_deref()), serialize(n.right.as_deref()))
            }
            None => "#".to_string(), // `#` represents a null pointer
        }
    }

    let root_str = serialize(root.as_deref());
    let sub_str = serialize(sub_root.as_deref());

    root_str.contains(&sub_str)
}

/// Optimized Zero-Allocation Serialization Approach
///
/// Pre-allocates a `String` buffer and appends nodes in-place using `write!`,
/// avoiding any temporary `String` creations during recursion.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_optimized_serialization(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    // We pass a mutable reference to our pre-allocated String buffer.
    fn serialize_fast(node: Option<&TreeNode>, buf: &mut String) {
        match node {
            Some(n) => {
                // RUST INSIGHT: `write!` appends formatted text directly to the buffer
                // avoiding any intermediate allocations like `format!` does.
                // We wrap the value in `[]` to establish solid node boundaries.
                let _ = write!(buf, "[{}]", n.val);
                serialize_fast(n.left.as_deref(), buf);
                serialize_fast(n.right.as_deref(), buf);
            }
            None => {
                buf.push('#');
            }
        }
    }

    // Allocate buffers with a reasonable capacity.
    // In a real scenario, this could be tuned based on tree size constraints.
    let mut root_str = String::with_capacity(1024);
    let mut sub_str = String::with_capacity(1024);

    serialize_fast(root.as_deref(), &mut root_str);
    serialize_fast(sub_root.as_deref(), &mut sub_str);

    // RUST INSIGHT: We don't prepend a global "start of tree" marker like `^` to `sub_str`,
    // because `subRoot` can match anywhere inside `root`, not just at the very beginning.
    root_str.contains(&sub_str)
}

/// Main entry point
#[must_use]
pub fn is_subtree(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    is_subtree_optimized_serialization(root, sub_root)
}

// Alternative Approaches:
// 1. **Merkle Hashing**: We could compute a hash for each subtree in `root` and compare the hashes in O(M+N).
//    However, in Rust this requires building a solid post-order hash function to prevent collisions, which is
//    typically overkill for small trees but optimal for massive ones.

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(val: i32) -> Option<Box<TreeNode>> {
        Some(Box::new(TreeNode::new(val)))
    }

    #[test]
    fn test_happy_path_is_subtree() {
        // root:
        //      3
        //     / \
        //    4   5
        //   / \
        //  1   2
        let mut root = TreeNode::new(3);
        let mut left = TreeNode::new(4);
        left.left = leaf(1);
        left.right = leaf(2);
        root.left = Some(Box::new(left));
        root.right = leaf(5);

        // subRoot:
        //    4
        //   / \
        //  1   2
        let mut sub = TreeNode::new(4);
        sub.left = leaf(1);
        sub.right = leaf(2);

        let r_box = Some(Box::new(root));
        let s_box = Some(Box::new(sub));

        assert!(is_subtree_brute_force(r_box.clone(), s_box.clone()));
        assert!(is_subtree_naive_serialization(r_box.clone(), s_box.clone()));
        assert!(is_subtree_optimized_serialization(r_box.clone(), s_box.clone()));
    }

    #[test]
    fn test_edge_case_not_subtree_due_to_extra_leaf() {
        // root:
        //      3
        //     / \
        //    4   5
        //   / \
        //  1   2
        //     /
        //    0
        let mut root = TreeNode::new(3);
        let mut left = TreeNode::new(4);
        left.left = leaf(1);
        let mut left_right = TreeNode::new(2);
        left_right.left = leaf(0);
        left.right = Some(Box::new(left_right));
        root.left = Some(Box::new(left));
        root.right = leaf(5);

        // subRoot:
        //    4
        //   / \
        //  1   2
        let mut sub = TreeNode::new(4);
        sub.left = leaf(1);
        sub.right = leaf(2);

        let r_box = Some(Box::new(root));
        let s_box = Some(Box::new(sub));

        assert!(!is_subtree_brute_force(r_box.clone(), s_box.clone()));
        assert!(!is_subtree_naive_serialization(r_box.clone(), s_box.clone()));
        assert!(!is_subtree_optimized_serialization(r_box.clone(), s_box.clone()));
    }

    #[test]
    fn test_stress_boundary_case_same_values_different_structure() {
        // root:
        //    12
        let root = TreeNode::new(12);

        // subRoot:
        //    2
        let sub = TreeNode::new(2);

        let r_box = Some(Box::new(root));
        let s_box = Some(Box::new(sub));

        // Testing the `[val]` boundary markers
        assert!(!is_subtree_brute_force(r_box.clone(), s_box.clone()));
        assert!(!is_subtree_naive_serialization(r_box.clone(), s_box.clone()));
        assert!(!is_subtree_optimized_serialization(r_box.clone(), s_box.clone()));
    }
}
