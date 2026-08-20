//! # 114. Flatten Binary Tree to Linked List
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/flatten-binary-tree-to-linked-list/>
//!
//! This problem requires flattening a binary tree into a "linked list" where the `right` child pointer points to the next node in a pre-order traversal, and the `left` child pointer is always `null`.
//! In Rust, mutating a tree structure in place provides an excellent exercise in dealing with `Option<Rc<RefCell<TreeNode>>>`, interior mutability, and the borrow checker's strict rules around multiple mutable borrows.
//!
//! ## Approach
//!
//! We provide three distinct approaches to illustrate the tradeoff between simplicity, space complexity, and idiomatic Rust patterns:
//!
//! 1. **Brute Force (O(N) Time, O(N) Space)**: Perform a standard pre-order traversal, collecting all nodes into a `Vec`. Then, iterate through the `Vec` to rewire the pointers. This is simple but requires O(N) auxiliary space for the `Vec`.
//! 2. **Optimized (O(N) Time, O(H) Space)**: Use a recursive approach (Reverse Post-Order Traversal - Right, Left, Node) to keep track of the previously visited node. As we traverse backward, we point the current node's `right` to the `prev` node, and its `left` to `None`. The space complexity is O(H) due to the call stack.
//! 3. **Optimal (O(N) Time, O(1) Space)**: Iterative Morris Traversal-like approach. For each node, if it has a left child, find the rightmost node of that left subtree (its predecessor). Wire the predecessor's `right` to the current node's `right`, move the left subtree to the `right` pointer, and nullify the `left` pointer. Then move to the next right node. This modifies the tree completely in-place with O(1) auxiliary space.
//!
//! ## Idiomatic Rust Comparisons
//!
//! Unlike Python or Java where you can freely mutate pointers (`node.left = None`), Rust requires explicit borrowing and ownership semantics.
//! - For the recursive approach, we must be careful with mutability. We pass around references to the nodes or use `RefCell` to safely mutate.
//! - For `LeetCode` binary tree problems requiring complex pointer manipulations (like this one), `Option<Rc<RefCell<TreeNode>>>` is the standard type. It introduces overhead but bypasses some static borrow checking at the cost of runtime borrow checks.

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

/// Brute Force: Collect and Re-wire
/// Time: O(N) - Visit every node to collect, then iterate to rewire.
/// Space: O(N) - Stores all nodes in a `Vec`.
#[allow(clippy::needless_pass_by_value)]
// Helper function for pre-order traversal
fn pre_order(node: Option<&Rc<RefCell<TreeNode>>>, nodes: &mut Vec<Rc<RefCell<TreeNode>>>) {
    if let Some(n) = node {
        nodes.push(Rc::clone(n));
        let left = n.borrow().left.clone();
        let right = n.borrow().right.clone();
        pre_order(left.as_ref(), nodes);
        pre_order(right.as_ref(), nodes);
    }
}

#[allow(clippy::needless_pass_by_value)]
pub fn flatten_brute_force(root: Option<Rc<RefCell<TreeNode>>>) {
    if root.is_none() {
        return;
    }

    let mut nodes = Vec::new();
    pre_order(root.as_ref(), &mut nodes);

    // Re-wire the tree
    // GOTCHA: We must use `.windows(2)` or iterate carefully to avoid multiple mutable borrows
    // if we try to borrow `nodes[i]` and `nodes[i+1]` simultaneously incorrectly.
    for i in 0..nodes.len() - 1 {
        let curr = &nodes[i];
        let next = &nodes[i + 1];

        let mut curr_mut = curr.borrow_mut();
        curr_mut.left = None;
        curr_mut.right = Some(Rc::clone(next));
    }

    // Ensure the last node points to None
    if let Some(last) = nodes.last() {
        let mut last_mut = last.borrow_mut();
        last_mut.left = None;
        last_mut.right = None;
    }
}

/// Optimized: Reverse Post-Order Traversal (Right, Left, Root)
/// Time: O(N) - Visit every node once.
/// Space: O(H) - Call stack depth is the height of the tree.
///
/// We maintain a mutable reference to an `Option<Rc<RefCell<TreeNode>>>` representing the `prev` node.
#[allow(clippy::needless_pass_by_value)]
pub fn flatten_optimized(root: Option<Rc<RefCell<TreeNode>>>) {
    fn reverse_post_order(
        node: Option<Rc<RefCell<TreeNode>>>,
        prev: &mut Option<Rc<RefCell<TreeNode>>>,
    ) {
        if let Some(n) = node {
            // Traverse right, then left
            let right = n.borrow().right.clone();
            let left = n.borrow().left.clone();

            reverse_post_order(right, prev);
            reverse_post_order(left, prev);

            // Process current node
            // RUST INSIGHT: We use `borrow_mut()` to gain mutable access to the node's internals.
            // This is checked at runtime. If we were holding another borrow to `n`, it would panic.
            let mut n_mut = n.borrow_mut();
            n_mut.right.clone_from(prev);
            n_mut.left = None;

            // Update prev to current node
            *prev = Some(Rc::clone(&n));
        }
    }

    let mut prev = None;
    reverse_post_order(root, &mut prev);
}

/// Optimal: Iterative O(1) Space Approach (Morris Traversal variant)
/// Time: O(N) - Each node is visited at most twice.
/// Space: O(1) - Only a few pointers are used.
///
/// We iterate through the tree. For each node, if it has a left child, we find the rightmost
/// node in the left subtree. We wire this rightmost node's right pointer to the current node's
/// right child. Then we move the left subtree to the right, and nullify the left pointer.
///
/// # Panics
/// Panics if a `left` pointer is corrupted during the traversal since we `unwrap()`
/// the known-to-exist left child.
#[allow(clippy::needless_pass_by_value)]
pub fn flatten_optimal(root: Option<Rc<RefCell<TreeNode>>>) {
    let mut curr = root;

    while let Some(curr_node) = curr {
        // We scope the borrow to ensure we release it before we might borrow it again
        let has_left = curr_node.borrow().left.is_some();

        if has_left {
            let left_child = curr_node.borrow().left.clone().unwrap();
            let mut rightmost = left_child.clone();

            // Find the rightmost node of the left subtree
            loop {
                let next_right = rightmost.borrow().right.clone();
                match next_right {
                    Some(node) => rightmost = node,
                    None => break,
                }
            }

            // Wire the rightmost node to the current's right subtree
            let curr_right = curr_node.borrow().right.clone();
            rightmost.borrow_mut().right = curr_right;

            // Move left subtree to right and nullify left
            let mut curr_mut = curr_node.borrow_mut();
            curr_mut.right = Some(left_child);
            curr_mut.left = None;
        }

        // Move to the next node (which is now guaranteed to be on the right)
        let next_curr = curr_node.borrow().right.clone();
        curr = next_curr;
    }
}

/// Main entry point
#[allow(clippy::needless_pass_by_value)]
pub fn flatten(root: Option<Rc<RefCell<TreeNode>>>) {
    flatten_optimal(root);
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create a node easily
    fn node(val: i32) -> Rc<RefCell<TreeNode>> {
        Rc::new(RefCell::new(TreeNode::new(val)))
    }

    // Helper to convert flattened tree to Vec for easy assertion
    fn to_vec(root: Option<Rc<RefCell<TreeNode>>>) -> Vec<Option<i32>> {
        let mut res = Vec::new();
        let mut curr = root;
        while let Some(n) = curr {
            res.push(Some(n.borrow().val));
            if n.borrow().left.is_some() {
                // If left is not none, it's invalid
                res.push(Some(-999));
            }
            curr = n.borrow().right.clone();
        }
        res
    }

    fn build_test_tree() -> Rc<RefCell<TreeNode>> {
        // Tree:
        //     1
        //    / \
        //   2   5
        //  / \   \
        // 3   4   6
        let root = node(1);
        let n2 = node(2);
        let n3 = node(3);
        let n4 = node(4);
        let n5 = node(5);
        let n6 = node(6);

        n2.borrow_mut().left = Some(n3);
        n2.borrow_mut().right = Some(n4);

        n5.borrow_mut().right = Some(n6);

        root.borrow_mut().left = Some(n2);
        root.borrow_mut().right = Some(n5);

        root
    }

    #[test]
    fn test_flatten_brute_force() {
        let root = Some(build_test_tree());
        flatten_brute_force(root.clone());
        let expected = vec![Some(1), Some(2), Some(3), Some(4), Some(5), Some(6)];
        assert_eq!(to_vec(root), expected);
    }

    #[test]
    fn test_flatten_optimized() {
        let root = Some(build_test_tree());
        flatten_optimized(root.clone());
        let expected = vec![Some(1), Some(2), Some(3), Some(4), Some(5), Some(6)];
        assert_eq!(to_vec(root), expected);
    }

    #[test]
    fn test_flatten_optimal() {
        let root = Some(build_test_tree());
        flatten_optimal(root.clone());
        let expected = vec![Some(1), Some(2), Some(3), Some(4), Some(5), Some(6)];
        assert_eq!(to_vec(root), expected);
    }

    #[test]
    fn test_empty_tree() {
        let root = None;
        flatten_optimal(root.clone());
        assert_eq!(to_vec(root), Vec::<Option<i32>>::new());
    }

    #[test]
    fn test_single_node() {
        let root = node(1);
        flatten_optimal(Some(root.clone()));
        let root = Some(root);
        assert_eq!(to_vec(root), vec![Some(1)]);
    }

    #[test]
    fn test_left_heavy_tree() {
        // Tree:
        //     1
        //    /
        //   2
        //  /
        // 3
        let root = node(1);
        let n2 = node(2);
        let n3 = node(3);

        n2.borrow_mut().left = Some(n3);
        root.borrow_mut().left = Some(n2);

        flatten_optimal(Some(root.clone()));
        let root = Some(root);
        let expected = vec![Some(1), Some(2), Some(3)];
        assert_eq!(to_vec(root), expected);
    }
}
