//! # 105. Construct Binary Tree from Preorder and Inorder Traversal
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/construct-binary-tree-from-preorder-and-inorder-traversal/>
//!
//! Given two integer arrays `preorder` and `inorder` where `preorder` is the preorder traversal
//! of a binary tree and `inorder` is the inorder traversal of the same tree, construct and return
//! the binary tree.
//!
//! This problem perfectly illustrates Rust's ownership model, borrowing rules, and slice
//! manipulation (`&[i32]`). It requires careful tracking of boundaries in the arrays and
//! building recursive data structures (`Option<Box<TreeNode>>`) safely.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::trees::construct_binary_tree_from_preorder_and_inorder_traversal::build_tree;
//!
//! let preorder = vec![3, 9, 20, 15, 7];
//! let inorder = vec![9, 3, 15, 20, 7];
//! let root = build_tree(preorder, inorder);
//!
//! // Result: 3 -> left: 9, right: 20 -> left: 15, right: 7
//! ```
//!
//! ## Constraints
//!
//! - `1 <= preorder.length <= 3000`
//! - `inorder.length == preorder.length`
//! - `-3000 <= preorder[i], inorder[i] <= 3000`
//! - `preorder` and `inorder` consist of unique values.
//! - Each value of `inorder` also appears in `preorder`.
//! - `preorder` is guaranteed to be the preorder traversal of the tree.
//! - `inorder` is guaranteed to be the inorder traversal of the tree.

use std::collections::HashMap;

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

/// Brute force approach: Slicing with linear search
/// Time: O(n²) - For each node, we search for its value in the inorder slice.
/// Space: O(n) - Recursion stack depth (worst case a completely skewed tree).
///
/// We recursively split the problem. The first element of `preorder` is the root.
/// We find this root in `inorder` to determine the left and right subtrees.
///
/// # Gotcha
/// Using slices (`&[i32]`) avoids copying the vectors, which is a common performance
/// pitfall when translating this algorithm from other languages.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn build_tree_brute_force(preorder: Vec<i32>, inorder: Vec<i32>) -> Option<Box<TreeNode>> {
    fn helper(preorder: &[i32], inorder: &[i32]) -> Option<Box<TreeNode>> {
        if preorder.is_empty() || inorder.is_empty() {
            return None;
        }

        let root_val = preorder[0];
        let mut root = Box::new(TreeNode::new(root_val));

        // Find the index of the root in the inorder slice
        // RUST INSIGHT: `position` safely returns an Option<usize>. Since the problem
        // guarantees the value exists, we can safely unwrap.
        let mid = inorder.iter().position(|&x| x == root_val).unwrap();

        // Recursively build left and right subtrees
        // Left subtree corresponds to the first `mid` elements after the root in preorder,
        // and the first `mid` elements in inorder.
        root.left = helper(&preorder[1..=mid], &inorder[..mid]);
        // Right subtree corresponds to the remaining elements in both arrays.
        root.right = helper(&preorder[mid + 1..], &inorder[mid + 1..]);

        Some(root)
    }

    helper(&preorder, &inorder)
}

/// Optimized approach: Hash map for O(1) index lookups
/// Time: O(n) - We build the hash map once, then lookups take O(1) time.
/// Space: O(n) - For the hash map and the recursion stack.
///
/// Instead of searching for the root value in the `inorder` array every time (O(n)),
/// we can build a `HashMap` mapping values to their indices in O(n) time beforehand.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn build_tree_optimized(preorder: Vec<i32>, inorder: Vec<i32>) -> Option<Box<TreeNode>> {
    let mut inorder_map = HashMap::with_capacity(inorder.len());
    for (i, &val) in inorder.iter().enumerate() {
        inorder_map.insert(val, i);
    }

    // We use a mutable index `pre_idx` to track our current root in preorder array
    fn helper(
        preorder: &[i32],
        inorder_map: &HashMap<i32, usize>,
        pre_idx: &mut usize,
        left: usize,
        right: usize,
    ) -> Option<Box<TreeNode>> {
        if left > right {
            return None;
        }

        let root_val = preorder[*pre_idx];
        *pre_idx += 1;
        let mut root = Box::new(TreeNode::new(root_val));

        let inorder_idx = *inorder_map.get(&root_val).unwrap();

        // RUST INSIGHT: We must check bounds carefully to avoid integer underflow on usize.
        // If inorder_idx == left, the left subtree is empty.
        if inorder_idx > left {
            root.left = helper(preorder, inorder_map, pre_idx, left, inorder_idx - 1);
        }

        // Right side is safe because we check `left > right` at the start of the function.
        if inorder_idx < right {
            root.right = helper(preorder, inorder_map, pre_idx, inorder_idx + 1, right);
        }

        Some(root)
    }

    let mut pre_idx = 0;
    // Note: right boundary is `inorder.len() - 1`, we handle the empty case beforehand
    if inorder.is_empty() {
        return None;
    }
    helper(&preorder, &inorder_map, &mut pre_idx, 0, inorder.len() - 1)
}

/// Optimal approach: Iterator-based with `Option`
/// Time: O(n)
/// Space: O(n) - No hashmap allocation, only recursion stack.
///
/// This avoids allocations except the TreeNodes and avoids the overhead of a HashMap
/// by leveraging `Peekable` and recursion bounds. It uses an elegant technique
/// matching the inorder elements as a stop boundary.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn build_tree_optimal(preorder: Vec<i32>, inorder: Vec<i32>) -> Option<Box<TreeNode>> {
    // Convert arrays into iterators
    // Peekable allows us to look at the next element without consuming it
    let mut preorder_iter = preorder.into_iter();
    let mut inorder_iter = inorder.into_iter().peekable();

    fn helper(
        preorder_iter: &mut std::vec::IntoIter<i32>,
        inorder_iter: &mut std::iter::Peekable<std::vec::IntoIter<i32>>,
        stop: Option<i32>,
    ) -> Option<Box<TreeNode>> {
        // RUST INSIGHT: `if let` simplifies checking if we have reached the boundary
        // mapped by the parent node in the inorder traversal.
        if let Some(&inorder_val) = inorder_iter.peek() {
            if Some(inorder_val) == stop {
                return None;
            }
        } else {
            return None;
        }

        let root_val = preorder_iter.next()?;
        let mut root = Box::new(TreeNode::new(root_val));

        // Build left subtree. It is bounded by the current root_val
        root.left = helper(preorder_iter, inorder_iter, Some(root_val));

        // After the left subtree is built, the inorder iterator MUST point to `root_val`.
        // We consume it here.
        inorder_iter.next();

        // Build right subtree. It inherits the parent's boundary.
        root.right = helper(preorder_iter, inorder_iter, stop);

        Some(root)
    }

    helper(&mut preorder_iter, &mut inorder_iter, None)
}

/// Main entry point - uses optimal solution
#[must_use]
pub fn build_tree(preorder: Vec<i32>, inorder: Vec<i32>) -> Option<Box<TreeNode>> {
    build_tree_optimal(preorder, inorder)
}

/// ## Alternative Approaches
///
/// - **Iterative with Stack**: You can build the tree iteratively using a stack, traversing
///   `preorder` and using the stack and a pointer into `inorder` to figure out when to attach
///   right children. This is O(n) space and time, but recursion is generally more idiomatic
///   for tree construction unless stack overflow is a concern.

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(val: i32) -> Option<Box<TreeNode>> {
        Some(Box::new(TreeNode::new(val)))
    }

    fn expected_tree() -> Option<Box<TreeNode>> {
        let mut root = TreeNode::new(3);
        root.left = leaf(9);

        let mut right = TreeNode::new(20);
        right.left = leaf(15);
        right.right = leaf(7);

        root.right = Some(Box::new(right));
        Some(Box::new(root))
    }

    #[test]
    fn test_brute_force() {
        let preorder = vec![3, 9, 20, 15, 7];
        let inorder = vec![9, 3, 15, 20, 7];
        assert_eq!(build_tree_brute_force(preorder, inorder), expected_tree());
    }

    #[test]
    fn test_optimized() {
        let preorder = vec![3, 9, 20, 15, 7];
        let inorder = vec![9, 3, 15, 20, 7];
        assert_eq!(build_tree_optimized(preorder, inorder), expected_tree());
    }

    #[test]
    fn test_optimal() {
        let preorder = vec![3, 9, 20, 15, 7];
        let inorder = vec![9, 3, 15, 20, 7];
        assert_eq!(build_tree_optimal(preorder, inorder), expected_tree());
    }

    #[test]
    fn test_empty() {
        let preorder = vec![];
        let inorder = vec![];
        assert_eq!(
            build_tree_brute_force(preorder.clone(), inorder.clone()),
            None
        );
        assert_eq!(
            build_tree_optimized(preorder.clone(), inorder.clone()),
            None
        );
        assert_eq!(build_tree_optimal(preorder, inorder), None);
    }

    #[test]
    fn test_single_node() {
        let preorder = vec![-1];
        let inorder = vec![-1];
        assert_eq!(
            build_tree_brute_force(preorder.clone(), inorder.clone()),
            leaf(-1)
        );
        assert_eq!(
            build_tree_optimized(preorder.clone(), inorder.clone()),
            leaf(-1)
        );
        assert_eq!(build_tree_optimal(preorder, inorder), leaf(-1));
    }

    #[test]
    fn test_all_approaches_left_skewed() {
        let preorder = vec![1, 2, 3];
        let inorder = vec![3, 2, 1];

        let mut expected = TreeNode::new(1);
        let mut left1 = TreeNode::new(2);
        left1.left = leaf(3);
        expected.left = Some(Box::new(left1));
        let expected = Some(Box::new(expected));

        assert_eq!(
            build_tree_brute_force(preorder.clone(), inorder.clone()),
            expected
        );
        assert_eq!(
            build_tree_optimized(preorder.clone(), inorder.clone()),
            expected
        );
        assert_eq!(build_tree_optimal(preorder, inorder), expected);
    }
}
