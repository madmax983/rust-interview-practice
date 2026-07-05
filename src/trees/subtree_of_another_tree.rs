//! # 572. Subtree of Another Tree
//!
//! Difficulty: Easy
//! Link: <https://leetcode.com/problems/subtree-of-another-tree/>
//!
//! Given the roots of two binary trees `root` and `subRoot`, return `true` if there is a subtree of `root` with the same structure and node values of `subRoot` and `false` otherwise.
//!
//! ## Why This Matters in Rust
//!
//! This problem emphasizes the difference between consuming and referencing data in Rust. A naive recursive comparison of trees
//! might try to consume (`move`) the `Option<Box<TreeNode>>` ownership, but since we are just searching/comparing, idiomatic
//! Rust requires passing `Option<&TreeNode>` to borrow the nodes without destroying the tree structure. It also provides a
//! great exercise in pattern matching. We will explore recursive pattern matching and efficient string serialization using
//! `String::with_capacity` and `write!` to avoid intermediate string allocations.

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

/// Helper function to check if two trees are structurally identical and have the same node values.
/// RUST INSIGHT: We use `Option<&TreeNode>` here (borrowing) rather than `Option<Box<TreeNode>>` (consuming)
/// because we only need to inspect the nodes, not take ownership. Using `.as_deref()` allows us to pass references.
fn is_same_tree(p: Option<&TreeNode>, q: Option<&TreeNode>) -> bool {
    match (p, q) {
        (Some(n_p), Some(n_q)) => {
            n_p.val == n_q.val
                && is_same_tree(n_p.left.as_deref(), n_q.left.as_deref())
                && is_same_tree(n_p.right.as_deref(), n_q.right.as_deref())
        }
        (None, None) => true,
        _ => false,
    }
}

/// Approach 1: Recursive Brute-Force Search
///
/// Time: O(M * N) - Where M is nodes in `root` and N is nodes in `subRoot`. For each node in `root`, we might check the whole `subRoot`.
/// Space: O(H) - Where H is the height of `root`, due to the call stack.
///
/// We traverse the `root` tree. At each node, we check if it matches the `subRoot`. If not, we recursively
/// search the left and right subtrees of `root`.
#[must_use]
pub fn is_subtree_brute_force(
    root: Option<Box<TreeNode>>,
    sub_root: Option<Box<TreeNode>>,
) -> bool {
    fn dfs(node: Option<&TreeNode>, sub_root_ref: Option<&TreeNode>) -> bool {
        if let Some(n) = node {
            // Check if current node's subtree matches sub_root
            if is_same_tree(Some(n), sub_root_ref) {
                return true;
            }
            // Otherwise, search left and right children
            return dfs(n.left.as_deref(), sub_root_ref)
                || dfs(n.right.as_deref(), sub_root_ref);
        }
        false
    }

    // We pass references by dereferencing the Option<Box<T>> into Option<&T>
    dfs(root.as_deref(), sub_root.as_deref())
}

/// Approach 2: Naive String Serialization
///
/// Time: O(M + N) - We serialize both trees in linear time, then perform substring search.
/// Space: O(M + N) - We allocate strings proportional to the size of each tree.
///
/// We serialize the trees into strings using a pre-order traversal. We must use delimiters (like `#` and `^`)
/// for nulls and node boundaries so we don't falsely match `2` inside `12`. We then check if the `subRoot`
/// string is a substring of the `root` string.
#[must_use]
pub fn is_subtree_naive_serialization(
    root: Option<Box<TreeNode>>,
    sub_root: Option<Box<TreeNode>>,
) -> bool {
    fn serialize(node: Option<&TreeNode>) -> String {
        match node {
            Some(n) => {
                // GOTCHA: We must wrap values in brackets (e.g. [val]) or similar boundary markers to prevent false matches.
                // For instance, a subtree "2" shouldn't match node "12".
                format!(
                    "[{val}]{left}{right}",
                    val = n.val,
                    left = serialize(n.left.as_deref()),
                    right = serialize(n.right.as_deref())
                )
            }
            None => "#".to_string(), // Null marker
        }
    }

    let root_str = serialize(root.as_deref());
    let sub_str = serialize(sub_root.as_deref());

    root_str.contains(&sub_str)
}

/// Approach 3: Optimal Zero-Allocation Serialization
///
/// Time: O(M + N) - Serialization and substring search are both linear.
/// Space: O(M + N) - We allocate strings exactly once for both trees, reusing buffers.
///
/// Instead of allocating a new `String` for every recursive step and concatenating them, we allocate
/// a pre-sized string buffer and use the `write!` macro to append directly. This significantly reduces
/// heap fragmentation and memory allocations.
#[must_use]
pub fn is_subtree_optimal_serialization(
    root: Option<Box<TreeNode>>,
    sub_root: Option<Box<TreeNode>>,
) -> bool {
    // Helper to serialize into an existing buffer to avoid intermediate allocations
    fn serialize(node: Option<&TreeNode>, buf: &mut String) {
        match node {
            Some(n) => {
                // RUST INSIGHT: write! directly appends to the String without creating temporary formats
                let _ = write!(buf, "[{}]", n.val);
                serialize(n.left.as_deref(), buf);
                serialize(n.right.as_deref(), buf);
            }
            None => {
                buf.push('#');
            }
        }
    }

    // Allocate strings with some initial capacity to avoid reallocations.
    let mut root_str = String::with_capacity(512);
    let mut sub_str = String::with_capacity(128);

    serialize(root.as_deref(), &mut root_str);
    serialize(sub_root.as_deref(), &mut sub_str);

    root_str.contains(&sub_str)
}

/// Main entry point
#[must_use]
pub fn is_subtree(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    is_subtree_optimal_serialization(root, sub_root)
}

// Alternative Approaches:
// 1. **KMP String Matching**: After optimal serialization, you could use the Knuth-Morris-Pratt (KMP) algorithm
//    to ensure worst-case O(M + N) time complexity for the substring search, as standard library `.contains()`
//    might fallback to naive O(M * N) search in the worst case (though practically fast).
// 2. **Merkle Hashing**: You can compute a hash for every subtree and store them in a HashSet or check on the fly.
//    This allows for O(1) comparison during the search phase, taking O(M + N) time total.

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

        assert!(is_subtree_brute_force(root_box.clone(), sub_root_box.clone()));
        assert!(is_subtree_naive_serialization(root_box.clone(), sub_root_box.clone()));
        assert!(is_subtree_optimal_serialization(root_box.clone(), sub_root_box.clone()));
    }

    #[test]
    fn test_not_a_subtree() {
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

        let mut two_node = TreeNode::new(2);
        two_node.left = leaf(0);
        left_child.right = Some(Box::new(two_node));

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

        assert!(!is_subtree_brute_force(root_box.clone(), sub_root_box.clone()));
        assert!(!is_subtree_naive_serialization(root_box.clone(), sub_root_box.clone()));
        assert!(!is_subtree_optimal_serialization(root_box.clone(), sub_root_box.clone()));
    }

    #[test]
    fn test_edge_case_substring_boundary() {
        // This tests that we properly demarcate nodes in serialization,
        // so that finding "2" doesn't falsely trigger on node "12".

        // root:
        //   12
        let root = TreeNode::new(12);

        // subRoot:
        //   2
        let sub_root = TreeNode::new(2);

        let root_box = Some(Box::new(root));
        let sub_root_box = Some(Box::new(sub_root));

        assert!(!is_subtree_brute_force(root_box.clone(), sub_root_box.clone()));
        assert!(!is_subtree_naive_serialization(root_box.clone(), sub_root_box.clone()));
        assert!(!is_subtree_optimal_serialization(root_box.clone(), sub_root_box.clone()));
    }
}
