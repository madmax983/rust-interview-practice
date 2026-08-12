//! # 114. Flatten Binary Tree to Linked List
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/flatten-binary-tree-to-linked-list/>
//!
//! Given the `root` of a binary tree, flatten the tree into a "linked list":
//! - The "linked list" should use the same `TreeNode` class where the `right` child pointer points to the next node in the list and the `left` child pointer is always `null`.
//! - The "linked list" should be in the same order as a pre-order traversal of the binary tree.
//!
//! This problem perfectly illustrates the challenges of in-place tree mutation in Rust. Because the tree is recursively nested with `Option<Rc<RefCell<TreeNode>>>`, we have to be extremely careful with mutable borrows and cloning reference-counted pointers to avoid runtime panics (e.g., borrowing a `RefCell` mutably twice).
//!
//! ## Approach
//!
//! We provide two approaches:
//! 1. **Straightforward (Post-order Traversal)**: We recursively flatten the right and left subtrees, then attach the flattened left subtree between the root and the flattened right subtree.
//!    - **Time Complexity:** O(N)
//!    - **Space Complexity:** O(H) for the call stack
//! 2. **Optimized (Morris-style iterative)**: We iterate through the tree. If a node has a left child, we find the rightmost node of the left subtree (the predecessor) and wire its right pointer to the current node's right child. Then we move the left child to the right child and nullify the left child.
//!    - **Time Complexity:** O(N)
//!    - **Space Complexity:** O(1)
//!
//! The optimized approach is idiomatic because it iteratively manipulates pointers without recursive function calls, dodging deep borrow checker nesting, while correctly managing Rc/RefCell clones to update links in place.
//!
//! ## Alternative approaches
//!
//! Another common approach is maintaining a global `prev` pointer and doing a reverse post-order traversal (`right` -> `left` -> `root`). While this is simple in Java/Python, managing a shared mutable `prev` pointer in Rust would require `Rc<RefCell<Option<Rc<RefCell<TreeNode>>>>>`, which is verbose and has a high runtime cost. The iterative approach is much cleaner for Rust.

use std::cell::RefCell;
use std::rc::Rc;

#[derive(Debug, PartialEq, Eq)]
pub struct TreeNode {
    pub val: i32,
    pub left: Option<Rc<RefCell<TreeNode>>>,
    pub right: Option<Rc<RefCell<TreeNode>>>,
}

impl TreeNode {
    #[inline]
    #[must_use]
    pub fn new(val: i32) -> Self {
        Self {
            val,
            left: None,
            right: None,
        }
    }
}

/// Recursively flattens the tree.
/// Returns the tail of the flattened tree to make appending O(1).
fn flatten_recursive_helper(
    node: Option<Rc<RefCell<TreeNode>>>,
) -> Option<Rc<RefCell<TreeNode>>> {
    if node.is_none() {
        return None;
    }

    let node_rc = node.unwrap();

    // We clone the Rc to maintain a handle to the children,
    // which allows us to temporarily borrow mutably later without conflict.
    let left_child = node_rc.borrow().left.clone();
    let right_child = node_rc.borrow().right.clone();

    let left_tail = flatten_recursive_helper(left_child.clone());
    let right_tail = flatten_recursive_helper(right_child.clone());

    let mut current_borrow = node_rc.borrow_mut();

    // RUST INSIGHT: Notice that we drop the borrow on `node_rc` when recursing,
    // and only borrow it mutably after recursion is complete to wire things up.
    if let Some(left) = left_child {
        current_borrow.right = Some(left);
        current_borrow.left = None;

        // Wire the tail of the left subtree to the head of the right subtree.
        if let Some(tail) = left_tail.as_ref() {
            tail.borrow_mut().right = right_child;
        } else {
            // GOTCHA: It's important to not accidentally drop the right child
            // if the left child existed but its tail was None (should not happen if logic is correct).
            // Actually, if left exists, left_tail won't be None.
        }
    }

    // Return the tail of the combined list
    if right_tail.is_some() {
        right_tail
    } else if left_tail.is_some() {
        left_tail
    } else {
        Some(node_rc.clone())
    }
}

/// O(N) Time, O(H) Space Recursive Approach
pub fn flatten_recursive(root: &mut Option<Rc<RefCell<TreeNode>>>) {
    flatten_recursive_helper(root.clone());
}

/// O(N) Time, O(1) Space Iterative Approach
/// This is the most idiomatic Rust approach for modifying in-place without recursion depth issues.
pub fn flatten(root: &mut Option<Rc<RefCell<TreeNode>>>) {
    let mut current = root.clone();

    while let Some(node) = current {
        let mut node_ref = node.borrow_mut();

        // RUST INSIGHT: We check if `left` exists. If it does, we need to wire it up.
        if node_ref.left.is_some() {
            let left_tree = node_ref.left.clone();
            let mut rightmost = left_tree.clone();

            // Find the rightmost node in the left subtree
            while let Some(right_node) = rightmost.clone() {
                let right_child = right_node.borrow().right.clone();
                if right_child.is_none() {
                    break;
                }
                rightmost = right_child;
            }

            // Wire the rightmost node's right pointer to the current node's right child
            if let Some(r) = rightmost {
                r.borrow_mut().right = node_ref.right.clone();
            }

            // Move left tree to right tree and nullify left
            node_ref.right = node_ref.left.take();
        }

        // Move to the next node (which is now guaranteed to be on the right)
        current = node_ref.right.clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn to_vec(root: &Option<Rc<RefCell<TreeNode>>>) -> Vec<Option<i32>> {
        let mut result = Vec::new();
        let mut current = root.clone();

        while let Some(node) = current {
            let node_ref = node.borrow();
            result.push(Some(node_ref.val));

            if node_ref.left.is_some() {
                panic!("Left child should be None after flattening");
            }

            current = node_ref.right.clone();
        }

        result
    }

    #[test]
    fn test_flatten_happy_path() {
        //   1
        //  / \
        // 2   5
        //  \   \
        //   3   6
        let root = Rc::new(RefCell::new(TreeNode::new(1)));
        let left = Rc::new(RefCell::new(TreeNode::new(2)));
        let right = Rc::new(RefCell::new(TreeNode::new(5)));

        left.borrow_mut().right = Some(Rc::new(RefCell::new(TreeNode::new(3))));
        right.borrow_mut().right = Some(Rc::new(RefCell::new(TreeNode::new(6))));

        root.borrow_mut().left = Some(left);
        root.borrow_mut().right = Some(right);

        let mut root_opt = Some(root);
        flatten(&mut root_opt);

        let vals = to_vec(&root_opt);
        assert_eq!(vals, vec![Some(1), Some(2), Some(3), Some(5), Some(6)]);
    }

    #[test]
    fn test_flatten_empty_tree() {
        let mut root: Option<Rc<RefCell<TreeNode>>> = None;
        flatten(&mut root);
        assert_eq!(to_vec(&root), vec![]);
    }

    #[test]
    fn test_flatten_single_node() {
        let mut root = Some(Rc::new(RefCell::new(TreeNode::new(0))));
        flatten(&mut root);
        assert_eq!(to_vec(&root), vec![Some(0)]);
    }

    #[test]
    fn test_flatten_left_heavy() {
        //     1
        //    /
        //   2
        //  /
        // 3
        let root = Rc::new(RefCell::new(TreeNode::new(1)));
        let left1 = Rc::new(RefCell::new(TreeNode::new(2)));
        let left2 = Rc::new(RefCell::new(TreeNode::new(3)));

        left1.borrow_mut().left = Some(left2);
        root.borrow_mut().left = Some(left1);

        let mut root_opt = Some(root);
        flatten(&mut root_opt);

        let vals = to_vec(&root_opt);
        assert_eq!(vals, vec![Some(1), Some(2), Some(3)]);
    }
}
