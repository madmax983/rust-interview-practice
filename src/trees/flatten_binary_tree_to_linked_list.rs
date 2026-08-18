//! # 114. Flatten Binary Tree to Linked List
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/flatten-binary-tree-to-linked-list/>
//!
//! This problem is a natural fit for Rust's `Rc<RefCell<TreeNode>>` and demonstrates why
//! handling mutable aliasing and pointer manipulations is strict in Rust. In-place mutation
//! of trees requires careful handling of ownership to avoid violating the borrow checker's
//! aliasing rules.
//!
//! ## Approach
//!
//! We provide two approaches:
//! 1. **Post-order Traversal (O(N) time, O(N) space):** We use a recursive function that
//!    maintains a pointer to the previously processed node. This requires tracking mutable
//!    state across recursive calls.
//! 2. **Morris Traversal-like (O(N) time, O(1) space):** This approach modifies the tree
//!    in place by moving the right subtree to the rightmost leaf of the left subtree, and
//!    then moving the left subtree to the right. This approach requires manipulating
//!    multiple mutable references simultaneously, which is safe with `Rc<RefCell>` because
//!    it dynamically enforces borrow rules.
//!
//! This is idiomatic Rust because we use `Option<Rc<RefCell<TreeNode>>>` which is
//! the standard way to represent a tree where nodes need to be mutated and shared
//! or manipulated intricately (unlike Python/Java/C++ where pointers are unrestricted).

use std::cell::RefCell;
use std::rc::Rc;

/// Definition for a binary tree node.
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

/// Recursively flattens the tree using reverse post-order traversal.
///
/// Time: O(N) where N is the number of nodes.
/// Space: O(H) for recursion stack, where H is the height of the tree.
///
/// # Arguments
/// * `root` - The root of the binary tree.
pub fn flatten_recursive(root: &Option<Rc<RefCell<TreeNode>>>) {
    let mut prev = None;
    flatten_helper(root.as_ref(), &mut prev);
}

fn flatten_helper(node: Option<&Rc<RefCell<TreeNode>>>, prev: &mut Option<Rc<RefCell<TreeNode>>>) {
    if let Some(n) = node {
        // RUST INSIGHT: We borrow the inner `TreeNode` mutably.
        // We must drop the borrow before recursive calls if we need to borrow it again,
        // or just keep it scoped. Here we clone the children options.
        let left = n.borrow().left.clone();
        let right = n.borrow().right.clone();

        // Reverse post-order: Right, Left, Root
        flatten_helper(right.as_ref(), prev);
        flatten_helper(left.as_ref(), prev);

        // Modify current node
        let mut n_mut = n.borrow_mut();
        n_mut.right.clone_from(prev);
        n_mut.left = None;

        // Update prev
        *prev = Some(Rc::clone(n));
    }
}

/// Flattens the tree using an O(1) space approach (similar to Morris traversal).
///
/// Time: O(N) since every node is visited at most twice.
/// Space: O(1) ignoring the space required for the output tree itself.
///
/// # Arguments
/// * `root` - The root of the binary tree.
/// # Panics
/// Panics if a node inexplicably loses its left child during traversal (which should not happen structurally here).
pub fn flatten_iterative(root: &mut Option<Rc<RefCell<TreeNode>>>) {
    // We start with the root
    let mut curr = root.clone();

    while let Some(node_rc) = curr.clone() {
        // RUST INSIGHT: We borrow the node mutably to inspect and modify it.
        // We must carefully manage the scope of this borrow.
        let mut node = node_rc.borrow_mut();

        if node.left.is_some() {
            // Find the rightmost node of the left subtree
            let mut rightmost = node.left.clone();

            // We use a loop to traverse down the right children.
            // GOTCHA: We must clone the Rc to move forward, to avoid keeping
            // a mutable borrow open across iterations.
            loop {
                // Peek at the right child
                let next_right = rightmost.as_ref().unwrap().borrow().right.clone();
                if next_right.is_some() {
                    rightmost = next_right;
                } else {
                    break;
                }
            }

            // Connect the rightmost leaf of the left subtree to the current right subtree
            // RUST INSIGHT: This operation works safely because `rightmost` and `node`
            // are distinct nodes in the tree, so their `RefCell`s do not conflict.
            rightmost.unwrap().borrow_mut().right = node.right.take();

            // Move the left subtree to the right, and clear left
            node.right = node.left.take();
        }

        // Move to the next right node (which is now the flattened left subtree)
        curr = node.right.as_ref().map(Rc::clone);
    }
}

/// Alternative approaches
///
/// 1. Using `Vec` to store the preorder traversal: You can traverse the tree
///    in O(N) time and store all nodes in a `Vec`. Then iterate through the `Vec`
///    to rewire the `left` and `right` pointers. This requires O(N) extra space
///    but avoids complex pointer tracking.
#[cfg(test)]
mod tests {
    use super::*;

    // Helper to build a tree: 1 -> left: 2, right: 5
    #[allow(clippy::unnecessary_wraps)]
    fn build_test_tree() -> Option<Rc<RefCell<TreeNode>>> {
        let n1 = Rc::new(RefCell::new(TreeNode::new(1)));
        let n2 = Rc::new(RefCell::new(TreeNode::new(2)));
        let n3 = Rc::new(RefCell::new(TreeNode::new(3)));
        let n4 = Rc::new(RefCell::new(TreeNode::new(4)));
        let n5 = Rc::new(RefCell::new(TreeNode::new(5)));
        let n6 = Rc::new(RefCell::new(TreeNode::new(6)));

        n2.borrow_mut().left = Some(n3);
        n2.borrow_mut().right = Some(n4);

        n5.borrow_mut().right = Some(n6);

        n1.borrow_mut().left = Some(n2);
        n1.borrow_mut().right = Some(n5);

        Some(n1)
    }

    // Helper to verify if the tree is flattened correctly
    fn verify_flattened(mut root: Option<Rc<RefCell<TreeNode>>>, expected_vals: &[i32]) {
        let mut idx = 0;
        while let Some(node_rc) = root {
            let node = node_rc.borrow();
            assert_eq!(node.val, expected_vals[idx]);
            assert!(node.left.is_none(), "Left child should be None");
            idx += 1;
            root = node.right.clone();
        }
        assert_eq!(
            idx,
            expected_vals.len(),
            "Not all expected nodes were found"
        );
    }

    #[test]
    fn test_flatten_recursive_happy_path() {
        let root = build_test_tree();
        flatten_recursive(&root);
        verify_flattened(root, &[1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn test_flatten_iterative_happy_path() {
        let mut root = build_test_tree();
        flatten_iterative(&mut root);
        verify_flattened(root, &[1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn test_flatten_empty_tree() {
        let root = None;
        flatten_recursive(&root);
        assert!(root.is_none());

        let mut root2 = None;
        flatten_iterative(&mut root2);
        assert!(root2.is_none());
    }

    #[test]
    fn test_flatten_single_node() {
        let root = Some(Rc::new(RefCell::new(TreeNode::new(1))));
        flatten_recursive(&root);
        verify_flattened(root, &[1]);

        let mut root2 = Some(Rc::new(RefCell::new(TreeNode::new(1))));
        flatten_iterative(&mut root2);
        verify_flattened(root2, &[1]);
    }
}
