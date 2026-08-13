//! # 114. Flatten Binary Tree to Linked List
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/flatten-binary-tree-to-linked-list/>
//!
//! Given the `root` of a binary tree, flatten the tree into a "linked list":
//! - The "linked list" should use the same `TreeNode` class where the `right` child pointer points to the next node in the list and the `left` child pointer is always null.
//! - The "linked list" should be in the same order as a pre-order traversal of the binary tree.
//!
//! This problem is an excellent exercise in understanding Rust's ownership model, specifically `Rc<RefCell<T>>`,
//! which is often used for graph-like structures requiring multiple mutable pointers. It demonstrates how to safely
//! mutate a tree in place, avoiding double borrows, while efficiently manipulating references.
//!
//! ## Examples
//!
//! ```
//! use std::rc::Rc;
//! use std::cell::RefCell;
//! use rust_interview_practice::trees::flatten_binary_tree_to_linked_list::{flatten, TreeNode};
//!
//! let mut root = TreeNode::new(1);
//! root.left = Some(Rc::new(RefCell::new(TreeNode::new(2))));
//! root.right = Some(Rc::new(RefCell::new(TreeNode::new(5))));
//!
//! let rc_root = Some(Rc::new(RefCell::new(root)));
//! flatten(&rc_root);
//!
//! let root_ref = rc_root.as_ref().unwrap().borrow();
//! assert_eq!(root_ref.val, 1);
//! assert!(root_ref.left.is_none());
//! let right1 = root_ref.right.as_ref().unwrap().borrow();
//! assert_eq!(right1.val, 2);
//! assert!(right1.left.is_none());
//! let right2 = right1.right.as_ref().unwrap().borrow();
//! assert_eq!(right2.val, 5);
//! ```
//!
//! ## Constraints
//!
//! - The number of nodes in the tree is in the range `[0, 2000]`.
//! - `-100 <= Node.val <= 100`

use std::cell::RefCell;
use std::rc::Rc;

// Definition for a binary tree node.
#[derive(Debug, PartialEq, Eq)]
pub struct TreeNode {
    pub val: i32,
    pub left: Option<Rc<RefCell<Self>>>,
    pub right: Option<Rc<RefCell<Self>>>,
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

/// Brute Force approach: Preorder traversal to Vec, then rewire.
///
/// Time: O(N) - One pass to collect nodes, one pass to rewire.
/// Space: O(N) - We store all `Rc<RefCell<TreeNode>>` clones in a `Vec`.
///
/// This approach simplifies the problem by first flattening the nodes into an array
/// using standard preorder traversal, and then sequentially rewiring their `left` and `right` pointers.
#[allow(clippy::needless_pass_by_value)]
pub fn flatten_brute_force(root: &Option<Rc<RefCell<TreeNode>>>) {
    if root.is_none() {
        return;
    }

    let mut nodes = Vec::new();

    // Helper to collect nodes in preorder
    fn preorder(node: &Option<Rc<RefCell<TreeNode>>>, nodes: &mut Vec<Rc<RefCell<TreeNode>>>) {
        if let Some(n) = node {
            // RUST INSIGHT: Cloning an Rc only increments the reference count (O(1)),
            // it does not deep copy the underlying RefCell data.
            nodes.push(Rc::clone(n));
            let n_borrowed = n.borrow();
            preorder(&n_borrowed.left, nodes);
            preorder(&n_borrowed.right, nodes);
        }
    }

    preorder(root, &mut nodes);

    // Rewire pointers
    for i in 0..nodes.len() {
        let mut curr = nodes[i].borrow_mut();
        curr.left = None;
        if i + 1 < nodes.len() {
            curr.right = Some(Rc::clone(&nodes[i + 1]));
        } else {
            curr.right = None;
        }
    }
}

/// Optimized approach: Post-order recursive traversal (Right, Left, Root).
///
/// Time: O(N) - We visit every node exactly once.
/// Space: O(H) - Recursion stack space (O(N) worst case, O(log N) balanced).
///
/// By visiting the right subtree first, then the left subtree, and keeping track of the
/// `prev` (previously visited) node, we can construct the flattened tree backwards.
/// This elegantly handles the rewiring without needing an external array.
#[allow(clippy::needless_pass_by_value)]
pub fn flatten_optimized(root: &Option<Rc<RefCell<TreeNode>>>) {
    // Shared state to keep track of the previously visited node in our reverse post-order traversal
    let mut prev: Option<Rc<RefCell<TreeNode>>> = None;

    fn reverse_post_order(node: &Option<Rc<RefCell<TreeNode>>>, prev: &mut Option<Rc<RefCell<TreeNode>>>) {
        if let Some(n) = node {
            // Traverse right then left
            // RUST INSIGHT: We must drop the borrow before recursive calls to avoid
            // BorrowMutError, since the recursive calls will borrow the same nodes!
            // GOTCHA: Do not hold `n.borrow_mut()` across recursive calls.

            // Extract the child references by temporarily borrowing.
            let right_child = n.borrow().right.clone();
            let left_child = n.borrow().left.clone();

            reverse_post_order(&right_child, prev);
            reverse_post_order(&left_child, prev);

            // Now safely borrow `n` mutably to update pointers.
            let mut current = n.borrow_mut();
            current.right = prev.clone();
            current.left = None;

            // Update prev for the next step up the call stack
            *prev = Some(Rc::clone(n));
        }
    }

    reverse_post_order(root, &mut prev);
}

/// Optimal approach: Iterative, Morris Traversal inspired (O(1) space).
///
/// Time: O(N) - We visit nodes and re-traverse some right spine links, but amortized O(N).
/// Space: O(1) - Constant auxiliary space, no recursion stack.
///
/// The algorithm relies on finding the rightmost node of the left subtree (the predecessor in inorder traversal,
/// or in this case, the node that immediately precedes the current right child in preorder).
/// We then attach the current right subtree to that predecessor's right child, move the left subtree
/// to become the new right subtree, and proceed down the tree to the right.
#[allow(clippy::needless_pass_by_value)]
pub fn flatten_optimal(root: &Option<Rc<RefCell<TreeNode>>>) {
    let mut curr = root.clone();

    while let Some(current_node) = curr {
        // We only need to act if there is a left child
        let left_child = current_node.borrow().left.clone();

        if let Some(left) = left_child {
            // Find the rightmost node in the left subtree
            let mut rightmost = left.clone();
            loop {
                // RUST INSIGHT: We use block scoping or careful borrows to avoid holding
                // a Ref or RefMut across iterations.
                let next_right = rightmost.borrow().right.clone();
                match next_right {
                    Some(n) => rightmost = n,
                    None => break,
                }
            }

            // Rewire: rightmost node's right child becomes current's right child
            rightmost.borrow_mut().right = current_node.borrow().right.clone();

            // Move left subtree to right, and nullify left
            let mut current_mut = current_node.borrow_mut();
            current_mut.right = Some(left);
            current_mut.left = None;
        }

        // Move to the next right node
        let next_node = current_node.borrow().right.clone();
        curr = next_node;
    }
}

/// Main entry point - uses the optimal space O(1) iterative approach.
pub fn flatten(root: &Option<Rc<RefCell<TreeNode>>>) {
    flatten_optimal(root);
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to extract a vec of values from a flattened tree (following right pointers)
    fn to_vec(root: &Option<Rc<RefCell<TreeNode>>>) -> Vec<i32> {
        let mut result = Vec::new();
        let mut curr = root.clone();
        while let Some(node) = curr {
            let n = node.borrow();
            assert!(n.left.is_none(), "Flattened tree must not have left children!");
            result.push(n.val);
            curr = n.right.clone();
        }
        result
    }

    // Helper to build a test tree
    fn build_test_tree() -> Option<Rc<RefCell<TreeNode>>> {
        //       1
        //      / \
        //     2   5
        //    / \   \
        //   3   4   6
        let mut root = TreeNode::new(1);
        let mut node2 = TreeNode::new(2);
        let node3 = TreeNode::new(3);
        let node4 = TreeNode::new(4);
        let mut node5 = TreeNode::new(5);
        let node6 = TreeNode::new(6);

        node2.left = Some(Rc::new(RefCell::new(node3)));
        node2.right = Some(Rc::new(RefCell::new(node4)));

        node5.right = Some(Rc::new(RefCell::new(node6)));

        root.left = Some(Rc::new(RefCell::new(node2)));
        root.right = Some(Rc::new(RefCell::new(node5)));

        Some(Rc::new(RefCell::new(root)))
    }

    #[test]
    fn test_flatten_brute_force() {
        let root = build_test_tree();
        flatten_brute_force(&root);
        assert_eq!(to_vec(&root), vec![1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn test_flatten_optimized() {
        let root = build_test_tree();
        flatten_optimized(&root);
        assert_eq!(to_vec(&root), vec![1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn test_flatten_optimal() {
        let root = build_test_tree();
        flatten_optimal(&root);
        assert_eq!(to_vec(&root), vec![1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn test_empty_tree() {
        let root = None;
        flatten_optimal(&root);
        assert_eq!(to_vec(&root), vec![]);
    }

    #[test]
    fn test_single_node() {
        let root = Some(Rc::new(RefCell::new(TreeNode::new(1))));
        flatten_optimal(&root);
        assert_eq!(to_vec(&root), vec![1]);
    }

    #[test]
    fn test_left_heavy_tree() {
        //      1
        //     /
        //    2
        //   /
        //  3
        let mut root = TreeNode::new(1);
        let mut node2 = TreeNode::new(2);
        let node3 = TreeNode::new(3);

        node2.left = Some(Rc::new(RefCell::new(node3)));
        root.left = Some(Rc::new(RefCell::new(node2)));

        let rc_root = Some(Rc::new(RefCell::new(root)));

        flatten_optimal(&rc_root);
        assert_eq!(to_vec(&rc_root), vec![1, 2, 3]);
    }
}
