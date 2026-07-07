//! # 572. Subtree of Another Tree
//!
//! Difficulty: Easy
//! Link: <https://leetcode.com/problems/subtree-of-another-tree/>
//!
//! Given the roots of two binary trees `root` and `subRoot`, return `true` if there is a subtree of `root` with the same structure and node values of `subRoot` and `false` otherwise.
//!
//! This problem provides a great opportunity to explore tree traversal and pattern matching with `Option<Box<TreeNode>>`.
//! It also demonstrates an advanced optimization technique: converting a tree problem into a string matching problem,
//! while highlighting the importance of zero-allocation string appending with `String::with_capacity` and `write!` macro.
//!
//! ## Approach
//!
//! 1. **Brute Force (Recursive)**: For each node in `root`, check if it and its descendants are structurally identical to `subRoot`.
//! 2. **Optimized (Naive Serialization)**: Serialize both trees to strings using pre-order traversal (including null markers and boundary markers). Then, check if `subRoot` string is a substring of `root` string.
//! 3. **Optimal (Zero-Allocation Serialization)**: Similar to the optimized approach, but carefully manages memory to avoid intermediate string allocations during serialization.
//!
//! RUST INSIGHT: When passing `Option<Box<TreeNode>>` by reference without consuming it, idiomatic Rust prefers `Option<&TreeNode>` over `&Option<Box<TreeNode>>`. We can achieve this conversion using `.as_deref()`.

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

/// Helper function to check if two trees are exactly the same
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

/// Brute Force approach: Recursive Subtree Search
///
/// Time: O(M * N) - Where M is nodes in `root` and N is nodes in `subRoot`. In the worst case, we check `is_same_tree` at every node.
/// Space: O(M) - Recursion stack depth.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_brute_force(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    fn dfs(node: Option<&TreeNode>, target: Option<&TreeNode>) -> bool {
        if node.is_none() {
            return false;
        }

        if is_same_tree(node, target) {
            return true;
        }

        // RUST INSIGHT: `.as_deref()` gracefully handles the conversion from `&Option<Box<TreeNode>>` to `Option<&TreeNode>` inside the Option wrapper.
        let left = node.and_then(|n| n.left.as_deref());
        let right = node.and_then(|n| n.right.as_deref());

        dfs(left, target) || dfs(right, target)
    }

    dfs(root.as_deref(), sub_root.as_deref())
}

/// Optimized approach: Naive String Serialization
///
/// Time: O(M + N) - Serialization takes O(M + N). Substring search is O(M * N) naively, or O(M + N) with KMP (which Rust's `contains` uses underneath).
/// Space: O(M + N) - For storing the serialized strings.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_optimized(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    // GOTCHA: We must use strict boundary markers like `^` and `#` (or specific characters) to avoid false positive substring matches.
    // E.g. Tree `12` vs Tree `2`. Serializing without boundaries: `12` contains `2`, which is wrong.
    // Serializing with boundaries: `^12#` does not contain `^2#`.
    fn serialize(node: Option<&TreeNode>) -> String {
        match node {
            Some(n) => format!("^{}#L{}R{}", n.val, serialize(n.left.as_deref()), serialize(n.right.as_deref())),
            None => "N".to_string(),
        }
    }

    let root_str = serialize(root.as_deref());
    let sub_str = serialize(sub_root.as_deref());

    root_str.contains(&sub_str)
}

/// Optimal approach: Zero-Allocation Serialization with String Buffer
///
/// Time: O(M + N)
/// Space: O(M + N) - Better constant factors due to single allocation per tree.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_optimal(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    fn serialize_to_buffer(node: Option<&TreeNode>, buf: &mut String) {
        match node {
            Some(n) => {
                // RUST INSIGHT: Instead of creating a new String at every step (like `format!`), we append to a single mutable buffer using `write!`.
                // This eliminates intermediate heap allocations.
                let _ = write!(buf, "^{}#", n.val);

                buf.push('L');
                serialize_to_buffer(n.left.as_deref(), buf);

                buf.push('R');
                serialize_to_buffer(n.right.as_deref(), buf);
            }
            None => {
                buf.push('N');
            }
        }
    }

    // RUST INSIGHT: `String::with_capacity` pre-allocates memory, preventing reallocations as the string grows.
    let mut root_buf = String::with_capacity(1000); // Guessed size
    let mut sub_buf = String::with_capacity(1000);

    serialize_to_buffer(root.as_deref(), &mut root_buf);
    serialize_to_buffer(sub_root.as_deref(), &mut sub_buf);

    root_buf.contains(&sub_buf)
}

/// Main entry point
#[must_use]
pub fn is_subtree(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    is_subtree_optimal(root, sub_root)
}

// Alternative Approaches:
// 1. **Merkle Hashing**: Compute a hash for every subtree bottom-up. Two identical subtrees will have the same hash. This achieves O(M+N) time and O(M) space without string manipulations, but handling hash collisions can be tricky.

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(val: i32) -> Option<Box<TreeNode>> {
        Some(Box::new(TreeNode::new(val)))
    }

    #[test]
    fn test_happy_path() {
        // root = [3,4,5,1,2], subRoot = [4,1,2]
        //       3
        //      / \
        //     4   5
        //    / \
        //   1   2
        let mut root = TreeNode::new(3);
        let mut left = TreeNode::new(4);
        left.left = leaf(1);
        left.right = leaf(2);
        root.left = Some(Box::new(left));
        root.right = leaf(5);

        let mut sub = TreeNode::new(4);
        sub.left = leaf(1);
        sub.right = leaf(2);

        let root_box = Some(Box::new(root));
        let sub_box = Some(Box::new(sub));

        assert!(is_subtree_brute_force(root_box.clone(), sub_box.clone()));
        assert!(is_subtree_optimized(root_box.clone(), sub_box.clone()));
        assert!(is_subtree_optimal(root_box, sub_box));
    }

    #[test]
    fn test_edge_case_similar_but_not_same() {
        // root = [3,4,5,1,2,null,null,null,null,0], subRoot = [4,1,2]
        //       3
        //      / \
        //     4   5
        //    / \
        //   1   2
        //      /
        //     0
        let mut root = TreeNode::new(3);
        let mut left = TreeNode::new(4);
        left.left = leaf(1);

        let mut right_child = TreeNode::new(2);
        right_child.left = leaf(0);
        left.right = Some(Box::new(right_child));

        root.left = Some(Box::new(left));
        root.right = leaf(5);

        let mut sub = TreeNode::new(4);
        sub.left = leaf(1);
        sub.right = leaf(2);

        let root_box = Some(Box::new(root));
        let sub_box = Some(Box::new(sub));

        assert!(!is_subtree_brute_force(root_box.clone(), sub_box.clone()));
        assert!(!is_subtree_optimized(root_box.clone(), sub_box.clone()));
        assert!(!is_subtree_optimal(root_box, sub_box));
    }

    #[test]
    fn test_boundary_markers_importance() {
        // root = [12], subRoot = [2]
        let root = leaf(12);
        let sub = leaf(2);

        assert!(!is_subtree_brute_force(root.clone(), sub.clone()));
        assert!(!is_subtree_optimized(root.clone(), sub.clone()));
        assert!(!is_subtree_optimal(root, sub));
    }
}