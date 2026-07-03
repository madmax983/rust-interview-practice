//! # 572. Subtree of Another Tree
//!
//! Difficulty: Easy
//! Link: <https://leetcode.com/problems/subtree-of-another-tree/>
//!
//! Given the roots of two binary trees `root` and `subRoot`, return `true` if there is a subtree
//! of `root` with the same structure and node values of `subRoot` and `false` otherwise.
//!
//! This problem perfectly demonstrates Rust's pattern matching, `Option` handling, and zero-allocation
//! string formatting techniques. Working with recursive tree structures in Rust frequently involves
//! `Option<Box<TreeNode>>`, and this exercise highlights the importance of borrowing trees without
//! consuming them (via `.as_deref()`), as well as efficient serialization techniques using
//! `String::with_capacity` and the `std::fmt::Write` trait.

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

/// Helper function to compare two trees for exact equality.
///
/// We take `Option<&TreeNode>` to borrow the nodes without taking ownership.
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

/// Brute Force approach: Recursive Search
///
/// Time: O(M * N) - Where M is the number of nodes in `root` and N is the number of nodes in `subRoot`.
/// Space: O(H) - Where H is the height of `root`, due to recursion stack.
///
/// This approach traverses every node in `root` and checks if the tree rooted at that node
/// is exactly the same as `subRoot`.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_recursive(
    root: Option<Box<TreeNode>>,
    sub_root: Option<Box<TreeNode>>,
) -> bool {
    fn dfs(node: Option<&TreeNode>, sub: Option<&TreeNode>) -> bool {
        match node {
            Some(n) => {
                if is_same_tree(Some(n), sub) {
                    true
                } else {
                    dfs(n.left.as_deref(), sub) || dfs(n.right.as_deref(), sub)
                }
            }
            None => false,
        }
    }

    // RUST INSIGHT: We pass references via as_deref() to avoid consuming the trees,
    // which allows us to compare sub_root multiple times.
    dfs(root.as_deref(), sub_root.as_deref())
}

/// Naive Serialization approach
///
/// Time: O(M + N) - We serialize both trees.
/// Space: O(M + N) - We allocate strings to store the serialized tree forms.
///
/// By serializing both trees using pre-order traversal with structural markers (e.g. `None` -> `#`),
/// we can simply check if `subRoot`'s string is a substring of `root`'s string.
/// This naive version uses `format!` which allocates a new `String` at each step, causing
/// heavy allocation overhead.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_naive(
    root: Option<Box<TreeNode>>,
    sub_root: Option<Box<TreeNode>>,
) -> bool {
    fn serialize(node: Option<&TreeNode>) -> String {
        match node {
            // GOTCHA: We must use a boundary marker like `,` before the value to ensure
            // `12` isn't incorrectly matched with `2` in string comparisons.
            Some(n) => format!(
                ",{}{}{}",
                n.val,
                serialize(n.left.as_deref()),
                serialize(n.right.as_deref())
            ),
            None => ",#".to_string(),
        }
    }

    let root_str = serialize(root.as_deref());
    let sub_str = serialize(sub_root.as_deref());

    root_str.contains(&sub_str)
}

/// Optimal Serialization approach: Zero-Allocation String formatting
///
/// Time: O(M + N) - We visit each node once and do a linear substring search.
/// Space: O(M + N) - We allocate exactly two Strings with pre-computed capacities.
///
/// This avoids intermediate heap allocations by directly writing to pre-allocated `String` buffers
/// using `write!`.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_optimal(
    root: Option<Box<TreeNode>>,
    sub_root: Option<Box<TreeNode>>,
) -> bool {
    // Helper to serialize directly into a pre-allocated String buffer
    fn serialize_optimal(node: Option<&TreeNode>, buf: &mut String) {
        match node {
            Some(n) => {
                // RUST INSIGHT: write! macro appending to a String does not allocate new
                // intermediate strings, making this significantly faster than format!
                let _ = write!(buf, ",{}", n.val);
                serialize_optimal(n.left.as_deref(), buf);
                serialize_optimal(n.right.as_deref(), buf);
            }
            None => {
                let _ = write!(buf, ",#");
            }
        }
    }

    // Heuristically assume around 10 chars per node to pre-allocate capacity
    let mut root_str = String::with_capacity(1024);
    let mut sub_str = String::with_capacity(1024);

    serialize_optimal(root.as_deref(), &mut root_str);
    serialize_optimal(sub_root.as_deref(), &mut sub_str);

    root_str.contains(&sub_str)
}

/// Main entry point
#[must_use]
pub fn is_subtree(
    root: Option<Box<TreeNode>>,
    sub_root: Option<Box<TreeNode>>,
) -> bool {
    is_subtree_optimal(root, sub_root)
}

// Alternative Approaches:
// 1. **KMP Algorithm**: Instead of using the standard `.contains()`, which under the hood might be
//    Two-Way string matching, you could implement Knuth-Morris-Pratt for worst-case O(M + N) guarantees.
// 2. **Merkle Hashing**: Hash each subtree structure and value. This is typically slower for this specific
//    problem but useful for distributed systems (like Git or DynamoDB).

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(val: i32) -> Option<Box<TreeNode>> {
        Some(Box::new(TreeNode::new(val)))
    }

    #[test]
    fn test_happy_path_is_subtree() {
        // root:
        //      3
        //     / \
        //    4   5
        //   / \
        //  1   2
        let mut root = TreeNode::new(3);
        let mut left = TreeNode::new(4);
        left.left = leaf(1);
        left.right = leaf(2);
        root.left = Some(Box::new(left));
        root.right = leaf(5);

        // subRoot:
        //    4
        //   / \
        //  1   2
        let mut sub = TreeNode::new(4);
        sub.left = leaf(1);
        sub.right = leaf(2);

        let root_box = Some(Box::new(root));
        let sub_box = Some(Box::new(sub));

        assert!(is_subtree_recursive(root_box.clone(), sub_box.clone()));
        assert!(is_subtree_naive(root_box.clone(), sub_box.clone()));
        assert!(is_subtree_optimal(root_box, sub_box));
    }

    #[test]
    fn test_edge_case_not_subtree() {
        // root:
        //      3
        //     / \
        //    4   5
        //   / \
        //  1   2
        //     /
        //    0
        let mut root = TreeNode::new(3);
        let mut left = TreeNode::new(4);
        left.left = leaf(1);
        let mut left_right = TreeNode::new(2);
        left_right.left = leaf(0);
        left.right = Some(Box::new(left_right));
        root.left = Some(Box::new(left));
        root.right = leaf(5);

        // subRoot:
        //    4
        //   / \
        //  1   2
        let mut sub = TreeNode::new(4);
        sub.left = leaf(1);
        sub.right = leaf(2);

        let root_box = Some(Box::new(root));
        let sub_box = Some(Box::new(sub));

        assert!(!is_subtree_recursive(root_box.clone(), sub_box.clone()));
        assert!(!is_subtree_naive(root_box.clone(), sub_box.clone()));
        assert!(!is_subtree_optimal(root_box, sub_box));
    }

    #[test]
    fn test_stress_boundary_case() {
        // subRoot is same as root
        let mut root = TreeNode::new(1);
        root.left = leaf(1);

        let mut sub = TreeNode::new(1);
        sub.left = leaf(1);

        let root_box = Some(Box::new(root));
        let sub_box = Some(Box::new(sub));

        assert!(is_subtree_recursive(root_box.clone(), sub_box.clone()));
        assert!(is_subtree_naive(root_box.clone(), sub_box.clone()));
        assert!(is_subtree_optimal(root_box, sub_box));
    }
}
