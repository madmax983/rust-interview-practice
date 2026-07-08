//! # 572. Subtree of Another Tree
//!
//! Difficulty: Easy
//! Link: <https://leetcode.com/problems/subtree-of-another-tree/>
//!
//! Given the roots of two binary trees `root` and `subRoot`, return `true` if there is a subtree of `root`
//! with the same structure and node values of `subRoot` and `false` otherwise.
//!
//! A subtree of a binary tree `tree` is a tree that consists of a node in `tree` and all of this node's descendants.
//! The tree `tree` could also be considered as a subtree of itself.
//!
//! This problem demonstrates pattern matching with `Option<Box<TreeNode>>` and recursion. It also provides an excellent
//! opportunity to explore different algorithmic approaches in Rust, contrasting a recursive brute-force search with an
//! optimal O(M+N) time string serialization approach using `String::with_capacity` and `write!` macro.
//!
//! ## Approach
//!
//! We provide two approaches here:
//!
//! 1.  **Recursive Brute Force (Standard Approach):**
//!     -   Traverse the main tree `root`. For every node, check if the tree starting at that node is identical to `subRoot`.
//!     -   This involves a helper function `is_same_tree` that recursively compares two trees.
//!     -   Time Complexity: O(M * N), where M is the number of nodes in `root` and N is the number of nodes in `subRoot`. In the worst case, for every node in `root`, we might check all nodes in `subRoot`.
//!     -   Space Complexity: O(M + N) worst-case recursion depth.
//!
//! 2.  **String Serialization (Optimized Approach):**
//!     -   Serialize both trees into strings. A preorder traversal is commonly used. To uniquely identify structure, we must include null nodes (e.g., as `#`) and add specific node boundary markers (e.g., `^val$`).
//!     -   Then, check if the serialized `subRoot` string is a substring of the serialized `root` string.
//!     -   **RUST INSIGHT:** To make this truly zero-allocation (beyond the initial two strings), we pre-allocate the strings using `String::with_capacity` and append to them iteratively using the `write!` macro instead of `format!`.
//!     -   Time Complexity: O(M + N) to serialize, plus the substring search time. Modern substring search algorithms (like KMP, or Rust's highly optimized standard library implementation) typically run in O(M + N) time.
//!     -   Space Complexity: O(M + N) to store the serialized strings.
//!
//! ## Alternative Approaches
//!
//! -   **Merkle Hashing:** Compute a hash for every subtree bottom-up. This can also achieve O(M + N) time but requires carefully handling hash collisions. It's more complex to implement correctly than serialization.

use std::fmt::Write;

// Definition for a binary tree node.
#[derive(Debug, PartialEq, Eq)]
#[derive(Clone)]
pub struct TreeNode {
    pub val: i32,
    pub left: Option<Box<TreeNode>>,
    pub right: Option<Box<TreeNode>>,
}

impl TreeNode {
    #[inline]
    pub fn new(val: i32) -> Self {
        TreeNode {
            val,
            left: None,
            right: None,
        }
    }
}

pub struct Solution;

impl Solution {
    /// Recursive Brute Force Approach
    /// O(M * N) Time, O(max(M, N)) Space
    pub fn is_subtree_recursive(
        root: Option<Box<TreeNode>>,
        sub_root: Option<Box<TreeNode>>,
    ) -> bool {
        Self::is_subtree_recursive_helper(root.as_deref(), sub_root.as_deref())
    }

    fn is_subtree_recursive_helper(root: Option<&TreeNode>, sub_root: Option<&TreeNode>) -> bool {
        if root.is_none() && sub_root.is_none() {
            return true;
        }
        if sub_root.is_none() {
            return true;
        }
        if root.is_none() {
            return false;
        }

        // RUST INSIGHT: We use `as_deref()` to convert `&Option<Box<TreeNode>>` to `Option<&TreeNode>`.
        // This is idiomatic Rust: passing references rather than taking ownership or cloning when we just need to read.
        if Self::is_same_tree(root, sub_root) {
            return true;
        }

        let root_node = root.unwrap();
        Self::is_subtree_recursive_helper(root_node.left.as_deref(), sub_root)
            || Self::is_subtree_recursive_helper(root_node.right.as_deref(), sub_root)
    }

    fn is_same_tree(p: Option<&TreeNode>, q: Option<&TreeNode>) -> bool {
        match (p, q) {
            (None, None) => true,
            // GOTCHA: Pattern matching `(Some(p_node), Some(q_node))` safely destructures the nodes,
            // avoiding unsafe `unwrap()` calls and null pointer exceptions entirely.
            (Some(p_node), Some(q_node)) => {
                p_node.val == q_node.val
                    && Self::is_same_tree(p_node.left.as_deref(), q_node.left.as_deref())
                    && Self::is_same_tree(p_node.right.as_deref(), q_node.right.as_deref())
            }
            _ => false,
        }
    }

    /// String Serialization Approach (Optimized)
    /// O(M + N) Time, O(M + N) Space
    pub fn is_subtree(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
        // Pre-allocate strings. We estimate sizes, though they will grow if needed.
        // A node representation `^val$` takes roughly 4-15 bytes. Let's assume a reasonable upper bound for LeetCode constraints (2000 nodes).
        let mut root_str = String::with_capacity(2048);
        let mut sub_root_str = String::with_capacity(1024);

        Self::serialize(root.as_deref(), &mut root_str);
        Self::serialize(sub_root.as_deref(), &mut sub_root_str);

        // Standard library contains() uses an optimized substring search.
        root_str.contains(&sub_root_str)
    }

    fn serialize(node: Option<&TreeNode>, buf: &mut String) {
        match node {
            None => {
                // RUST INSIGHT: Using `write!` macro instead of `format!` or string concatenation.
                // This appends directly to the pre-allocated buffer without intermediate heap allocations.
                let _ = write!(buf, "#");
            }
            Some(n) => {
                // GOTCHA: When serializing binary trees to strings to perform subtree matching via substring search,
                // avoid naively prepending a global 'start of tree' marker (like `^`) to the subtree string,
                // as it will prevent matching subtrees that are not the root.
                // Use proper node boundary markers instead (e.g., `^val$`).
                // Example: tree [12] and subtree [2].
                // Preorder without boundaries: "#12##" and "#2##" -> "#12##" contains "#2##", which is wrong!
                // Preorder with boundaries: "^12$#" and "^2$#" -> "^12$#" does not contain "^2$#", which is correct!
                let _ = write!(buf, "^{}$", n.val);
                Self::serialize(n.left.as_deref(), buf);
                Self::serialize(n.right.as_deref(), buf);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to build trees easily
    fn node(val: i32, left: Option<Box<TreeNode>>, right: Option<Box<TreeNode>>) -> Option<Box<TreeNode>> {
        Some(Box::new(TreeNode { val, left, right }))
    }

    fn leaf(val: i32) -> Option<Box<TreeNode>> {
        node(val, None, None)
    }

    #[test]
    fn test_happy_path() {
        // root: [3,4,5,1,2]
        let root = node(
            3,
            node(4, leaf(1), leaf(2)),
            leaf(5),
        );
        // subRoot: [4,1,2]
        let sub_root = node(4, leaf(1), leaf(2));

        assert!(Solution::is_subtree(root.clone(), sub_root.clone()));
        assert!(Solution::is_subtree_recursive(root, sub_root));
    }

    #[test]
    fn test_edge_case_not_subtree_due_to_extra_leaf() {
        // root: [3,4,5,1,2,null,null,null,null,0]
        let root = node(
            3,
            node(4, leaf(1), node(2, leaf(0), None)),
            leaf(5),
        );
        // subRoot: [4,1,2]
        let sub_root = node(4, leaf(1), leaf(2));

        assert!(!Solution::is_subtree(root.clone(), sub_root.clone()));
        assert!(!Solution::is_subtree_recursive(root, sub_root));
    }

    #[test]
    fn test_boundary_case_same_tree() {
        // root: [1,2,3]
        let root = node(1, leaf(2), leaf(3));
        // subRoot: [1,2,3]
        let sub_root = node(1, leaf(2), leaf(3));

        assert!(Solution::is_subtree(root.clone(), sub_root.clone()));
        assert!(Solution::is_subtree_recursive(root, sub_root));
    }

    #[test]
    fn test_boundary_case_value_substring_trap() {
        // This test ensures our boundary markers (^ and $) prevent false positives.
        // root: [12]
        let root = leaf(12);
        // subRoot: [2]
        let sub_root = leaf(2);

        // Without boundaries, serialization might look like:
        // root: "12##"
        // sub:  "2##"
        // "12##" contains "2##", which is wrong.
        assert!(!Solution::is_subtree(root.clone(), sub_root.clone()));
        assert!(!Solution::is_subtree_recursive(root, sub_root));
    }
}
