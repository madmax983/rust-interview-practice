//! # 572. Subtree of Another Tree
//!
//! Given the roots of two binary trees `root` and `subRoot`, return `true` if there is a subtree of `root` with the same structure and node values of `subRoot` and `false` otherwise.
//! A subtree of a binary tree `tree` is a tree that consists of a node in `tree` and all of this node's descendants. The tree `tree` could also be considered as a subtree of itself.
//!
//! - Difficulty: Easy
//! - LeetCode: <https://leetcode.com/problems/subtree-of-another-tree/>
//!
//! ## Why this matters in Rust
//! This problem provides excellent practice for Rust's `Option<Box<TreeNode>>` ownership model, lifetime semantics within recursive tree structures, and enum pattern matching. It highlights how to cleanly compare nested structures via exhaustive `match` statements instead of cumbersome `null` checks, ensuring safe traversal.
//!
//! ## Approach
//!
//! We explore two implementations:
//! 1.  **Brute Force Recursive**: At each node of the main tree, we check if it is identically equal to the `subRoot`. If not, we recursively check the left and right children. This approach clearly demonstrates double recursion and deep structural equality in Rust.
//! 2.  **Optimized Serialization (Preorder Traversal)**: We serialize both trees into strings representing their structures (handling `null` nodes explicitly to avoid false positives). Then we simply check if `subRoot`'s string is a substring of `root`'s string. This reduces the problem to string matching.

use std::cell::RefCell;
use std::rc::Rc;

// Definition for a binary tree node.
#[derive(Debug, PartialEq, Eq)]
pub struct TreeNode {
    pub val: i32,
    pub left: Option<Rc<RefCell<TreeNode>>>,
    pub right: Option<Rc<RefCell<TreeNode>>>,
}

impl TreeNode {
    #[inline]
    pub fn new(val: i32) -> Self {
        TreeNode {
            val,
            left: None,
            right: None,
        }
    }
}

/// Helper function to check if two trees are identical.
///
/// RUST INSIGHT: We take `&Option<Rc<RefCell<TreeNode>>>` to avoid cloning the `Rc` pointers
/// during traversal. We borrow the values for comparison. Pattern matching perfectly handles
/// the concurrent `None` / `Some` cases.
fn is_identical(
    node1: &Option<Rc<RefCell<TreeNode>>>,
    node2: &Option<Rc<RefCell<TreeNode>>>,
) -> bool {
    match (node1, node2) {
        // Both nodes are None (empty trees are identical)
        (None, None) => true,
        // Both nodes exist, we must compare their values and structure recursively
        (Some(n1), Some(n2)) => {
            let n1_ref = n1.borrow();
            let n2_ref = n2.borrow();
            n1_ref.val == n2_ref.val
                && is_identical(&n1_ref.left, &n2_ref.left)
                && is_identical(&n1_ref.right, &n2_ref.right)
        }
        // One is None and the other is Some, so they are not identical
        // RUST INSIGHT: The compiler guarantees we handled all cases of the tuple!
        _ => false,
    }
}

/// Brute Force Recursive Approach
///
/// We traverse the `root` tree. For each node, we check if the subtree starting at that node
/// is structurally identical to `subRoot`.
///
/// - **Time Complexity**: O(M * N), where M is the number of nodes in `root` and N is the number of nodes in `subRoot`. In the worst case, we might do an O(N) comparison for every node in `root`.
/// - **Space Complexity**: O(H), where H is the height of `root` (due to recursion stack). In the worst case (skewed tree), it could be O(M).
pub fn is_subtree_brute_force(
    root: Option<Rc<RefCell<TreeNode>>>,
    sub_root: Option<Rc<RefCell<TreeNode>>>,
) -> bool {
    // If root is empty, it can't contain a non-empty sub_root (constraints say subRoot is non-empty, but we handle it safely)
    if root.is_none() {
        return sub_root.is_none();
    }

    // Check if trees are identical starting from current node
    if is_identical(&root, &sub_root) {
        return true;
    }

    // Otherwise, recursively check left and right subtrees
    let node = root.as_ref().unwrap().borrow();
    is_subtree_brute_force(node.left.clone(), sub_root.clone())
        || is_subtree_brute_force(node.right.clone(), sub_root)
}

/// Helper function to serialize the tree in preorder
fn serialize(node: &Option<Rc<RefCell<TreeNode>>>, buf: &mut String) {
    match node {
        None => {
            // Use a specific marker for null nodes to prevent overlap ambiguity
            buf.push_str("#,");
        }
        Some(n) => {
            let n_ref = n.borrow();
            // Prepend a marker to distinguish e.g., '12' from '2' inside '12'
            buf.push('^');
            buf.push_str(&n_ref.val.to_string());
            buf.push(',');
            serialize(&n_ref.left, buf);
            serialize(&n_ref.right, buf);
        }
    }
}

/// Optimized Approach: Preorder Serialization
///
/// We serialize both trees to strings representing their preorder traversals, including
/// explicit markers for null nodes and value boundaries. Then, we check if `subRoot`'s string
/// is a substring of `root`'s string.
///
/// - **Time Complexity**: O(M + N), where M and N are the number of nodes in `root` and `subRoot`. Traversal takes O(M + N). Substring matching (using Rust's standard library which employs optimizations like Two-Way algorithm) is very efficient, typically O(M + N).
/// - **Space Complexity**: O(M + N) to store the serialized strings.
pub fn is_subtree_optimized(
    root: Option<Rc<RefCell<TreeNode>>>,
    sub_root: Option<Rc<RefCell<TreeNode>>>,
) -> bool {
    let mut root_str = String::new();
    let mut sub_root_str = String::new();

    serialize(&root, &mut root_str);
    serialize(&sub_root, &mut sub_root_str);

    // RUST INSIGHT: `contains` on strings is implemented efficiently in the standard library.
    root_str.contains(&sub_root_str)
}

/// Main entry point
pub fn is_subtree(
    root: Option<Rc<RefCell<TreeNode>>>,
    sub_root: Option<Rc<RefCell<TreeNode>>>,
) -> bool {
    is_subtree_optimized(root, sub_root)
}

// Alternative Approaches:
// 1. **KMP Algorithm**: You could implement KMP for the string matching to guarantee O(M + N) time even in pathologically crafted worst-case scenarios, but standard `.contains()` is usually sufficient and simpler.
// 2. **Merkle Hashing**: We could compute a hash for each subtree bottom-up. Two subtrees are identical if their hashes match. Time is O(M + N) but requires handling hash collisions.

#[cfg(test)]
mod tests {
    use super::*;

    // Helper macro to easily create trees
    macro_rules! tree {
        ($val:expr) => {
            Some(Rc::new(RefCell::new(TreeNode::new($val))))
        };
        ($val:expr, $left:expr, $right:expr) => {
            Some(Rc::new(RefCell::new(TreeNode {
                val: $val,
                left: $left,
                right: $right,
            })))
        };
    }

    #[test]
    fn test_brute_force_true() {
        // root: [3,4,5,1,2]
        let sub_root = tree!(4, tree!(1), tree!(2));
        let root = tree!(3, sub_root.clone(), tree!(5));
        assert!(is_subtree_brute_force(root, sub_root));
    }

    #[test]
    fn test_brute_force_false() {
        // root: [3,4,5,1,2,null,null,null,null,0]
        let sub_root = tree!(4, tree!(1), tree!(2));
        let left_branch = tree!(4, tree!(1), tree!(2, tree!(0), None));
        let root = tree!(3, left_branch, tree!(5));
        assert!(!is_subtree_brute_force(root, sub_root));
    }

    #[test]
    fn test_optimized_true() {
        let sub_root = tree!(4, tree!(1), tree!(2));
        let root = tree!(3, sub_root.clone(), tree!(5));
        assert!(is_subtree_optimized(root, sub_root));
    }

    #[test]
    fn test_optimized_false() {
        let sub_root = tree!(4, tree!(1), tree!(2));
        let left_branch = tree!(4, tree!(1), tree!(2, tree!(0), None));
        let root = tree!(3, left_branch, tree!(5));
        assert!(!is_subtree_optimized(root, sub_root));
    }

    #[test]
    fn test_edge_case_same_values_different_structure() {
        // root: [1, 2], subRoot: [1, null, 2]
        let root = tree!(1, tree!(2), None);
        let sub_root = tree!(1, None, tree!(2));
        assert!(!is_subtree_optimized(root.clone(), sub_root.clone()));
        assert!(!is_subtree_brute_force(root, sub_root));
    }

    #[test]
    fn test_edge_case_value_substring_trap() {
        // root: [12], subRoot: [2]
        // This checks that our string serialization boundaries correctly prevent
        // '2' from matching inside '12'.
        let root = tree!(12);
        let sub_root = tree!(2);
        assert!(!is_subtree_optimized(root.clone(), sub_root.clone()));
        assert!(!is_subtree_brute_force(root, sub_root));
    }
}
