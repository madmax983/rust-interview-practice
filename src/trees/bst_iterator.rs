//! # 173. Binary Search Tree Iterator
//!
//! Link: <https://leetcode.com/problems/binary-search-tree-iterator/>
//!
//! Implement the `BSTIterator` class that represents an iterator over the **in-order traversal** of a binary search tree (BST).
//!
//! This problem is a fundamental exercise in:
//! 1.  **Iterators**: Implementing the `Iterator` trait manually (or mimicking it).
//! 2.  **State Management**: Converting a recursive process (In-order traversal: Left -> Node -> Right) into an iterative state machine using a stack.
//! 3.  **Ownership & Sharing**: Handling shared ownership of tree nodes (`Rc<RefCell<TreeNode>>`) to traverse without consuming the tree.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::trees::bst_iterator::{BSTIterator, TreeNode};
//! use std::rc::Rc;
//! use std::cell::RefCell;
//!
//! //      7
//! //     / \
//! //    3   15
//! //       /  \
//! //      9    20
//! let root = TreeNode::new(7);
//! let n3 = TreeNode::new(3);
//! let n15 = TreeNode::new(15);
//! let n9 = TreeNode::new(9);
//! let n20 = TreeNode::new(20);
//!
//! root.borrow_mut().left = Some(Rc::clone(&n3));
//! root.borrow_mut().right = Some(Rc::clone(&n15));
//! n15.borrow_mut().left = Some(Rc::clone(&n9));
//! n15.borrow_mut().right = Some(Rc::clone(&n20));
//!
//! let mut iterator = BSTIterator::new(Some(root));
//!
//! assert_eq!(iterator.next(), 3);   // return 3
//! assert_eq!(iterator.next(), 7);   // return 7
//! assert_eq!(iterator.has_next(), true); // return True
//! assert_eq!(iterator.next(), 9);   // return 9
//! assert_eq!(iterator.next(), 15);  // return 15
//! assert_eq!(iterator.next(), 20);  // return 20
//! assert_eq!(iterator.has_next(), false); // return False
//! ```
//!
//! ## Constraints
//!
//! - The number of nodes in the tree is in the range `[1, 10^5]`.
//! - `0 <= Node.val <= 10^6`
//! - At most `10^5` calls will be made to `hasNext` and `next`.
//! - **Follow up**: Could you implement `next()` and `hasNext()` to run in average O(1) time and use O(h) memory, where h is the height of the tree?

use std::cell::RefCell;
use std::rc::Rc;

// =========================================================================================
// Data Structures
// =========================================================================================

/// Definition for a binary tree node.
///
/// We use `Rc<RefCell<TreeNode>>` here instead of `Box<TreeNode>` to allow
/// shared ownership. This is crucial for iterator implementations that need to
/// hold references to nodes deep in the tree while the tree itself is owned elsewhere.
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

// =========================================================================================
// Solution: Controlled Recursion Stack
// =========================================================================================

/// Iterator for Binary Search Tree.
///
/// This struct manually manages the recursion stack for an in-order traversal.
/// - In-order: Left -> Node -> Right.
/// - We initialize by pushing all left children of the root onto the stack.
/// - When `next()` is called, we pop the top node (which is the "left-most" unprocessed node),
///   return its value, and then process its right child (pushing the right child and all its left descendants).
///
/// Time: Average O(1) for `next()`, O(1) for `has_next()`.
/// Space: O(h) where h is the height of the tree (stack size).
pub struct BSTIterator {
    // Stack stores nodes that are waiting to be visited.
    // The top of the stack is always the next node to be returned.
    stack: Vec<Rc<RefCell<TreeNode>>>,
}

impl BSTIterator {
    /// Initializes the iterator object.
    ///
    /// The root of the BST is given as part of the constructor. The pointer should be initialized to a non-existent number smaller than any element in the BST.
    /// effectively, we just prepare the stack.
    #[must_use]
    pub fn new(root: Option<Rc<RefCell<TreeNode>>>) -> Self {
        let mut iterator = BSTIterator { stack: Vec::new() };
        iterator.push_all_left(root);
        iterator
    }

    /// Returns the next smallest number.
    ///
    /// # Panics
    /// Panics if called when `has_next()` is false. (Though LeetCode guarantees valid calls).
    ///
    /// # Gotcha: Name Collision
    /// This method is named `next`, which is the same as `Iterator::next`.
    /// Because this is an inherent method, it shadows the trait method.
    /// Calling `iter.next()` will call this method (returning `i32`).
    /// To call the trait method (returning `Option<i32>`), you must use `Iterator::next(&mut iter)`.
    #[allow(clippy::should_implement_trait)] // We implement Iterator trait below separately
    pub fn next(&mut self) -> i32 {
        // RUST INSIGHT: `pop()` returns `Option<T>`, handling empty stack gracefully.
        // `unwrap()` is safe here IF we assume the caller respects `has_next()`.
        let node = self.stack.pop().expect("next() called on empty iterator");

        // Before returning, we need to process the right child of this node.
        // Since in-order is Left -> Node -> Right, after visiting Node, we go Right.
        // We push the right child and all its left descendants onto the stack.

        // We need to clone the Rc to push it to the stack or read its fields.
        // Note: `node` is an Rc<RefCell<TreeNode>>. `node.borrow()` gives us a Ref to the TreeNode.
        let right_child = node.borrow().right.clone();
        self.push_all_left(right_child);

        node.borrow().val
    }

    /// Returns whether we have a next smallest number.
    #[must_use]
    pub fn has_next(&self) -> bool {
        !self.stack.is_empty()
    }

    /// Helper function to push a node and all its left descendants onto the stack.
    fn push_all_left(&mut self, mut node: Option<Rc<RefCell<TreeNode>>>) {
        while let Some(n) = node {
            self.stack.push(Rc::clone(&n));
            // Move to left child
            node = n.borrow().left.clone();
        }
    }
}

// =========================================================================================
// Idiomatic Rust Implementation
// =========================================================================================

// Implementing the standard Iterator trait allows this to be used with
// for loops, map, filter, etc.
impl Iterator for BSTIterator {
    type Item = i32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.has_next() {
            Some(self.next())
        } else {
            None
        }
    }
}

// =========================================================================================
// Alternative Approaches
// =========================================================================================

// 1. **Flattening (Brute Force)**:
//    Traverse the entire tree upfront and store values in a `Vec<i32>`.
//    Then just iterate the vector.
//    - Pros: Simpler to implement.
//    - Cons: O(N) memory regardless of tree height. Fails the O(h) memory requirement.
//
// 2. **Morris Traversal**:
//    Modify the tree structure (threading) to point predecessors to successors.
//    - Pros: O(1) space!
//    - Cons: Modifies the tree (temporarily), not thread-safe, complex to implement.
//
// 3. **Parent Pointers**:
//    If nodes had parent pointers, we could traverse without a stack.
//    - Pros: O(1) space.
//    - Cons: Requires modifying Node definition.

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create a leaf node
    fn leaf(val: i32) -> Rc<RefCell<TreeNode>> {
        TreeNode::new(val)
    }

    #[test]
    fn test_happy_path() {
        //      7
        //     / \
        //    3   15
        //       /  \
        //      9    20
        let root = TreeNode::new(7);
        let n3 = leaf(3);
        let n15 = leaf(15);
        let n9 = leaf(9);
        let n20 = leaf(20);

        root.borrow_mut().left = Some(Rc::clone(&n3));
        root.borrow_mut().right = Some(Rc::clone(&n15));
        n15.borrow_mut().left = Some(Rc::clone(&n9));
        n15.borrow_mut().right = Some(Rc::clone(&n20));

        let mut iter = BSTIterator::new(Some(root));

        assert_eq!(iter.next(), 3);
        assert_eq!(iter.next(), 7);
        assert!(iter.has_next());
        assert_eq!(iter.next(), 9);
        assert_eq!(iter.next(), 15);
        assert_eq!(iter.next(), 20);
        assert!(!iter.has_next());
    }

    #[test]
    fn test_empty_tree() {
        let mut iter = BSTIterator::new(None);
        assert!(!iter.has_next());
        // Explicitly call Iterator::next to verify the trait impl
        assert_eq!(Iterator::next(&mut iter), None);
    }

    #[test]
    fn test_single_node() {
        let root = leaf(42);
        let mut iter = BSTIterator::new(Some(root));
        assert!(iter.has_next());
        assert_eq!(iter.next(), 42); // Inherent method
        assert!(!iter.has_next());
    }

    #[test]
    fn test_left_skewed() {
        //    3
        //   /
        //  2
        // /
        //1
        let root = TreeNode::new(3);
        let n2 = leaf(2);
        let n1 = leaf(1);

        n2.borrow_mut().left = Some(Rc::clone(&n1));
        root.borrow_mut().left = Some(Rc::clone(&n2));

        let mut iter = BSTIterator::new(Some(root));
        assert_eq!(iter.next(), 1);
        assert_eq!(iter.next(), 2);
        assert_eq!(iter.next(), 3);
        assert!(!iter.has_next());
    }

    #[test]
    fn test_right_skewed() {
        // 1
        //  \
        //   2
        //    \
        //     3
        let root = TreeNode::new(1);
        let n2 = leaf(2);
        let n3 = leaf(3);

        n2.borrow_mut().right = Some(Rc::clone(&n3));
        root.borrow_mut().right = Some(Rc::clone(&n2));

        let mut iter = BSTIterator::new(Some(root));
        assert_eq!(iter.next(), 1);
        assert_eq!(iter.next(), 2);
        assert_eq!(iter.next(), 3);
        assert!(!iter.has_next());
    }
}
