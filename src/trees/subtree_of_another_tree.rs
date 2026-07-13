//! # 572. Subtree of Another Tree
//!
//! Difficulty: Easy
//! Link: <https://leetcode.com/problems/subtree-of-another-tree/>
//!
//! Given the roots of two binary trees `root` and `subRoot`, return `true` if there is a subtree of `root`
//! with the same structure and node values of `subRoot` and `false` otherwise.
//!
//! This problem teaches us about pattern matching with `Option<Box<TreeNode>>` and recursion. It highlights
//! the idiomatic usage of `.as_deref()` to safely pass `Option<&TreeNode>` when we need to traverse or compare
//! trees without consuming them. It also shows a powerful string serialization approach and an optimal
//! zero-allocation serialization using `String::with_capacity` and `write!`.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::trees::subtree_of_another_tree::{is_subtree, TreeNode};
//!
//! // Create trees (using a helper or manually)
//! let mut root = TreeNode::new(3);
//! let mut left = TreeNode::new(4);
//! left.left = Some(Box::new(TreeNode::new(1)));
//! left.right = Some(Box::new(TreeNode::new(2)));
//! root.left = Some(Box::new(left));
//! root.right = Some(Box::new(TreeNode::new(5)));
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

/// Brute-force approach: Recursive Matching
///
/// Time: O(M * N) where M is nodes in root, N is nodes in subRoot
/// Space: O(H) where H is the height of root, for the call stack
///
/// We check if the trees match starting at the current node. If not, we recursively
/// check the left and right children of the current node.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_brute_force(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    // Helper to check if two trees are identical
    // RUST INSIGHT: Passing Option<&TreeNode> allows us to compare nodes without consuming them.
    // We can use `.as_deref()` on an Option<Box<T>> to get an Option<&T>.
    fn is_same_tree(p: Option<&TreeNode>, q: Option<&TreeNode>) -> bool {
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

    // Helper for finding if sub_root exists in root
    fn check_subtree(current: Option<&TreeNode>, target: Option<&TreeNode>) -> bool {
        if current.is_none() {
            return false;
        }

        if is_same_tree(current, target) {
            return true;
        }

        let node = current.unwrap();
        check_subtree(node.left.as_deref(), target) || check_subtree(node.right.as_deref(), target)
    }

    check_subtree(root.as_deref(), sub_root.as_deref())
}

/// Serialized approach: Naive String Matching
///
/// Time: O(M + N) to serialize and search
/// Space: O(M + N) for the strings
///
/// Serialize both trees with structure markers, then do a substring search.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_string(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    // GOTCHA: When serializing binary trees to perform subtree matching via substring search,
    // avoid prepending a global "start of tree" marker (like `^`) to the subtree string,
    // as it will prevent matching subtrees that are not the global root.
    // Instead, use proper node boundary markers (like # for nulls, and commas/brackets).
    fn serialize(node: Option<&TreeNode>) -> String {
        match node {
            Some(n) => format!("({}{}{})", n.val, serialize(n.left.as_deref()), serialize(n.right.as_deref())),
            None => "#".to_string(),
        }
    }

    let root_str = serialize(root.as_deref());
    let sub_root_str = serialize(sub_root.as_deref());

    root_str.contains(&sub_root_str)
}

/// Optimal Serialized approach: Zero-Allocation String Matching
///
/// Time: O(M + N) to serialize and search
/// Space: O(M + N) for the single strings, but zero intermediate allocations
///
/// Like the naive string approach, but optimized to use `String::with_capacity` and `write!`
/// to eliminate intermediate heap allocations during serialization.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_optimal(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    // We pre-allocate enough capacity to avoid reallocations. A reasonable estimate is 10 chars per node.
    let mut root_str = String::with_capacity(20000);
    let mut sub_root_str = String::with_capacity(10000);

    // RUST INSIGHT: Using `write!` to append to an existing buffer is much more efficient
    // than using `format!` which allocates a new String on every call.
    fn serialize_fast(node: Option<&TreeNode>, out: &mut String) {
        match node {
            Some(n) => {
                let _ = write!(out, "({}", n.val);
                serialize_fast(n.left.as_deref(), out);
                serialize_fast(n.right.as_deref(), out);
                out.push(')');
            }
            None => {
                out.push('#');
            }
        }
    }

    serialize_fast(root.as_deref(), &mut root_str);
    serialize_fast(sub_root.as_deref(), &mut sub_root_str);

    root_str.contains(&sub_root_str)
}

/// Main entry point
#[must_use]
pub fn is_subtree(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    is_subtree_optimal(root, sub_root)
}

// Alternative Approaches:
// 1. **Merkle Tree / Hashing**: Compute a hash for each subtree and compare hashes. This is O(M + N)
//    time and O(M) space for a hash map. It handles extremely large trees well but is complex to implement.
// 2. **KMP Algorithm**: After serializing to an array/string, one could use KMP to guarantee O(M + N) time
//    matching, though Rust's standard library `str::contains` is usually fast enough in practice (often using two-way string matching).

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(val: i32) -> Option<Box<TreeNode>> {
        Some(Box::new(TreeNode::new(val)))
    }

    #[test]
    fn test_happy_path() {
        // root = [3,4,5,1,2], subRoot = [4,1,2]
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
        assert!(is_subtree_string(root_box.clone(), sub_root_box.clone()));
        assert!(is_subtree_optimal(root_box, sub_root_box));
    }

    #[test]
    fn test_edge_case_not_subtree() {
        // root = [3,4,5,1,2,null,null,null,null,0], subRoot = [4,1,2]
        // This fails because the '2' node in root has a left child '0', while the subRoot's '2' node does not.
        let mut root = TreeNode::new(3);
        let mut left = TreeNode::new(4);
        left.left = leaf(1);

        let mut right_of_left = TreeNode::new(2);
        right_of_left.left = leaf(0); // The extra node

        left.right = Some(Box::new(right_of_left));
        root.left = Some(Box::new(left));
        root.right = leaf(5);

        let mut sub_root = TreeNode::new(4);
        sub_root.left = leaf(1);
        sub_root.right = leaf(2);

        let root_box = Some(Box::new(root));
        let sub_root_box = Some(Box::new(sub_root));

        assert!(!is_subtree_brute_force(root_box.clone(), sub_root_box.clone()));
        assert!(!is_subtree_string(root_box.clone(), sub_root_box.clone()));
        assert!(!is_subtree_optimal(root_box, sub_root_box));
    }

    #[test]
    fn test_stress_case_identical_values() {
        // root = [1,1,1,1,1,1,1], subRoot = [1,1,1]
        let mut root = TreeNode::new(1);
        let mut left1 = TreeNode::new(1);
        left1.left = leaf(1);
        left1.right = leaf(1);
        let mut right1 = TreeNode::new(1);
        right1.left = leaf(1);
        right1.right = leaf(1);

        root.left = Some(Box::new(left1));
        root.right = Some(Box::new(right1));

        let mut sub_root = TreeNode::new(1);
        sub_root.left = leaf(1);
        sub_root.right = leaf(1);

        let root_box = Some(Box::new(root));
        let sub_root_box = Some(Box::new(sub_root));

        assert!(is_subtree_brute_force(root_box.clone(), sub_root_box.clone()));
        assert!(is_subtree_string(root_box.clone(), sub_root_box.clone()));
        assert!(is_subtree_optimal(root_box, sub_root_box));
    }
}
