//! # 572. Subtree of Another Tree
//!
//! Difficulty: Easy
//! Link: <https://leetcode.com/problems/subtree-of-another-tree/>
//!
//! Given the roots of two binary trees `root` and `subRoot`, return `true` if there is a subtree of `root` with the same structure and node values of `subRoot` and `false` otherwise.
//!
//! This problem demonstrates traversing and comparing recursive structures (`Option<Box<TreeNode>>`) without consuming them. It highlights idiomatic Rust borrowing by using `.as_deref()` to gracefully convert `&Option<Box<T>>` to `Option<&T>`, satisfying strict Clippy rules and avoiding unnecessary cloning or ownership transfer.
//!
//! ## Approach
//!
//! We provide two approaches:
//! 1. **Brute Force (Recursive Search):** We traverse the `root` tree. At each node, we recursively check if it structurally matches `subRoot`. This is O(n * m) time but highly readable, leveraging recursive pattern matching.
//! 2. **Optimal (String Serialization):** By serializing both trees into pre-order strings with strict structural markers (e.g., `|val|` and `|#|`), we reduce the problem to substring search, running in O(n + m) time using Rust's optimized string search algorithms.
//!
//! Idiomatically, we use `Option<&TreeNode>` to borrow sub-trees efficiently, contrasting with garbage-collected languages where pointer comparisons or deep-copies might be naively used.

use std::fmt::Write;

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

/// Helper function to check if two subtrees are identical.
fn is_same_tree(p: Option<&TreeNode>, q: Option<&TreeNode>) -> bool {
    // RUST INSIGHT: Exhaustive pattern matching on two `Option`s guarantees safe traversal.
    match (p, q) {
        (Some(node_p), Some(node_q)) => {
            node_p.val == node_q.val
                && is_same_tree(node_p.left.as_deref(), node_q.left.as_deref())
                && is_same_tree(node_p.right.as_deref(), node_q.right.as_deref())
        }
        (None, None) => true,
        _ => false,
    }
}

/// Brute Force approach: Recursive Search
///
/// Time: O(n * m) - Where n is the number of nodes in `root` and m is the number of nodes in `subRoot`.
/// Space: O(h) - Where h is the tree height, due to the recursion stack.
///
/// We traverse the root tree, checking at each node if the tree from there exactly matches `subRoot`.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_brute_force(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    fn dfs(node: Option<&TreeNode>, target: Option<&TreeNode>) -> bool {
        // RUST INSIGHT: Matching an Option avoids dereferencing null pointers entirely.
        match node {
            Some(n) => {
                if is_same_tree(Some(n), target) {
                    true
                } else {
                    dfs(n.left.as_deref(), target) || dfs(n.right.as_deref(), target)
                }
            }
            None => target.is_none(),
        }
    }

    // GOTCHA: We must use `.as_deref()` instead of passing references directly to prevent consuming or cloning the boxes.
    dfs(root.as_deref(), sub_root.as_deref())
}

/// Optimal approach: String Serialization
///
/// Time: O(n + m) - Where n and m are the number of nodes in the trees.
/// Space: O(n + m) - To store the serialized representations.
///
/// We serialize both trees using pre-order traversal into a strict string format.
/// Subtree matching then reduces to a single substring check.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_optimal(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    // Helper to serialize the tree into a String buffer
    fn serialize(node: Option<&TreeNode>, buf: &mut String) {
        match node {
            Some(n) => {
                // GOTCHA: Using write! directly to the buffer avoids intermediate String allocations.
                let _ = write!(buf, "|{}|", n.val);
                serialize(n.left.as_deref(), buf);
                serialize(n.right.as_deref(), buf);
            }
            None => {
                let _ = write!(buf, "|#|");
            }
        }
    }

    // Pre-allocate to minimize heap reallocations. (Assume roughly 5 bytes per node)
    let mut root_str = String::with_capacity(512);
    let mut sub_str = String::with_capacity(128);

    serialize(root.as_deref(), &mut root_str);
    serialize(sub_root.as_deref(), &mut sub_str);

    // Subtree matching reduces to substring search
    root_str.contains(&sub_str)
}

/// Main entry point
#[must_use]
pub fn is_subtree(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    is_subtree_optimal(root, sub_root)
}

// Alternative Approaches:
// 1. **Merkle Hashing**: Hash each subtree and compare hashes. Efficient for very large trees but over-complicated here.
// 2. **KMP Search on Array**: Serializing into an array and running KMP instead of string search avoids string allocation overhead entirely.

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(val: i32) -> Option<Box<TreeNode>> {
        Some(Box::new(TreeNode::new(val)))
    }

    #[test]
    fn test_happy_path() {
        // Root: [3, 4, 5, 1, 2]
        // SubRoot: [4, 1, 2]
        let mut root = TreeNode::new(3);
        let mut left = TreeNode::new(4);
        left.left = leaf(1);
        left.right = leaf(2);
        root.left = Some(Box::new(left));
        root.right = leaf(5);

        let mut sub_root = TreeNode::new(4);
        sub_root.left = leaf(1);
        sub_root.right = leaf(2);

        let root_box = Some(Box::new(root));
        let sub_root_box = Some(Box::new(sub_root));

        assert!(is_subtree_brute_force(root_box.clone(), sub_root_box.clone()));
        assert!(is_subtree_optimal(root_box, sub_root_box));
    }

    #[test]
    fn test_structural_failure() {
        // Root: [3, 4, 5, 1, 2, null, null, null, null, 0]
        // SubRoot: [4, 1, 2]
        let mut root = TreeNode::new(3);
        let mut left = TreeNode::new(4);
        left.left = leaf(1);

        let mut left_right = TreeNode::new(2);
        left_right.left = leaf(0); // This makes it different

        left.right = Some(Box::new(left_right));
        root.left = Some(Box::new(left));
        root.right = leaf(5);

        let mut sub_root = TreeNode::new(4);
        sub_root.left = leaf(1);
        sub_root.right = leaf(2);

        let root_box = Some(Box::new(root));
        let sub_root_box = Some(Box::new(sub_root));

        assert!(!is_subtree_brute_force(root_box.clone(), sub_root_box.clone()));
        assert!(!is_subtree_optimal(root_box, sub_root_box));
    }

    #[test]
    fn test_edge_case_same_tree() {
        // Root: [1, 2, 3]
        // SubRoot: [1, 2, 3]
        let mut root = TreeNode::new(1);
        root.left = leaf(2);
        root.right = leaf(3);

        let mut sub_root = TreeNode::new(1);
        sub_root.left = leaf(2);
        sub_root.right = leaf(3);

        let root_box = Some(Box::new(root));
        let sub_root_box = Some(Box::new(sub_root));

        assert!(is_subtree_brute_force(root_box.clone(), sub_root_box.clone()));
        assert!(is_subtree_optimal(root_box, sub_root_box));
    }

    #[test]
    fn test_edge_case_sub_is_none() {
        let mut root = TreeNode::new(1);
        root.left = leaf(2);
        let root_box = Some(Box::new(root));

        assert!(is_subtree_brute_force(root_box.clone(), None));
        assert!(is_subtree_optimal(root_box, None));
    }
}
