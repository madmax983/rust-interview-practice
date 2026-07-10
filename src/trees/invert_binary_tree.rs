//! # 226. Invert Binary Tree
//!
//! Given the root of a binary tree, invert the tree, and return its root.
//!
//! This problem is a classic for understanding tree traversal and recursion.
//! In Rust, it's particularly interesting because it forces you to deal with
//! `Option<Box<TreeNode>>` handling, ownership transfer, and the borrow checker.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::trees::invert_binary_tree::{invert_tree, TreeNode};
//!
//! let mut root = TreeNode::new(2);
//! root.left = Some(Box::new(TreeNode::new(1)));
//! root.right = Some(Box::new(TreeNode::new(3)));
//!
//! let inverted = invert_tree(Some(Box::new(root)));
//! // Result: 2 -> left: 3, right: 1
//! ```
//!
//! ## Constraints
//!
//! - The number of nodes in the tree is in the range `[0, 100]`.
//! - `-100 <= Node.val <= 100`

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

/// Brute Force approach: Explicit ownership transfer.
///
/// This approach deconstructs the `Option` and explicitly moves the children out
/// before recursing. It's verbose but very clear about what owns what.
///
/// Time: O(n) - Visits every node once.
/// Space: O(h) - Recursion depth equals tree height.
///
/// # Gotcha
/// You cannot do `node.left = invert(node.right)` directly if you haven't moved
/// `node.left` out first, because `node.left` would be overwritten before you use it!
/// (Though here we are swapping, so we'd need a temp variable anyway).
#[must_use]
#[allow(clippy::needless_pass_by_value)]
#[allow(clippy::option_if_let_else)]
pub fn invert_tree_brute_force(root: Option<Box<TreeNode>>) -> Option<Box<TreeNode>> {
    match root {
        None => None,
        Some(mut node) => {
            // Move children out of the node to avoid borrow checker issues
            // and to allow swapping.
            let left = node.left;
            let right = node.right;

            // Recurse and swap
            node.left = invert_tree_brute_force(right);
            node.right = invert_tree_brute_force(left);

            Some(node)
        }
    }
}

/// Optimized approach: Mutable Reference & `std::mem::swap`.
///
/// Instead of moving ownership of the nodes, we pass a mutable reference to the
/// `Option<Box<TreeNode>>`. This allows us to modify the tree "in-place" without
/// rebuilding the `Option` wrappers, though `Box` semantics mean we are just
/// swapping pointers.
///
/// Time: O(n)
/// Space: O(h)
///
/// # Rust Insight
/// `std::mem::swap` is a powerful tool in Rust. Since we can't easily have two mutable
/// references to parts of the same struct at the same time to swap them manually
/// (without temporary moves), `mem::swap` handles the unsafe bits under the hood
/// to swap the values of two mutable references.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn invert_tree_optimized(mut root: Option<Box<TreeNode>>) -> Option<Box<TreeNode>> {
    fn helper(node: &mut Option<Box<TreeNode>>) {
        if let Some(n) = node {
            // Swap the left and right children (Option<Box<TreeNode>>)
            std::mem::swap(&mut n.left, &mut n.right);

            // Recurse on the children
            // Note: We can borrow `n.left` and `n.right` mutably here because
            // they are distinct fields.
            helper(&mut n.left);
            helper(&mut n.right);
        }
    }

    helper(&mut root);
    root
}

/// Optimal approach: Idiomatic Functional Style.
///
/// Uses `Option::map` to handle the `Some` case and `None` propagation elegantly.
/// This is the most "Rustacean" way to write the solution.
///
/// Time: O(n)
/// Space: O(h)
///
/// # Rust Insight
/// `Option::map` takes ownership of the value inside the `Option` (if it exists),
/// processes it with the closure, and wraps the result back in `Some`.
/// This handles the `None` case automatically.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
#[allow(clippy::single_option_map)]
pub fn invert_tree_optimal(root: Option<Box<TreeNode>>) -> Option<Box<TreeNode>> {
    root.map(|mut node| {
        let left = node.left;
        node.left = invert_tree_optimal(node.right);
        node.right = invert_tree_optimal(left);
        node
    })
}

/// Main entry point
#[must_use]
pub fn invert_tree(root: Option<Box<TreeNode>>) -> Option<Box<TreeNode>> {
    invert_tree_optimal(root)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create a leaf node
    fn leaf(val: i32) -> Option<Box<TreeNode>> {
        Some(Box::new(TreeNode::new(val)))
    }

    // Helper to create a tree from a nested structure is hard,
    // let's just build manually in tests or use a helper.
    // A simple recursive helper to build from a simpler representation would be nice
    // but explicit construction is fine for small trees.

    #[test]
    fn test_brute_force_simple() {
        // Input:
        //      2
        //     / \
        //    1   3
        let mut root = TreeNode::new(2);
        root.left = leaf(1);
        root.right = leaf(3);

        let result = invert_tree_brute_force(Some(Box::new(root)));

        // Expected:
        //      2
        //     / \
        //    3   1
        let unboxed = result.unwrap();
        assert_eq!(unboxed.val, 2);
        assert_eq!(unboxed.left.unwrap().val, 3);
        assert_eq!(unboxed.right.unwrap().val, 1);
    }

    #[test]
    fn test_optimized_complex() {
        // Input:
        //      4
        //     / \
        //    2   7
        //   / \ / \
        //  1  3 6  9
        let mut root = TreeNode::new(4);
        let mut left = TreeNode::new(2);
        left.left = leaf(1);
        left.right = leaf(3);
        let mut right = TreeNode::new(7);
        right.left = leaf(6);
        right.right = leaf(9);

        root.left = Some(Box::new(left));
        root.right = Some(Box::new(right));

        let result = invert_tree_optimized(Some(Box::new(root)));

        // Expected:
        //      4
        //     / \
        //    7   2
        //   / \ / \
        //  9  6 3  1
        let node = result.unwrap();
        assert_eq!(node.val, 4);

        let left = node.left.unwrap();
        assert_eq!(left.val, 7);
        assert_eq!(left.left.unwrap().val, 9);
        assert_eq!(left.right.unwrap().val, 6);

        let right = node.right.unwrap();
        assert_eq!(right.val, 2);
        assert_eq!(right.left.unwrap().val, 3);
        assert_eq!(right.right.unwrap().val, 1);
    }

    #[test]
    fn test_optimal_empty() {
        let result = invert_tree_optimal(None);
        assert_eq!(result, None);
    }

    #[test]
    fn test_all_approaches_single_node() {
        for f in [
            invert_tree_brute_force,
            invert_tree_optimized,
            invert_tree_optimal,
        ] {
            let node = f(leaf(42)).unwrap();
            assert_eq!(node.val, 42);
            assert!(node.left.is_none());
            assert!(node.right.is_none());
        }
    }

    #[test]
    fn test_all_approaches_consistency() {
        // Tree: 1 -> left: 2
        let mut root_tmpl = TreeNode::new(1);
        root_tmpl.left = leaf(2);

        let input1 = Some(Box::new(TreeNode {
            val: 1,
            left: leaf(2),
            right: None,
        }));
        let input2 = Some(Box::new(TreeNode {
            val: 1,
            left: leaf(2),
            right: None,
        }));
        let input3 = Some(Box::new(TreeNode {
            val: 1,
            left: leaf(2),
            right: None,
        }));

        let res1 = invert_tree_brute_force(input1);
        let res2 = invert_tree_optimized(input2);
        let res3 = invert_tree_optimal(input3);

        assert_eq!(res1, res2);
        assert_eq!(res2, res3);

        // Verify structure
        let node = res1.unwrap();
        assert_eq!(node.val, 1);
        assert!(node.left.is_none());
        assert_eq!(node.right.unwrap().val, 2);
    }
}
