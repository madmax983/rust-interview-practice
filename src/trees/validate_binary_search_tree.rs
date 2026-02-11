//! # 98. Validate Binary Search Tree
//!
//! Link: <https://leetcode.com/problems/validate-binary-search-tree/>
//!
//! Given the root of a binary tree, determine if it is a valid binary search tree (BST).
//!
//! A valid BST is defined as follows:
//! - The left subtree of a node contains only nodes with keys **less than** the node's key.
//! - The right subtree of a node contains only nodes with keys **greater than** the node's key.
//! - Both the left and right subtrees must also be binary search trees.
//!
//! This problem is a classic example of recursive validation. In Rust, it highlights
//! how `Option` types can elegantly represent "unbounded" ranges (infinity/negative infinity)
//! without resorting to potentially unsafe sentinel values like `i64::MIN` or tricky
//! `i32::MAX` comparisons.
//!
//! ## Examples
//!
//! ```
//! use leetcode::trees::validate_binary_search_tree::{is_valid_bst, TreeNode};
//!
//! // Valid BST:
//! //      2
//! //     / \
//! //    1   3
//! let mut root = TreeNode::new(2);
//! root.left = Some(Box::new(TreeNode::new(1)));
//! root.right = Some(Box::new(TreeNode::new(3)));
//! assert!(is_valid_bst(Some(Box::new(root))));
//!
//! // Invalid BST:
//! //      5
//! //     / \
//! //    1   4  <-- 4 is not > 5
//! //       / \
//! //      3   6
//! let mut root = TreeNode::new(5);
//! root.left = Some(Box::new(TreeNode::new(1)));
//! let mut right = TreeNode::new(4);
//! right.left = Some(Box::new(TreeNode::new(3)));
//! right.right = Some(Box::new(TreeNode::new(6)));
//! root.right = Some(Box::new(right));
//! assert!(!is_valid_bst(Some(Box::new(root))));
//! ```
//!
//! ## Constraints
//!
//! - The number of nodes in the tree is in the range `[1, 10^4]`.
//! - `-2^31 <= Node.val <= 2^31 - 1`

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

/// Validates if the tree is a valid BST.
///
/// This implementation uses a recursive helper function that carries the valid range `(min, max)`
/// down the tree. `None` in the range represents negative or positive infinity.
///
/// Time: O(n) - We visit every node exactly once.
/// Space: O(h) - Recursion stack depth equals tree height (O(n) worst case, O(log n) average).
///
/// # Rust Insight
/// Using `Option<i32>` for bounds avoids the need for `i64` or sentinel values.
/// - `min: None` means "negative infinity".
/// - `max: None` means "positive infinity".
///
/// Pattern matching makes handling these cases explicit and robust.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_valid_bst(root: Option<Box<TreeNode>>) -> bool {
    validate(root.as_deref(), None, None)
}

/// Helper function to validate a node against a range (min, max).
///
/// - `node`: The current node to check.
/// - `min`: The strict lower bound (exclusive). `None` implies no lower bound.
/// - `max`: The strict upper bound (exclusive). `None` implies no upper bound.
fn validate(node: Option<&TreeNode>, min: Option<i32>, max: Option<i32>) -> bool {
    match node {
        // Base case: An empty tree is a valid BST.
        None => true,
        Some(n) => {
            // Check lower bound violation
            if matches!(min, Some(min_val) if n.val <= min_val) {
                return false;
            }

            // Check upper bound violation
            if matches!(max, Some(max_val) if n.val >= max_val) {
                return false;
            }

            // GOTCHA: It's easy to accidentally swap the bounds or use the wrong values.
            // - For the left child, the new MAX is the current node's value. Min stays same.
            // - For the right child, the new MIN is the current node's value. Max stays same.
            // Also, bounds are exclusive for a standard BST (no duplicates).

            // Recurse left: strict upper bound is current node val
            let left_valid = validate(n.left.as_deref(), min, Some(n.val));

            // Recurse right: strict lower bound is current node val
            let right_valid = validate(n.right.as_deref(), Some(n.val), max);

            left_valid && right_valid
        }
    }
}

/// Alternative Approach: In-Order Traversal
///
/// Perform an in-order traversal and verify that the values are strictly increasing.
/// This works because an in-order traversal of a valid BST always yields sorted values.
///
/// While conceptually simpler, it requires tracking the "previous" value.
/// In Rust, this can be done with a mutable reference to `Option<i32>` passed through the recursion.
#[allow(dead_code)]
fn is_valid_bst_inorder(root: Option<&TreeNode>) -> bool {
    fn inorder(node: Option<&TreeNode>, prev: &mut Option<i32>) -> bool {
        if let Some(n) = node {
            if !inorder(n.left.as_deref(), prev) {
                return false;
            }

            // Check if current value is strictly greater than previous
            if matches!(*prev, Some(p) if n.val <= p) {
                return false;
            }
            *prev = Some(n.val);

            return inorder(n.right.as_deref(), prev);
        }
        true
    }

    let mut prev = None;
    inorder(root, &mut prev)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create a leaf node
    fn leaf(val: i32) -> Option<Box<TreeNode>> {
        Some(Box::new(TreeNode::new(val)))
    }

    #[test]
    fn test_valid_bst_simple() {
        //      2
        //     / \
        //    1   3
        let mut root = TreeNode::new(2);
        root.left = leaf(1);
        root.right = leaf(3);
        assert!(is_valid_bst(Some(Box::new(root))));
    }

    #[test]
    fn test_invalid_bst_simple() {
        //      5
        //     / \
        //    1   4  <-- invalid
        let mut root = TreeNode::new(5);
        root.left = leaf(1);
        let mut right = TreeNode::new(4);
        right.left = leaf(3);
        right.right = leaf(6);
        root.right = Some(Box::new(right));

        assert!(!is_valid_bst(Some(Box::new(root))));
    }

    #[test]
    fn test_invalid_bst_deep() {
        // A tricky case where a node violates the grand-parent constraint but not the parent.
        //      5
        //     / \
        //    4   6
        //       / \
        //      3   7
        //
        // 3 is < 6 (valid local parent)
        // BUT 3 is < 5 (invalid grandparent constraint for right subtree)
        let mut root = TreeNode::new(5);
        root.left = leaf(4);

        let mut right = TreeNode::new(6);
        right.left = leaf(3); // Violation: 3 is in right subtree of 5
        right.right = leaf(7);
        root.right = Some(Box::new(right));

        assert!(!is_valid_bst(Some(Box::new(root))));
    }

    #[test]
    fn test_single_node() {
        assert!(is_valid_bst(leaf(1)));
    }

    #[test]
    fn test_empty_tree() {
        assert!(is_valid_bst(None));
    }

    #[test]
    fn test_i32_limits() {
        // Tree with i32::MAX
        //      MAX
        //     /
        //   MAX-1
        let mut root = TreeNode::new(i32::MAX);
        root.left = leaf(i32::MAX - 1);
        assert!(is_valid_bst(Some(Box::new(root))));

        // Tree with i32::MIN
        //      MIN
        //         \
        //        MIN+1
        let mut root = TreeNode::new(i32::MIN);
        root.right = leaf(i32::MIN + 1);
        assert!(is_valid_bst(Some(Box::new(root))));

        // Invalid limits
        //      MAX
        //         \
        //        MAX (duplicate)
        let mut root = TreeNode::new(i32::MAX);
        root.right = leaf(i32::MAX); // Equal not allowed
        assert!(!is_valid_bst(Some(Box::new(root))));
    }

    #[test]
    fn test_inorder_alternative() {
         //      2
        //     / \
        //    1   3
        let mut root = TreeNode::new(2);
        root.left = leaf(1);
        root.right = leaf(3);
        assert!(is_valid_bst_inorder(Some(&root)));
    }
}
