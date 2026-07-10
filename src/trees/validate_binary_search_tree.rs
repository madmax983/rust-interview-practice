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
//! This problem is a fundamental check for understanding recursive invariants and tree properties.
//! In Rust, it highlights the use of `Option` to represent unbounded ranges and strict ownership
//! handling during traversal.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::trees::validate_binary_search_tree::{is_valid_bst, TreeNode};
//!
//! let mut root = TreeNode::new(2);
//! root.left = Some(Box::new(TreeNode::new(1)));
//! root.right = Some(Box::new(TreeNode::new(3)));
//!
//! assert!(is_valid_bst(Some(Box::new(root))));
//! ```
//!
//! ## Constraints
//!
//! - The number of nodes in the tree is in the range `[1, 10_000]`.
//! - `-2^31 <= Node.val <= 2^31 - 1`

// Definition for a binary tree node.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct TreeNode {
    pub val: i32,
    pub left: Option<Box<TreeNode>>,
    pub right: Option<Box<TreeNode>>,
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

/// Brute Force approach: In-order Traversal to Vector
///
/// Perform an in-order traversal to collect all values into a vector.
/// A valid BST will produce a strictly increasing sequence.
///
/// Time: O(N) - Visit every node once.
/// Space: O(N) - Store all node values in a vector.
///
/// # Why this works
/// In-order traversal visits nodes in (Left, Root, Right) order. For a BST,
/// this naturally results in sorted order.
///
/// # Gotcha
/// Make sure to check for *strict* inequality. Duplicate values are not allowed
/// in a standard BST unless specified otherwise.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_valid_bst_brute_force(root: Option<Box<TreeNode>>) -> bool {
    fn inorder(node: Option<&Box<TreeNode>>, values: &mut Vec<i32>) {
        if let Some(n) = node {
            inorder(n.left.as_ref(), values);
            values.push(n.val);
            inorder(n.right.as_ref(), values);
        }
    }

    let mut values = Vec::new();
    inorder(root.as_ref(), &mut values);

    // Check strictly increasing
    // windows(2) gives us an iterator over overlapping pairs
    // RUST INSIGHT: `windows` is a zero-cost abstraction that lets us inspect
    // adjacent elements without manual index manipulation or bounds checking.
    values.windows(2).all(|w| w[0] < w[1])
}

/// Optimized approach: recursive valid-range propagation
///
/// We traverse the tree, passing down the valid range (min, max) for each node.
/// - When going left, the max value becomes the current node's value.
/// - When going right, the min value becomes the current node's value.
///
/// This improves on the brute force by using O(H) stack space instead of O(N) for a
/// materialized value vector, and short-circuits on the first violation.
///
/// Time: O(N) - Visit every node once.
/// Space: O(H) - Recursion stack depth (H = height of tree, O(N) worst case for a skewed tree).
///
/// # Rust Insight
/// We use `Option<i32>` for the bounds. `None` represents positive/negative infinity.
/// This avoids tricky edge cases with `i32::MIN` and `i32::MAX` which are valid node values.
///
/// # Pattern
/// This is a classic "Top-Down" recursion where we pass state down to children.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_valid_bst_optimized(root: Option<Box<TreeNode>>) -> bool {
    fn validate(node: Option<&Box<TreeNode>>, min: Option<i32>, max: Option<i32>) -> bool {
        match node {
            None => true,
            Some(n) => {
                // Check lower bound (must be strictly greater than min)
                // RUST INSIGHT: `is_some_and` (stabilized in 1.70) elegantly handles
                // the "if exists and satisfies condition" pattern, removing nested `if let`.
                if min.is_some_and(|min_val| n.val <= min_val) {
                    return false;
                }
                // Check upper bound (must be strictly less than max)
                if max.is_some_and(|max_val| n.val >= max_val) {
                    return false;
                }

                // Recurse left: max becomes current val
                // Recurse right: min becomes current val
                validate(n.left.as_ref(), min, Some(n.val))
                    && validate(n.right.as_ref(), Some(n.val), max)
            }
        }
    }

    validate(root.as_ref(), None, None)
}

/// Optimal approach: Iterative In-order Traversal
///
/// Instead of collecting all values (like brute force) or recursing (like the optimized version),
/// we simulate the in-order traversal iteratively using an explicit stack.
/// We only need to keep track of the *previous* value visited to ensure strictly increasing order.
///
/// Time: O(N) - Visit every node once.
/// Space: O(H) - Explicit stack size depends on tree height (O(N) worst case for a skewed tree).
///
/// # Rust Insight
/// We use `Vec` as a stack.
/// The `prev` variable is `Option<i32>` to handle the first node (which has no predecessor).
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_valid_bst_optimal(root: Option<Box<TreeNode>>) -> bool {
    let mut stack = Vec::new();
    let mut current = &root;
    let mut prev: Option<i32> = None;

    while current.is_some() || !stack.is_empty() {
        // Go as left as possible
        while let Some(node) = current {
            stack.push(node);
            current = &node.left;
        }

        // Process the node
        if let Some(node) = stack.pop() {
            // Check order
            if prev.is_some_and(|p| node.val <= p) {
                return false;
            }
            prev = Some(node.val);

            // Go right
            current = &node.right;
        }
    }

    true
}

/// Main entry point - uses the optimal (iterative in-order) solution.
#[must_use]
pub fn is_valid_bst(root: Option<Box<TreeNode>>) -> bool {
    is_valid_bst_optimal(root)
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

        let input = Some(Box::new(root));
        assert!(is_valid_bst_brute_force(input.clone()));
        assert!(is_valid_bst_optimized(input.clone()));
        assert!(is_valid_bst_optimal(input));
    }

    #[test]
    fn test_invalid_bst_simple() {
        //      5
        //     / \
        //    1   4
        //       / \
        //      3   6
        let mut root = TreeNode::new(5);
        root.left = leaf(1);
        let mut right = TreeNode::new(4);
        right.left = leaf(3);
        right.right = leaf(6);
        root.right = Some(Box::new(right));

        let input = Some(Box::new(root));
        assert!(!is_valid_bst_brute_force(input.clone()));
        assert!(!is_valid_bst_optimized(input.clone()));
        assert!(!is_valid_bst_optimal(input));
    }

    #[test]
    fn test_invalid_bst_duplicates() {
        //      2
        //     / \
        //    2   2
        let mut root = TreeNode::new(2);
        root.left = leaf(2);
        root.right = leaf(2);

        let input = Some(Box::new(root));
        assert!(!is_valid_bst_brute_force(input.clone()));
        assert!(!is_valid_bst_optimized(input.clone()));
        assert!(!is_valid_bst_optimal(input));
    }

    #[test]
    fn test_valid_bst_limits() {
        // Test with i32 limits
        let mut root = TreeNode::new(i32::MAX);
        root.left = leaf(i32::MAX - 1);

        let input = Some(Box::new(root));
        assert!(is_valid_bst_optimized(input));
    }

    #[test]
    fn test_empty_tree() {
        assert!(is_valid_bst_brute_force(None));
        assert!(is_valid_bst_optimized(None));
        assert!(is_valid_bst_optimal(None));
    }

    #[test]
    fn test_single_node() {
        let input = leaf(42);
        assert!(is_valid_bst_brute_force(input.clone()));
        assert!(is_valid_bst_optimized(input.clone()));
        assert!(is_valid_bst_optimal(input));
    }
}
