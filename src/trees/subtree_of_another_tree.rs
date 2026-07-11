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
//! This problem naturally builds upon checking if two trees are identical. It demonstrates how to perform a search
//! combined with a matching algorithm. In Rust, it provides excellent practice for avoiding unnecessary allocations
//! by passing references (`Option<&TreeNode>`) using `.as_deref()` instead of cloning or moving `Box`ed nodes.
//! It also explores structural serialization to strings as an alternative approach, contrasting standard format macros
//! against optimized `std::fmt::Write` zero-allocation buffers.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::trees::subtree_of_another_tree::{is_subtree, TreeNode};
//!
//! // root: [3,4,5,1,2]
//! let mut root = TreeNode::new(3);
//! let mut left = TreeNode::new(4);
//! left.left = Some(Box::new(TreeNode::new(1)));
//! left.right = Some(Box::new(TreeNode::new(2)));
//! root.left = Some(Box::new(left));
//! root.right = Some(Box::new(TreeNode::new(5)));
//!
//! // subRoot: [4,1,2]
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
//!
//! ## Approach
//!
//! The solution involves three distinct approaches:
//! 1. **Brute Force (Recursive Matching):** Traverse the main tree, and for each node, treat it as a potential start
//!    of the subtree and perform a deep equality check against `subRoot`. This approach has a worst-case time complexity of O(M * N).
//!    It is idiomatic Rust because it uses `.as_deref()` to pass non-consuming references.
//!
//! 2. **Naive Serialization:** Perform a pre-order traversal on both trees to serialize their structure (including `Null`s
//!    to disambiguate nodes with 1 child) into `String`s, then perform a substring search. While the concept is O(M + N),
//!    frequent string appending (`+` or `format!`) can create large intermediate heap allocations.
//!
//! 3. **Optimized Serialization:** The serialization approach is refined by using a pre-allocated string buffer and the
//!    `write!` macro. This approach eliminates intermediate string allocations completely, executing the serialization in
//!    a single O(M + N) pass over each tree and using the KMP-like `.contains()` method for the search.

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

/// Helper function to check if two trees are structurally and identically equal.
fn is_same_tree(p: Option<&TreeNode>, q: Option<&TreeNode>) -> bool {
    // RUST INSIGHT: Matching on a tuple handles all `Some`/`None` combinations exhaustively without deep nesting.
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

/// Brute Force approach: Recursive traversal and matching.
///
/// Time Complexity: O(M * N) where M is the number of nodes in `root` and N is the number of nodes in `subRoot`.
/// In the worst case (a highly skewed tree), we might check `is_same_tree` for every node.
/// Space Complexity: O(H) where H is the height of the `root` tree (for the recursion stack).
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_brute_force(
    root: Option<Box<TreeNode>>,
    sub_root: Option<Box<TreeNode>>,
) -> bool {
    // Helper function that takes references to avoid consuming the `Box`es prematurely.
    fn check_subtree(r: Option<&TreeNode>, sr: Option<&TreeNode>) -> bool {
        match r {
            None => false,
            Some(node) => {
                // Check if the trees match starting at this node.
                if is_same_tree(r, sr) {
                    return true;
                }
                // Otherwise, search the left and right subtrees.
                check_subtree(node.left.as_deref(), sr)
                    || check_subtree(node.right.as_deref(), sr)
            }
        }
    }

    // GOTCHA: `root` and `sub_root` are owned `Option<Box<TreeNode>>`.
    // `.as_deref()` safely converts them to `Option<&TreeNode>`.
    check_subtree(root.as_deref(), sub_root.as_deref())
}

/// Naive Serialization approach: Convert trees to strings and check for substring.
///
/// Time Complexity: O(M + N + M*N) where string construction is O(M + N), but `contains` can be O(M*N) for naive search.
/// Space Complexity: O(M + N) to hold the serialized strings.
///
/// Note: This approach uses intermediate strings and concatenation, which incurs a heavy heap allocation penalty.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_naive(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    fn serialize(node: Option<&TreeNode>) -> String {
        match node {
            // RUST INSIGHT: We must surround values with markers (like `^` and `$`) to prevent partial matches.
            // For example, if a node has value `12`, it shouldn't match a subtree root with value `2`.
            Some(n) => format!(
                "^{}${}{}",
                n.val,
                serialize(n.left.as_deref()),
                serialize(n.right.as_deref())
            ),
            None => "#".to_string(), // '#' represents a null node to maintain structural integrity.
        }
    }

    let root_str = serialize(root.as_deref());
    let sub_root_str = serialize(sub_root.as_deref());

    root_str.contains(&sub_root_str)
}

/// Optimized Serialization approach: Zero intermediate allocations using `std::fmt::Write`.
///
/// Time Complexity: O(M + N) - Serialization runs in linear time. Substring matching using standard lib is highly optimized.
/// Space Complexity: O(M + N) - For the pre-allocated string buffers.
///
/// This approach mitigates the heavy memory overhead of the naive implementation by pre-allocating a single buffer
/// and appending to it in-place.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_optimal(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    fn serialize(node: Option<&TreeNode>, buffer: &mut String) {
        match node {
            Some(n) => {
                // RUST INSIGHT: `write!` appends directly to the `String` buffer without creating intermediate strings.
                // It returns a `Result`, which we must handle (or explicitly ignore via `let _ = `).
                let _ = write!(buffer, "^{}$", n.val);
                serialize(n.left.as_deref(), buffer);
                serialize(n.right.as_deref(), buffer);
            }
            None => {
                buffer.push('#');
            }
        }
    }

    // Allocate buffers with a reasonable initial capacity based on constraints (max 2000 nodes).
    // Each node serialization adds a few characters, so 10000 bytes is a safe bet to avoid reallocation.
    let mut root_str = String::with_capacity(10000);
    let mut sub_root_str = String::with_capacity(5000);

    serialize(root.as_deref(), &mut root_str);
    serialize(sub_root.as_deref(), &mut sub_root_str);

    root_str.contains(&sub_root_str)
}

/// Main entry point
#[must_use]
pub fn is_subtree(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    // The brute force approach is chosen as the default because it doesn't incur the string allocation overhead
    // and is very fast in practice for reasonably balanced trees.
    is_subtree_brute_force(root, sub_root)
}

// Alternative Approaches:
// 1. Merkle Hashing: You could compute a hash for every subtree (incorporating the value and the hashes of its children).
//    Then finding a subtree becomes an O(1) hash lookup. This is heavily used in distributed systems (like Merkle Trees).
// 2. KMP Algorithm: If we strictly use the string serialization, implementing a manual Knuth-Morris-Pratt (KMP)
//    search algorithm would guarantee O(M + N) worst-case time complexity, avoiding the standard library's
//    potential O(M*N) fallback depending on the exact string search implementation (though Rust's `str::contains` uses Two-Way).

#[cfg(test)]
mod tests {
    #![allow(clippy::unnecessary_wraps)]

    use super::*;

    fn leaf(val: i32) -> Option<Box<TreeNode>> {
        Some(Box::new(TreeNode::new(val)))
    }

    #[test]
    fn test_is_subtree_happy_path() {
        // root: [3,4,5,1,2]
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

        // subRoot: [4,1,2]
        let mut sub_root = TreeNode::new(4);
        sub_root.left = leaf(1);
        sub_root.right = leaf(2);

        let root_box = Some(Box::new(root));
        let sub_root_box = Some(Box::new(sub_root));

        assert!(is_subtree_brute_force(
            root_box.clone(),
            sub_root_box.clone()
        ));
        assert!(is_subtree_naive(root_box.clone(), sub_root_box.clone()));
        assert!(is_subtree_optimal(root_box, sub_root_box));
    }

    #[test]
    fn test_is_subtree_edge_case_same_tree() {
        // When the root and sub_root are exactly the same tree
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
        assert!(is_subtree_naive(root_box.clone(), sub_root_box.clone()));
        assert!(is_subtree_optimal(root_box, sub_root_box));
    }

    #[test]
    fn test_is_subtree_boundary_almost_match() {
        // root: [3,4,5,1,2,null,null,null,null,0]
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
        let mut two = TreeNode::new(2);
        two.left = leaf(0); // This makes it different from subRoot
        left.right = Some(Box::new(two));
        root.left = Some(Box::new(left));
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
        assert!(!is_subtree_naive(root_box.clone(), sub_root_box.clone()));
        assert!(!is_subtree_optimal(root_box, sub_root_box));
    }

    #[test]
    fn test_is_subtree_partial_match_values() {
        // Ensures that a node with value `12` doesn't wrongly match `2`.
        let mut root = TreeNode::new(1);
        root.left = leaf(12);

        let sub_root = TreeNode::new(2);

        let root_box = Some(Box::new(root));
        let sub_root_box = Some(Box::new(sub_root));

        assert!(!is_subtree_brute_force(
            root_box.clone(),
            sub_root_box.clone()
        ));
        assert!(!is_subtree_naive(root_box.clone(), sub_root_box.clone()));
        assert!(!is_subtree_optimal(root_box, sub_root_box));
    }
}
