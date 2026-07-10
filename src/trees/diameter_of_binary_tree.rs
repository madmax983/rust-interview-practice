//! # 543. Diameter of Binary Tree
//!
//! Difficulty: Easy
//! Link: <https://leetcode.com/problems/diameter-of-binary-tree/>
//!
//! Given the root of a binary tree, return the length of the diameter of the tree.
//! The diameter of a binary tree is the length of the longest path between any two nodes in a tree.
//! This path may or may not pass through the root.
//! The length of a path between two nodes is represented by the number of edges between them.
//!
//! ## Why This Matters in Rust
//!
//! In languages like Python or Java, it's common to solve this by creating a global or class-level
//! mutable variable `self.max_diameter`, which is updated as a side effect during recursive depth calculations.
//! Rust's strict ownership and mutability rules intentionally make this "ambient mutability" difficult.
//!
//! Instead, Rust forces us to manage state explicitly. We have two idiomatic choices:
//! 1. Pass a mutable reference (`&mut i32`) down the recursion stack.
//! 2. Use a pure functional approach, where the recursive function returns both the height and the maximum diameter seen so far as a tuple.
//!
//! This problem perfectly illustrates how Rust pushes you toward clearer state management without sacrificing performance.
//!
//! ## Approach
//!
//! Both implementations run in the same asymptotic complexity (`O(N)` time, `O(H)` stack
//! space); they differ in state-management technique, so we label them by relative quality:
//!
//! - **Brute force (Pure Functional, Tuple Return)**: The helper function returns
//!   `(current_height, max_diameter_so_far)`. This avoids side-effects entirely but threads
//!   and copies an extra value through every return.
//!    - **Time**: `O(N)` - Every node is visited once.
//!    - **Space**: `O(H)` - Call stack depth equals tree height (`O(N)` worst case, skewed).
//!
//! - **Optimal (Mutable Reference State)**: We pass `&mut i32` to the recursive helper. This
//!   mirrors the "global variable" approach from other languages but does so safely, and tends
//!   to compile to tighter code. This is the main entry point.
//!    - **Time**: `O(N)` - Every node is visited once.
//!    - **Space**: `O(H)` - Call stack depth equals tree height (`O(N)` worst case, skewed).

use std::cmp;

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

/// Optimal approach: mutable reference state (idiomatic).
///
/// We use a helper function that returns the height of the current subtree.
/// While calculating the height, we update a shared mutable counter (`&mut i32`)
/// with the maximum diameter found so far.
///
/// Time: O(N) - every node is visited once.
/// Space: O(H) - recursion stack, tree height (O(N) worst case for a skewed tree).
///
/// # Rust Insight
/// By taking `&mut diameter` as an argument, we guarantee safe, exclusive access
/// to the counter during the depth-first search. This avoids the need for interior
/// mutability (like `RefCell`) or atomic counters (`AtomicI32`), keeping it a zero-cost abstraction.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn diameter_of_binary_tree_optimal(root: Option<Box<TreeNode>>) -> i32 {
    // Helper function that returns the height of the tree, while updating the maximum diameter.
    fn height(node: Option<&TreeNode>, max_diameter: &mut i32) -> i32 {
        node.map_or(0, |n| {
            let left_height = height(n.left.as_deref(), max_diameter);
            let right_height = height(n.right.as_deref(), max_diameter);

            // Update the global maximum diameter if the path through the current node is longer
            *max_diameter = cmp::max(*max_diameter, left_height + right_height);

            // Return the height of the current node's subtree
            1 + cmp::max(left_height, right_height)
        })
    }

    let mut max_diameter = 0;
    height(root.as_deref(), &mut max_diameter);
    max_diameter
}

/// Brute force approach: pure functional recursion returning tuples.
///
/// Instead of side-effects, our helper function returns both pieces of information
/// we care about: `(height, max_diameter)`. Same complexity as the optimal version,
/// but it threads and copies an extra value through every return.
///
/// Time: O(N) - every node is visited once.
/// Space: O(H) - recursion stack, tree height (O(N) worst case for a skewed tree).
///
/// # Gotcha
/// Returning multiple values as a tuple is elegant, but it requires allocating and copying
/// the tuple on every return. In performance-critical hot paths, the `&mut` approach
/// can sometimes compile to tighter assembly, though LLVM is very good at optimizing
/// small tuples away entirely.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn diameter_of_binary_tree_brute_force(root: Option<Box<TreeNode>>) -> i32 {
    // Returns (height of current subtree, max diameter found in current subtree)
    fn helper(node: Option<&TreeNode>) -> (i32, i32) {
        node.map_or((0, 0), |n| {
            let (left_height, left_diameter) = helper(n.left.as_deref());
            let (right_height, right_diameter) = helper(n.right.as_deref());

            let current_diameter = left_height + right_height;
            let max_diameter = cmp::max(current_diameter, cmp::max(left_diameter, right_diameter));

            (1 + cmp::max(left_height, right_height), max_diameter)
        })
    }

    helper(root.as_deref()).1
}

/// Main entry point - uses the optimal (mutable reference) approach.
#[must_use]
pub fn diameter_of_binary_tree(root: Option<Box<TreeNode>>) -> i32 {
    diameter_of_binary_tree_optimal(root)
}

/// ## Alternative Approaches
///
/// - **Iterative DFS**: You can implement this using a stack, but simulating the post-order
///   traversal required to gather left and right heights simultaneously is exceptionally verbose.
///   For tree traversal problems like this, recursion is strongly preferred in Rust unless
///   call-stack limits are a strict concern.
#[cfg(test)]
mod tests {
    // test-code: helpers return Option<Box<TreeNode>> to match the tree's child field type.
    #![allow(clippy::unnecessary_wraps)]

    use super::*;

    // Helper to create a leaf node
    fn leaf(val: i32) -> Option<Box<TreeNode>> {
        Some(Box::new(TreeNode::new(val)))
    }

    #[test]
    fn test_happy_path() {
        // Tree:
        //       1
        //      / \
        //     2   3
        //    / \
        //   4   5
        // Path can be 4 -> 2 -> 1 -> 3 or 5 -> 2 -> 1 -> 3, length is 4 nodes, 3 edges.
        let mut root = TreeNode::new(1);
        let mut left = TreeNode::new(2);
        left.left = leaf(4);
        left.right = leaf(5);
        root.left = Some(Box::new(left));
        root.right = leaf(3);

        let boxed_root = Some(Box::new(root));
        assert_eq!(diameter_of_binary_tree_optimal(boxed_root.clone()), 3);
        assert_eq!(diameter_of_binary_tree_brute_force(boxed_root), 3);
    }

    #[test]
    fn test_edge_case_single_node() {
        let root = leaf(1);
        assert_eq!(diameter_of_binary_tree_optimal(root.clone()), 0);
        assert_eq!(diameter_of_binary_tree_brute_force(root), 0);
    }

    #[test]
    fn test_edge_case_empty() {
        assert_eq!(diameter_of_binary_tree_optimal(None), 0);
        assert_eq!(diameter_of_binary_tree_brute_force(None), 0);
    }

    #[test]
    fn test_stress_boundary_skewed() {
        // Tree:
        //   1
        //    \
        //     2
        //      \
        //       3
        //        \
        //         4
        let mut n3 = TreeNode::new(3);
        n3.right = leaf(4);
        let mut n2 = TreeNode::new(2);
        n2.right = Some(Box::new(n3));
        let mut n1 = TreeNode::new(1);
        n1.right = Some(Box::new(n2));

        let boxed_root = Some(Box::new(n1));
        assert_eq!(diameter_of_binary_tree_optimal(boxed_root.clone()), 3);
        assert_eq!(diameter_of_binary_tree_brute_force(boxed_root), 3);
    }
}
