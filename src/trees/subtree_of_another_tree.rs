//! # 572. Subtree of Another Tree
//!
//! Difficulty: Easy
//! Link: <https://leetcode.com/problems/subtree-of-another-tree/>
//!
//! Given the roots of two binary trees `root` and `subRoot`, return `true` if there is a subtree
//! of `root` with the same structure and node values of `subRoot` and `false` otherwise.
//!
//! This problem provides an excellent context for comparing pattern matching over nested `Option<Rc<RefCell<TreeNode>>>`
//! (a structural, pointer-heavy approach) versus leveraging Rust's `String` and serialization
//! for an optimized comparison. It shows how modeling state directly versus transforming state changes
//! algorithms.
//!
//! ## Examples
//!
//! ```
//! use std::rc::Rc;
//! use std::cell::RefCell;
//! use rust_interview_practice::trees::subtree_of_another_tree::{is_subtree, TreeNode};
//!
//! let mut sub = TreeNode::new(4);
//! sub.borrow_mut().left = Some(TreeNode::new(1));
//! sub.borrow_mut().right = Some(TreeNode::new(2));
//!
//! let mut root = TreeNode::new(3);
//! root.borrow_mut().left = Some(Rc::clone(&sub));
//! root.borrow_mut().right = Some(TreeNode::new(5));
//!
//! assert_eq!(is_subtree(Some(root), Some(sub)), true);
//! ```

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
    #[must_use]
    pub fn new(val: i32) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            val,
            left: None,
            right: None,
        }))
    }
}

/// Brute Force approach: Recursive Tree Traversal
///
/// Time Complexity: O(M * N) - Where M is the number of nodes in `root` and N is the number of nodes in `subRoot`.
/// For every node in `root`, we might check up to N nodes.
/// Space Complexity: O(H_root + H_subRoot) - The call stack depth is determined by tree height.
///
/// This approach traverses the `root` tree. At each node, it initiates a recursive check (`is_same_tree`)
/// to see if the subtree starting there exactly matches `subRoot`. This is straightforward but potentially slow
/// if there are many nodes with identical values.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_brute_force(
    root: Option<Rc<RefCell<TreeNode>>>,
    sub_root: Option<Rc<RefCell<TreeNode>>>,
) -> bool {
    // Helper to check if two trees are identical structurally and by value.
    fn is_same_tree(p: &Option<Rc<RefCell<TreeNode>>>, q: &Option<Rc<RefCell<TreeNode>>>) -> bool {
        // RUST INSIGHT: Exhaustive pattern matching naturally handles all combinations of Node/Null.
        match (p, q) {
            (Some(n_p), Some(n_q)) => {
                let (borrowed_p, borrowed_q) = (n_p.borrow(), n_q.borrow());
                borrowed_p.val == borrowed_q.val
                    && is_same_tree(&borrowed_p.left, &borrowed_q.left)
                    && is_same_tree(&borrowed_p.right, &borrowed_q.right)
            }
            (None, None) => true,
            _ => false,
        }
    }

    match root {
        Some(node) => {
            if is_same_tree(&Some(Rc::clone(&node)), &sub_root) {
                return true;
            }
            let borrowed = node.borrow();
            is_subtree_brute_force(borrowed.left.clone(), sub_root.clone())
                || is_subtree_brute_force(borrowed.right.clone(), sub_root)
        }
        None => sub_root.is_none(),
    }
}

/// Optimized approach: String Serialization
///
/// Time Complexity: O(M + N) - We serialize both trees in linear time. `contains` takes O(M+N).
/// Space Complexity: O(M + N) - We store string representations of both trees.
///
/// By converting the tree structures into unique strings (using Pre-Order traversal and markers for nulls),
/// we transform a 2D graph matching problem into a 1D substring matching problem.
/// Rust's underlying `str::contains` uses the highly optimized Two-Way algorithm,
/// making this approach exceptionally fast despite the initial string allocation overhead.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_optimized(
    root: Option<Rc<RefCell<TreeNode>>>,
    sub_root: Option<Rc<RefCell<TreeNode>>>,
) -> bool {
    // We allocate roughly M*4 and N*4 capacity as a reasonable guess, but `format!` handles dynamic sizing.
    // GOTCHA: It is crucial to denote left/right boundaries uniquely, typically by enclosing values or distinct null markers.
    fn serialize(node: &Option<Rc<RefCell<TreeNode>>>, out: &mut String) {
        match node {
            Some(n) => {
                let borrowed = n.borrow();
                // Adding unique markers like '^' before the value prevents substring overlap errors
                // e.g., value "12" vs "2" in a subtree search.
                out.push('^');
                out.push_str(&borrowed.val.to_string());
                serialize(&borrowed.left, out);
                serialize(&borrowed.right, out);
            }
            None => out.push('#'), // Essential to mark null branches to capture structural uniqueness
        }
    }

    let mut root_str = String::with_capacity(256);
    let mut sub_root_str = String::with_capacity(128);

    serialize(&root, &mut root_str);
    serialize(&sub_root, &mut sub_root_str);

    // RUST INSIGHT: `str::contains` uses the Two-Way algorithm internally, which operates in O(N + M) worst-case time
    // without needing external space like KMP.
    root_str.contains(&sub_root_str)
}

/// Main entry point defaulting to the optimized string match.
#[must_use]
pub fn is_subtree(
    root: Option<Rc<RefCell<TreeNode>>>,
    sub_root: Option<Rc<RefCell<TreeNode>>>,
) -> bool {
    is_subtree_optimized(root, sub_root)
}

// Alternative Approaches:
// 1. **KMP Algorithm / Merkle Hashing**: You could compute hashes for each subtree bottom-up.
//    If a node's hash matches the sub_root hash, you verify if they are identical.
//    This achieves O(M+N) time and avoids string allocations, but requires complex custom hash combining.

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(val: i32) -> Option<Rc<RefCell<TreeNode>>> {
        Some(TreeNode::new(val))
    }

    #[test]
    fn test_happy_path() {
        // root = [3,4,5,1,2], subRoot = [4,1,2]
        //      3
        //     / \
        //    4   5
        //   / \
        //  1   2
        let root = TreeNode::new(3);
        let sub = TreeNode::new(4);
        sub.borrow_mut().left = leaf(1);
        sub.borrow_mut().right = leaf(2);

        root.borrow_mut().left = Some(Rc::clone(&sub));
        root.borrow_mut().right = leaf(5);

        let sub_clone = Some(Rc::clone(&sub));
        assert!(is_subtree_brute_force(Some(Rc::clone(&root)), sub_clone.clone()));
        assert!(is_subtree_optimized(Some(Rc::clone(&root)), sub_clone.clone()));
        assert!(is_subtree(Some(root), sub_clone));
    }

    #[test]
    fn test_edge_case_overlapping_strings() {
        // A tricky case where string values overlap if not delimited.
        // root = [12], subRoot = [2]
        // If we serialized 12 as "12" and 2 as "2", "12".contains("2") is true.
        // Delimiters e.g., "^12" and "^2" fix this.
        let root = TreeNode::new(12);
        let sub = TreeNode::new(2);

        assert!(!is_subtree_brute_force(Some(Rc::clone(&root)), Some(Rc::clone(&sub))));
        assert!(!is_subtree_optimized(Some(Rc::clone(&root)), Some(Rc::clone(&sub))));
        assert!(!is_subtree(Some(root), Some(sub)));
    }

    #[test]
    fn test_stress_boundary() {
        // Mismatched internal structure, overlapping values.
        // root = [3,4,5,1,2,null,null,null,null,0]
        //      3
        //     / \
        //    4   5
        //   / \
        //  1   2
        //     /
        //    0
        // subRoot = [4,1,2]

        let root = TreeNode::new(3);
        let sub_original = TreeNode::new(4);
        sub_original.borrow_mut().left = leaf(1);

        let node2 = TreeNode::new(2);
        node2.borrow_mut().left = leaf(0);
        sub_original.borrow_mut().right = Some(Rc::clone(&node2));

        root.borrow_mut().left = Some(Rc::clone(&sub_original));
        root.borrow_mut().right = leaf(5);

        // Sub tree we are looking for is missing the 0
        let sub_search = TreeNode::new(4);
        sub_search.borrow_mut().left = leaf(1);
        sub_search.borrow_mut().right = leaf(2);

        assert!(!is_subtree_brute_force(Some(Rc::clone(&root)), Some(Rc::clone(&sub_search))));
        assert!(!is_subtree_optimized(Some(Rc::clone(&root)), Some(Rc::clone(&sub_search))));
        assert!(!is_subtree(Some(root), Some(sub_search)));
    }
}
