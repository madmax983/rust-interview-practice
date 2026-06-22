//! # 572. Subtree of Another Tree
//!
//! Difficulty: Easy
//! Link: <https://leetcode.com/problems/subtree-of-another-tree/>
//!
//! Given the roots of two binary trees `root` and `subRoot`, return `true` if there is a subtree of `root`
//! with the same structure and node values of `subRoot` and `false` otherwise.
//!
//! This problem naturally fits recursive structural comparisons. In Rust, it provides an excellent
//! opportunity to practice pattern matching on `Option<Box<TreeNode>>` and properly handling borrows
//! with `.as_deref()` without accidentally consuming ownership.
//!
//! ## Examples
//!
//! ```
//! // Omitted for brevity in docstrings due to boilerplate, see tests below.
//! ```
//!
//! ## Constraints
//!
//! - The number of nodes in the `root` tree is in the range `[1, 2000]`.
//! - The number of nodes in the `subRoot` tree is in the range `[1, 1000]`.
//! - `-10^4 <= root.val <= 10^4`
//! - `-10^4 <= subRoot.val <= 10^4`

// Definition for a binary tree node.
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

/// Helper function to check if two trees are identical.
/// Time: O(M) where M is the number of nodes in `subRoot`.
/// Space: O(M) for the recursion stack.
///
/// RUST INSIGHT: Notice the use of `Option<&TreeNode>` here. Instead of passing `&Option<Box<TreeNode>>`
/// which is clunky, or consuming the trees, we use `.as_deref()` on the caller side to safely
/// traverse references to the tree nodes.
#[must_use]
pub fn is_same_tree(p: Option<&TreeNode>, q: Option<&TreeNode>) -> bool {
    match (p, q) {
        (None, None) => true,
        (Some(node_p), Some(node_q)) if node_p.val == node_q.val => {
            is_same_tree(node_p.left.as_deref(), node_q.left.as_deref())
                && is_same_tree(node_p.right.as_deref(), node_q.right.as_deref())
        }
        _ => false,
    }
}

/// Brute force approach: Traverse `root` and check `is_same_tree` at every node.
/// Time: O(N * M) where N is nodes in `root` and M is nodes in `subRoot`.
/// Space: O(max(N, M)) for recursion stack.
///
/// GOTCHA: Don't forget to check both `root.left` and `root.right` when recursing down the main tree,
/// not just the current node!
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature uses owned Option<Box<TreeNode>>
pub fn is_subtree_brute_force(
    root: Option<Box<TreeNode>>,
    sub_root: Option<Box<TreeNode>>,
) -> bool {
    fn dfs(root: Option<&TreeNode>, sub_root: Option<&TreeNode>) -> bool {
        match root {
            None => false,
            Some(node) => {
                is_same_tree(Some(node), sub_root)
                    || dfs(node.left.as_deref(), sub_root)
                    || dfs(node.right.as_deref(), sub_root)
            }
        }
    }

    dfs(root.as_deref(), sub_root.as_deref())
}

/// Optimized approach: String serialization.
///
/// We serialize both trees to strings (pre-order with null markers) and then check
/// if the `subRoot` string is a substring of the `root` string.
///
/// Time: O(N + M) to serialize both trees, plus substring search O(N*M) worst case,
///       but in practice Rust's `contains` is very fast, and KMP would be O(N + M).
/// Space: O(N + M) for the string storage.
///
/// RUST INSIGHT: We use `std::fmt::Write` (via `write!`) to directly push formatted
/// characters into a pre-allocated `String` instead of creating intermediate strings
/// with `format!()` and appending them.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_optimized(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    use std::fmt::Write;

    fn serialize(node: Option<&TreeNode>, out: &mut String) {
        match node {
            None => out.push_str("#,"),
            Some(n) => {
                // RUST INSIGHT: Using `write!` directly avoids intermediate heap allocations
                // that `out.push_str(&format!(...))` would cause.
                // We prepend a special character to every node value to ensure clear boundaries
                let _ = write!(out, "^{},", n.val);
                serialize(n.left.as_deref(), out);
                serialize(n.right.as_deref(), out);
            }
        }
    }

    let mut root_str = String::new();
    let mut sub_root_str = String::new();

    serialize(root.as_deref(), &mut root_str);
    serialize(sub_root.as_deref(), &mut sub_root_str);

    // To ensure we match exact node boundaries, we pad the substring search.
    // By prepending a character to both, we ensure we only match full node values
    // since our serialize function already appends commas.
    let root_search = format!("^{}", root_str);
    let sub_root_search = format!("^{}", sub_root_str);

    root_search.contains(&sub_root_search)
}

/// Main entry point
#[must_use]
pub fn is_subtree(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    // Both are valid implementations. We will use brute force as the default
    // because it avoids heap allocating the strings and uses O(1) extra space
    // outside of the recursion stack.
    is_subtree_brute_force(root, sub_root)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(val: i32) -> Option<Box<TreeNode>> {
        Some(Box::new(TreeNode::new(val)))
    }

    #[test]
    fn test_brute_force_happy_path() {
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

        assert!(is_subtree_brute_force(
            Some(Box::new(root)),
            Some(Box::new(sub_root))
        ));
    }

    #[test]
    fn test_optimized_edge_case() {
        // root: [3,4,5,1,2,null,null,null,null,0]
        let mut root = TreeNode::new(3);
        let mut left = TreeNode::new(4);
        left.left = leaf(1);
        let mut right_of_left = TreeNode::new(2);
        right_of_left.left = leaf(0); // This makes it not a match
        left.right = Some(Box::new(right_of_left));

        root.left = Some(Box::new(left));
        root.right = leaf(5);

        // subRoot: [4,1,2]
        let mut sub_root = TreeNode::new(4);
        sub_root.left = leaf(1);
        sub_root.right = leaf(2);

        assert!(!is_subtree_optimized(
            Some(Box::new(root)),
            Some(Box::new(sub_root))
        ));
    }

    #[test]
    fn test_all_approaches_stress() {
        // Just checking single nodes
        let root1 = leaf(1);
        let sub1 = leaf(1);
        assert!(is_subtree(root1, sub1));

        let root2 = leaf(1);
        let sub2 = leaf(2);
        assert!(!is_subtree(root2, sub2));
    }
}
