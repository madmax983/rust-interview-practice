//! # 572. Subtree of Another Tree
//!
//! Difficulty: Easy
//! Link: <https://leetcode.com/problems/subtree-of-another-tree/>
//!
//! Given the roots of two binary trees `root` and `subRoot`, return `true` if there is a subtree
//! of `root` with the same structure and node values of `subRoot` and `false` otherwise.
//!
//! This problem perfectly demonstrates pattern matching with `Option<Box<TreeNode>>` and recursion.
//! We provide multiple approaches to show how to traverse trees, including a recursive brute-force search,
//! a naive string serialization approach, and an optimal zero-allocation serialization
//! using `String::with_capacity` and `write!`.
//!
//! ## Examples
//!
//! ```
//! // See tests module for examples due to verbose tree construction.
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

/// Helper function: Checks if two trees are exactly identical.
/// Time: O(M) where M is the number of nodes in the tree.
/// Space: O(H) where H is the height of the tree.
fn is_same_tree(p: Option<&TreeNode>, q: Option<&TreeNode>) -> bool {
    // RUST INSIGHT: Notice we take `Option<&TreeNode>` instead of `Option<Box<TreeNode>>`.
    // This allows us to traverse the tree structure without consuming it, satisfying
    // borrow checker rules when we need to reuse the `subRoot` multiple times.
    match (p, q) {
        (Some(np), Some(nq)) => {
            np.val == nq.val
                && is_same_tree(np.left.as_deref(), nq.left.as_deref())
                && is_same_tree(np.right.as_deref(), nq.right.as_deref())
        }
        (None, None) => true,
        _ => false,
    }
}

/// Brute Force approach: Recursive DFS
///
/// Time: O(N * M) - In the worst case, we check `is_same_tree` for every node in `root`.
/// Space: O(H) - Recursion depth up to tree height.
#[must_use]
pub fn is_subtree_brute_force(
    root: Option<Box<TreeNode>>,
    sub_root: Option<Box<TreeNode>>,
) -> bool {
    fn dfs(node: Option<&TreeNode>, target: Option<&TreeNode>) -> bool {
        match node {
            Some(n) => {
                if is_same_tree(Some(n), target) {
                    return true;
                }
                dfs(n.left.as_deref(), target) || dfs(n.right.as_deref(), target)
            }
            None => false,
        }
    }

    dfs(root.as_deref(), sub_root.as_deref())
}

/// Optimal approach: Tree Serialization with Substring Search
///
/// Time: O(N + M) - We serialize both trees and then do a substring search.
/// Space: O(N + M) - We store string representations of both trees.
///
/// Converts the trees to pre-order traversal strings and uses `.contains()` to find
/// the substring. By using `String::with_capacity` and `write!`, we avoid intermediate
/// string allocations.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_optimal(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    // Helper to serialize the tree.
    // GOTCHA: We must use structural markers (like "^" and "$") to prevent matching subtrees
    // that are only partial prefixes/suffixes of a valid node match. We can't just prepend
    // a global "start" marker as that prevents matching non-root subtrees.
    fn serialize(node: Option<&TreeNode>, buffer: &mut String) {
        match node {
            Some(n) => {
                // RUST INSIGHT: `write!` macro appends directly to the `String` buffer
                // without creating intermediate allocations like `format!` would.
                let _ = write!(buffer, "^{}$", n.val);
                serialize(n.left.as_deref(), buffer);
                serialize(n.right.as_deref(), buffer);
            }
            None => {
                buffer.push('#');
            }
        }
    }

    // Estimate capacity: rough guess of ~10 bytes per node.
    let mut root_str = String::with_capacity(20000);
    let mut sub_root_str = String::with_capacity(10000);

    serialize(root.as_deref(), &mut root_str);
    serialize(sub_root.as_deref(), &mut sub_root_str);

    root_str.contains(&sub_root_str)
}

/// Main entry point
#[must_use]
pub fn is_subtree(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    is_subtree_optimal(root, sub_root)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create a leaf node
    fn leaf(val: i32) -> Option<Box<TreeNode>> {
        Some(Box::new(TreeNode::new(val)))
    }

    #[test]
    fn test_valid_subtree() {
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

        // sub_root:
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
        assert!(is_subtree_optimal(root_box, sub_root_box));
    }

    #[test]
    fn test_invalid_subtree_extra_node() {
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
        let mut two = TreeNode::new(2);
        two.left = leaf(0);
        left.right = Some(Box::new(two));
        root.left = Some(Box::new(left));
        root.right = leaf(5);

        // sub_root:
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
        assert!(!is_subtree_optimal(root_box, sub_root_box));
    }

    #[test]
    fn test_identical_trees() {
        let root = leaf(1);
        let sub_root = leaf(1);

        assert!(is_subtree_brute_force(root.clone(), sub_root.clone()));
        assert!(is_subtree_optimal(root, sub_root));
    }

    #[test]
    fn test_value_prefix_matching() {
        // Test edge case where node value serialization could cause false positives.
        // e.g., root has value 12, sub_root has value 2.
        let mut root = TreeNode::new(12);
        root.left = leaf(3);

        let sub_root = leaf(2);

        let root_box = Some(Box::new(root));
        let sub_root_box = sub_root;

        assert!(!is_subtree_brute_force(
            root_box.clone(),
            sub_root_box.clone()
        ));
        assert!(!is_subtree_optimal(root_box, sub_root_box));
    }
}
