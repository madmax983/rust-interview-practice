//! # 572. Subtree of Another Tree
//!
//! Difficulty: Easy
//! Link: <https://leetcode.com/problems/subtree-of-another-tree/>
//!
//! Given the roots of two binary trees `root` and `subRoot`, return `true` if there is a subtree
//! of `root` with the same structure and node values of `subRoot` and `false` otherwise.
//!
//! A subtree of a binary tree `tree` is a tree that consists of a node in `tree` and all of this
//! node's descendants. The tree `tree` could also be considered as a subtree of itself.
//!
//! This problem emphasizes recursion and tree matching. In Rust, utilizing `Option<Box<TreeNode>>`
//! heavily leans on recursive pattern matching, demonstrating how ownership and references (`&Option<...>`)
//! are used to traverse trees without moving or copying nodes out of their allocation.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::trees::subtree_of_another_tree::{is_subtree, TreeNode};
//!
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

/// Helper function to determine if two trees are exactly the same
/// Time: O(M) where M is the number of nodes in subRoot.
/// Space: O(H) where H is the height of the tree.
fn is_same_tree(p: &Option<Box<TreeNode>>, q: &Option<Box<TreeNode>>) -> bool {
    // RUST INSIGHT: Matching on a tuple of references cleanly destructures options.
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

/// Recursive brute-force approach.
///
/// Time: O(N * M) - N is nodes in `root`, M is nodes in `subRoot`. Worst case compares `subRoot` at every node.
/// Space: O(N) - recursion depth
///
/// This approach traverses every node of `root` and performs a full tree comparison
/// if a node looks like it could be the start of `subRoot`.
///
/// We take `Option<Box<TreeNode>>` by value as given by the LeetCode signature,
/// but delegate to a reference-taking helper so we don't consume `subRoot` prematurely.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_brute_force(
    root: Option<Box<TreeNode>>,
    sub_root: Option<Box<TreeNode>>,
) -> bool {
    fn helper(r: &Option<Box<TreeNode>>, s: &Option<Box<TreeNode>>) -> bool {
        match r {
            None => false,
            Some(node) => is_same_tree(r, s) || helper(&node.left, s) || helper(&node.right, s),
        }
    }
    helper(&root, &sub_root)
}

/// Optimized string serialization approach.
///
/// Time: O(N + M) - We traverse both trees once to build strings, and then do a substring search.
/// Space: O(N + M) - Creating the string representations.
///
/// By generating a unique pre-order string representation for both trees, we can simply
/// check if the `subRoot` string is a substring of the `root` string.
///
/// RUST INSIGHT: We use `std::fmt::Write` via `write!` to avoid intermediate string allocations.
///
/// GOTCHA: The string representation must unambiguously represent structure. A naive traversal
/// might represent `Tree(1, Left(2))` and `Tree(1, Right(2))` the same way. We must include
/// explicit markers for `None` branches (e.g. `^`) and frame nodes (e.g. `#1#`) to prevent
/// substring matches where a node value `2` matches the `2` in `12`.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_optimized(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    use std::fmt::Write;

    fn serialize(node: &Option<Box<TreeNode>>, s: &mut String) {
        match node {
            None => {
                s.push('^');
            }
            Some(n) => {
                // Wrap value in delimiters to prevent e.g. "2" matching "12"
                let _ = write!(s, "#{val}#", val = n.val);
                serialize(&n.left, s);
                serialize(&n.right, s);
            }
        }
    }

    let mut root_str = String::with_capacity(1024);
    let mut sub_str = String::with_capacity(1024);

    serialize(&root, &mut root_str);
    serialize(&sub_root, &mut sub_str);

    root_str.contains(&sub_str)
}

/// Main entry point (uses optimal approach or brute force based on preference, here we use brute force
/// since Leetcode trees are small, N <= 2000, and it avoids heavy string allocations)
#[must_use]
pub fn is_subtree(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    is_subtree_brute_force(root, sub_root)
}

// Alternative Approaches:
// 1. **KMP (Knuth-Morris-Pratt) Algorithm**: Similar to string matching, you could compute a LPS
//    (Longest Prefix Suffix) array on the tree nodes themselves. Overkill for this difficulty,
//    but strictly O(N+M) without huge string allocations.
// 2. **Merkle Hashing**: Hash each subtree. A leaf's hash is based on its value. An internal node's
//    hash is based on its value, left child's hash, and right child's hash. This turns tree comparison
//    into a simple O(1) integer comparison, bringing the search time to O(N + M).

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

        // sub:
        //    4
        //   / \
        //  1   2
        let mut sub = TreeNode::new(4);
        sub.left = leaf(1);
        sub.right = leaf(2);

        let r = Some(Box::new(root));
        let s = Some(Box::new(sub));

        assert!(is_subtree_brute_force(r.clone(), s.clone()));
        assert!(is_subtree_optimized(r, s));
    }

    #[test]
    fn test_edge_case_not_subtree_extra_leaf() {
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
        let mut right_of_left = TreeNode::new(2);
        right_of_left.left = leaf(0);
        left.right = Some(Box::new(right_of_left));
        root.left = Some(Box::new(left));
        root.right = leaf(5);

        // sub:
        //    4
        //   / \
        //  1   2
        let mut sub = TreeNode::new(4);
        sub.left = leaf(1);
        sub.right = leaf(2);

        let r = Some(Box::new(root));
        let s = Some(Box::new(sub));

        assert!(!is_subtree_brute_force(r.clone(), s.clone()));
        assert!(!is_subtree_optimized(r, s));
    }

    #[test]
    fn test_boundary_case_same_tree() {
        let mut root = TreeNode::new(1);
        root.left = leaf(1);

        let mut sub = TreeNode::new(1);
        sub.left = leaf(1);

        let r = Some(Box::new(root));
        let s = Some(Box::new(sub));

        assert!(is_subtree_brute_force(r.clone(), s.clone()));
        assert!(is_subtree_optimized(r, s));
    }

    #[test]
    fn test_value_matching_trap() {
        // root: [12]
        // sub: [2]
        // If serialization is naive "12", "2" will be a substring.
        let root = leaf(12);
        let sub = leaf(2);

        assert!(!is_subtree_brute_force(root.clone(), sub.clone()));
        assert!(!is_subtree_optimized(root, sub));
    }
}
