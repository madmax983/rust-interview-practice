//! # 230. Kth Smallest Element in a BST
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/kth-smallest-element-in-a-bst/>
//!
//! Given the root of a binary search tree, and an integer `k`, return the `k`th smallest value
//! (1-indexed) of all the values of the nodes in the tree.
//!
//! This problem is a natural fit for demonstrating tree traversal patterns in Rust. It shows how to
//! handle recursive state management using mutable references and how to implement iterative tree
//! traversals explicitly using a `Vec` as a stack, avoiding call stack limits.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::trees::kth_smallest_element_in_a_bst::{kth_smallest, TreeNode};
//!
//! let mut root = TreeNode::new(3);
//! root.left = Some(Box::new(TreeNode::new(1)));
//! root.left.as_mut().unwrap().right = Some(Box::new(TreeNode::new(2)));
//! root.right = Some(Box::new(TreeNode::new(4)));
//!
//! assert_eq!(kth_smallest(Some(Box::new(root)), 1), 1);
//! ```
//!
//! ## Constraints
//!
//! - The number of nodes in the tree is `n`.
//! - `1 <= k <= n <= 10^4`
//! - `0 <= Node.val <= 10^4`

// Note: LeetCode actually uses Rc<RefCell<TreeNode>> for tree problems often, but the
// existing patterns in this repo for trees (e.g. invert_binary_tree) use Option<Box<TreeNode>>.
// We'll stick to Option<Box<TreeNode>> for consistency unless we need Rc<RefCell>.
// For kth smallest, Option<Box<TreeNode>> is perfectly fine as we only need read access.

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

/// Brute Force approach: Collect all elements in-order.
///
/// This approach traverses the entire tree and collects all values into a `Vec`.
/// Since it's a BST, an in-order traversal yields sorted values. We then just index into the `Vec`.
///
/// Time: O(N) - Visits every node once.
/// Space: O(N) - Stores all node values in a vector, plus O(H) for the call stack.
///
/// # Gotcha
/// This completely ignores the BST property that allows us to stop early once we find the `k`th element.
/// It also allocates memory for the entire tree structure, which is inefficient.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
#[allow(clippy::cast_sign_loss)]
pub fn kth_smallest_brute_force(root: Option<Box<TreeNode>>, k: i32) -> i32 {
    fn inorder(node: Option<&TreeNode>, vals: &mut Vec<i32>) {
        if let Some(n) = node {
            inorder(n.left.as_deref(), vals);
            vals.push(n.val);
            inorder(n.right.as_deref(), vals);
        }
    }

    let mut vals = Vec::new();
    inorder(root.as_deref(), &mut vals);
    // k is 1-indexed, so we return the element at k - 1
    // RUST INSIGHT: usize conversion is required for indexing.
    vals[(k - 1) as usize]
}

/// Optimized approach: Recursive in-order traversal with early exit.
///
/// Instead of collecting all elements, we keep track of how many elements we've visited.
/// Once we hit `k`, we store the result and return early from the recursion.
///
/// Time: O(H + k) - We only traverse down to the leftmost node (height H), then visit k nodes.
/// Space: O(H) - Recursion depth equals tree height. No extra heap allocation for values.
///
/// # Rust Insight
/// We pass mutable references (`&mut i32`, `&mut Option<i32>`) to maintain state across recursive
/// calls. This avoids global variables (which are unsafe in Rust) and explicitly shows state flow.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
/// # Panics
///
/// Panics if `k` is out of bounds (which violates the problem constraints).
pub fn kth_smallest_optimized(root: Option<Box<TreeNode>>, k: i32) -> i32 {
    fn inorder(node: Option<&TreeNode>, count: &mut i32, k: i32, result: &mut Option<i32>) {
        // Early exit if we already found the answer
        if result.is_some() {
            return;
        }

        if let Some(n) = node {
            inorder(n.left.as_deref(), count, k, result);

            // Check if we found it in the left subtree to avoid unnecessary work
            if result.is_some() {
                return;
            }

            *count += 1;
            if *count == k {
                *result = Some(n.val);
                return;
            }

            inorder(n.right.as_deref(), count, k, result);
        }
    }

    let mut count = 0;
    let mut result = None;
    inorder(root.as_deref(), &mut count, k, &mut result);

    // We know from constraints that k is valid, so result will always be Some.
    result.unwrap()
}

/// Optimal approach: Iterative in-order traversal using an explicit stack.
///
/// This approach manually manages the stack instead of relying on the call stack.
/// It's often preferred in production to avoid stack overflow on very deep unbalanced trees.
///
/// Time: O(H + k)
/// Space: O(H) - The explicit stack stores at most H nodes.
///
/// # Rust Insight
/// We use `&Box<TreeNode>` to avoid taking ownership of the tree.
/// This allows us to traverse the tree without modifying it or cloning nodes.
///
/// # Gotcha
/// When pushing `node.left` to the stack, we need to deal with `Option` borrowing.
/// `node.left.as_ref()` allows us to borrow the inner `Box<TreeNode>` without consuming the `Option`.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
/// # Panics
///
/// Panics if `k` is out of bounds (which violates the problem constraints) and the stack is exhausted.
pub fn kth_smallest_optimal(root: Option<Box<TreeNode>>, k: i32) -> i32 {
    let mut stack = Vec::new();
    let mut curr = root.as_ref();
    let mut count = 0;

    // We continue as long as there's a current node to process OR the stack is not empty.
    while curr.is_some() || !stack.is_empty() {
        // Go as far left as possible, pushing nodes onto the stack.
        while let Some(node) = curr {
            stack.push(node);
            curr = node.left.as_ref();
        }

        // Pop the top node (this is the leftmost unvisited node).
        let node = stack.pop().unwrap();
        count += 1;

        if count == k {
            return node.val;
        }

        // Move to the right child.
        curr = node.right.as_ref();
    }

    // Constraints guarantee we will find the element, so this is unreachable.
    unreachable!("k is out of bounds")
}

/// Main entry point - uses optimal solution
#[must_use]
pub fn kth_smallest(root: Option<Box<TreeNode>>, k: i32) -> i32 {
    kth_smallest_optimal(root, k)
}

// Alternative approaches footer:
// - If the BST is modified frequently and we need to query `kth_smallest` often,
//   we could augment the BST structure to store the size of the left subtree in each node.
//   This would reduce the query time to O(H). However, this changes the node struct.

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create a leaf node
    fn leaf(val: i32) -> Option<Box<TreeNode>> {
        Some(Box::new(TreeNode::new(val)))
    }

    #[test]
    fn test_brute_force_example_1() {
        // Input: root = [3,1,4,null,2], k = 1
        //      3
        //     / \
        //    1   4
        //     \
        //      2
        let mut root = TreeNode::new(3);
        let mut left = TreeNode::new(1);
        left.right = leaf(2);
        root.left = Some(Box::new(left));
        root.right = leaf(4);

        assert_eq!(kth_smallest_brute_force(Some(Box::new(root)), 1), 1);
    }

    #[test]
    fn test_optimized_example_2() {
        // Input: root = [5,3,6,2,4,null,null,1], k = 3
        //        5
        //       / \
        //      3   6
        //     / \
        //    2   4
        //   /
        //  1
        let mut root = TreeNode::new(5);
        let mut left = TreeNode::new(3);
        let mut left_left = TreeNode::new(2);
        left_left.left = leaf(1);
        left.left = Some(Box::new(left_left));
        left.right = leaf(4);
        root.left = Some(Box::new(left));
        root.right = leaf(6);

        assert_eq!(kth_smallest_optimized(Some(Box::new(root)), 3), 3);
    }

    #[test]
    fn test_optimal_single_node() {
        let root = TreeNode::new(1);
        assert_eq!(kth_smallest_optimal(Some(Box::new(root)), 1), 1);
    }

    #[test]
    fn test_all_approaches_consistency() {
        let mut root = TreeNode::new(5);
        let mut left = TreeNode::new(3);
        left.right = leaf(4);
        root.left = Some(Box::new(left));

        // Create clones by manually rebuilding
        let tree_builder = || {
            let mut root = TreeNode::new(5);
            let mut left = TreeNode::new(3);
            left.right = leaf(4);
            root.left = Some(Box::new(left));
            Some(Box::new(root))
        };

        let t1 = tree_builder();
        let t2 = tree_builder();
        let t3 = tree_builder();

        assert_eq!(kth_smallest_brute_force(t1, 2), 4);
        assert_eq!(kth_smallest_optimized(t2, 2), 4);
        assert_eq!(kth_smallest_optimal(t3, 2), 4);
    }
}
