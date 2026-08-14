//! # 114. Flatten Binary Tree to Linked List
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/flatten-binary-tree-to-linked-list/>
//!
//! This problem matters in Rust because it forces us to deal with complex pointer manipulation
//! and in-place mutation of a recursive data structure. It is a natural fit for demonstrating the power
//! and trade-offs of `Rc<RefCell<T>>` when multiple mutable references seemingly need to overlap.

use std::cell::RefCell;
use std::rc::Rc;

// Definition for a binary tree node.
// RUST INSIGHT: We use `Option<Rc<RefCell<TreeNode>>>` instead of `Option<Box<TreeNode>>` here.
// This is because flattening the tree requires complex pointer rewiring (often referencing the
// same node from multiple places temporarily during the process), which `Rc<RefCell<T>>`
// elegantly handles via shared ownership and interior mutability.
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

/// Approach 1: Brute Force (Pre-order Traversal into Vec)
///
/// Time Complexity: O(N) where N is the number of nodes.
/// Space Complexity: O(N) to store the nodes in a `Vec`.
///
/// We first do a standard pre-order traversal and clone the `Rc` pointers into a `Vec`.
/// Then, we iterate through the `Vec` and rewire the `left` and `right` pointers.
/// This approach is simple and avoids complex concurrent borrows.
pub fn flatten_brute_force(root: &mut Option<Rc<RefCell<TreeNode>>>) {
    if root.is_none() {
        return;
    }

    // Helper for pre-order traversal
    fn preorder(node: Option<&Rc<RefCell<TreeNode>>>, nodes: &mut Vec<Rc<RefCell<TreeNode>>>) {
        if let Some(n) = node {
            nodes.push(Rc::clone(n));
            let borrowed = n.borrow();
            preorder(borrowed.left.as_ref(), nodes);
            preorder(borrowed.right.as_ref(), nodes);
        }
    }

    let mut nodes = Vec::new();
    preorder(root.as_ref(), &mut nodes);

    // Rewire the tree into a linked list
    for i in 0..nodes.len().saturating_sub(1) {
        let mut curr = nodes[i].borrow_mut();
        curr.left = None;
        curr.right = Some(Rc::clone(&nodes[i + 1]));
    }

    // Clear the last node's children
    if let Some(last) = nodes.last() {
        let mut curr = last.borrow_mut();
        curr.left = None;
        curr.right = None;
    }
}

/// Approach 2: Optimized (Reverse Pre-order Traversal)
///
/// Time Complexity: O(N)
/// Space Complexity: O(H) for the recursion stack (where H is the height of the tree).
///
/// We traverse the tree in a "reverse pre-order" (Right, Left, Current). By doing this,
/// we visit the nodes in the exact reverse order of the flattened list. We keep a `prev`
/// pointer to link the current node's right child to the previously visited node.
pub fn flatten_optimized(root: &mut Option<Rc<RefCell<TreeNode>>>) {
    fn rev_preorder(node: Option<&Rc<RefCell<TreeNode>>>, prev: &mut Option<Rc<RefCell<TreeNode>>>) {
        if let Some(n) = node {
            // GOTCHA: We must extract the child pointers and drop our borrow of `n` before recursing.
            // If we kept `n.borrow()` alive during the recursive calls, we would hit a RefCell
            // panic (BorrowMutError) when trying to mutate `prev` or `n` later in the chain.
            let right_child = n.borrow().right.clone();
            let left_child = n.borrow().left.clone();

            rev_preorder(right_child.as_ref(), prev);
            rev_preorder(left_child.as_ref(), prev);

            let mut curr = n.borrow_mut();
            curr.right.clone_from(prev);
            curr.left = None;

            *prev = Some(Rc::clone(n));
        }
    }

    let mut prev = None;
    rev_preorder(root.as_ref(), &mut prev);
}

/// Approach 3: Optimal (Morris Traversal-like approach)
///
/// Time Complexity: O(N)
/// Space Complexity: O(1) auxiliary space (no recursion stack).
///
/// For each node, if it has a left child, we find the rightmost node in the left subtree.
/// We attach the current node's right subtree to that rightmost node. Then, we move the
/// entire left subtree to the right, clearing the left child. We continue this process
/// moving down the right side of the tree.
pub fn flatten_optimal(root: &mut Option<Rc<RefCell<TreeNode>>>) {
    let mut curr = root.clone();

    while let Some(node) = curr {
        let left_child = node.borrow().left.clone();

        if let Some(left) = left_child {
            // Find the rightmost node of the left subtree
            let mut rightmost = left.clone();
            loop {
                let next_right = rightmost.borrow().right.clone();
                match next_right {
                    Some(r) => rightmost = r,
                    None => break,
                }
            }

            // Connect the original right subtree to the rightmost node of the left subtree
            rightmost.borrow_mut().right.clone_from(&node.borrow().right);

            // Move the left subtree to the right, and clear the left child
            let mut curr_mut = node.borrow_mut();
            curr_mut.right = Some(left);
            curr_mut.left = None;
        }

        // Move to the next node on the right
        let next = node.borrow().right.clone();
        curr = next;
    }
}

/// Main entry point
pub fn flatten(root: &mut Option<Rc<RefCell<TreeNode>>>) {
    flatten_optimal(root);
}

// Alternative approaches:
// 1. **Iterative Stack Pre-order**: Similar to the brute force, but using a `Vec` as a stack
//    to iteratively traverse the tree while rewiring pointers on the fly. Space complexity is O(H).

#[cfg(test)]
mod tests {
    use super::*;

    fn create_node(val: i32) -> Rc<RefCell<TreeNode>> {
        Rc::new(RefCell::new(TreeNode::new(val)))
    }

    fn to_vec(root: Option<&Rc<RefCell<TreeNode>>>) -> Vec<Option<i32>> {
        let mut result = Vec::new();
        let mut curr = root.cloned();
        while let Some(node) = curr {
            let b = node.borrow();
            if b.left.is_some() {
                // If it's not strictly a linked list, signal an error in layout
                result.push(None);
            }
            result.push(Some(b.val));
            curr = b.right.clone();
        }
        result
    }

    fn setup_tree() -> std::rc::Rc<std::cell::RefCell<TreeNode>> {
        //       1
        //      / \
        //     2   5
        //    / \   \
        //   3   4   6
        let root = create_node(1);
        let n2 = create_node(2);
        let n3 = create_node(3);
        let n4 = create_node(4);
        let n5 = create_node(5);
        let n6 = create_node(6);

        n2.borrow_mut().left = Some(Rc::clone(&n3));
        n2.borrow_mut().right = Some(Rc::clone(&n4));
        n5.borrow_mut().right = Some(Rc::clone(&n6));

        root.borrow_mut().left = Some(Rc::clone(&n2));
        root.borrow_mut().right = Some(Rc::clone(&n5));

        root
    }

    #[test]
    fn test_happy_path_brute_force() {
        let mut root = Some(setup_tree());
        flatten_brute_force(&mut root);
        assert_eq!(to_vec(root.as_ref()), vec![Some(1), Some(2), Some(3), Some(4), Some(5), Some(6)]);
    }

    #[test]
    fn test_happy_path_optimized() {
        let mut root = Some(setup_tree());
        flatten_optimized(&mut root);
        assert_eq!(to_vec(root.as_ref()), vec![Some(1), Some(2), Some(3), Some(4), Some(5), Some(6)]);
    }

    #[test]
    fn test_happy_path_optimal() {
        let mut root = Some(setup_tree());
        flatten_optimal(&mut root);
        assert_eq!(to_vec(root.as_ref()), vec![Some(1), Some(2), Some(3), Some(4), Some(5), Some(6)]);
    }

    #[test]
    fn test_edge_cases() {
        let mut empty: Option<Rc<RefCell<TreeNode>>> = None;
        flatten(&mut empty);
        assert!(empty.is_none());

        let mut single = Some(create_node(1));
        flatten(&mut single);
        assert_eq!(to_vec(single.as_ref()), vec![Some(1)]);
    }

    #[test]
    fn test_stress_boundaries() {
        // Tree completely unbalanced to the left
        //     1
        //    /
        //   2
        //  /
        // 3
        let root = create_node(1);
        let n2 = create_node(2);
        let n3 = create_node(3);
        n2.borrow_mut().left = Some(Rc::clone(&n3));
        root.borrow_mut().left = Some(Rc::clone(&n2));

        let mut tree = Some(root);
        flatten(&mut tree);
        assert_eq!(to_vec(tree.as_ref()), vec![Some(1), Some(2), Some(3)]);

        // Tree completely unbalanced to the right
        // 1
        //  \
        //   2
        //    \
        //     3
        let root2 = create_node(1);
        let n2_2 = create_node(2);
        let n3_2 = create_node(3);
        n2_2.borrow_mut().right = Some(Rc::clone(&n3_2));
        root2.borrow_mut().right = Some(Rc::clone(&n2_2));

        let mut tree2 = Some(root2);
        flatten(&mut tree2);
        assert_eq!(to_vec(tree2.as_ref()), vec![Some(1), Some(2), Some(3)]);
    }
}
