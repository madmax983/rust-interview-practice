//! # 572. Subtree of Another Tree
//!
//! Difficulty: Easy
//! Link: <https://leetcode.com/problems/subtree-of-another-tree/>
//!
//! Given the roots of two binary trees `root` and `subRoot`, return `true` if there is a subtree of `root` with the same structure and node values of `subRoot` and `false` otherwise.
//!
//! A subtree of a binary tree `tree` is a tree that consists of a node in `tree` and all of this node's descendants. The tree `tree` could also be considered as a subtree of itself.
//!
//! This problem provides a great opportunity to explore recursive tree traversal in Rust and to see how Rust's string formatting and memory allocation behavior can impact string serialization algorithms.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::trees::subtree_of_another_tree::{is_subtree, TreeNode};
//!
//! let mut root = TreeNode::new(3);
//! root.left = Some(Box::new(TreeNode::new(4)));
//! root.right = Some(Box::new(TreeNode::new(5)));
//! root.left.as_mut().unwrap().left = Some(Box::new(TreeNode::new(1)));
//! root.left.as_mut().unwrap().right = Some(Box::new(TreeNode::new(2)));
//!
//! let mut sub_root = TreeNode::new(4);
//! sub_root.left = Some(Box::new(TreeNode::new(1)));
//! sub_root.right = Some(Box::new(TreeNode::new(2)));
//!
//! assert_eq!(is_subtree(Some(Box::new(root)), Some(Box::new(sub_root))), true);
//! ```
//!
//! ## Constraints
//!
//! - The number of nodes in the `root` tree is in the range `[1, 2000]`.
//! - The number of nodes in the `subRoot` tree is in the range `[1, 1000]`.
//! - `-10^4 <= root.val <= 10^4`
//! - `-10^4 <= subRoot.val <= 10^4`

use std::fmt::Write;

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

/// Helper function to check if two subtrees are identical.
fn is_same(p: Option<&TreeNode>, q: Option<&TreeNode>) -> bool {
    // RUST INSIGHT: This match is exhaustive. The compiler guarantees we handle every possible combination
    // of `Option::Some` and `Option::None` between the two trees.
    match (p, q) {
        (Some(n_p), Some(n_q)) => {
            n_p.val == n_q.val
                && is_same(n_p.left.as_deref(), n_q.left.as_deref())
                && is_same(n_p.right.as_deref(), n_q.right.as_deref())
        }
        (None, None) => true,
        _ => false, // Handles (Some, None) and (None, Some)
    }
}

/// Brute Force approach: Recursive DFS Search
///
/// Time: O(n * m) - In the worst case, we check `is_same` (which takes O(m) where m is the number of nodes in `subRoot`) for every node in `root` (which has n nodes).
/// Space: O(n + m) - Where n and m are the height of the respective trees for the recursive call stacks.
///
/// We recursively traverse `root`. For each node, we check if the tree starting at that node is identical to `subRoot`.
/// If it's not, we recursively check its left and right children.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_brute_force(
    root: Option<Box<TreeNode>>,
    sub_root: Option<Box<TreeNode>>,
) -> bool {
    // Helper function that takes `Option<&TreeNode>` to avoid consuming or cloning the tree during the recursive traversal.
    fn check_subtree(root_ref: Option<&TreeNode>, sub_root_ref: Option<&TreeNode>) -> bool {
        match root_ref {
            None => false,
            Some(n_root) => {
                if is_same(Some(n_root), sub_root_ref) {
                    return true;
                }
                check_subtree(n_root.left.as_deref(), sub_root_ref)
                    || check_subtree(n_root.right.as_deref(), sub_root_ref)
            }
        }
    }
    // RUST INSIGHT: By using `.as_deref()`, we borrow the contents of the `Box` instead of consuming the value.
    check_subtree(root.as_deref(), sub_root.as_deref())
}

/// Optimized approach: String Serialization and Substring Search (Naive Allocations)
///
/// Time: O(n + m) - Where n and m are the number of nodes in `root` and `subRoot` respectively.
///                  Preorder traversal takes O(n) and O(m). The `contains` substring search typically takes O(n * m) in the worst case (naive string search) or O(n + m) using KMP (which Rust's standard library often uses under the hood depending on pattern size).
/// Space: O(n + m) - To store the serialized string representation of the trees.
///
/// Instead of repeatedly traversing the subtree, we can serialize both trees into strings using a pre-order traversal (including Null nodes).
/// Then, checking if `subRoot` is a subtree of `root` is equivalent to checking if the serialized `subRoot` is a substring of the serialized `root`.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_optimized(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    fn serialize(node: Option<&TreeNode>) -> String {
        match node {
            None => "#".to_string(), // '#' represents a Null node
            // GOTCHA: We must wrap the value in distinct markers like `,{val},` so that "12" doesn't accidentally match "2".
            Some(n) => format!(
                ",{val},{left},{right}",
                val = n.val,
                left = serialize(n.left.as_deref()),
                right = serialize(n.right.as_deref())
            ),
        }
    }

    let root_str = serialize(root.as_deref());
    let sub_root_str = serialize(sub_root.as_deref());

    root_str.contains(&sub_root_str)
}

/// Optimal approach: String Serialization with Zero Intermediate Allocations
///
/// Time: O(n + m) - Preorder traversal and substring search.
/// Space: O(n + m) - We pre-allocate exactly one String for each tree.
///
/// This approach optimizes the string serialization method. `format!` and string concatenation create numerous intermediate, short-lived heap allocations, which can be slow.
/// Instead, we create a single, pre-allocated `String` buffer and append to it directly using `write!`.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_optimal(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    // Helper to serialize directly into a pre-allocated String buffer to avoid intermediate allocations.
    fn serialize(node: Option<&TreeNode>, buffer: &mut String) {
        match node {
            None => {
                buffer.push_str(",#");
            }
            Some(n) => {
                // RUST INSIGHT: `write!` appends directly to the existing String buffer.
                let _ = write!(buffer, ",{}", n.val);
                serialize(n.left.as_deref(), buffer);
                serialize(n.right.as_deref(), buffer);
            }
        }
    }

    // Estimate capacity: rough guess of nodes * chars per node to minimize reallocations
    // We don't know the exact number of nodes, but a small initial capacity is better than zero.
    let mut root_str = String::with_capacity(128);
    serialize(root.as_deref(), &mut root_str);

    let mut sub_root_str = String::with_capacity(128);
    serialize(sub_root.as_deref(), &mut sub_root_str);

    // GOTCHA: Do not naively prepend a global start marker like `^` to the subtree string,
    // as it will prevent matching subtrees that are not the root.
    root_str.contains(&sub_root_str)
}

/// Main entry point - uses the optimal solution
#[must_use]
pub fn is_subtree(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    is_subtree_optimal(root, sub_root)
}

// Alternative Approaches:
// 1. **Merkle Hashing**: Similar to string serialization, we can hash the structure and values of subtrees bottom-up.
//    This allows for an O(n + m) time complexity with potentially less space overhead if we only store hashes, but
//    handling collisions makes the implementation more complex.
// 2. **KMP Algorithm**: If we want guaranteed O(n + m) time complexity, we could serialize the trees into arrays of `Option<i32>`
//    and then implement the Knuth-Morris-Pratt substring search algorithm over the arrays.

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create a leaf node
    fn leaf(val: i32) -> Option<Box<TreeNode>> {
        Some(Box::new(TreeNode::new(val)))
    }

    #[test]
    fn test_happy_path() {
        // root:
        //      3
        //     / \
        //    4   5
        //   / \
        //  1   2
        let mut root = TreeNode::new(3);
        let mut left_child = TreeNode::new(4);
        left_child.left = leaf(1);
        left_child.right = leaf(2);
        root.left = Some(Box::new(left_child));
        root.right = leaf(5);

        // subRoot:
        //    4
        //   / \
        //  1   2
        let mut sub_root = TreeNode::new(4);
        sub_root.left = leaf(1);
        sub_root.right = leaf(2);

        let root_box = Some(Box::new(root));
        let sub_root_box = Some(Box::new(sub_root));

        assert!(is_subtree_brute_force(
            root_box.clone(),
            sub_root_box.clone()
        ));
        assert!(is_subtree_optimized(
            root_box.clone(),
            sub_root_box.clone()
        ));
        assert!(is_subtree_optimal(root_box, sub_root_box));
    }

    #[test]
    fn test_not_a_subtree_extra_node() {
        // root:
        //      3
        //     / \
        //    4   5
        //   / \
        //  1   2
        //     /
        //    0
        let mut root = TreeNode::new(3);
        let mut left_child = TreeNode::new(4);
        left_child.left = leaf(1);

        let mut right_leaf = TreeNode::new(2);
        right_leaf.left = leaf(0);

        left_child.right = Some(Box::new(right_leaf));
        root.left = Some(Box::new(left_child));
        root.right = leaf(5);

        // subRoot:
        //    4
        //   / \
        //  1   2
        let mut sub_root = TreeNode::new(4);
        sub_root.left = leaf(1);
        sub_root.right = leaf(2);

        let root_box = Some(Box::new(root));
        let sub_root_box = Some(Box::new(sub_root));

        assert!(!is_subtree_brute_force(
            root_box.clone(),
            sub_root_box.clone()
        ));
        assert!(!is_subtree_optimized(
            root_box.clone(),
            sub_root_box.clone()
        ));
        assert!(!is_subtree_optimal(root_box, sub_root_box));
    }

    #[test]
    fn test_edge_case_same_tree() {
        let mut root = TreeNode::new(1);
        root.left = leaf(1);
        let sub_root = root.clone();

        let root_box = Some(Box::new(root));
        let sub_root_box = Some(Box::new(sub_root));

        assert!(is_subtree_brute_force(
            root_box.clone(),
            sub_root_box.clone()
        ));
        assert!(is_subtree_optimized(
            root_box.clone(),
            sub_root_box.clone()
        ));
        assert!(is_subtree_optimal(root_box, sub_root_box));
    }

    #[test]
    fn test_stress_case_string_matching_gotcha() {
        // This test ensures that the string matching approach doesn't accidentally
        // match "12" when searching for "2".
        // root:
        //      12
        let root = TreeNode::new(12);

        // subRoot:
        //      2
        let sub_root = TreeNode::new(2);

        let root_box = Some(Box::new(root));
        let sub_root_box = Some(Box::new(sub_root));

        assert!(!is_subtree_brute_force(
            root_box.clone(),
            sub_root_box.clone()
        ));
        assert!(!is_subtree_optimized(
            root_box.clone(),
            sub_root_box.clone()
        ));
        assert!(!is_subtree_optimal(root_box, sub_root_box));
    }
}
