//! # 572. Subtree of Another Tree
//!
//! Difficulty: Easy
//! Link: <https://leetcode.com/problems/subtree-of-another-tree/>
//!
//! Given the roots of two binary trees `root` and `subRoot`, return `true` if there is a subtree of `root`
//! with the same structure and node values of `subRoot` and `false` otherwise.
//!
//! A subtree of a binary tree `tree` is a tree that consists of a node in `tree` and all of this node's
//! descendants. The tree `tree` could also be considered as a subtree of itself.
//!
//! This problem emphasizes tree traversal and structural comparison. In Rust, it provides an excellent
//! opportunity to explore how `Option<Box<TreeNode>>` and ownership interact, particularly highlighting
//! the idiomatic pattern of taking `Option<&TreeNode>` via `.as_deref()` instead of passing
//! `&Option<Box<TreeNode>>`, avoiding unnecessary cloning or value consumption.
//!
//! ## Approaches
//!
//! 1.  **Brute Force (Recursive Search & Compare):** For every node in `root`, check if the subtree rooted at
//!     that node is identical to `subRoot`. This is straightforward but potentially slow if there are many
//!     matching nodes and deep trees. Time: O(M * N), Space: O(M + N).
//! 2.  **Optimized (Naive String Serialization):** Serialize both trees into strings (e.g., pre-order traversal
//!     with null markers) and check if `subRoot`'s string is a substring of `root`'s string.
//!     Time: O(M + N), Space: O(M + N).
//! 3.  **Optimal (Zero-Allocation Serialization):** Similar to the string serialization, but heavily minimizes
//!     allocations by estimating capacity up front and using `write!` directly to pre-allocated buffers.
//!     Time: O(M + N), Space: O(M + N) but significantly lower constant overhead.
//!
//! ## Alternative Approaches
//!
//! - **Merkle Tree / Hashing:** Compute a hash for every subtree bottom-up. This achieves O(M + N) time
//!   without serializing the entire tree to a string, which can be memory intensive for very large trees.
//!   However, handling hash collisions robustly adds complexity, and a simple string substring search is
//!   often fast enough in practice (especially in languages with efficient substring implementations like Rust).
//! - **KMP Algorithm:** For the string matching part of the serialization approach, one could implement the
//!   Knuth-Morris-Pratt (KMP) algorithm for guaranteed O(M + N) time string search instead of relying on
//!   `str::contains` (which might fall back to naive search in worst-case scenarios, though standard library
//!   implementations are highly optimized).

use std::fmt::Write as _;

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

// =========================================================================================
// Brute Force Approach
// =========================================================================================

/// Brute Force: Recursive Search & Compare
///
/// Time: O(M * N) - In the worst case (e.g., highly imbalanced trees with many matching prefixes),
/// we might compare the entire `subRoot` tree of size N for every node in the `root` tree of size M.
/// Space: O(max(M, N)) - The recursion stack depth, which is the maximum height of either tree.
///
/// We recursively traverse `root`. At each node, we perform a full check (using a helper function similar
/// to "Same Tree") to see if the current node and its descendants exactly match `subRoot`.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_brute_force(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    // Helper to check if two trees are identical.
    // RUST INSIGHT: We pass Option<&TreeNode> instead of &Option<Box<TreeNode>>.
    // This allows us to work with references to the nodes directly, avoiding the Box indirection
    // and making pattern matching cleaner. We get Option<&TreeNode> using .as_deref().
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

    // Main recursive function to traverse `root`.
    fn dfs(current: Option<&TreeNode>, target: Option<&TreeNode>) -> bool {
        match current {
            Some(node) => {
                // Check if the trees match starting at the current node.
                if is_same_tree(current, target) {
                    return true;
                }
                // Otherwise, search in left and right subtrees.
                dfs(node.left.as_deref(), target) || dfs(node.right.as_deref(), target)
            }
            None => false, // We reached the end of `root` without finding a match.
        }
    }

    // GOTCHA: `sub_root` could conceptually be `None`, but LeetCode constraints typically say
    // it has at least 1 node. Regardless, our logic handles `None` correctly.
    dfs(root.as_deref(), sub_root.as_deref())
}

// =========================================================================================
// Optimized Approach
// =========================================================================================

/// Optimized: Naive String Serialization
///
/// Time: O(M + N) - Serializing both trees takes O(M + N) time. The substring search `contains`
/// is highly optimized in standard libraries and generally performs in linear time on average.
/// Space: O(M + N) - Allocating the full string representations for both trees.
///
/// We serialize both trees using pre-order traversal, including explicit markers for null nodes
/// and node boundaries to ensure unique structural representations. Then we check if the serialized
/// `subRoot` is a substring of the serialized `root`.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_optimized(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    // Helper to serialize the tree to a String.
    // We use a specific format like "#valL..R.." to mark node start, value, and left/right children,
    // and "N" for null.
    fn serialize(node: Option<&TreeNode>) -> String {
        match node {
            Some(n) => {
                // GOTCHA: We must mark the start of a node clearly (e.g., with `#`).
                // Otherwise, a value like `12` in the main tree might match a subtree root value of `2`.
                format!(
                    "#{}{}{}",
                    n.val,
                    serialize(n.left.as_deref()),
                    serialize(n.right.as_deref())
                )
            }
            None => "N".to_string(), // Null marker
        }
    }

    let root_str = serialize(root.as_deref());
    let sub_root_str = serialize(sub_root.as_deref());

    // Simple substring search.
    root_str.contains(&sub_root_str)
}

// =========================================================================================
// Optimal Approach
// =========================================================================================

/// Optimal: Zero-Allocation Serialization (Capacity pre-estimation & `write!`)
///
/// Time: O(M + N) - Linear traversal to serialize, followed by linear (average) substring search.
/// Space: O(M + N) - For the final string buffers, but we avoid intermediate heap allocations.
///
/// This refines the string serialization approach by avoiding the recursive creation of intermediate
/// `String` objects (like `format!` does in the optimized version). We pre-allocate a single buffer
/// with an estimated capacity and use `write!` to append to it iteratively.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_optimal(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    // Helper to count nodes to estimate string capacity.
    fn count_nodes(node: Option<&TreeNode>) -> usize {
        match node {
            Some(n) => 1 + count_nodes(n.left.as_deref()) + count_nodes(n.right.as_deref()),
            None => 0,
        }
    }

    // Helper to serialize directly into a pre-allocated String buffer.
    fn serialize_to_buffer(node: Option<&TreeNode>, buffer: &mut String) {
        match node {
            Some(n) => {
                // RUST INSIGHT: write! macro allows us to append formatted data directly into an
                // existing String buffer without creating intermediate String allocations.
                // It returns a Result, but appending to a String only fails on OOM, so we can
                // safely unwrap or discard the result in this context (we use `let _ = ...`).
                let _ = write!(buffer, "#{}_", n.val);
                serialize_to_buffer(n.left.as_deref(), buffer);
                serialize_to_buffer(n.right.as_deref(), buffer);
            }
            None => {
                let _ = write!(buffer, "N");
            }
        }
    }

    let root_ref = root.as_deref();
    let sub_root_ref = sub_root.as_deref();

    // Estimate capacity: Each node contributes about 5-10 chars (e.g., "#123_NN").
    // We multiply node count by 8 as a reasonable heuristic.
    let root_count = count_nodes(root_ref);
    let sub_root_count = count_nodes(sub_root_ref);

    let mut root_str = String::with_capacity(root_count * 8);
    let mut sub_root_str = String::with_capacity(sub_root_count * 8);

    serialize_to_buffer(root_ref, &mut root_str);
    serialize_to_buffer(sub_root_ref, &mut sub_root_str);

    root_str.contains(&sub_root_str)
}

/// Main entry point
#[must_use]
pub fn is_subtree(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    is_subtree_optimal(root, sub_root)
}

// =========================================================================================
// Tests
// =========================================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create a leaf node
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
        let mut node4 = TreeNode::new(4);
        node4.left = leaf(1);
        node4.right = leaf(2);
        root.left = Some(Box::new(node4));
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
    fn test_edge_case_not_subtree_extra_node() {
        // root:
        //      3
        //     / \
        //    4   5
        //   / \
        //  1   2
        //     /
        //    0
        let mut root = TreeNode::new(3);
        let mut node4 = TreeNode::new(4);
        let mut node2 = TreeNode::new(2);
        node2.left = leaf(0);

        node4.left = leaf(1);
        node4.right = Some(Box::new(node2));
        root.left = Some(Box::new(node4));
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
    fn test_stress_boundary_value_match_issue() {
        // This test ensures our serialization doesn't falsely match numbers.
        // For example, if root is [12], its string might be "12".
        // If subRoot is [2], its string might be "2", which is a substring of "12"
        // if boundaries aren't properly marked.

        // root:
        //    12
        let root = leaf(12);

        // subRoot:
        //    2
        let sub_root = leaf(2);

        assert!(!is_subtree_brute_force(
            root.clone(),
            sub_root.clone()
        ));
        assert!(!is_subtree_optimized(
            root.clone(),
            sub_root.clone()
        ));
        assert!(!is_subtree_optimal(root, sub_root));
    }

    #[test]
    fn test_same_tree_is_subtree() {
        // A tree is a subtree of itself.
        let mut root = TreeNode::new(1);
        root.left = leaf(2);
        root.right = leaf(3);

        let mut sub_root = TreeNode::new(1);
        sub_root.left = leaf(2);
        sub_root.right = leaf(3);

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
}
