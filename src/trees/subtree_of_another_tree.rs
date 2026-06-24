//! # 572. Subtree of Another Tree
//!
//! Difficulty: Easy
//! Link: <https://leetcode.com/problems/subtree-of-another-tree/>
//!
//! Given the roots of two binary trees `root` and `subRoot`, return `true` if there is a subtree of `root`
//! with the same structure and node values of `subRoot` and `false` otherwise.
//!
//! A subtree of a binary tree `tree` is a tree that consists of a node in `tree` and all of this node's descendants.
//! The tree `tree` could also be considered as a subtree of itself.
//!
//! This problem emphasizes the correct handling of recursive structures and `Option<Box<TreeNode>>` matching
//! in Rust. It highlights ownership versus borrowing, particularly why we pass `Option<&TreeNode>` or use
//! `.as_deref()` to avoid consuming the original trees while performing matching.

use std::collections::VecDeque;

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

/// Helper function to determine if two trees are identical.
///
/// RUST INSIGHT: We take `Option<&TreeNode>` instead of consuming `Option<Box<TreeNode>>`.
/// This prevents us from destroying the tree while merely checking if a subtree matches.
fn is_same_tree(p: Option<&TreeNode>, q: Option<&TreeNode>) -> bool {
    match (p, q) {
        (Some(node_p), Some(node_q)) => {
            node_p.val == node_q.val
                && is_same_tree(node_p.left.as_deref(), node_q.left.as_deref())
                && is_same_tree(node_p.right.as_deref(), node_q.right.as_deref())
        }
        (None, None) => true,
        _ => false, // Structural mismatch or one is empty
    }
}

/// Brute Force / Recursive Approach
///
/// Time: O(M * N) - Where M is the number of nodes in `root` and N is the number of nodes in `subRoot`.
///                  In the worst case, we might check `is_same_tree` at every node.
/// Space: O(H) - The maximum depth of the recursion stack, where H is the height of `root`.
///
/// This approach navigates through every node of `root`. At each node, it initiates a recursive check
/// (`is_same_tree`) to see if the subtree starting at that node matches `subRoot`.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_recursive(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    fn dfs(node: Option<&TreeNode>, sub_root_ref: Option<&TreeNode>) -> bool {
        match node {
            Some(n) => {
                // If the current tree matches the subRoot, we're done.
                if is_same_tree(Some(n), sub_root_ref) {
                    return true;
                }
                // Otherwise, check the left and right children.
                dfs(n.left.as_deref(), sub_root_ref) || dfs(n.right.as_deref(), sub_root_ref)
            }
            // If the current node is None, it can't contain the subRoot (unless subRoot is also None,
            // but constraints usually say subRoot has >= 1 node).
            None => false,
        }
    }

    // Convert the owned roots to borrowed references to avoid consuming them.
    dfs(root.as_deref(), sub_root.as_deref())
}

/// Optimized Approach: Serialization String Matching
///
/// Time: O(M + N) - We serialize both trees in O(M) and O(N) time, then perform substring search.
/// Space: O(M + N) - Storing the serialized strings takes space proportional to the number of nodes.
///
/// Instead of repeatedly traversing `subRoot`, we can serialize both trees into strings (pre-order).
/// If `subRoot` is a subtree of `root`, its serialized string will be a substring of `root`'s serialized string.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_optimized(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    // Helper to perform pre-order serialization
    fn serialize(node: Option<&TreeNode>, out: &mut String) {
        match node {
            Some(n) => {
                // GOTCHA: We must include strict node boundary markers like `,` and `#` to avoid false positives.
                // For instance, a node with value `12` should not match a `subRoot` string of `2`.
                // By formatting as `,12,`, we ensure distinct node matching.
                out.push(',');
                out.push_str(&n.val.to_string());
                out.push(',');
                serialize(n.left.as_deref(), out);
                serialize(n.right.as_deref(), out);
            }
            None => {
                out.push_str("#");
            }
        }
    }

    let mut root_str = String::new();
    let mut sub_str = String::new();

    serialize(root.as_deref(), &mut root_str);
    serialize(sub_root.as_deref(), &mut sub_str);

    // Using Rust's built-in substring search (which may use advanced algorithms like two-way string matching).
    root_str.contains(&sub_str)
}

/// Main entry point delegating to the recursive approach (typically preferred for avoiding large allocations).
#[must_use]
pub fn is_subtree(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    is_subtree_recursive(root, sub_root)
}

// Alternative Approaches:
// 1. KMP Algorithm: You could implement KMP over a traversal array to achieve strict O(M + N)
//    without the overhead of string allocations, but it is highly complex to implement correctly in an interview.
// 2. Merkle Tree Hashing: Compute a hash for every subtree in a bottom-up manner. If a hash in `root` matches
//    the root hash of `subRoot`, they might be identical (verify to avoid hash collisions). Time: O(M + N).

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to quickly build leaf nodes
    fn leaf(val: i32) -> Option<Box<TreeNode>> {
        Some(Box::new(TreeNode::new(val)))
    }

    #[test]
    fn test_happy_path_is_subtree() {
        // root: [3,4,5,1,2]
        let mut root = TreeNode::new(3);
        let mut left = TreeNode::new(4);
        left.left = leaf(1);
        left.right = leaf(2);
        root.left = Some(Box::new(left));
        root.right = leaf(5);

        // subRoot: [4,1,2]
        let mut sub_root = TreeNode::new(4);
        sub_root.left = leaf(1);
        sub_root.right = leaf(2);

        let r_box = Some(Box::new(root));
        let s_box = Some(Box::new(sub_root));

        assert!(is_subtree_recursive(r_box.clone(), s_box.clone()));
        assert!(is_subtree_optimized(r_box, s_box));
    }

    #[test]
    fn test_edge_case_not_subtree_due_to_extra_children() {
        // root: [3,4,5,1,2,null,null,null,null,0]
        let mut root = TreeNode::new(3);
        let mut left = TreeNode::new(4);
        left.left = leaf(1);

        let mut right_of_left = TreeNode::new(2);
        right_of_left.left = leaf(0); // Extra child here ruins the subtree match!
        left.right = Some(Box::new(right_of_left));

        root.left = Some(Box::new(left));
        root.right = leaf(5);

        // subRoot: [4,1,2]
        let mut sub_root = TreeNode::new(4);
        sub_root.left = leaf(1);
        sub_root.right = leaf(2);

        let r_box = Some(Box::new(root));
        let s_box = Some(Box::new(sub_root));

        assert!(!is_subtree_recursive(r_box.clone(), s_box.clone()));
        assert!(!is_subtree_optimized(r_box, s_box));
    }

    #[test]
    fn test_stress_boundary_string_prefix_trap() {
        // root: [12]
        let root = TreeNode::new(12);

        // subRoot: [2]
        let sub_root = TreeNode::new(2);

        // A naive string serialization like "12#" contains "2#", which would be a false positive!
        // Our optimized logic using strict `,val,` boundaries (`.contains(",2,")`) prevents this.
        let r_box = Some(Box::new(root));
        let s_box = Some(Box::new(sub_root));

        assert!(!is_subtree_recursive(r_box.clone(), s_box.clone()));
        assert!(!is_subtree_optimized(r_box, s_box));
    }
}
