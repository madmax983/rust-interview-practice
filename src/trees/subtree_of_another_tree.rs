//! # 572. Subtree of Another Tree
//!
//! Difficulty: Easy
//! Link: <https://leetcode.com/problems/subtree-of-another-tree/>
//!
//! Given the roots of two binary trees `root` and `subRoot`, return `true` if there is a subtree of `root`
//! with the same structure and node values of `subRoot` and `false` otherwise.
//!
//! This problem emphasizes `Rc<RefCell<TreeNode>>` manipulation (though `Option<Box<TreeNode>>` is
//! more common for LeetCode Rust, we'll stick to the standard `Box` as defined in the repo for consistency),
//! recursion, and how Rust's exhaustive pattern matching easily handles traversing complex nested structures.
//!
//! ## Approach

In GC languages like Java or Python, traversing and passing tree nodes relies on shared references. Rust's `Option<Box<T>>` enforces unique ownership, meaning recursive approaches often require passing references (`&Option<Box<T>>`) to avoid consuming the tree during the search.
//!
//! We present two approaches:
//! 1. **Straightforward Recursive Approach**: For every node in `root`, check if the subtree starting there
//!    is identical to `subRoot` using a helper `is_same_tree` function. Time: O(M * N) where M is nodes in root
//!    and N is nodes in subRoot. Space: O(max(H1, H2)) for the recursion stack.
//! 2. **Optimal String Serialization Approach**: Serialize both trees into strings using a pre-order traversal
//!    (with null markers to preserve structure). Then simply check if the serialized `subRoot` is a substring
//!    of the serialized `root`. Time: O(M + N), Space: O(M + N).
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

// Helper function to check if two trees are strictly identical.
fn is_same_tree(p: &Option<Box<TreeNode>>, q: &Option<Box<TreeNode>>) -> bool {
    // RUST INSIGHT: Exhaustive tuple matching on Option references is highly idiomatic
    // and eliminates deeply nested `if let Some` blocks.
    match (p, q) {
        (Some(node_p), Some(node_q)) => {
            node_p.val == node_q.val
                && is_same_tree(&node_p.left, &node_q.left)
                && is_same_tree(&node_p.right, &node_q.right)
        }
        (None, None) => true,
        _ => false,
    }
}

/// Straightforward Approach: Recursive Check
///
/// Time: O(M * N) - In the worst case (e.g., highly unbalanced or repetitive trees),
/// we might call `is_same_tree` (which takes O(N)) for every node in `root` (M nodes).
/// Space: O(max(H_m, H_n)) - For the recursion stack.
#[must_use]
pub fn is_subtree_straightforward(
    root: Option<Box<TreeNode>>,
    sub_root: Option<Box<TreeNode>>,
) -> bool {
    // Helper that takes references to avoid moving the sub_root out on every check
    fn helper(r: &Option<Box<TreeNode>>, s: &Option<Box<TreeNode>>) -> bool {
        if r.is_none() {
            return false;
        }

        if is_same_tree(r, s) {
            return true;
        }

        // Check left and right subtrees
        let node = r.as_ref().unwrap();
        helper(&node.left, s) || helper(&node.right, s)
    }

    helper(&root, &sub_root)
}

/// Optimal Approach: String Serialization
///
/// Time: O(M + N) - Serializing takes O(M) and O(N). Substring search is generally fast,
/// and can be O(M + N) with KMP, though Rust's `contains` uses a highly optimized Two-Way algorithm.
/// Space: O(M + N) - Allocating the serialization strings.
#[must_use]
pub fn is_subtree_optimal(
    root: Option<Box<TreeNode>>,
    sub_root: Option<Box<TreeNode>>,
) -> bool {
    // Serialize tree with pre-order traversal. Null nodes are denoted with " #"
    // GOTCHA: We must wrap values with delimiters (e.g., "^1$") to prevent false positives
    // like "12" matching "2" within a substring search.
    fn serialize(node: &Option<Box<TreeNode>>, out: &mut String) {
        match node {
            Some(n) => {
                out.push('^');
                out.push_str(&n.val.to_string());
                out.push('$');
                serialize(&n.left, out);
                serialize(&n.right, out);
            }
            None => out.push_str("#"),
        }
    }

    let mut root_str = String::with_capacity(2048);
    let mut sub_root_str = String::with_capacity(1024);

    serialize(&root, &mut root_str);
    serialize(&sub_root, &mut sub_root_str);

    root_str.contains(&sub_root_str)
}

/// Main entry point - uses optimal solution
#[must_use]
pub fn is_subtree(root: Option<Box<TreeNode>>, sub_root: Option<Box<TreeNode>>) -> bool {
    is_subtree_optimal(root, sub_root)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(val: i32) -> Option<Box<TreeNode>> {
        Some(Box::new(TreeNode::new(val)))
    }

    #[test]
    fn test_happy_path_exists() {
        let mut root = TreeNode::new(3);
        let mut left = TreeNode::new(4);
        left.left = leaf(1);
        left.right = leaf(2);
        root.left = Some(Box::new(left));
        root.right = leaf(5);

        let mut sub_root = TreeNode::new(4);
        sub_root.left = leaf(1);
        sub_root.right = leaf(2);

        assert!(is_subtree_straightforward(Some(Box::new(root.clone())), Some(Box::new(sub_root.clone()))));
        assert!(is_subtree_optimal(Some(Box::new(root)), Some(Box::new(sub_root))));
    }

    #[test]
    fn test_edge_cases_does_not_exist() {
        let mut root = TreeNode::new(3);
        let mut left = TreeNode::new(4);
        left.left = leaf(1);

        let mut two_node = TreeNode::new(2);
        two_node.left = leaf(0); // This makes it different

        left.right = Some(Box::new(two_node));
        root.left = Some(Box::new(left));
        root.right = leaf(5);

        let mut sub_root = TreeNode::new(4);
        sub_root.left = leaf(1);
        sub_root.right = leaf(2);

        assert!(!is_subtree_straightforward(Some(Box::new(root.clone())), Some(Box::new(sub_root.clone()))));
        assert!(!is_subtree_optimal(Some(Box::new(root)), Some(Box::new(sub_root))));
    }

    #[test]
    fn test_stress_boundary_case_same_tree() {
        let mut root = TreeNode::new(1);
        root.left = leaf(1);

        let mut sub_root = TreeNode::new(1);
        sub_root.left = leaf(1);

        assert!(is_subtree_straightforward(Some(Box::new(root.clone())), Some(Box::new(sub_root.clone()))));
        assert!(is_subtree_optimal(Some(Box::new(root)), Some(Box::new(sub_root))));
    }

    #[test]
    fn test_edge_case_nulls() {
        let root = Some(Box::new(TreeNode::new(1)));
        assert!(!is_subtree_straightforward(None, root.clone()));
        assert!(!is_subtree_optimal(None, root.clone()));
    }
}

// Alternative Approaches:
// 1. KMP Algorithm on Serialized String: Instead of relying on Rust's `contains` method,
//    one could implement the Knuth-Morris-Pratt string matching algorithm to ensure strictly
//    O(M + N) time complexity for the substring search. However, Rust's standard library
//    `str::contains` uses a highly optimized Two-Way string matching algorithm, which is effectively
//    O(M + N) in practice and much simpler to write.
