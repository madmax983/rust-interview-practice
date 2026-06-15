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
//! This problem naturally builds upon checking if two trees are identical. It demonstrates pattern matching
//! with `Option<Box<TreeNode>>` and recursion to compare nested structures. It provides an excellent
//! opportunity to discuss algorithmic complexity when comparing trees versus using string serialization for optimization.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::trees::subtree_of_another_tree::{is_subtree, TreeNode};
//!
//! // root: [3,4,5,1,2]
//! let mut root = TreeNode::new(3);
//! let mut left = TreeNode::new(4);
//! left.left = Some(Box::new(TreeNode::new(1)));
//! left.right = Some(Box::new(TreeNode::new(2)));
//! root.left = Some(Box::new(left));
//! root.right = Some(Box::new(TreeNode::new(5)));
//!
//! // subRoot: [4,1,2]
//! let mut sub_root = TreeNode::new(4);
//! sub_root.left = Some(Box::new(TreeNode::new(1)));
//! sub_root.right = Some(Box::new(TreeNode::new(2)));
//!
//! assert_eq!(is_subtree(Some(Box::new(root)), Some(Box::new(sub_root))), true);
//! ```

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

/// Helper function to determine if two trees are exactly the same.
/// This uses the exact same recursive logic as LeetCode #100 "Same Tree".
#[must_use]
fn is_same_tree(p: &Option<Box<TreeNode>>, q: &Option<Box<TreeNode>>) -> bool {
    // RUST INSIGHT: We borrow the Options to avoid unnecessary cloning.
    // The match compares references to the Boxed nodes.
    match (p, q) {
        (Some(n_p), Some(n_q)) => {
            n_p.val == n_q.val
                && is_same_tree(&n_p.left, &n_q.left)
                && is_same_tree(&n_p.right, &n_q.right)
        }
        (None, None) => true,
        _ => false,
    }
}

/// Brute Force approach: Recursive DFS
///
/// Time: O(M * N) - Where M is the number of nodes in `root` and N is the number of nodes in `subRoot`.
/// For every node in `root`, we might potentially compare it with the entire `subRoot` tree.
/// Space: O(H) - Where H is the height of the `root` tree, due to the recursion stack.
///
/// We recursively traverse the main tree. At each node, we check if the tree starting at that node
/// is identical to `subRoot`. If it is, we return true. Otherwise, we check its left and right children.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_brute_force(
    root: Option<Box<TreeNode>>,
    sub_root: Option<Box<TreeNode>>,
) -> bool {
    // RUST INSIGHT: We define a helper function that takes references.
    // This allows us to traverse the tree without ever calling `.clone()`,
    // strictly adhering to zero-cost abstractions and O(1) extra space (excluding call stack).
    fn dfs(r: &Option<Box<TreeNode>>, sub: &Option<Box<TreeNode>>) -> bool {
        if r.is_none() {
            return false;
        }

        if is_same_tree(r, sub) {
            return true;
        }

        // RUST INSIGHT: Because `r` is `&Option<Box<TreeNode>>`, matching on it
        // yields references to the inner nodes, avoiding moves or clones.
        if let Some(node) = r {
            dfs(&node.left, sub) || dfs(&node.right, sub)
        } else {
            false
        }
    }

    if sub_root.is_none() {
        return true;
    }

    dfs(&root, &sub_root)
}


/// Optimized approach: Tree Serialization
///
/// Time: O(M + N) - We serialize both trees and then perform string matching.
/// Space: O(M + N) - For storing the serialized strings.
///
/// Instead of nested traversals, we serialize both trees into strings using a specific format
/// (e.g., Preorder traversal with markers for nulls and node boundaries). Then, we check if the
/// serialized `subRoot` is a substring of the serialized `root`.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_optimized(
    root: Option<Box<TreeNode>>,
    sub_root: Option<Box<TreeNode>>,
) -> bool {
    // Helper to serialize the tree into a String
    fn serialize(node: &Option<Box<TreeNode>>, out: &mut String) {
        match node {
            Some(n) => {
                // RUST INSIGHT: We use `^` as a start marker and `$` as an end marker for the value.
                // This prevents false positives like "12" matching inside "112".
                out.push('^');
                out.push_str(&n.val.to_string());
                out.push('$');
                serialize(&n.left, out);
                serialize(&n.right, out);
            }
            None => {
                out.push('#'); // Marker for null node to preserve structure
            }
        }
    }

    let mut root_str = String::new();
    let mut sub_root_str = String::new();

    serialize(&root, &mut root_str);
    serialize(&sub_root, &mut sub_root_str);

    // GOTCHA: `contains` performs substring matching. Rust's standard library uses an efficient
    // algorithm for string matching (usually two-way algorithm, O(N+M)).
    root_str.contains(&sub_root_str)
}

/// Main entry point
#[must_use]
pub fn is_subtree(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    is_subtree_brute_force(root, sub_root)
}

// Alternative Approaches:
// 1. **KMP Algorithm on Tree Serializations**: If we want guaranteed O(M+N) time, we can implement the
//    Knuth-Morris-Pratt algorithm directly on the serialized lists or strings. Rust's `str::contains`
//    is already highly optimized and usually achieves O(M+N), making manual KMP unnecessary for most practical use cases.
// 2. **Merkle Hashing**: We can compute a hash for every subtree bottom-up. Two identical subtrees will
//    have the same hash. This achieves O(M+N) time and O(M) space. This is highly efficient and common in distributed systems.

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create a leaf node
    fn leaf(val: i32) -> Option<Box<TreeNode>> {
        Some(Box::new(TreeNode::new(val)))
    }

    #[test]
    fn test_happy_path_is_subtree() {
        // root: [3,4,5,1,2]
        let mut root = TreeNode::new(3);
        let mut left = TreeNode::new(4);
        left.left = leaf(1);
        left.right = leaf(2);
        root.left = Some(Box::new(left));
        root.right = leaf(5);

        // subRoot: [4,1,2]
        let mut sub_root = TreeNode::new(4);
        sub_root.left = leaf(1);
        sub_root.right = leaf(2);

        let root_box = Some(Box::new(root));
        let sub_root_box = Some(Box::new(sub_root));

        assert!(is_subtree_brute_force(root_box.clone(), sub_root_box.clone()));
        assert!(is_subtree_optimized(root_box.clone(), sub_root_box.clone()));
        assert!(is_subtree(root_box, sub_root_box));
    }

    #[test]
    fn test_happy_path_is_not_subtree() {
        // root: [3,4,5,1,2,null,null,null,null,0]
        let mut root = TreeNode::new(3);
        let mut left = TreeNode::new(4);
        left.left = leaf(1);
        let mut right_of_left = TreeNode::new(2);
        right_of_left.left = leaf(0);
        left.right = Some(Box::new(right_of_left));
        root.left = Some(Box::new(left));
        root.right = leaf(5);

        // subRoot: [4,1,2]
        let mut sub_root = TreeNode::new(4);
        sub_root.left = leaf(1);
        sub_root.right = leaf(2);

        let root_box = Some(Box::new(root));
        let sub_root_box = Some(Box::new(sub_root));

        assert!(!is_subtree_brute_force(root_box.clone(), sub_root_box.clone()));
        assert!(!is_subtree_optimized(root_box.clone(), sub_root_box.clone()));
        assert!(!is_subtree(root_box, sub_root_box));
    }

    #[test]
    fn test_edge_case_identical_trees() {
        let mut root = TreeNode::new(1);
        root.left = leaf(2);
        root.right = leaf(3);

        let mut sub_root = TreeNode::new(1);
        sub_root.left = leaf(2);
        sub_root.right = leaf(3);

        let root_box = Some(Box::new(root));
        let sub_root_box = Some(Box::new(sub_root));

        assert!(is_subtree_brute_force(root_box.clone(), sub_root_box.clone()));
        assert!(is_subtree_optimized(root_box.clone(), sub_root_box.clone()));
        assert!(is_subtree(root_box, sub_root_box));
    }

    #[test]
    fn test_edge_case_empty_subroot() {
        let root_box = leaf(1);
        let sub_root_box = None;

        assert!(is_subtree_brute_force(root_box.clone(), sub_root_box.clone()));
        assert!(is_subtree_optimized(root_box.clone(), sub_root_box.clone()));
        assert!(is_subtree(root_box, sub_root_box));
    }

    #[test]
    fn test_boundary_case_similar_values_different_structure() {
        // root: [12]
        let root = leaf(12);

        // subRoot: [2]
        let sub_root = leaf(2);

        let root_box = root;
        let sub_root_box = sub_root;

        assert!(!is_subtree_brute_force(root_box.clone(), sub_root_box.clone()));
        assert!(!is_subtree_optimized(root_box.clone(), sub_root_box.clone()));
        assert!(!is_subtree(root_box, sub_root_box));
    }
}
