//! # 572. Subtree of Another Tree
//!
//! Difficulty: Easy
//! Link: <https://leetcode.com/problems/subtree-of-another-tree/>
//!
//! Given the roots of two binary trees `root` and `subRoot`, return `true` if there is a subtree of `root` with the same structure and node values of `subRoot` and `false` otherwise.
//!
//! This problem perfectly demonstrates traversing recursive data structures and pattern matching in Rust.
//! It also highlights Rust's string formatting capabilities and zero-cost abstraction when using
//! advanced serialization approaches to avoid allocating strings inside recursive calls.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::trees::subtree_of_another_tree::{is_subtree, TreeNode};
//!
//! let mut root = TreeNode::new(3);
//! root.left = Some(Box::new(TreeNode::new(4)));
//! root.right = Some(Box::new(TreeNode::new(5)));
//! root.left.as_mut().unwrap().left = Some(Box::new(TreeNode::new(1)));
//! root.left.as_mut().unwrap().right = Some(Box::new(TreeNode::new(2)));
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

/// Helper function to determine if two trees are identical.
fn is_same_tree(p: Option<&TreeNode>, q: Option<&TreeNode>) -> bool {
    // RUST INSIGHT: Exhaustive pattern matching guarantees we handle all cases.
    match (p, q) {
        (Some(n_p), Some(n_q)) => {
            n_p.val == n_q.val
                && is_same_tree(n_p.left.as_deref(), n_q.left.as_deref())
                && is_same_tree(n_p.right.as_deref(), n_q.right.as_deref())
        }
        (None, None) => true,
        _ => false,
    }
}

/// Brute Force approach: Recursive Search
///
/// Time: O(m * n) - In the worst case, we might need to check `is_same_tree` (which takes O(n)) for each of the m nodes in the main tree.
/// Space: O(m) - Recursion depth in the worst case (skewed tree).
///
/// This approach traverses the `root` tree. For each node, it checks if the subtree starting at that
/// node is identical to the `subRoot` tree.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_brute_force(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    fn dfs(node: Option<&TreeNode>, target: Option<&TreeNode>) -> bool {
        if node.is_none() {
            return false;
        }
        if is_same_tree(node, target) {
            return true;
        }

        let n = node.unwrap();
        // RUST INSIGHT: .as_deref() takes Option<Box<T>> and yields Option<&T>, avoiding moves
        dfs(n.left.as_deref(), target) || dfs(n.right.as_deref(), target)
    }

    dfs(root.as_deref(), sub_root.as_deref())
}


/// Naive String Serialization Approach
///
/// Time: O(m + n + m*n) - Serialization takes O(m) and O(n). Substring check is O(m*n) standard, though often O(m+n) with optimizations.
/// Space: O(m + n) - For storing the serialized strings.
///
/// Converts both trees into strings using pre-order traversal. It includes null markers to preserve structural uniqueness
/// and wraps nodes in boundaries to prevent false positive substring matches (e.g., matching "12" in "112").
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_naive_serialization(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    fn serialize(node: Option<&TreeNode>, acc: &mut String) {
        match node {
            Some(n) => {
                // GOTCHA: Proper node boundaries are required to prevent substring match errors.
                acc.push_str(&format!(" ^{}$ ", n.val));
                serialize(n.left.as_deref(), acc);
                serialize(n.right.as_deref(), acc);
            }
            None => {
                acc.push_str(" # ");
            }
        }
    }

    let mut root_str = String::new();
    let mut sub_root_str = String::new();

    serialize(root.as_deref(), &mut root_str);
    serialize(sub_root.as_deref(), &mut sub_root_str);

    root_str.contains(&sub_root_str)
}

/// Optimal String Serialization Approach
///
/// Time: O(m + n) - Assuming an efficient substring algorithm (like KMP, though Rust's standard library is usually fast enough).
/// Space: O(m + n) - Pre-allocating strings prevents multiple intermediate heap allocations.
///
/// This approach optimizes the string serialization by pre-allocating memory and using the `write!` macro
/// to avoid the intermediate allocations introduced by `format!`.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_optimal_serialization(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    fn serialize(node: Option<&TreeNode>, acc: &mut String) {
        match node {
            Some(n) => {
                // RUST INSIGHT: write! appends directly to the pre-allocated string without temporary string creations.
                let _ = write!(acc, " ^{}$ ", n.val);
                serialize(n.left.as_deref(), acc);
                serialize(n.right.as_deref(), acc);
            }
            None => {
                acc.push_str(" # ");
            }
        }
    }

    // Estimate capacity based on constraints (up to 2000 nodes for root, 1000 for subroot).
    // Each node string format " ^{}$ " is ~6-10 chars. Let's pre-allocate to avoid reallocations.
    let mut root_str = String::with_capacity(2000 * 10);
    let mut sub_root_str = String::with_capacity(1000 * 10);

    serialize(root.as_deref(), &mut root_str);
    serialize(sub_root.as_deref(), &mut sub_root_str);

    root_str.contains(&sub_root_str)
}


/// Main entry point
#[must_use]
pub fn is_subtree(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    is_subtree_optimal_serialization(root, sub_root)
}

// Alternative Approaches:
// 1. **KMP or Merkle Hashing**: For huge trees, serializing to a string might be memory intensive. You could
//    hash the subtrees (Merkle Tree pattern) to do O(1) equality comparisons during traversal, reducing
//    time to strict O(m + n) without massive string allocations.

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(val: i32) -> Option<Box<TreeNode>> {
        Some(Box::new(TreeNode::new(val)))
    }

    #[test]
    fn test_happy_path_is_subtree() {
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

        assert!(is_subtree_brute_force(root_box.clone(), sub_root_box.clone()));
        assert!(is_subtree_naive_serialization(root_box.clone(), sub_root_box.clone()));
        assert!(is_subtree_optimal_serialization(root_box.clone(), sub_root_box.clone()));
    }

    #[test]
    fn test_edge_case_not_subtree_due_to_extra_leaf() {
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

        // subRoot:
        //    4
        //   / \
        //  1   2
        let mut sub_root = TreeNode::new(4);
        sub_root.left = leaf(1);
        sub_root.right = leaf(2);

        let root_box = Some(Box::new(root));
        let sub_root_box = Some(Box::new(sub_root));

        assert!(!is_subtree_brute_force(root_box.clone(), sub_root_box.clone()));
        assert!(!is_subtree_naive_serialization(root_box.clone(), sub_root_box.clone()));
        assert!(!is_subtree_optimal_serialization(root_box.clone(), sub_root_box.clone()));
    }

    #[test]
    fn test_stress_case_identical_trees() {
        // Both trees are identical
        let mut root = TreeNode::new(1);
        root.left = leaf(1);

        let mut sub_root = TreeNode::new(1);
        sub_root.left = leaf(1);

        let root_box = Some(Box::new(root));
        let sub_root_box = Some(Box::new(sub_root));

        assert!(is_subtree_brute_force(root_box.clone(), sub_root_box.clone()));
        assert!(is_subtree_naive_serialization(root_box.clone(), sub_root_box.clone()));
        assert!(is_subtree_optimal_serialization(root_box.clone(), sub_root_box.clone()));
    }

    #[test]
    fn test_substring_boundary_gotcha() {
        // A tricky case where simple string conversion might incorrectly match
        // if boundaries aren't preserved.
        // root string: [12]
        // subRoot string: [2]
        // Without boundaries, "12" contains "2".

        let mut root = TreeNode::new(12);
        let mut sub_root = TreeNode::new(2);

        let root_box = Some(Box::new(root));
        let sub_root_box = Some(Box::new(sub_root));

        assert!(!is_subtree_brute_force(root_box.clone(), sub_root_box.clone()));
        assert!(!is_subtree_naive_serialization(root_box.clone(), sub_root_box.clone()));
        assert!(!is_subtree_optimal_serialization(root_box.clone(), sub_root_box.clone()));
    }
}
