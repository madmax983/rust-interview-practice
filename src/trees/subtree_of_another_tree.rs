//! # 572. Subtree of Another Tree
//!
//! Difficulty: Easy
//! Link: <https://leetcode.com/problems/subtree-of-another-tree/>
//!
//! Given the roots of two binary trees `root` and `subRoot`, return `true` if there is a subtree of `root`
//! with the same structure and node values of `subRoot` and `false` otherwise.
//!
//! This problem highlights Rust's `Option<Box<TreeNode>>` and recursive match semantics. We explore two
//! approaches: a brute-force recursive traversal that leans heavily on pattern matching, and an optimized
//! tree serialization approach that transforms the tree search into a substring search.
//!
//! ## Approach
//!
//! ### Brute Force (Recursive)
//! 1. Create a helper function `is_same_tree` to compare two trees exactly (from LeetCode #100).
//! 2. In the main function, if `root` is None, return `false`.
//! 3. If `is_same_tree(root, subRoot)` is true, return `true`.
//! 4. Recursively check if `subRoot` is a subtree of `root.left` or `root.right`.
//! **Time**: O(M * N) where M is nodes in `root` and N is nodes in `subRoot`. We might check `is_same_tree` at every node.
//! **Space**: O(H) where H is the height of `root`, due to the call stack.
//!
//! ### Optimized (Serialization)
//! 1. Serialize both trees into strings, taking care to uniquely identify nulls (e.g., `#`) and wrap values (e.g., `,val,`)
//!    so `12` doesn't match `2` in `12`. Preorder traversal is typically used.
//! 2. Check if the serialized `subRoot` is a substring of the serialized `root`.
//! **Time**: O(M + N) - serialization takes linear time, and finding a substring can be O(M + N) (though Rust's `contains` uses an optimized two-way string matching algorithm).
//! **Space**: O(M + N) to store the serialized strings.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::trees::subtree_of_another_tree::{is_subtree, TreeNode};
//!
//! // Create trees (simplified for example)
//! // root: [3,4,5,1,2]
//! // subRoot: [4,1,2]
//! // Result: true
//! ```

use std::cell::RefCell;
use std::rc::Rc;

// Definition for a binary tree node.
#[derive(Debug, PartialEq, Eq)]
pub struct TreeNode {
    pub val: i32,
    pub left: Option<Rc<RefCell<TreeNode>>>,
    pub right: Option<Rc<RefCell<TreeNode>>>,
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

type Node = Option<Rc<RefCell<TreeNode>>>;

/// Helper function to check if two trees are strictly identical
fn is_same_tree(p: &Node, q: &Node) -> bool {
    // RUST INSIGHT: We match on a tuple of references to options.
    // This concisely handles all 4 possible combinations of (Some, None)
    match (p, q) {
        (None, None) => true,
        (Some(p_node), Some(q_node)) => {
            let p_ref = p_node.borrow();
            let q_ref = q_node.borrow();
            p_ref.val == q_ref.val
                && is_same_tree(&p_ref.left, &q_ref.left)
                && is_same_tree(&p_ref.right, &q_ref.right)
        }
        _ => false,
    }
}

/// Brute force approach: Recursively check every node
/// Time: O(M * N)
/// Space: O(max(M, N)) for call stack
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_brute_force(root: Node, sub_root: Node) -> bool {
    if sub_root.is_none() {
        return true;
    }
    if root.is_none() {
        return false;
    }

    if is_same_tree(&root, &sub_root) {
        return true;
    }

    // GOTCHA: We need to borrow the root to access its children, but we only need it briefly
    // so we can extract clones of the Rc pointers for the recursive calls.
    let (left, right) = {
        let node = root.as_ref().unwrap().borrow();
        (node.left.clone(), node.right.clone())
    };

    is_subtree_brute_force(left, sub_root.clone()) || is_subtree_brute_force(right, sub_root)
}

/// Helper function to serialize a tree into a specific string format
fn serialize(node: &Node, out: &mut String) {
    match node {
        None => out.push_str("#,"),
        Some(n) => {
            let n_ref = n.borrow();
            // Wrap in commas to prevent matching "2" in "12"
            out.push(',');
            out.push_str(&n_ref.val.to_string());
            out.push(',');
            serialize(&n_ref.left, out);
            serialize(&n_ref.right, out);
        }
    }
}

/// Optimized approach: Serialize to strings and check substring
/// Time: O(M + N)
/// Space: O(M + N)
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_subtree_optimal(root: Node, sub_root: Node) -> bool {
    let mut root_str = String::new();
    let mut sub_root_str = String::new();

    serialize(&root, &mut root_str);
    serialize(&sub_root, &mut sub_root_str);

    // RUST INSIGHT: The `contains` method on string slices uses the Two-Way search algorithm,
    // which has O(N) worst-case time complexity, avoiding the O(N*M) worst-case of naive substring search.
    root_str.contains(&sub_root_str)
}

/// Main entry point - uses optimal solution
#[must_use]
pub fn is_subtree(root: Node, sub_root: Node) -> bool {
    is_subtree_optimal(root, sub_root)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to build a tree from a nested definition for testing
    // None is represented by -1 for simplicity in these basic tests
    fn build_tree(nodes: &[i32]) -> Node {
        if nodes.is_empty() || nodes[0] == -1 {
            return None;
        }

        let root = Rc::new(RefCell::new(TreeNode::new(nodes[0])));
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(root.clone());

        let mut i = 1;
        while i < nodes.len() {
            let curr = queue.pop_front().unwrap();

            if nodes[i] != -1 {
                let left = Rc::new(RefCell::new(TreeNode::new(nodes[i])));
                curr.borrow_mut().left = Some(left.clone());
                queue.push_back(left);
            }
            i += 1;

            if i < nodes.len() && nodes[i] != -1 {
                let right = Rc::new(RefCell::new(TreeNode::new(nodes[i])));
                curr.borrow_mut().right = Some(right.clone());
                queue.push_back(right);
            }
            i += 1;
        }

        Some(root)
    }

    #[test]
    fn test_brute_force() {
        let root = build_tree(&[3, 4, 5, 1, 2]);
        let sub_root = build_tree(&[4, 1, 2]);
        assert!(is_subtree_brute_force(root, sub_root));

        let root2 = build_tree(&[3, 4, 5, 1, 2, -1, -1, -1, -1, 0]);
        let sub_root2 = build_tree(&[4, 1, 2]);
        assert!(!is_subtree_brute_force(root2, sub_root2));
    }

    #[test]
    fn test_optimal() {
        let root = build_tree(&[3, 4, 5, 1, 2]);
        let sub_root = build_tree(&[4, 1, 2]);
        assert!(is_subtree_optimal(root, sub_root));

        let root2 = build_tree(&[3, 4, 5, 1, 2, -1, -1, -1, -1, 0]);
        let sub_root2 = build_tree(&[4, 1, 2]);
        assert!(!is_subtree_optimal(root2, sub_root2));
    }

    #[test]
    fn test_edge_cases() {
        // Subtree matches root exactly
        let root = build_tree(&[1, 2, 3]);
        let sub_root = build_tree(&[1, 2, 3]);
        assert!(is_subtree(root, sub_root));

        // Substring mismatch trap
        // e.g., root has value 12, subRoot has value 2
        let root2 = build_tree(&[12]);
        let sub_root2 = build_tree(&[2]);
        assert!(!is_subtree(root2, sub_root2));

        // Empty subRoot (always true)
        let root3 = build_tree(&[1]);
        assert!(is_subtree(root3, None));
    }
}
