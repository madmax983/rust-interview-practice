//! # 101. Symmetric Tree
//!
//! Difficulty: Easy
//! Link: <https://leetcode.com/problems/symmetric-tree/>
//!
//! Given the `root` of a binary tree, check whether it is a mirror of itself
//! (i.e., symmetric around its center).
//!
//! This problem naturally models recursion and tree traversal. In Rust, it
//! demonstrates how to safely pass around and pattern match on nested
//! `Option<Box<TreeNode>>` structures. It provides a great opportunity to explore
//! matching tuples of options, which is a powerful idiom for comparing two trees simultaneously.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::trees::symmetric_tree::{is_symmetric, TreeNode};
//!
//! // Example 1: Symmetric Tree
//! //     1
//! //    / \
//! //   2   2
//! //  / \ / \
//! // 3  4 4  3
//! let mut root = TreeNode::new(1);
//! let mut left = TreeNode::new(2);
//! left.left = Some(Box::new(TreeNode::new(3)));
//! left.right = Some(Box::new(TreeNode::new(4)));
//! let mut right = TreeNode::new(2);
//! right.left = Some(Box::new(TreeNode::new(4)));
//! right.right = Some(Box::new(TreeNode::new(3)));
//! root.left = Some(Box::new(left));
//! root.right = Some(Box::new(right));
//!
//! assert_eq!(is_symmetric(Some(Box::new(root))), true);
//! ```
//!
//! ## Constraints
//!
//! - The number of nodes in the tree is in the range `[1, 1000]`.
//! - `-100 <= Node.val <= 100`

// Definition for a binary tree node.
#[derive(Debug, PartialEq, Eq, Clone)]
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

/// Brute force approach: Collect nodes into vectors and compare
///
/// Time: O(N) - visits each node once for traversal, then O(N) to compare vectors
/// Space: O(N) - stores node values (and None markers) in a vector
///
/// We can perform a left-to-right traversal for the left subtree and a right-to-left
/// traversal for the right subtree. If the collected values match exactly, the tree is symmetric.
/// This approach involves allocation and is non-optimal compared to pure recursion.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_symmetric_brute_force(root: Option<Box<TreeNode>>) -> bool {
    // Helper to traverse the left side (standard pre-order)
    fn collect_left(node: Option<&TreeNode>, vec: &mut Vec<Option<i32>>) {
        if let Some(n) = node {
            vec.push(Some(n.val));
            collect_left(n.left.as_deref(), vec);
            collect_left(n.right.as_deref(), vec);
        } else {
            vec.push(None); // Marker for null nodes
        }
    }

    // Helper to traverse the right side (mirror pre-order: root, right, left)
    fn collect_right(node: Option<&TreeNode>, vec: &mut Vec<Option<i32>>) {
        if let Some(n) = node {
            vec.push(Some(n.val));
            collect_right(n.right.as_deref(), vec);
            collect_right(n.left.as_deref(), vec);
        } else {
            vec.push(None); // Marker for null nodes
        }
    }

    root.is_none_or(|node| {
        let mut left_vec = Vec::new();
        let mut right_vec = Vec::new();

        collect_left(node.left.as_deref(), &mut left_vec);
        collect_right(node.right.as_deref(), &mut right_vec);

        left_vec == right_vec
    })
}

/// Optimized approach: Iterative using a Queue
/// Time: O(N) - visits each node once
/// Space: O(N) - worst case queue size can be O(N) for a wide tree
///
/// This avoids the call stack overhead of recursion by using an explicit queue (or stack).
/// We process pairs of nodes that should be mirrors of each other.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_symmetric_optimized(root: Option<Box<TreeNode>>) -> bool {
    let Some(node) = root else {
        return true;
    };

    let mut queue = std::collections::VecDeque::new();
    queue.push_back(node.left);
    queue.push_back(node.right);

    while let (Some(left_opt), Some(right_opt)) = (queue.pop_front(), queue.pop_front()) {
        match (left_opt, right_opt) {
            (None, None) => {} // Both are empty, which is symmetric
            (Some(left_node), Some(right_node)) => {
                if left_node.val != right_node.val {
                    return false; // Values don't match
                }
                // GOTCHA: It is easy to accidentally push `left_node.right` and `right_node.right`
                // together. For symmetry, you must compare the outer children together and
                // the inner children together!
                // Push the next pairs to compare:
                // Outside children
                queue.push_back(left_node.left);
                queue.push_back(right_node.right);
                // Inside children
                queue.push_back(left_node.right);
                queue.push_back(right_node.left);
            }
            _ => return false, // One is Some, the other is None
        }
    }

    true
}

/// Optimal approach: Recursive tuple matching
/// Time: O(N) - visits each node once
/// Space: O(h) - where h is the tree height, due to recursion stack frames
///
/// RUST INSIGHT: We borrow the tree by reference (`&Option<Box<TreeNode>>`) using
/// pattern matching on simultaneous options as a tuple. This is incredibly elegant
/// and ensures exhaustive handling of all structural combinations without manually
/// checking for null pointers as you would in C++ or Java.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_symmetric_optimal(root: Option<Box<TreeNode>>) -> bool {
    fn is_mirror(left: Option<&TreeNode>, right: Option<&TreeNode>) -> bool {
        match (left, right) {
            // Both are empty: symmetric
            (None, None) => true,
            // Both have nodes: check values and recurse on mirrored children
            (Some(l), Some(r)) => {
                l.val == r.val
                    && is_mirror(l.left.as_deref(), r.right.as_deref())
                    && is_mirror(l.right.as_deref(), r.left.as_deref())
            }
            // Structure mismatch (one is Some, one is None)
            _ => false,
        }
    }

    root.as_ref()
        .is_none_or(|node| is_mirror(node.left.as_deref(), node.right.as_deref()))
}

/// Main entry point
#[must_use]
pub fn is_symmetric(root: Option<Box<TreeNode>>) -> bool {
    is_symmetric_optimal(root)
}

// ============================================================================
// Alternative Approaches Summary
// ============================================================================
// 1. Recursive (Optimal): This is the most idiomatic and readable solution in Rust.
//    You should prefer this for most tree problems unless the tree height is unbounded
//    and might cause a stack overflow.
// 2. Iterative (Optimized): Use this when you are working in an environment with
//    strict call-stack limits or dealing with extremely deep trees where recursion
//    is unsafe. It requires heap allocation for the queue/stack but is safer against overflows.
// 3. Traversal Collection (Brute Force): This is mostly educational to understand
//    how tree serialization works, but it wastes O(N) auxiliary memory and should be avoided in production.

#[cfg(test)]
mod tests {
    use super::*;

    fn build_symmetric_tree() -> Option<Box<TreeNode>> {
        let mut root = TreeNode::new(1);
        let mut left = TreeNode::new(2);
        left.left = Some(Box::new(TreeNode::new(3)));
        left.right = Some(Box::new(TreeNode::new(4)));

        let mut right = TreeNode::new(2);
        right.left = Some(Box::new(TreeNode::new(4)));
        right.right = Some(Box::new(TreeNode::new(3)));

        root.left = Some(Box::new(left));
        root.right = Some(Box::new(right));

        Some(Box::new(root))
    }

    fn build_asymmetric_tree() -> Option<Box<TreeNode>> {
        let mut root = TreeNode::new(1);
        let mut left = TreeNode::new(2);
        left.right = Some(Box::new(TreeNode::new(3)));

        let mut right = TreeNode::new(2);
        right.right = Some(Box::new(TreeNode::new(3)));

        root.left = Some(Box::new(left));
        root.right = Some(Box::new(right));

        Some(Box::new(root))
    }

    #[test]
    fn test_brute_force() {
        assert!(is_symmetric_brute_force(build_symmetric_tree()));
        assert!(!is_symmetric_brute_force(build_asymmetric_tree()));
        assert!(is_symmetric_brute_force(None));
    }

    #[test]
    fn test_optimized() {
        assert!(is_symmetric_optimized(build_symmetric_tree()));
        assert!(!is_symmetric_optimized(build_asymmetric_tree()));
        assert!(is_symmetric_optimized(None));
    }

    #[test]
    fn test_optimal() {
        assert!(is_symmetric_optimal(build_symmetric_tree()));
        assert!(!is_symmetric_optimal(build_asymmetric_tree()));
        assert!(is_symmetric_optimal(None));
    }

    #[test]
    fn test_all_approaches_consistency() {
        let tree1 = build_symmetric_tree();
        assert!(is_symmetric_brute_force(tree1.clone()));
        assert!(is_symmetric_optimized(tree1.clone()));
        assert!(is_symmetric_optimal(tree1));

        let tree2 = build_asymmetric_tree();
        assert!(!is_symmetric_brute_force(tree2.clone()));
        assert!(!is_symmetric_optimized(tree2.clone()));
        assert!(!is_symmetric_optimal(tree2));
    }
}
