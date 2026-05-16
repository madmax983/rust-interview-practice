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
//! This problem naturally builds upon the "Same Tree" (LeetCode #100) logic. In Rust, it reinforces
//! pattern matching with `Option<Box<TreeNode>>` and showcases how recursion can elegantly traverse
//! and compare nested structures without manual pointer management.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::trees::subtree_of_another_tree::{is_subtree, TreeNode};
//!
//! let mut root = TreeNode::new(3);
//! let mut left = TreeNode::new(4);
//! left.left = Some(Box::new(TreeNode::new(1)));
//! left.right = Some(Box::new(TreeNode::new(2)));
//! root.left = Some(Box::new(left));
//! root.right = Some(Box::new(TreeNode::new(5)));
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
/// Identical to LeetCode #100 "Same Tree".
#[must_use]
pub fn is_same_tree(p: &Option<Box<TreeNode>>, q: &Option<Box<TreeNode>>) -> bool {
    // RUST INSIGHT: We borrow the Options to avoid consuming the trees, which we might
    // need to traverse further if they don't match.
    match (p, q) {
        (Some(node_p), Some(node_q)) => {
            node_p.val == node_q.val
                && is_same_tree(&node_p.left, &node_q.left)
                && is_same_tree(&node_p.right, &node_q.right)
        }
        (None, None) => true,
        _ => false,
    }
}

/// Brute Force approach: Traverse and compare
///
/// Time: O(M * N) - Where M is the number of nodes in `root` and N is the number of nodes in `subRoot`.
///                  In the worst case, we compare `subRoot` with every node in `root`.
/// Space: O(H_M + H_N) - Where H_M and H_N are the heights of `root` and `subRoot` respectively,
///                       representing the maximum recursion stack depth.
///
/// For every node in the `root` tree, we perform a full check to see if the subtree starting at
/// that node is identical to `subRoot`.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_brute_force(
    root: Option<Box<TreeNode>>,
    sub_root: Option<Box<TreeNode>>,
) -> bool {
    // We pass references to avoid taking ownership during the recursive checks.
    fn helper(root: &Option<Box<TreeNode>>, sub_root: &Option<Box<TreeNode>>) -> bool {
        match root {
            None => false, // We've reached a leaf and haven't found a match
            Some(node) => {
                // If the current tree matches sub_root, we are done
                if is_same_tree(root, sub_root) {
                    return true;
                }
                // Otherwise, check the left and right subtrees
                helper(&node.left, sub_root) || helper(&node.right, sub_root)
            }
        }
    }

    helper(&root, &sub_root)
}

/// Optimized approach: Serialization and Substring search
///
/// Time: O(M + N) - Serialization takes O(M + N). Substring matching (using KMP implicitly by `contains`) is theoretically O(M + N), but in Rust standard library it is highly optimized.
/// Space: O(M + N) - To store the serialized strings of both trees.
///
/// We serialize both trees into a string representation using pre-order traversal.
/// We must uniquely encode the structure (e.g., using markers like `^` before a node and padding nulls with `#`).
/// If `subRoot`'s serialized string is a substring of `root`'s serialized string, then `subRoot` is a subtree.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_optimized(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    fn serialize(node: &Option<Box<TreeNode>>, out: &mut String) {
        match node {
            None => out.push_str("#,"), // GOTCHA: Differentiate null nodes explicitly to preserve structure
            Some(n) => {
                // Add a unique prefix character to avoid false positive matches where
                // a leaf value "12" matches the end of value "112".
                out.push('^');
                out.push_str(&n.val.to_string());
                out.push(',');
                serialize(&n.left, out);
                serialize(&n.right, out);
            }
        }
    }

    let mut root_str = String::new();
    let mut sub_root_str = String::new();

    serialize(&root, &mut root_str);
    serialize(&sub_root, &mut sub_root_str);

    // RUST INSIGHT: `contains` on strings is very fast.
    root_str.contains(&sub_root_str)
}

/// Optimal approach: Same as brute force for this specific problem constraints
///
/// While the serialization approach has better theoretical time complexity O(M+N),
/// the constraints (M <= 2000, N <= 1000) mean the O(M*N) recursion is incredibly fast in practice
/// and avoids heap allocations for large strings.
/// Thus, the recursive approach is often preferred for its simplicity and zero-allocation nature.
#[must_use]
pub fn is_subtree_optimal(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    is_subtree_brute_force(root, sub_root)
}

/// Main entry point
#[must_use]
pub fn is_subtree(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    is_subtree_optimal(root, sub_root)
}

// Alternative Approaches:
// 1. **Merkle Hashing**: We could compute a structural hash for each subtree bottom-up.
//    If the hash of any subtree in `root` matches the hash of `subRoot`, we can verify equality.
//    This is O(M + N) time and O(M + N) space, useful for extremely large trees.

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create a leaf node
    fn leaf(val: i32) -> Option<Box<TreeNode>> {
        Some(Box::new(TreeNode::new(val)))
    }

    #[test]
    fn test_is_subtree_true() {
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
        let mut sub_root = TreeNode::new(4);
        sub_root.left = leaf(1);
        sub_root.right = leaf(2);

        let root_box = Some(Box::new(root));
        let sub_root_box = Some(Box::new(sub_root));

        assert!(is_subtree_brute_force(
            root_box.clone(),
            sub_root_box.clone()
        ));
        assert!(is_subtree_optimized(
            root_box.clone(),
            sub_root_box.clone()
        ));
        assert!(is_subtree_optimal(root_box, sub_root_box));
    }

    #[test]
    fn test_is_subtree_false() {
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
        let mut sub_root = TreeNode::new(4);
        sub_root.left = leaf(1);
        sub_root.right = leaf(2);

        let root_box = Some(Box::new(root));
        let sub_root_box = Some(Box::new(sub_root));

        assert!(!is_subtree_brute_force(
            root_box.clone(),
            sub_root_box.clone()
        ));
        assert!(!is_subtree_optimized(
            root_box.clone(),
            sub_root_box.clone()
        ));
        assert!(!is_subtree_optimal(root_box, sub_root_box));
    }

    #[test]
    fn test_same_tree_is_subtree() {
        // A tree is a subtree of itself
        let mut root = TreeNode::new(1);
        root.left = leaf(2);
        root.right = leaf(3);

        let root_box = Some(Box::new(root));
        let sub_root_box = root_box.clone();

        assert!(is_subtree_brute_force(
            root_box.clone(),
            sub_root_box.clone()
        ));
        assert!(is_subtree_optimized(
            root_box.clone(),
            sub_root_box.clone()
        ));
        assert!(is_subtree_optimal(root_box, sub_root_box));
    }

    #[test]
    fn test_false_positive_serialization_prevention() {
        // root:
        //    12
        let root = leaf(12);

        // subRoot:
        //    2
        let sub_root = leaf(2);

        // Without careful serialization separators (like `^12,` vs `^2,`), "2" might match within "12".
        assert!(!is_subtree_optimized(root.clone(), sub_root.clone()));
        assert!(!is_subtree_optimal(root, sub_root));
    }
}
