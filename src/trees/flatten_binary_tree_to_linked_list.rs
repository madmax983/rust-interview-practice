//! # 114. Flatten Binary Tree to Linked List
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/flatten-binary-tree-to-linked-list/>
//!
//! Given the `root` of a binary tree, flatten the tree into a "linked list":
//! - The "linked list" should use the same `TreeNode` class where the `right` child pointer points to the next node in the list and the `left` child pointer is always `null`.
//! - The "linked list" should be in the same order as a pre-order traversal of the binary tree.
//!
//! This problem perfectly illustrates the challenges and solutions of mutating a tree structure in Rust.
//! It is an excellent vehicle to understand how `Rc<RefCell<T>>` is used for internal mutability when
//! we need to rearrange pointers dynamically, and why we must carefully manage `RefCell` borrows to avoid
//! runtime panics.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::trees::flatten_binary_tree_to_linked_list::{flatten_optimal, TreeNode};
//!
//! let mut root = TreeNode::new(1);
//! root.borrow_mut().left = Some(TreeNode::new(2));
//! root.borrow_mut().right = Some(TreeNode::new(5));
//!
//! let mut root_opt = Some(root);
//! flatten_optimal(&mut root_opt);
//!
//! assert_eq!(root_opt.as_ref().unwrap().borrow().val, 1);
//! assert!(root_opt.as_ref().unwrap().borrow().left.is_none());
//! assert_eq!(root_opt.as_ref().unwrap().borrow().right.as_ref().unwrap().borrow().val, 2);
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
    pub fn new(val: i32) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            val,
            left: None,
            right: None,
        }))
    }
}

/// Brute Force approach: Traverse, collect, and rebuild.
///
/// Time: O(n) - We visit every node during pre-order traversal and then loop again.
/// Space: O(n) - We allocate a `Vec` to store cloned `Rc` pointers to all nodes.
///
/// This approach cleanly avoids borrow checker issues by decoupling traversal
/// from mutation. We first collect all nodes in pre-order into a flat list,
/// and then we iterate over that list to rewrite the `left` and `right` pointers.
pub fn flatten_brute_force(root: &mut Option<Rc<RefCell<TreeNode>>>) {
    if root.is_none() {
        return;
    }

    // Helper closure to perform pre-order traversal and collect `Rc` pointers.
    #[allow(clippy::items_after_statements)]
    fn pre_order(node: Option<&Rc<RefCell<TreeNode>>>, nodes: &mut Vec<Rc<RefCell<TreeNode>>>) {
        if let Some(n) = node {
            nodes.push(Rc::clone(n));

            // GOTCHA: We must clone the `Rc` pointers to the children *before* recursively
            // passing them. If we passed `&n.borrow().left`, we would hold the `Ref` borrow
            // across the recursive call, which could lead to multiple active borrows.
            let left = n.borrow().left.clone();
            let right = n.borrow().right.clone();

            pre_order(left.as_ref(), nodes);
            pre_order(right.as_ref(), nodes);
        }
    }

    let mut nodes = Vec::new();
    pre_order(root.as_ref(), &mut nodes);

    // Rebuild the tree into a linked list
    for i in 0..nodes.len() {
        // RUST INSIGHT: We are guaranteed exclusive mutable access to this specific node's interior
        // because we're not currently borrowing it elsewhere.
        let mut curr = nodes[i].borrow_mut();
        curr.left = None;
        if i + 1 < nodes.len() {
            curr.right = Some(Rc::clone(&nodes[i + 1]));
        } else {
            curr.right = None;
        }
    }
}

/// Optimal approach: Reverse Post-Order Traversal.
///
/// Time: O(n) - We visit every node exactly once.
/// Space: O(h) - Where h is the height of the tree (O(n) in worst case), due to recursion stack.
///
/// The optimal solution achieves O(1) extra space (ignoring the recursion stack) by doing a
/// "reverse post-order" traversal: Right -> Left -> Root.
/// We keep track of the previously visited node. At each root, we set its right child to `prev`,
/// its left child to `None`, and then update `prev` to the current root.
pub fn flatten_optimal(root: &mut Option<Rc<RefCell<TreeNode>>>) {
    // We use an internal helper function that accepts a `prev` mutable reference.
    fn dfs(node: Option<&Rc<RefCell<TreeNode>>>, prev: &mut Option<Rc<RefCell<TreeNode>>>) {
        if let Some(n) = node {
            // RUST INSIGHT: To mutate `n` safely later, we must drop the `Ref` immediately.
            // We clone the `Rc`s pointing to the children, dropping the borrow of `n`.
            let right = n.borrow().right.clone();
            let left = n.borrow().left.clone();

            // Reverse post-order: explore right, then left.
            dfs(right.as_ref(), prev);
            dfs(left.as_ref(), prev);

            // Now it's safe to take a `RefMut` because no other borrows of `n` are active
            // in the current call stack frame.
            let mut n_mut = n.borrow_mut();
            n_mut.right.clone_from(prev);
            n_mut.left = None;

            // Update `prev` to point to the current node.
            *prev = Some(Rc::clone(n));
        }
    }

    let mut prev = None;
    dfs(root.as_ref(), &mut prev);
}

// Alternative Approaches:
// 1. **Iterative with Stack**: Pre-order traversal using a `Vec` as a stack. We pop a node, push
//    its right child, then push its left child. We then link the popped node to the next node
//    we peek from the stack.
// 2. **Morris Traversal**: An O(1) space algorithm (including avoiding recursion stack) that
//    temporarily modifies the tree's structure to create thread pointers back to ancestors.
//    While optimal for space, it's highly complex and rarely expected in an interview setting
//    unless specifically requested, especially given Rust's strict mutability rules making it tricky.

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to extract values into a Vec for easy assertion
    fn to_vec(root: Option<&Rc<RefCell<TreeNode>>>) -> Vec<Option<i32>> {
        let mut res = Vec::new();
        let mut curr = root.cloned();
        while let Some(n) = curr {
            res.push(Some(n.borrow().val));
            assert!(
                n.borrow().left.is_none(),
                "Flattened tree must not have left children"
            );
            let next = n.borrow().right.clone();
            curr = next;
        }
        res
    }

    #[test]
    fn test_happy_path() {
        // Tree:
        //     1
        //    / \
        //   2   5
        //  / \   \
        // 3   4   6
        let root = TreeNode::new(1);
        root.borrow_mut().left = Some(TreeNode::new(2));
        root.borrow_mut().left.as_ref().unwrap().borrow_mut().left = Some(TreeNode::new(3));
        root.borrow_mut().left.as_ref().unwrap().borrow_mut().right = Some(TreeNode::new(4));
        root.borrow_mut().right = Some(TreeNode::new(5));
        root.borrow_mut().right.as_ref().unwrap().borrow_mut().right = Some(TreeNode::new(6));

        // Test Brute Force
        let mut root_bf = Some(root);
        flatten_brute_force(&mut root_bf);
        assert_eq!(
            to_vec(root_bf.as_ref()),
            vec![Some(1), Some(2), Some(3), Some(4), Some(5), Some(6)]
        );

        // Test Optimal
        // Rebuild tree since brute force mutated it
        let root2 = TreeNode::new(1);
        root2.borrow_mut().left = Some(TreeNode::new(2));
        root2.borrow_mut().left.as_ref().unwrap().borrow_mut().left = Some(TreeNode::new(3));
        root2.borrow_mut().left.as_ref().unwrap().borrow_mut().right = Some(TreeNode::new(4));
        root2.borrow_mut().right = Some(TreeNode::new(5));
        root2
            .borrow_mut()
            .right
            .as_ref()
            .unwrap()
            .borrow_mut()
            .right = Some(TreeNode::new(6));

        let mut root_opt = Some(root2);
        flatten_optimal(&mut root_opt);
        assert_eq!(
            to_vec(root_opt.as_ref()),
            vec![Some(1), Some(2), Some(3), Some(4), Some(5), Some(6)]
        );
    }

    #[test]
    fn test_empty_tree() {
        let mut root1: Option<Rc<RefCell<TreeNode>>> = None;
        flatten_brute_force(&mut root1);
        assert!(root1.is_none());

        let mut root2: Option<Rc<RefCell<TreeNode>>> = None;
        flatten_optimal(&mut root2);
        assert!(root2.is_none());
    }

    #[test]
    fn test_single_node() {
        let mut root1 = Some(TreeNode::new(0));
        flatten_brute_force(&mut root1);
        assert_eq!(to_vec(root1.as_ref()), vec![Some(0)]);

        let mut root2 = Some(TreeNode::new(0));
        flatten_optimal(&mut root2);
        assert_eq!(to_vec(root2.as_ref()), vec![Some(0)]);
    }

    #[test]
    fn test_left_heavy_tree() {
        // Tree:
        //     1
        //    /
        //   2
        //  /
        // 3
        let root = TreeNode::new(1);
        root.borrow_mut().left = Some(TreeNode::new(2));
        root.borrow_mut().left.as_ref().unwrap().borrow_mut().left = Some(TreeNode::new(3));

        let mut root_opt = Some(root);
        flatten_optimal(&mut root_opt);
        assert_eq!(to_vec(root_opt.as_ref()), vec![Some(1), Some(2), Some(3)]);
    }
}
