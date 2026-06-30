//! # 572. Subtree of Another Tree
//!
//! Difficulty: Easy
//! Link: <https://leetcode.com/problems/subtree-of-another-tree/>
//!
//! Given the roots of two binary trees `root` and `subRoot`, return `true` if there is a
//! subtree of `root` with the same structure and node values of `subRoot` and `false` otherwise.
//!
//! This problem provides a fantastic opportunity to explore Rust's recursive type definitions,
//! pattern matching, and efficient string operations. It highlights the use of `.as_deref()` for
//! borrowing inner values of `Option<Box<T>>` without consuming the outer `Option`.
//!
//! ## Approaches
//!
//! 1.  **Brute Force Search (Recursive)**:
//!     -   For every node in the main tree, check if the subtree rooted at this node is identical to `subRoot`.
//!     -   Time: O(M * N) where M is the number of nodes in `root` and N is the number of nodes in `subRoot`.
//!     -   Space: O(max(H_m, H_n)) where H_m and H_n are heights of the trees.
//! 2.  **Naive Serialization**:
//!     -   Serialize both trees into strings (e.g., using pre-order traversal) and check if the `subRoot` string is a substring of the `root` string.
//!     -   Time: O(M + N) but with high constant factors due to many intermediate string allocations.
//!     -   Space: O(M + N) for the strings.
//! 3.  **Optimal Serialization**:
//!     -   Similar to naive serialization, but avoids intermediate allocations by passing a single `String` buffer and using `write!`.
//!     -   Time: O(M + N) with much better performance in Rust.
//!     -   Space: O(M + N).
//!
//! ## Alternative Approaches
//!
//! - **Merkle Hashing**: Hash each subtree to quickly compare subtrees in O(1) time after an O(M+N) preprocessing step.

use std::fmt::Write;

// =========================================================================================
// Data Structures
// =========================================================================================

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

// =========================================================================================
// Approach 1: Brute Force Search
// =========================================================================================

/// Time: O(M * N)
/// Space: O(max(H_m, H_n))
#[must_use]
pub fn is_subtree_brute_force(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    // Helper function to check if two trees are identical.
    // RUST INSIGHT: We pass Option<&TreeNode> instead of &Option<Box<TreeNode>> to satisfy
    // clippy::pedantic and idiomatic borrow rules.
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

    fn search(r: Option<&TreeNode>, s: Option<&TreeNode>) -> bool {
        if r.is_none() {
            return false;
        }

        if is_same_tree(r, s) {
            return true;
        }

        let node = r.unwrap();
        // RUST INSIGHT: `.as_deref()` is used here to safely borrow the Box's contents
        // without taking ownership of the Option or its contents.
        search(node.left.as_deref(), s) || search(node.right.as_deref(), s)
    }

    search(root.as_deref(), sub_root.as_deref())
}

// =========================================================================================
// Approach 2: Naive String Serialization
// =========================================================================================

/// Time: O(M + N) with large constants due to intermediate strings
/// Space: O(M + N) for strings
#[must_use]
pub fn is_subtree_naive_serialize(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    fn serialize(node: Option<&TreeNode>) -> String {
        match node {
            // GOTCHA: We must use proper node boundary markers (like "#val#").
            // If we just concatenate "val", then node 12 could match node 2 within 12!
            Some(n) => format!("#{val}#{left}{right}",
                val = n.val,
                left = serialize(n.left.as_deref()),
                right = serialize(n.right.as_deref())
            ),
            None => "X".to_string(),
        }
    }

    let root_str = serialize(root.as_deref());
    let sub_root_str = serialize(sub_root.as_deref());

    // RUST INSIGHT: We do NOT prepend a global start marker like `^` to sub_root_str.
    // If we did, `root_str.contains(&sub_root_str)` would fail if the subtree is not
    // at the very root of the main tree.
    root_str.contains(&sub_root_str)
}

// =========================================================================================
// Approach 3: Optimal Zero-Allocation Serialization
// =========================================================================================

/// Time: O(M + N)
/// Space: O(M + N) for the final strings, but zero intermediate allocations
#[must_use]
pub fn is_subtree_optimal(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    fn serialize_fast(node: Option<&TreeNode>, buf: &mut String) {
        match node {
            Some(n) => {
                // write! macro appends directly to the pre-allocated string
                let _ = write!(buf, "#{}#", n.val);
                serialize_fast(n.left.as_deref(), buf);
                serialize_fast(n.right.as_deref(), buf);
            }
            None => {
                let _ = write!(buf, "X");
            }
        }
    }

    // Allocate strings with arbitrary capacity to minimize reallocations
    let mut root_str = String::with_capacity(1024);
    let mut sub_root_str = String::with_capacity(256);

    serialize_fast(root.as_deref(), &mut root_str);
    serialize_fast(sub_root.as_deref(), &mut sub_root_str);

    root_str.contains(&sub_root_str)
}

/// Main entry point - uses optimal solution
#[must_use]
pub fn is_subtree(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    is_subtree_optimal(root, sub_root)
}

// =========================================================================================
// Tests
// =========================================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn build_node(val: i32, left: Option<Box<TreeNode>>, right: Option<Box<TreeNode>>) -> Option<Box<TreeNode>> {
        Some(Box::new(TreeNode { val, left, right }))
    }

    fn leaf(val: i32) -> Option<Box<TreeNode>> {
        build_node(val, None, None)
    }

    #[test]
    fn test_happy_path() {
        // root: [3,4,5,1,2]
        let root = build_node(
            3,
            build_node(4, leaf(1), leaf(2)),
            leaf(5)
        );
        // subRoot: [4,1,2]
        let sub_root = build_node(4, leaf(1), leaf(2));

        assert!(is_subtree_brute_force(root.clone(), sub_root.clone()));
        assert!(is_subtree_naive_serialize(root.clone(), sub_root.clone()));
        assert!(is_subtree_optimal(root.clone(), sub_root.clone()));
        assert!(is_subtree(root, sub_root));
    }

    #[test]
    fn test_edge_case_not_subtree_due_to_extra_leaf() {
        // root: [3,4,5,1,2,null,null,null,null,0]
        let root = build_node(
            3,
            build_node(
                4,
                leaf(1),
                build_node(2, leaf(0), None)
            ),
            leaf(5)
        );
        // subRoot: [4,1,2]
        let sub_root = build_node(4, leaf(1), leaf(2));

        assert!(!is_subtree_brute_force(root.clone(), sub_root.clone()));
        assert!(!is_subtree_naive_serialize(root.clone(), sub_root.clone()));
        assert!(!is_subtree_optimal(root.clone(), sub_root.clone()));
        assert!(!is_subtree(root, sub_root));
    }

    #[test]
    fn test_boundary_case_same_values_but_different_structure() {
        // root: [1,1]
        let root = build_node(1, leaf(1), None);
        // subRoot: [1]
        let sub_root = leaf(1);

        assert!(is_subtree_brute_force(root.clone(), sub_root.clone()));
        assert!(is_subtree_naive_serialize(root.clone(), sub_root.clone()));
        assert!(is_subtree_optimal(root.clone(), sub_root.clone()));
        assert!(is_subtree(root, sub_root));
    }

    #[test]
    fn test_boundary_case_identical_trees() {
        // root: [1,2,3], subRoot: [1,2,3]
        let root = build_node(1, leaf(2), leaf(3));
        let sub_root = build_node(1, leaf(2), leaf(3));

        assert!(is_subtree(root, sub_root));
    }

    #[test]
    fn test_stress_case_string_boundaries() {
        // Node 12 vs Node 2. Ensure they don't incorrectly match.
        // root: [12]
        let root = leaf(12);
        // subRoot: [2]
        let sub_root = leaf(2);

        assert!(!is_subtree(root, sub_root));
    }
}
