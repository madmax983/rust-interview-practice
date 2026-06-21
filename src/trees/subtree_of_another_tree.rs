//! # 572. Subtree of Another Tree
//!
//! Difficulty: Easy
//! Link: <https://leetcode.com/problems/subtree-of-another-tree/>
//!
//! Given the roots of two binary trees `root` and `subRoot`, return `true` if there is a subtree
//! of `root` with the same structure and node values of `subRoot` and `false` otherwise.
//!
//! A subtree of a binary tree `tree` is a tree that consists of a node in `tree` and all of this node's descendants.
//! The tree `tree` could also be considered as a subtree of itself.
//!
//! This problem provides a great opportunity to explore tree traversal and pattern matching in Rust.
//! It highlights how recursive algorithms naturally map to Rust's `Option<Box<TreeNode>>` and showcases
//! alternative approaches like string serialization to optimize matching.
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
#[derive(Debug, PartialEq, Eq)]
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

/// Helper function for brute-force approach to check if two trees are identical.
///
/// We use Rust's powerful `match` statement on a tuple of two Options to safely and
/// exhaustively handle all cases without fear of null pointer dereferencing.
fn is_same_tree(p: Option<&TreeNode>, q: Option<&TreeNode>) -> bool {
    // RUST INSIGHT: Matching on a tuple of references cleanly handles parallel traversal.
    // The compiler enforces that we cover all combinations.
    match (p, q) {
        (Some(n1), Some(n2)) => {
            n1.val == n2.val && is_same_tree(n1.left.as_deref(), n2.left.as_deref()) && is_same_tree(n1.right.as_deref(), n2.right.as_deref())
        }
        (None, None) => true,
        _ => false, // One is Some, the other is None
    }
}

/// Brute force approach: Recursive Search
///
/// For every node in the main tree (`root`), we check if the tree starting at that node
/// is identical to `subRoot`.
///
/// Time: O(M * N) where M is the number of nodes in `root` and N is the number of nodes in `subRoot`.
/// In the worst case, we might need to check if they are identical at every node.
/// Space: O(H) where H is the height of the `root` tree (max recursion depth).
///
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_brute_force(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    fn is_subtree_helper(root: Option<&TreeNode>, sub_root: Option<&TreeNode>) -> bool {
        if sub_root.is_none() {
            return true;
        }
        if root.is_none() {
            return false;
        }

        // RUST INSIGHT: We borrow the tree here so we don't consume it if `is_same_tree` fails,
        // allowing us to continue recursing down the left and right children.
        if is_same_tree(root, sub_root) {
            return true;
        }

        let root_node = root.unwrap();
        is_subtree_helper(root_node.left.as_deref(), sub_root) ||
        is_subtree_helper(root_node.right.as_deref(), sub_root)
    }

    is_subtree_helper(root.as_deref(), sub_root.as_deref())
}


/// Optimized approach: String Serialization
///
/// We can serialize both trees into strings using a preorder traversal. If `subRoot` is a subtree
/// of `root`, its serialized string must be a substring of the `root`'s serialized string.
/// We must include markers for null nodes and boundaries to prevent false positives
/// (e.g., node 12 containing subtree 2).
///
/// Time: O(M + N) - Serializing takes O(M + N) and string search (substring) takes O(M * N) naive or O(M + N) KMP.
/// Rust's `str::contains` uses a fast substring search algorithm (often Two-Way), making this very efficient.
/// Space: O(M + N) - to store the serialized strings.
///
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_optimized(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    // Helper to perform preorder serialization.
    // We use a mutable String reference to build the result without intermediate allocations.
    fn serialize(node: Option<&TreeNode>, out: &mut String) {
        match node {
            Some(n) => {
                // Prepend a marker '^' to unambiguously identify the start of a value.
                // RUST INSIGHT: write! macro directly appends to the String buffer,
                // avoiding the cost of `format!()` which would allocate temporary strings.
                let _ = write!(out, "^{}", n.val);
                serialize(n.left.as_deref(), out);
                serialize(n.right.as_deref(), out);
            }
            None => {
                out.push('#');
            }
        }
    }

    let mut root_str = String::new();
    let mut sub_root_str = String::new();

    serialize(root.as_deref(), &mut root_str);
    serialize(sub_root.as_deref(), &mut sub_root_str);

    // GOTCHA: Just checking if `sub_root_str` is in `root_str` might fail if values overlap.
    // For example, finding `2` inside `12`. By prefixing values with a delimiter (e.g. `^`),
    // `^2` won't match inside `^12`. Also, explicitly marking null nodes ensures the structure matches.
    root_str.contains(&sub_root_str)
}


/// Main entry point
#[must_use]
pub fn is_subtree(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    // Default to the optimized string serialization approach for better expected time complexity.
    is_subtree_optimized(root, sub_root)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(val: i32) -> Option<Box<TreeNode>> {
        Some(Box::new(TreeNode::new(val)))
    }

    #[test]
    fn test_brute_force_example1() {
        // root: [3,4,5,1,2]
        let mut root = TreeNode::new(3);
        let mut left = TreeNode::new(4);
        left.left = leaf(1);
        left.right = leaf(2);
        root.left = Some(Box::new(left));
        root.right = leaf(5);

        // subRoot: [4,1,2]
        let mut sub = TreeNode::new(4);
        sub.left = leaf(1);
        sub.right = leaf(2);

        assert!(is_subtree_brute_force(Some(Box::new(root)), Some(Box::new(sub))));
    }

    #[test]
    fn test_optimized_example2() {
        // root: [3,4,5,1,2,null,null,null,null,0]
        let mut root = TreeNode::new(3);
        let mut left = TreeNode::new(4);
        left.left = leaf(1);
        let mut right = TreeNode::new(2);
        right.left = leaf(0); // This makes it NOT a subtree
        left.right = Some(Box::new(right));
        root.left = Some(Box::new(left));
        root.right = leaf(5);

        // subRoot: [4,1,2]
        let mut sub = TreeNode::new(4);
        sub.left = leaf(1);
        sub.right = leaf(2);

        assert!(!is_subtree_optimized(Some(Box::new(root)), Some(Box::new(sub))));
    }

    #[test]
    fn test_all_approaches_edge_cases() {
        // Edge case: subRoot is None (always a subtree)

        // Note: is_subtree takes ownership, so we need to construct new trees or avoid sharing.
        // It's just a test so we can do whatever.

        assert!(is_subtree_brute_force(Some(Box::new(TreeNode::new(1))), None));
        assert!(is_subtree_optimized(Some(Box::new(TreeNode::new(1))), None));

        // Edge case: Both are None
        assert!(is_subtree_brute_force(None, None));
        assert!(is_subtree_optimized(None, None));

        // Edge case: root is None, subRoot is Some
        assert!(!is_subtree_brute_force(None, Some(Box::new(TreeNode::new(1)))));
        assert!(!is_subtree_optimized(None, Some(Box::new(TreeNode::new(1)))));
    }

    #[test]
    fn test_overlap_gotcha() {
        // Tests the string serialization overlap issue.
        assert!(!is_subtree_brute_force(Some(Box::new(TreeNode::new(12))), Some(Box::new(TreeNode::new(2)))));
        assert!(!is_subtree_optimized(Some(Box::new(TreeNode::new(12))), Some(Box::new(TreeNode::new(2)))));
    }
}
