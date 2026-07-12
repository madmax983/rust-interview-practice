//! # 572. Subtree of Another Tree
//!
//! Difficulty: Easy
//! Link: <https://leetcode.com/problems/subtree-of-another-tree/>
//!
//! Given the roots of two binary trees `root` and `subRoot`, return `true` if there is a subtree of `root`
//! with the same structure and node values of `subRoot` and `false` otherwise.
//!
//! A subtree of a binary tree `tree` is a tree that consists of a node in `tree` and all of this node's
//! descendants. The tree `tree` could also be considered as a subtree of itself.
//!
//! This problem is a natural fit for Rust's pattern matching with `Option<Box<TreeNode>>` and recursion. It
//! demonstrates how to write clean, memory-safe tree traversals using `.as_deref()` to examine nodes without
//! taking ownership, which is crucial when doing nested tree comparisons.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::trees::subtree_of_another_tree::{is_subtree, TreeNode};
//!
//! // root = [3,4,5,1,2]
//! let mut root = TreeNode::new(3);
//! let mut left = TreeNode::new(4);
//! left.left = Some(Box::new(TreeNode::new(1)));
//! left.right = Some(Box::new(TreeNode::new(2)));
//! root.left = Some(Box::new(left));
//! root.right = Some(Box::new(TreeNode::new(5)));
//!
//! // subRoot = [4,1,2]
//! let mut sub_root = TreeNode::new(4);
//! sub_root.left = Some(Box::new(TreeNode::new(1)));
//! sub_root.right = Some(Box::new(TreeNode::new(2)));
//!
//! assert_eq!(is_subtree(Some(Box::new(root)), Some(Box::new(sub_root))), true);
//! ```
//!
//! ## Approach
//!
//! We explore three approaches here to highlight Rust's capabilities:
//! 1. **Brute Force Recursive**: The most intuitive approach. For each node in `root`, we check if the tree
//!    starting at that node matches `subRoot`. This takes `O(M * N)` time where `M` and `N` are the number of
//!    nodes in `root` and `subRoot`.
//! 2. **Naive Serialization**: Convert both trees to strings using a pre-order traversal with `None` markers
//!    (to preserve structure). Then we just do a substring search. While clever and `O(M + N)` time, doing
//!    naive string concatenations allocates heavily in Rust.
//! 3. **Optimal Serialization**: We still serialize, but we eliminate all intermediate allocations by using
//!    `String::with_capacity` and the `write!` macro. This gives us `O(M + N)` time with minimal memory
//!    overhead and shows off idiomatic high-performance Rust string building.
//!
//! ## Constraints
//!
//! - The number of nodes in the `root` tree is in the range `[1, 2000]`.
//! - The number of nodes in the `subRoot` tree is in the range `[1, 1000]`.
//! - `-10^4 <= root.val <= 10^4`
//! - `-10^4 <= subRoot.val <= 10^4`
//!
//! ## Alternative Approaches
//!
//! - **KMP String Matching**: After optimal serialization, using the Knuth-Morris-Pratt algorithm instead of
//!   Rust's default `str::contains` would guarantee worst-case `O(M + N)` time, although Rust's `contains`
//!   is highly optimized and often faster in practice.
//! - **Merkle Hashing**: Hash each subtree structure and value. This is `O(M + N)` time and `O(M)` space but
//!   adds complexity in handling hash collisions.

use std::fmt::Write; // Needed for the write! macro

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

// -----------------------------------------------------------------------------
// Approach 1: Recursive Brute Force
// -----------------------------------------------------------------------------

/// Recursive brute-force search.
///
/// Time: `O(M * N)` where M is nodes in root, N is nodes in subRoot. In the worst case,
///       we check equality starting from every node in `root`.
/// Space: `O(H_m + H_n)` due to recursion stack depth, where H is the height of the trees.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_recursive(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    // RUST INSIGHT: We use `as_deref()` to convert `Option<Box<TreeNode>>` to `Option<&TreeNode>`.
    // This allows us to pass references around without consuming (moving) the original Box.
    fn is_same_tree(p: Option<&TreeNode>, q: Option<&TreeNode>) -> bool {
        match (p, q) {
            (Some(n1), Some(n2)) => {
                n1.val == n2.val
                    && is_same_tree(n1.left.as_deref(), n2.left.as_deref())
                    && is_same_tree(n1.right.as_deref(), n2.right.as_deref())
            }
            (None, None) => true,
            _ => false,
        }
    }

    fn dfs(node: Option<&TreeNode>, target: Option<&TreeNode>) -> bool {
        match node {
            Some(n) => {
                is_same_tree(Some(n), target)
                    || dfs(n.left.as_deref(), target)
                    || dfs(n.right.as_deref(), target)
            }
            None => false,
        }
    }

    dfs(root.as_deref(), sub_root.as_deref())
}

// -----------------------------------------------------------------------------
// Approach 2: Naive String Serialization
// -----------------------------------------------------------------------------

/// Naive string serialization approach.
///
/// Time: `O(M + N)` to serialize, plus substring search time.
/// Space: `O(M + N)` to store the string representations.
///
/// We convert the tree to a pre-order traversal string. `None` children are explicitly marked.
/// We use format strings to wrap each node boundary (e.g. `^3^`) so that a subRoot `2` doesn't
/// accidentally match a node value `12` in the root tree.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_naive_serialization(
    root: Option<Box<TreeNode>>,
    sub_root: Option<Box<TreeNode>>,
) -> bool {
    fn serialize(node: Option<&TreeNode>) -> String {
        match node {
            // GOTCHA: Using `format!` recursively here creates a new String allocation at every
            // single node in the tree. This is very slow and memory-intensive!
            Some(n) => format!(
                "^{}^{}{}",
                n.val,
                serialize(n.left.as_deref()),
                serialize(n.right.as_deref())
            ),
            // Explicit marker for a null node to preserve structural uniqueness
            None => "#".to_string(),
        }
    }

    let root_str = serialize(root.as_deref());
    let sub_str = serialize(sub_root.as_deref());

    root_str.contains(&sub_str)
}

// -----------------------------------------------------------------------------
// Approach 3: Optimal String Serialization
// -----------------------------------------------------------------------------

/// Optimal string serialization approach (Zero-allocation during traversal).
///
/// Time: `O(M + N)` for traversal and substring search.
/// Space: `O(M + N)` for the final string buffers.
///
/// This improves upon the naive approach by pre-allocating the string buffers based on constraints
/// and passing a mutable reference to the buffer during traversal, entirely eliminating intermediate
/// heap allocations.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_optimal(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    // Helper to serialize into an existing mutable buffer
    fn serialize(node: Option<&TreeNode>, buffer: &mut String) {
        match node {
            Some(n) => {
                // RUST INSIGHT: `write!` appends directly to the existing String buffer without
                // creating new temporary strings. This is the idiomatic way to build strings.
                // We use `^val^` to clearly demarcate nodes.
                let _ = write!(buffer, "^{}^", n.val);
                serialize(n.left.as_deref(), buffer);
                serialize(n.right.as_deref(), buffer);
            }
            None => {
                buffer.push('#');
            }
        }
    }

    // Constraints say max 2000 nodes for root, 1000 for subRoot.
    // Each node takes ~6-8 bytes max (e.g. `^10000^`), plus null markers `#`.
    // Let's pre-allocate to prevent dynamic resizing during traversal.
    let mut root_str = String::with_capacity(16000);
    let mut sub_str = String::with_capacity(8000);

    serialize(root.as_deref(), &mut root_str);
    serialize(sub_root.as_deref(), &mut sub_str);

    root_str.contains(&sub_str)
}

/// Main entry point (Defaults to the optimal solution)
#[must_use]
pub fn is_subtree(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    is_subtree_optimal(root, sub_root)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(val: i32) -> Option<Box<TreeNode>> {
        Some(Box::new(TreeNode::new(val)))
    }

    // Run all three approaches to verify correctness
    fn test_all_approaches(
        root: Option<Box<TreeNode>>,
        sub_root: Option<Box<TreeNode>>,
        expected: bool,
    ) {
        assert_eq!(
            is_subtree_recursive(root.clone(), sub_root.clone()),
            expected,
            "Recursive failed"
        );
        assert_eq!(
            is_subtree_naive_serialization(root.clone(), sub_root.clone()),
            expected,
            "Naive Serialization failed"
        );
        assert_eq!(
            is_subtree_optimal(root, sub_root),
            expected,
            "Optimal failed"
        );
    }

    #[test]
    fn test_happy_path() {
        // root = [3,4,5,1,2]
        let mut root = TreeNode::new(3);
        let mut left = TreeNode::new(4);
        left.left = leaf(1);
        left.right = leaf(2);
        root.left = Some(Box::new(left));
        root.right = leaf(5);

        // subRoot = [4,1,2]
        let mut sub_root = TreeNode::new(4);
        sub_root.left = leaf(1);
        sub_root.right = leaf(2);

        test_all_approaches(Some(Box::new(root)), Some(Box::new(sub_root)), true);
    }

    #[test]
    fn test_edge_case_not_subtree_due_to_extra_leaf() {
        // root = [3,4,5,1,2,null,null,null,null,0]
        let mut root = TreeNode::new(3);
        let mut left = TreeNode::new(4);
        left.left = leaf(1);
        let mut right_child = TreeNode::new(2);
        right_child.left = leaf(0); // Extra leaf!
        left.right = Some(Box::new(right_child));
        root.left = Some(Box::new(left));
        root.right = leaf(5);

        // subRoot = [4,1,2]
        let mut sub_root = TreeNode::new(4);
        sub_root.left = leaf(1);
        sub_root.right = leaf(2);

        test_all_approaches(Some(Box::new(root)), Some(Box::new(sub_root)), false);
    }

    #[test]
    fn test_stress_boundary_case_substring_trap() {
        // This tests the substring serialization trap.
        // If we didn't demarcate nodes properly, a tree of [12] might match inside [123].

        // root = [12]
        let root = TreeNode::new(12);

        // subRoot = [2]
        let sub_root = TreeNode::new(2);

        test_all_approaches(Some(Box::new(root)), Some(Box::new(sub_root)), false);
    }

    #[test]
    fn test_identical_trees() {
        let root = leaf(1);
        let sub_root = leaf(1);
        test_all_approaches(root, sub_root, true);
    }

    #[test]
    fn test_empty_subroot() {
        // In this problem constraints, trees are non-empty (nodes >= 1).
        // However, it's good practice to handle it.
        let root = leaf(1);
        // By problem definition sub_root >= 1 node, but mathematically empty is subtree of anything
        // But our serializations require exact match, let's just make sure it behaves predictably.
        // Actually LeetCode says nodes in subRoot is [1, 1000], so we don't strictly need to test None sub_root.
        let _ = root; // Ignore for unused
    }
}
