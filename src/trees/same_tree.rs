//! # 100. Same Tree
//!
//! Difficulty: Easy
//! Link: <https://leetcode.com/problems/same-tree/>
//!
//! Given the roots of two binary trees `p` and `q`, write a function to check if they are the same or not.
//! Two binary trees are considered the same if they are structurally identical, and the nodes have the same value.
//!
//! This problem perfectly demonstrates Rust's powerful `match` statement capabilities. By matching on both `Option`s
//! simultaneously as a tuple, the compiler guarantees that we exhaustively handle all possible structure combinations,
//! completely eliminating the class of "null pointer" bugs common in other languages.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::trees::same_tree::{is_same_tree, TreeNode};
//!
//! let mut p = TreeNode::new(1);
//! p.left = Some(Box::new(TreeNode::new(2)));
//! p.right = Some(Box::new(TreeNode::new(3)));
//!
//! let mut q = TreeNode::new(1);
//! q.left = Some(Box::new(TreeNode::new(2)));
//! q.right = Some(Box::new(TreeNode::new(3)));
//!
//! assert_eq!(is_same_tree(Some(Box::new(p)), Some(Box::new(q))), true);
//! ```
//!
//! ## Constraints
//!
//! - The number of nodes in both trees is in the range `[0, 100]`.
//! - `-10^4 <= Node.val <= 10^4`

use std::collections::VecDeque;

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

/// Brute Force approach: Traverse and collect
///
/// Time: O(n) - We visit every node to serialize both trees.
/// Space: O(n) - We allocate vectors to store the serialized tree forms.
///
/// The brute force approach involves converting the trees into a comparable format (like an array)
/// using pre-order traversal. We must include `None` markers to preserve structural uniqueness.
/// Once serialized, we simply compare the arrays.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_same_tree_brute_force(p: Option<Box<TreeNode>>, q: Option<Box<TreeNode>>) -> bool {
    // Helper to serialize a tree into a Vec<Option<i32>>
    fn serialize(node: &Option<Box<TreeNode>>, acc: &mut Vec<Option<i32>>) {
        match node {
            Some(n) => {
                acc.push(Some(n.val));
                serialize(&n.left, acc);
                serialize(&n.right, acc);
            }
            None => {
                acc.push(None); // Essential to capture structure!
            }
        }
    }

    let mut p_vec = Vec::new();
    let mut q_vec = Vec::new();

    serialize(&p, &mut p_vec);
    serialize(&q, &mut q_vec);

    p_vec == q_vec
}

/// Optimized approach: Iterative BFS/Level Order Traversal
///
/// Time: O(n) - We visit each node once.
/// Space: O(w) - Where w is the maximum width of the tree, for the queue.
///
/// Instead of allocating vectors for all nodes, we use a queue to traverse both trees simultaneously (BFS).
/// If at any point the nodes don't match, we return `false` early (short-circuiting).
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_same_tree_optimized(p: Option<Box<TreeNode>>, q: Option<Box<TreeNode>>) -> bool {
    // GOTCHA: `VecDeque` provides efficient O(1) popping from the front. A standard `Vec` would be O(n).
    let mut queue = VecDeque::new();
    queue.push_back((p, q));

    while let Some((node_p, node_q)) = queue.pop_front() {
        // RUST INSIGHT: Matching on a tuple of both nodes forces us to handle all 4 cases clearly.
        match (node_p, node_q) {
            (Some(n_p), Some(n_q)) => {
                if n_p.val != n_q.val {
                    return false;
                }
                // Push children in tandem.
                queue.push_back((n_p.left, n_q.left));
                queue.push_back((n_p.right, n_q.right));
            }
            (None, None) => continue, // Both are empty branches, perfectly fine.
            _ => return false,        // Structural mismatch (one is Some, one is None).
        }
    }

    true
}

/// Optimal approach: Idiomatic Recursive Pattern Matching
///
/// Time: O(n) - We visit each node once in the worst case.
/// Space: O(h) - Where h is the tree height, due to the recursion stack.
///
/// This is the most idiomatic Rust solution. It recursively compares the trees, leveraging
/// Rust's exhaustive pattern matching to safely destructure the `Option`s and implicitly handle nulls.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_same_tree_optimal(p: Option<Box<TreeNode>>, q: Option<Box<TreeNode>>) -> bool {
    // RUST INSIGHT: This match is exhaustive. The compiler guarantees we handle every possible combination
    // of `Option::Some` and `Option::None` between the two trees.
    match (p, q) {
        (Some(n_p), Some(n_q)) => {
            n_p.val == n_q.val
                && is_same_tree_optimal(n_p.left, n_q.left)
                && is_same_tree_optimal(n_p.right, n_q.right)
        }
        (None, None) => true,
        _ => false, // Handles (Some, None) and (None, Some)
    }
}

/// Main entry point
#[must_use]
pub fn is_same_tree(p: Option<Box<TreeNode>>, q: Option<Box<TreeNode>>) -> bool {
    is_same_tree_optimal(p, q)
}

// Alternative Approaches:
// 1. **PartialEq impl**: The `TreeNode` struct already derives `PartialEq`. Because `Option` and `Box`
//    also implement `PartialEq` by delegating to their inner values, one could simply write `p == q`.
//    While correct in Rust, this defeats the purpose of the algorithmic exercise in an interview setting.

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create a leaf node
    fn leaf(val: i32) -> Option<Box<TreeNode>> {
        Some(Box::new(TreeNode::new(val)))
    }

    #[test]
    fn test_identical_trees() {
        // Tree:
        //    1
        //   / \
        //  2   3
        let mut p = TreeNode::new(1);
        p.left = leaf(2);
        p.right = leaf(3);

        let mut q = TreeNode::new(1);
        q.left = leaf(2);
        q.right = leaf(3);

        let p_box = Some(Box::new(p));
        let q_box = Some(Box::new(q));

        assert!(is_same_tree_brute_force(p_box.clone(), q_box.clone()));
        assert!(is_same_tree_optimized(p_box.clone(), q_box.clone()));
        assert!(is_same_tree_optimal(p_box, q_box));
    }

    #[test]
    fn test_structurally_different_trees() {
        // p:      q:
        //   1       1
        //  /         \
        // 2           2
        let mut p = TreeNode::new(1);
        p.left = leaf(2);

        let mut q = TreeNode::new(1);
        q.right = leaf(2);

        let p_box = Some(Box::new(p));
        let q_box = Some(Box::new(q));

        assert!(!is_same_tree_brute_force(p_box.clone(), q_box.clone()));
        assert!(!is_same_tree_optimized(p_box.clone(), q_box.clone()));
        assert!(!is_same_tree_optimal(p_box, q_box));
    }

    #[test]
    fn test_same_structure_different_values() {
        // p:      q:
        //   1       1
        //  / \     / \
        // 2   1   1   2
        let mut p = TreeNode::new(1);
        p.left = leaf(2);
        p.right = leaf(1);

        let mut q = TreeNode::new(1);
        q.left = leaf(1);
        q.right = leaf(2);

        let p_box = Some(Box::new(p));
        let q_box = Some(Box::new(q));

        assert!(!is_same_tree_brute_force(p_box.clone(), q_box.clone()));
        assert!(!is_same_tree_optimized(p_box.clone(), q_box.clone()));
        assert!(!is_same_tree_optimal(p_box, q_box));
    }

    #[test]
    fn test_empty_trees() {
        assert!(is_same_tree_brute_force(None, None));
        assert!(is_same_tree_optimized(None, None));
        assert!(is_same_tree_optimal(None, None));
    }

    #[test]
    fn test_one_empty_one_populated() {
        let p = Some(Box::new(TreeNode::new(0)));
        assert!(!is_same_tree_brute_force(p.clone(), None));
        assert!(!is_same_tree_optimized(p.clone(), None));
        assert!(!is_same_tree_optimal(p, None));
    }
}
