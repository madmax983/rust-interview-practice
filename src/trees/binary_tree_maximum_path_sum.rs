//! # 124. Binary Tree Maximum Path Sum
//!
//! A **path** in a binary tree is a sequence of nodes where each pair of adjacent nodes in the sequence has an edge connecting them.
//! A node can only appear in the sequence **at most once**. Note that the path does not need to pass through the root.
//!
//! The **path sum** of a path is the sum of the node's values in the path.
//!
//! Given the `root` of a binary tree, return the maximum **path sum** of any **non-empty** path.
//!
//! Difficulty: Hard
//!
//! [LeetCode Problem 124](https://leetcode.com/problems/binary-tree-maximum-path-sum/)
//!
//! ## Why this matters in Rust
//! This problem is a brilliant showcase of state management during tree traversal in Rust. It highlights:
//! - **Interior Mutability vs. Ownership**: Managing global/shared state (the maximum path sum found so far) during recursion. Passing a mutable reference `&mut i32` is the idiomatic zero-cost way to accumulate state compared to `RefCell` or returning tuples.
//! - **Recursive Type Handling**: Elegantly matching on `Option<Box<TreeNode>>` and properly handling the `None` base cases.
//! - **Zero-cost Abstractions**: Using `std::cmp::max` and clean borrowing rules to ensure high performance without memory leaks or garbage collection overhead.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::trees::binary_tree_maximum_path_sum::{max_path_sum, TreeNode};
//!
//! // Tree:
//! //   1
//! //  / \
//! // 2   3
//! let mut root = TreeNode::new(1);
//! root.left = Some(Box::new(TreeNode::new(2)));
//! root.right = Some(Box::new(TreeNode::new(3)));
//!
//! let result = max_path_sum(Some(Box::new(root)));
//! assert_eq!(result, 6); // Path: 2 -> 1 -> 3
//! ```
//!
//! ## Constraints
//!
//! - The number of nodes in the tree is in the range `[1, 3 * 10^4]`.
//! - `-1000 <= Node.val <= 1000`

use std::cmp;

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

/// Brute force approach: recompute each node's best downward path independently.
///
/// For every node we treat it as the "peak" of the path (the highest node on it) and
/// compute the best downward path into its left and right subtrees with a *separate*
/// traversal each time. The answer is the maximum over all nodes of
/// `node.val + max(0, left_down) + max(0, right_down)`.
///
/// Because the downward-path computation (`max_down`) re-walks each subtree from scratch
/// for every node, work is heavily duplicated. The optimal version below folds both the
/// downward gain and the global maximum into a single post-order pass.
///
/// Time: O(n^2) - for each of the n nodes, `max_down` may walk its entire subtree (O(n)).
/// Space: O(h) - recursion stack, tree height (O(n) worst case for a skewed tree).
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn max_path_sum_brute_force(root: Option<Box<TreeNode>>) -> i32 {
    // Best downward path sum that starts at `node` and descends (always includes `node`).
    fn max_down(node: &Option<Box<TreeNode>>) -> i32 {
        match node {
            None => 0,
            Some(n) => n.val + cmp::max(0, cmp::max(max_down(&n.left), max_down(&n.right))),
        }
    }

    // Visit every node, treating each as the peak of a candidate path.
    fn visit(node: &Option<Box<TreeNode>>, best: &mut i32) {
        if let Some(n) = node {
            let left_down = cmp::max(0, max_down(&n.left));
            let right_down = cmp::max(0, max_down(&n.right));
            *best = cmp::max(*best, n.val + left_down + right_down);
            visit(&n.left, best);
            visit(&n.right, best);
        }
    }

    let mut best = i32::MIN;
    visit(&root, &mut best);
    best
}

/// Optimal approach: Post-order Traversal with Mutable State
///
/// We need to find the maximum path sum. A path might look like an inverted 'V',
/// going up from one leaf, through a parent, and down to another leaf.
///
/// For any given node, the maximum path passing through it as the "highest" node is:
/// `node.val + max(0, left_contribution) + max(0, right_contribution)`
///
/// However, if this node is part of a path that continues upwards to its parent,
/// it can only contribute ONE of its branches (either left or right) to the parent.
/// So the function must return:
/// `node.val + max(0, max(left_contribution, right_contribution))`
///
/// Time: O(n) - We visit every node exactly once.
/// Space: O(h) - Where h is the height of the tree, representing the call stack.
///
/// # Rust Insight
/// We use a `helper` function that takes a mutable reference `&mut i32` to keep track
/// of the global maximum. This is much more idiomatic and performant in Rust than
/// returning a `(i32, i32)` tuple for every recursive call, as it avoids unnecessary
/// tuple packing/unpacking and clearly expresses the intent of side-effect accumulation.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn max_path_sum_optimal(root: Option<Box<TreeNode>>) -> i32 {
    let mut global_max = i32::MIN;

    // Helper function that returns the max contribution a node can add to its parent,
    // while updating the global_max if the path passing through this node (as the peak) is better.
    fn get_max_gain(node: &Option<Box<TreeNode>>, current_max: &mut i32) -> i32 {
        match node {
            None => 0,
            Some(n) => {
                // Recursively get the max gain from left and right children.
                // We use cmp::max(x, 0) because if a branch has a negative sum,
                // we're better off not including it in our path at all.

                // RUST INSIGHT: Explicit Deref
                // We use &n.left and &n.right to borrow the Options. We could also just let
                // `n.left.as_deref()` or borrow checking handle it depending on our signature,
                // but passing `&Option<Box<TreeNode>>` allows us to traverse without taking ownership.
                let left_gain = cmp::max(get_max_gain(&n.left, current_max), 0);
                let right_gain = cmp::max(get_max_gain(&n.right, current_max), 0);

                // The price of a path that passes THROUGH this node and both its children
                let price_new_path = n.val + left_gain + right_gain;

                // Update the global max if this new path is better
                *current_max = cmp::max(*current_max, price_new_path);

                // For the parent of this node, we can only return the node's value
                // plus the best of its left OR right branches (not both).
                n.val + cmp::max(left_gain, right_gain)
            }
        }
    }

    // GOTCHA: We must pass `&root` because `get_max_gain` borrows the tree.
    // If we passed by value, we'd consume the tree, which isn't strictly necessary
    // for just reading values, though we *do* take ownership of `root` in the main
    // `max_path_sum` function signature as per LeetCode's standard.
    get_max_gain(&root, &mut global_max);

    global_max
}

/// Main entry point
#[must_use]
pub fn max_path_sum(root: Option<Box<TreeNode>>) -> i32 {
    max_path_sum_optimal(root)
}

// =========================================================================================
// Alternative Approaches
// =========================================================================================
//
// 1. Returning Tuples:
//    Instead of mutating a passed `&mut i32`, the recursive function could return
//    a tuple `(max_path_including_node, max_path_overall)`. This avoids the mutable
//    reference but allocates and copies more data on the stack. Passing `&mut` is generally
//    preferred in Rust for this specific pattern to minimize overhead.
//
// 2. RefCell State:
//    If building a complex object-oriented structure where traversing functions don't take
//    arguments cleanly, you could use `Rc<RefCell<i32>>` to hold the state. However, this incurs
//    runtime borrow checking overhead and is considered an anti-pattern for simple algorithms
//    where standard mutable references (`&mut`) suffice.

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create a leaf node
    fn leaf(val: i32) -> Option<Box<TreeNode>> {
        Some(Box::new(TreeNode::new(val)))
    }

    #[test]
    fn test_happy_path() {
        // Tree:
        //   1
        //  / \
        // 2   3
        let mut root = TreeNode::new(1);
        root.left = leaf(2);
        root.right = leaf(3);

        assert_eq!(max_path_sum(Some(Box::new(root))), 6);
    }

    #[test]
    fn test_negative_values() {
        // Tree:
        //   -10
        //   / \
        //  9  20
        //    /  \
        //   15   7
        //
        // Max path: 15 -> 20 -> 7 = 42
        let mut root = TreeNode::new(-10);
        root.left = leaf(9);

        let mut right = TreeNode::new(20);
        right.left = leaf(15);
        right.right = leaf(7);

        root.right = Some(Box::new(right));

        assert_eq!(max_path_sum(Some(Box::new(root))), 42);
    }

    #[test]
    fn test_all_negatives() {
        // Tree:
        //   -3
        //   / \
        // -5  -2
        //
        // We MUST include at least one node, so we pick the largest negative number: -2.
        // Wait, if -2 is a leaf, the max path is just -2.
        // Let's verify:
        // node -5 returns -5, gain 0.
        // node -2 returns -2, gain 0.
        // node -3: price_new_path = -3 + 0 + 0 = -3. Global max is updated to -3.
        // Wait, when evaluating -2, price_new_path = -2 + 0 + 0 = -2. Global max updated to -2.
        // -3 is evaluated later. The max should indeed be -2.
        let mut root = TreeNode::new(-3);
        root.left = leaf(-5);
        root.right = leaf(-2);

        assert_eq!(max_path_sum(Some(Box::new(root))), -2);
    }

    #[test]
    fn test_single_node() {
        let root = leaf(5);
        assert_eq!(max_path_sum(root), 5);
    }

    #[test]
    fn test_complex_tree() {
        // Tree:
        //       5
        //      / \
        //     4   8
        //    /   / \
        //   11  13  4
        //  /  \      \
        // 7    2      1
        //
        // Max path: 7 -> 11 -> 4 -> 5 -> 8 -> 13 = 48

        let mut n11 = TreeNode::new(11);
        n11.left = leaf(7);
        n11.right = leaf(2);

        let mut n4_left = TreeNode::new(4);
        n4_left.left = Some(Box::new(n11));

        let mut n4_right = TreeNode::new(4);
        n4_right.right = leaf(1);

        let mut n8 = TreeNode::new(8);
        n8.left = leaf(13);
        n8.right = Some(Box::new(n4_right));

        let mut root = TreeNode::new(5);
        root.left = Some(Box::new(n4_left));
        root.right = Some(Box::new(n8));

        assert_eq!(max_path_sum(Some(Box::new(root))), 48);
    }

    // Rebuilds the [-10, 9, 20(15,7)] tree used in `test_negative_values`.
    fn negative_values_tree() -> Option<Box<TreeNode>> {
        let mut root = TreeNode::new(-10);
        root.left = leaf(9);
        let mut right = TreeNode::new(20);
        right.left = leaf(15);
        right.right = leaf(7);
        root.right = Some(Box::new(right));
        Some(Box::new(root))
    }

    #[test]
    fn test_brute_force_examples() {
        // Simple positive tree: 2 -> 1 -> 3
        let mut root = TreeNode::new(1);
        root.left = leaf(2);
        root.right = leaf(3);
        assert_eq!(max_path_sum_brute_force(Some(Box::new(root))), 6);

        // Negative-root tree, best path 15 -> 20 -> 7 = 42
        assert_eq!(max_path_sum_brute_force(negative_values_tree()), 42);

        // All negatives: single largest node wins
        let mut neg = TreeNode::new(-3);
        neg.left = leaf(-5);
        neg.right = leaf(-2);
        assert_eq!(max_path_sum_brute_force(Some(Box::new(neg))), -2);
    }

    #[test]
    fn test_brute_force_single_node() {
        assert_eq!(max_path_sum_brute_force(leaf(5)), 5);
    }

    #[test]
    fn test_all_approaches_agree() {
        assert_eq!(
            max_path_sum_brute_force(negative_values_tree()),
            max_path_sum_optimal(negative_values_tree())
        );
        assert_eq!(
            max_path_sum_brute_force(leaf(-7)),
            max_path_sum_optimal(leaf(-7))
        );
    }
}
