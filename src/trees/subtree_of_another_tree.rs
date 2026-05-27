//! # 572. Subtree of Another Tree
//!
//! Difficulty: Easy
//! Link: <https://leetcode.com/problems/subtree-of-another-tree/>
//!
//! Given the roots of two binary trees `root` and `subRoot`, return `true` if there is a subtree of `root`
//! with the same structure and node values of `subRoot` and `false` otherwise.
//!
//! This problem matters in Rust because it perfectly demonstrates pattern matching with `Option<Box<TreeNode>>`
//! and recursion to compare nested structures. It teaches how to manage borrowing rules when checking a node
//! while traversing the tree, and contrasts recursive brute-force with optimized string serialization.

use std::cell::RefCell;
use std::rc::Rc;

// Definition for a binary tree node.
#[derive(Debug, PartialEq, Eq, Clone)]
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

/// Brute Force approach: DFS with node-by-node matching
///
/// Time: O(M * N) - Where M is the number of nodes in `root` and N is the number of nodes in `subRoot`.
///                  In the worst case, we might need to check if the tree matches starting from every node.
/// Space: O(max(H_m, H_n)) - Where H_m and H_n are the heights of the trees, due to recursion stack.
///
/// The brute force approach recursively traverses `root`. At each node, we launch a secondary recursion
/// `is_same_tree` to see if the tree rooted there matches `subRoot`.
///
/// Why this is idiomatic Rust: We leverage structural pattern matching `match (p, q)` to concisely and
/// safely handle all combinations of `Some` and `None` branches without the risk of null pointer dereferences.
#[must_use]
pub fn is_subtree_brute_force(
    root: Option<Rc<RefCell<TreeNode>>>,
    sub_root: Option<Rc<RefCell<TreeNode>>>,
) -> bool {
    // Helper function to check if two trees are identical
    fn is_same_tree(p: &Option<Rc<RefCell<TreeNode>>>, q: &Option<Rc<RefCell<TreeNode>>>) -> bool {
        // RUST INSIGHT: Matching on a tuple allows us to exhaustively check all 4 states in a clean way.
        match (p, q) {
            (Some(n_p), Some(n_q)) => {
                let p_ref = n_p.borrow();
                let q_ref = n_q.borrow();
                p_ref.val == q_ref.val
                    && is_same_tree(&p_ref.left, &q_ref.left)
                    && is_same_tree(&p_ref.right, &q_ref.right)
            }
            (None, None) => true,
            _ => false,
        }
    }

    fn check_subtree(
        current: &Option<Rc<RefCell<TreeNode>>>,
        target: &Option<Rc<RefCell<TreeNode>>>,
    ) -> bool {
        // Check if the trees match starting at this node
        if is_same_tree(current, target) {
            return true;
        }

        if current.is_none() {
            return false;
        }

        // If not, try checking the left and right subtrees of `current`
        // GOTCHA: We must borrow the inner RefCell to access `.left` and `.right`, avoiding moving the Rc.
        let current_ref = current.as_ref().unwrap().borrow();
        check_subtree(&current_ref.left, target) || check_subtree(&current_ref.right, target)
    }

    // We only need to borrow during comparisons, so passing references keeps cloning to a minimum.
    check_subtree(&root, &sub_root)
}

/// Optimized approach: String Serialization (Pre-order)
///
/// Time: O(M + N) - We visit every node in both trees exactly once to build the string representations,
///                  and string matching (via `.contains()`) takes O(M + N) on average.
/// Space: O(M + N) - Allocating the serialized strings takes space proportional to the size of the trees.
///
/// We can convert the tree into a unique string representation using Pre-order traversal.
/// By wrapping values with special markers (e.g., `#val#`) and explicitly recording `Null` nodes,
/// we guarantee that identical structures produce identical strings. If `subRoot` is a subtree of `root`,
/// its serialized form will be a substring of `root`'s serialized form.
#[must_use]
pub fn is_subtree_optimized(
    root: Option<Rc<RefCell<TreeNode>>>,
    sub_root: Option<Rc<RefCell<TreeNode>>>,
) -> bool {
    // Helper to serialize the tree
    fn serialize(node: &Option<Rc<RefCell<TreeNode>>>, out: &mut String) {
        match node {
            Some(n) => {
                let node_ref = n.borrow();
                // RUST INSIGHT: Using `format!` with special boundary markers ensures that "12"
                // is not accidentally matched inside "212" or "123".
                out.push_str(&format!(" #{val}# ", val = node_ref.val));
                serialize(&node_ref.left, out);
                serialize(&node_ref.right, out);
            }
            None => {
                out.push_str(" null ");
            }
        }
    }

    let mut root_str = String::new();
    let mut sub_root_str = String::new();

    serialize(&root, &mut root_str);
    serialize(&sub_root, &mut sub_root_str);

    // Using Rust's built-in substring search
    root_str.contains(&sub_root_str)
}

/// Main entry point
#[must_use]
pub fn is_subtree(
    root: Option<Rc<RefCell<TreeNode>>>,
    sub_root: Option<Rc<RefCell<TreeNode>>>,
) -> bool {
    is_subtree_optimized(root, sub_root)
}

// Alternative Approaches:
// 1. **KMP String Matching**: While `contains` uses a fast algorithm under the hood, we could manually
//    implement the Knuth-Morris-Pratt (KMP) algorithm for O(M + N) worst-case guaranteed time.
// 2. **Merkle Hashing**: We could hash every subtree structure and compare hashes in O(1) time after an O(M + N) setup.
//    This requires maintaining hash values at each node but is highly memory efficient compared to string serialization.

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create a node easily
    fn node(val: i32) -> Option<Rc<RefCell<TreeNode>>> {
        Some(Rc::new(RefCell::new(TreeNode::new(val))))
    }

    #[test]
    fn test_happy_path_is_subtree() {
        // root: [3,4,5,1,2]
        //      3
        //     / \
        //    4   5
        //   / \
        //  1   2
        let root = node(3);
        let left = node(4);
        left.as_ref().unwrap().borrow_mut().left = node(1);
        left.as_ref().unwrap().borrow_mut().right = node(2);
        root.as_ref().unwrap().borrow_mut().left = left;
        root.as_ref().unwrap().borrow_mut().right = node(5);

        // subRoot: [4,1,2]
        //    4
        //   / \
        //  1   2
        let sub_root = node(4);
        sub_root.as_ref().unwrap().borrow_mut().left = node(1);
        sub_root.as_ref().unwrap().borrow_mut().right = node(2);

        assert!(is_subtree_brute_force(root.clone(), sub_root.clone()));
        assert!(is_subtree_optimized(root.clone(), sub_root.clone()));
    }

    #[test]
    fn test_edge_case_not_subtree_due_to_extra_child() {
        // root: [3,4,5,1,2,null,null,null,null,0]
        //      3
        //     / \
        //    4   5
        //   / \
        //  1   2
        //     /
        //    0
        let root = node(3);
        let left = node(4);
        left.as_ref().unwrap().borrow_mut().left = node(1);
        let right_of_left = node(2);
        right_of_left.as_ref().unwrap().borrow_mut().left = node(0);
        left.as_ref().unwrap().borrow_mut().right = right_of_left;
        root.as_ref().unwrap().borrow_mut().left = left;
        root.as_ref().unwrap().borrow_mut().right = node(5);

        // subRoot: [4,1,2]
        //    4
        //   / \
        //  1   2
        let sub_root = node(4);
        sub_root.as_ref().unwrap().borrow_mut().left = node(1);
        sub_root.as_ref().unwrap().borrow_mut().right = node(2);

        assert!(!is_subtree_brute_force(root.clone(), sub_root.clone()));
        assert!(!is_subtree_optimized(root.clone(), sub_root.clone()));
    }

    #[test]
    fn test_stress_boundary_identical_trees() {
        // Both trees are just one identical node.
        let root = node(1);
        let sub_root = node(1);
        assert!(is_subtree_brute_force(root.clone(), sub_root.clone()));
        assert!(is_subtree_optimized(root.clone(), sub_root.clone()));

        // Both trees are empty. (Problem says roots are given, but good to handle Option::None gracefully)
        assert!(is_subtree_brute_force(None, None));
        assert!(is_subtree_optimized(None, None));
    }
}
