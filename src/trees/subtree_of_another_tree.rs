//! # 572. Subtree of Another Tree
//!
//! Difficulty: Easy
//! Link: https://leetcode.com/problems/subtree-of-another-tree/
//!
//! Given the roots of two binary trees `root` and `subRoot`, return `true` if there is a subtree of `root`
//! with the same structure and node values of `subRoot` and `false` otherwise.
//!
//! A subtree of a binary tree `tree` is a tree that consists of a node in `tree` and all of this node's
//! descendants. The tree `tree` could also be considered as a subtree of itself.
//!
//! ## Approach
//! This problem is a natural fit for pattern matching with `Option<Box<TreeNode>>` and recursion,
//! providing an excellent opportunity to explore differences between recursive brute-force search and
//! optimal zero-allocation serialization tradeoffs in Rust. In languages like Java or Python, serialization
//! often involves building numerous intermediate strings on the heap, stressing the garbage collector. In Rust,
//! we can use `String::with_capacity` and the `write!` macro to achieve a highly performant, zero-allocation
//! (after initial buffer creation) O(N+M) serialization string.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::trees::subtree_of_another_tree::{is_subtree, TreeNode};
//!
//! let mut root = TreeNode::new(3);
//! let mut left = TreeNode::new(4);
//! left.left = Some(Box::new(TreeNode::new(1)));
//! left.right = Some(Box::new(TreeNode::new(2)));
//! root.left = Some(Box::new(left));
//! root.right = Some(Box::new(TreeNode::new(5)));
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

/// Helper function for the brute force approach to check if two trees are identical.
/// Takes `Option<&TreeNode>` to avoid consuming or moving the nodes.
fn is_same_tree(p: Option<&TreeNode>, q: Option<&TreeNode>) -> bool {
    match (p, q) {
        (None, None) => true,
        (Some(n1), Some(n2)) => {
            n1.val == n2.val
                && is_same_tree(n1.left.as_deref(), n2.left.as_deref())
                && is_same_tree(n1.right.as_deref(), n2.right.as_deref())
        }
        _ => false,
    }
}

/// Brute Force approach: Recursive Search
///
/// For each node in `root`, we treat it as a potential root of the `sub_root`
/// and do a full tree comparison using `is_same_tree`.
///
/// Time: O(M * N) - Where N is nodes in `root` and M is nodes in `sub_root`.
/// Space: O(H) - Max recursion depth, where H is the height of `root`.
///
/// # Rust Insight
/// We use `.as_deref()` to gracefully convert `&Option<Box<TreeNode>>` into
/// `Option<&TreeNode>`. This is crucial for traversing recursive data structures
/// without consuming them, avoiding unnecessary `.clone()` calls and satisfying
/// the borrow checker.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_brute_force(
    root: Option<Box<TreeNode>>,
    sub_root: Option<Box<TreeNode>>,
) -> bool {
    fn dfs(node: Option<&TreeNode>, sub: Option<&TreeNode>) -> bool {
        if node.is_none() {
            return false;
        }

        // Check if trees are identical starting from `node`.
        if is_same_tree(node, sub) {
            return true;
        }

        // Otherwise, recursively check left and right subtrees of `node`.
        let node_ref = node.unwrap();
        dfs(node_ref.left.as_deref(), sub) || dfs(node_ref.right.as_deref(), sub)
    }

    dfs(root.as_deref(), sub_root.as_deref())
}

/// Optimized approach: Naive String Serialization
///
/// We serialize both trees into strings and use `.contains()` to find a match.
///
/// Time: O(N + M) - O(N) to serialize root, O(M) to serialize sub_root, and O(N+M) for string search.
/// Space: O(N + M) - To store the serialized strings.
///
/// # Gotcha
/// We cannot just append `sub_root_str` to `root_str`. We need careful boundary markers
/// (e.g. `^` for null, `#val` for node values) so that we don't accidentally match
/// `12` inside `212`! Using proper node boundary markers is essential.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_optimized(
    root: Option<Box<TreeNode>>,
    sub_root: Option<Box<TreeNode>>,
) -> bool {
    fn serialize(node: Option<&TreeNode>) -> String {
        match node {
            None => "^".to_string(),
            Some(n) => format!(
                "#{}({})({})",
                n.val,
                serialize(n.left.as_deref()),
                serialize(n.right.as_deref())
            ),
        }
    }

    let root_str = serialize(root.as_deref());
    let sub_root_str = serialize(sub_root.as_deref());

    root_str.contains(&sub_root_str)
}

/// Optimal approach: Zero-Allocation Serialization
///
/// Instead of allocating a new `String` for every node using `format!`, we pre-allocate
/// a capacity and use `write!` to append iteratively to the existing buffer.
///
/// Time: O(N + M)
/// Space: O(N + M)
///
/// # Rust Insight
/// Direct `write!` to a `String` avoids hundreds of intermediate heap allocations
/// during serialization, giving a significant performance boost over the naive approach.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_optimal(
    root: Option<Box<TreeNode>>,
    sub_root: Option<Box<TreeNode>>,
) -> bool {
    fn serialize(node: Option<&TreeNode>, buf: &mut String) {
        match node {
            None => {
                buf.push('^');
            }
            Some(n) => {
                let _ = write!(buf, "#{}", n.val);
                buf.push('(');
                serialize(n.left.as_deref(), buf);
                buf.push_str(")(");
                serialize(n.right.as_deref(), buf);
                buf.push(')');
            }
        }
    }

    // 2000 nodes * ~10 chars per node = 20,000 bytes capacity
    let mut root_str = String::with_capacity(20000);
    serialize(root.as_deref(), &mut root_str);

    let mut sub_root_str = String::with_capacity(10000);
    serialize(sub_root.as_deref(), &mut sub_root_str);

    root_str.contains(&sub_root_str)
}

/// Main entry point - uses optimal solution
#[must_use]
pub fn is_subtree(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    is_subtree_optimal(root, sub_root)
}

/// Alternative Approaches
///
/// 1. KMP Algorithm (Knuth-Morris-Pratt) on tree traversals: You can do an in-order
///    and pre-order traversal sequence and perform KMP search, which would strictly
///    guarantee O(N+M) time complexity. The string search (`.contains()`) inside Rust
///    is highly optimized but technically could degrade on pathological patterns.
/// 2. Merkle Hashing: Hash each subtree recursively. Time O(N+M), very efficient comparison.

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create a leaf node
    fn leaf(val: i32) -> Option<Box<TreeNode>> {
        Some(Box::new(TreeNode::new(val)))
    }

    #[test]
    fn test_brute_force_happy() {
        let mut root = TreeNode::new(3);
        let mut left = TreeNode::new(4);
        left.left = leaf(1);
        left.right = leaf(2);
        root.left = Some(Box::new(left));
        root.right = leaf(5);

        let mut sub_root = TreeNode::new(4);
        sub_root.left = leaf(1);
        sub_root.right = leaf(2);

        assert!(is_subtree_brute_force(
            Some(Box::new(root)),
            Some(Box::new(sub_root))
        ));
    }

    #[test]
    fn test_brute_force_false() {
        let mut root = TreeNode::new(3);
        let mut left = TreeNode::new(4);
        left.left = leaf(1);
        let mut left_right = TreeNode::new(2);
        left_right.left = leaf(0);
        left.right = Some(Box::new(left_right));
        root.left = Some(Box::new(left));
        root.right = leaf(5);

        let mut sub_root = TreeNode::new(4);
        sub_root.left = leaf(1);
        sub_root.right = leaf(2);

        assert!(!is_subtree_brute_force(
            Some(Box::new(root)),
            Some(Box::new(sub_root))
        ));
    }

    #[test]
    fn test_optimized_happy() {
        let mut root = TreeNode::new(3);
        let mut left = TreeNode::new(4);
        left.left = leaf(1);
        left.right = leaf(2);
        root.left = Some(Box::new(left));
        root.right = leaf(5);

        let mut sub_root = TreeNode::new(4);
        sub_root.left = leaf(1);
        sub_root.right = leaf(2);

        assert!(is_subtree_optimized(
            Some(Box::new(root)),
            Some(Box::new(sub_root))
        ));
    }

    #[test]
    fn test_optimized_boundary_marker() {
        // Tree: 12 -> left: ^, right: ^
        let root = leaf(12);
        // SubRoot: 2 -> left: ^, right: ^
        let sub_root = leaf(2);

        assert!(!is_subtree_optimized(root, sub_root));
    }

    #[test]
    fn test_optimal_happy() {
        let mut root = TreeNode::new(3);
        let mut left = TreeNode::new(4);
        left.left = leaf(1);
        left.right = leaf(2);
        root.left = Some(Box::new(left));
        root.right = leaf(5);

        let mut sub_root = TreeNode::new(4);
        sub_root.left = leaf(1);
        sub_root.right = leaf(2);

        assert!(is_subtree_optimal(
            Some(Box::new(root)),
            Some(Box::new(sub_root))
        ));
    }

    #[test]
    fn test_cross_implementation() {
        let mut root = TreeNode::new(3);
        root.left = leaf(4);
        root.right = leaf(5);

        let sub_root = leaf(4);

        let r1 = is_subtree_brute_force(
            Some(Box::new(
                TreeNode {
                    val: 3,
                    left: leaf(4),
                    right: leaf(5),
                }
            )),
            leaf(4)
        );

        let r2 = is_subtree_optimized(
            Some(Box::new(
                TreeNode {
                    val: 3,
                    left: leaf(4),
                    right: leaf(5),
                }
            )),
            leaf(4)
        );

        let r3 = is_subtree_optimal(
            Some(Box::new(
                TreeNode {
                    val: 3,
                    left: leaf(4),
                    right: leaf(5),
                }
            )),
            leaf(4)
        );

        assert_eq!(r1, true);
        assert_eq!(r2, true);
        assert_eq!(r3, true);
    }
}
