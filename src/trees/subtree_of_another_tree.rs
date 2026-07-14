//! # 572. Subtree of Another Tree
//!
//! Difficulty: Easy
//! Link: <https://leetcode.com/problems/subtree-of-another-tree/>
//!
//! Given the roots of two binary trees `root` and `subRoot`, return `true` if there is a subtree of `root`
//! with the same structure and node values of `subRoot` and `false` otherwise.
//!
//! This problem demonstrates pattern matching with `Option<Box<TreeNode>>` and recursion. It also highlights
//! how we can traverse trees, compare their equality structurally, and how to optimize algorithms like tree
//! serialization into substrings for efficient pattern matching, contrasting memory overhead in typical solutions.
//!
//! ## Approach
//!
//! **Brute Force:** Traverse the original tree and for each node encountered, check if it matches the `subRoot`.
//! This uses `O(N*M)` time where `N` is nodes in `root` and `M` is nodes in `subRoot`.
//!
//! **Naive Serialize:** Serialize both trees into strings (pre-order, including `Null` markers to retain shape),
//! then perform string matching (e.g. `root_str.contains(&subRoot_str)`). Time is `O(N+M)` but space is `O(N+M)`
//! because it constructs huge formatted strings.
//!
//! **Optimal Serialize:** Zero-allocation serialization using `String::with_capacity` and `write!`. Time is `O(N+M)`,
//! space is `O(N+M)` but with drastically lower constant factors and heap reallocations.

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

/// Helper function to check if two trees are identical.
fn is_same_tree(p: Option<&TreeNode>, q: Option<&TreeNode>) -> bool {
    // RUST INSIGHT: Matching on a tuple allows exhaustive checking of both `Option`s simultaneously.
    match (p, q) {
        (Some(node_p), Some(node_q)) => {
            node_p.val == node_q.val
                && is_same_tree(node_p.left.as_deref(), node_q.left.as_deref())
                && is_same_tree(node_p.right.as_deref(), node_q.right.as_deref())
        }
        (None, None) => true,
        _ => false,
    }
}

/// Brute Force Approach: Recursive structural checking
///
/// Time: O(N * M) - For each of the N nodes in `root`, we might check up to M nodes in `subRoot`.
/// Space: O(H) - Max recursion depth where H is the height of `root`.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_brute_force(
    root: Option<Box<TreeNode>>,
    sub_root: Option<Box<TreeNode>>,
) -> bool {
    // Helper so we don't consume the tree structures, just pass by reference
    fn dfs(node: Option<&TreeNode>, target: Option<&TreeNode>) -> bool {
        if let Some(n) = node {
            if is_same_tree(Some(n), target) {
                return true;
            }
            // GOTCHA: `.as_deref()` converts Option<&Box<TreeNode>> to Option<&TreeNode> cleanly.
            dfs(n.left.as_deref(), target) || dfs(n.right.as_deref(), target)
        } else {
            false
        }
    }

    // Edge case handling. An empty sub_root is technically a subtree of any tree.
    if sub_root.is_none() {
        return true;
    }

    dfs(root.as_deref(), sub_root.as_deref())
}

/// Naive Serialization Approach
///
/// Time: O(N + M) - O(N) to build root string, O(M) to build subRoot string, O(N) to find substring.
/// Space: O(N + M) - Requires allocating strings representing the entire tree.
///
/// We do a pre-order traversal and build a string representation of the tree.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_naive_serialize(
    root: Option<Box<TreeNode>>,
    sub_root: Option<Box<TreeNode>>,
) -> bool {
    fn serialize(node: Option<&TreeNode>) -> String {
        match node {
            Some(n) => {
                // GOTCHA: using `format!` here on every recursion creates tons of temporary Strings
                // and concatenates them, scaling very poorly in memory.
                format!(
                    "^{}${}{}",
                    n.val,
                    serialize(n.left.as_deref()),
                    serialize(n.right.as_deref())
                )
            }
            None => "#".to_string(),
        }
    }

    let root_str = serialize(root.as_deref());
    let sub_root_str = serialize(sub_root.as_deref());

    root_str.contains(&sub_root_str)
}

/// Optimal Serialization Approach
///
/// Time: O(N + M) - Linear traversal to build strings, and linear substring matching.
/// Space: O(N + M) - Still needs strings, but we allocate once with `String::with_capacity`
///                   and use the `write!` macro for zero-allocation appending.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_optimal(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    // Helper that appends directly to an existing mutable String buffer
    fn serialize(node: Option<&TreeNode>, buf: &mut String) {
        match node {
            Some(n) => {
                // RUST INSIGHT: `write!` macro writes directly to `String` (which impls `fmt::Write`).
                // This avoids intermediate string allocations like `format!` would do.
                let _ = write!(buf, "^{}$", n.val);
                serialize(n.left.as_deref(), buf);
                serialize(n.right.as_deref(), buf);
            }
            None => {
                let _ = write!(buf, "#");
            }
        }
    }

    // In a real scenario, we could guess capacities. 1000 nodes ~ a few KB max.
    let mut root_str = String::with_capacity(4096);
    let mut sub_root_str = String::with_capacity(1024);

    serialize(root.as_deref(), &mut root_str);
    serialize(sub_root.as_deref(), &mut sub_root_str);

    root_str.contains(&sub_root_str)
}

/// Main entry point
#[must_use]
pub fn is_subtree(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    is_subtree_optimal(root, sub_root)
}

// Alternative Approaches:
// 1. **Merkle Hashing**: You can compute a hash for every subtree bottom-up. Then compare the hashes of `subRoot`
//    and nodes in `root`. This takes O(N+M) time and O(N+M) space but handles huge trees efficiently without strings.
// 2. **KMP String Matching**: Standard `.contains()` in Rust is usually highly optimized (using two-way algorithm),
//    but a manual KMP implementation on a Vec of enums could theoretically avoid string formatting overhead.

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(val: i32) -> Option<Box<TreeNode>> {
        Some(Box::new(TreeNode::new(val)))
    }

    #[test]
    fn test_happy_path() {
        // root: [3,4,5,1,2]
        //      3
        //     / \
        //    4   5
        //   / \
        //  1   2
        let mut root = TreeNode::new(3);
        let mut node4 = TreeNode::new(4);
        node4.left = leaf(1);
        node4.right = leaf(2);
        root.left = Some(Box::new(node4));
        root.right = leaf(5);

        // subRoot: [4,1,2]
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
        assert!(is_subtree_naive_serialize(
            root_box.clone(),
            sub_root_box.clone()
        ));
        assert!(is_subtree_optimal(root_box, sub_root_box));
    }

    #[test]
    fn test_edge_case_not_subtree_due_to_extra_leaf() {
        // root: [3,4,5,1,2,null,null,null,null,0]
        //      3
        //     / \
        //    4   5
        //   / \
        //  1   2
        //     /
        //    0
        let mut root = TreeNode::new(3);
        let mut node4 = TreeNode::new(4);
        node4.left = leaf(1);

        let mut node2 = TreeNode::new(2);
        node2.left = leaf(0);
        node4.right = Some(Box::new(node2));

        root.left = Some(Box::new(node4));
        root.right = leaf(5);

        // subRoot: [4,1,2]
        let mut sub_root = TreeNode::new(4);
        sub_root.left = leaf(1);
        sub_root.right = leaf(2);

        let root_box = Some(Box::new(root));
        let sub_root_box = Some(Box::new(sub_root));

        assert!(!is_subtree_brute_force(
            root_box.clone(),
            sub_root_box.clone()
        ));
        assert!(!is_subtree_naive_serialize(
            root_box.clone(),
            sub_root_box.clone()
        ));
        assert!(!is_subtree_optimal(root_box, sub_root_box));
    }

    #[test]
    fn test_stress_boundary_identical_trees() {
        let root = leaf(1);
        let sub_root = leaf(1);

        assert!(is_subtree_brute_force(root.clone(), sub_root.clone()));
        assert!(is_subtree_naive_serialize(root.clone(), sub_root.clone()));
        assert!(is_subtree_optimal(root, sub_root));
    }
}
