//! # 236. Lowest Common Ancestor of a Binary Tree
//!
//! Link: <https://leetcode.com/problems/lowest-common-ancestor-of-a-binary-tree/>
//!
//! Given a binary tree, find the lowest common ancestor (LCA) of two given nodes in the tree.
//!
//! According to the definition of LCA on Wikipedia: “The lowest common ancestor is defined between two nodes p and q as the lowest node in T that has both p and q as descendants (where we allow a node to be a descendant of itself).”
//!
//! This problem is crucial for understanding tree traversals and the difference between:
//! 1.  **Top-down** (pre-order) vs **Bottom-up** (post-order) thinking.
//! 2.  **Shared Ownership** (`Rc<RefCell>`) in recursive structures.
//! 3.  **Path finding** vs **State propagation**.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::trees::lowest_common_ancestor::{lowest_common_ancestor, TreeNode};
//! use std::rc::Rc;
//! use std::cell::RefCell;
//!
//! //       3
//! //      / \
//! //     5   1
//! //    / \
//! let root = TreeNode::new(3);
//! let n5 = TreeNode::new(5);
//! let n1 = TreeNode::new(1);
//! let n6 = TreeNode::new(6);
//! let n2 = TreeNode::new(2);
//!
//! // Connect manually for Rc structure
//! root.borrow_mut().left = Some(Rc::clone(&n5));
//! root.borrow_mut().right = Some(Rc::clone(&n1));
//! n5.borrow_mut().left = Some(Rc::clone(&n6));
//! n5.borrow_mut().right = Some(Rc::clone(&n2));
//!
//! let lca = lowest_common_ancestor(Some(Rc::clone(&root)), Some(Rc::clone(&n5)), Some(Rc::clone(&n1)));
//! assert_eq!(lca.unwrap().borrow().val, 3);
//! ```
//!
//! ## Constraints
//!
//! - The number of nodes in the tree is in the range `[2, 10^5]`.
//! - `-10^9 <= Node.val <= 10^9`
//! - All `Node.val` are unique.
//! - `p != q`
//! - `p` and `q` will exist in the tree.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

// =========================================================================================
// Data Structures
// =========================================================================================

/// Definition for a binary tree node.
///
/// Unlike other tree problems in this repo that use `Box<TreeNode>`, LCA requires `Rc<RefCell<TreeNode>>`
/// because we need to pass shared references to `p` and `q`, and traverse/compare them.
#[derive(Debug, PartialEq, Eq)]
pub struct TreeNode {
    pub val: i32,
    pub left: Option<Rc<RefCell<Self>>>,
    pub right: Option<Rc<RefCell<Self>>>,
}

impl TreeNode {
    #[inline]
    #[must_use]
    pub fn new(val: i32) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            val,
            left: None,
            right: None,
        }))
    }
}

// =========================================================================================
// Brute Force Approach
// =========================================================================================

/// Brute Force: Path Tracing with Parents Map.
///
/// We traverse the tree (BFS/DFS) to build a `parent` map (`child -> parent`).
/// Once we have parents for both `p` and `q`, we trace back from `p` to root, storing visited nodes.
/// Then we trace back from `q`; the first node we encounter that is already visited is the LCA.
///
/// Time: O(N) - We visit every node to build the parent map.
/// Space: O(N) - To store parent pointers and the recursion stack/queue.
///
/// # Rust Insight
/// Because `TreeNode` doesn't have a `parent` pointer, we must build this map externally.
/// Since we can't key a `HashMap` by `Rc<RefCell<TreeNode>>` easily (no Hash impl),
/// and `val`s are unique, we use `val` as the key.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn lowest_common_ancestor_brute_force(
    root: Option<Rc<RefCell<TreeNode>>>,
    p: Option<Rc<RefCell<TreeNode>>>,
    q: Option<Rc<RefCell<TreeNode>>>,
) -> Option<Rc<RefCell<TreeNode>>> {
    let root = root?;
    let p = p?;
    let q = q?;

    let p_val = p.borrow().val;
    let q_val = q.borrow().val;

    // Map: Child Value -> Parent Node
    let mut parent: HashMap<i32, Rc<RefCell<TreeNode>>> = HashMap::new();
    let mut stack = Vec::new();

    // We can stop early if we've found parents for both (or if they are root).
    // However, simplest is to just ensure we've visited both nodes.
    // A more robust check:
    let mut found_p = p_val == root.borrow().val;
    let mut found_q = q_val == root.borrow().val;

    stack.push(Rc::clone(&root));

    while !found_p || !found_q {
        if let Some(node) = stack.pop() {
            let node_ref = node.borrow();

            if let Some(left) = &node_ref.left {
                let left_val = left.borrow().val;
                parent.insert(left_val, Rc::clone(&node));
                if left_val == p_val {
                    found_p = true;
                }
                if left_val == q_val {
                    found_q = true;
                }
                stack.push(Rc::clone(left));
            }
            if let Some(right) = &node_ref.right {
                let right_val = right.borrow().val;
                parent.insert(right_val, Rc::clone(&node));
                if right_val == p_val {
                    found_p = true;
                }
                if right_val == q_val {
                    found_q = true;
                }
                stack.push(Rc::clone(right));
            }
        } else {
            break;
        }
    }

    // 2. Trace back from p to root
    let mut ancestors = HashSet::new();
    let mut curr = Some(Rc::clone(&p));
    while let Some(node) = curr {
        ancestors.insert(node.borrow().val);
        // Move up
        curr = parent.get(&node.borrow().val).cloned();
    }

    // 3. Trace back from q to find first match
    let mut curr = Some(Rc::clone(&q));
    while let Some(node) = curr {
        if ancestors.contains(&node.borrow().val) {
            return Some(node);
        }
        curr = parent.get(&node.borrow().val).cloned();
    }

    None
}

// =========================================================================================
// Optimized Approach
// =========================================================================================

/// Optimized: Recursive Post-Order Traversal.
///
/// This is the standard, elegant solution. We traverse depth-first.
/// - If we encounter `p` or `q`, we return it up.
/// - If a node receives non-null values from both left and right, it means `p` and `q` are
///   in different subtrees, so the current node is the LCA.
/// - If a node receives a non-null value from only one side, it propagates that value up.
///
/// Time: O(N) - Worst case visits every node.
/// Space: O(H) - Recursion stack depth.
///
/// # Rust Insight
/// We use `Rc::ptr_eq` to compare node identities efficiently without needing `PartialEq`
/// implementation on the whole tree structure or unique IDs (though we have IDs here).
///
/// # Panics
/// Panics if `p` or `q` is `None`. The `LeetCode` contract guarantees both nodes exist in the tree.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn lowest_common_ancestor_optimized(
    root: Option<Rc<RefCell<TreeNode>>>,
    p: Option<Rc<RefCell<TreeNode>>>,
    q: Option<Rc<RefCell<TreeNode>>>,
) -> Option<Rc<RefCell<TreeNode>>> {
    // Base case: root is None, or root is p, or root is q
    let node = root?;

    // We can't easily move p and q into recursive calls if we need them for comparison.
    // So we pass references or clones. Since they are Rc, cloning is cheap.
    let p_unwrapped = p.as_ref().unwrap();
    let q_unwrapped = q.as_ref().unwrap();

    if Rc::ptr_eq(&node, p_unwrapped) || Rc::ptr_eq(&node, q_unwrapped) {
        return Some(node);
    }

    let left = lowest_common_ancestor_optimized(node.borrow().left.clone(), p.clone(), q.clone());
    let right = lowest_common_ancestor_optimized(node.borrow().right.clone(), p.clone(), q.clone());

    if left.is_some() && right.is_some() {
        return Some(node);
    }

    left.or(right)
}

// =========================================================================================
// Optimal Approach
// =========================================================================================

/// Optimal: Iterative Post-Order Traversal.
///
/// This approach simulates the system recursion stack using a `Vec` to avoid stack overflow
/// on extremely deep trees. It implements the exact same logic as the recursive solution
/// but explicitly manages state.
///
/// Time: O(N)
/// Space: O(H)
///
/// # Rust Insight
/// Explicit state machines are a common pattern in Rust to turn recursive algorithms
/// into iterative ones, ensuring "Zero Cost Abstractions" don't turn into "Hidden Stack Costs".
///
/// # Panics
/// Panics if `p` or `q` is `None`. The `LeetCode` contract guarantees both nodes exist in the tree.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn lowest_common_ancestor_optimal(
    root: Option<Rc<RefCell<TreeNode>>>,
    p: Option<Rc<RefCell<TreeNode>>>,
    q: Option<Rc<RefCell<TreeNode>>>,
) -> Option<Rc<RefCell<TreeNode>>> {
    // Stack items: (Node, State, LeftResult).
    // Hoisted above the statements below so it exists from the start of the scope.
    enum State {
        VisitLeft,
        VisitRight,
        Process,
    }

    let root = root?;
    let p = p.unwrap();
    let q = q.unwrap();

    // Stack to simulate recursion
    let mut stack = Vec::new();
    stack.push((Rc::clone(&root), State::VisitLeft, None));

    // Variable to hold the result of the "last returned" function call
    let mut last_result: Option<Rc<RefCell<TreeNode>>> = None;

    while let Some((node, state, left_res)) = stack.pop() {
        match state {
            State::VisitLeft => {
                // Pre-check: if this node is p or q, we return it immediately (Base case)
                if Rc::ptr_eq(&node, &p) || Rc::ptr_eq(&node, &q) {
                    last_result = Some(node);
                    continue;
                }

                // Push back current node with next state: VisitRight
                stack.push((Rc::clone(&node), State::VisitRight, None));

                // Push left child if exists
                if let Some(left) = &node.borrow().left {
                    stack.push((Rc::clone(left), State::VisitLeft, None));
                } else {
                    // Simulating return None from left
                    last_result = None;
                }
            }
            State::VisitRight => {
                // We just returned from left. last_result holds the left child's result.
                let stored_left_res = last_result.take();

                // Push back current node with next state: Process
                stack.push((Rc::clone(&node), State::Process, stored_left_res));

                // Push right child if exists
                if let Some(right) = &node.borrow().right {
                    stack.push((Rc::clone(right), State::VisitLeft, None));
                } else {
                    last_result = None;
                }
            }
            State::Process => {
                // We just returned from right. last_result holds the right child's result.
                let right_res = last_result.take();
                let stored_left_res = left_res;

                if stored_left_res.is_some() && right_res.is_some() {
                    last_result = Some(node);
                } else {
                    last_result = stored_left_res.or(right_res);
                }
            }
        }
    }

    last_result
}

/// Main entry point - uses optimal solution
#[must_use]
pub fn lowest_common_ancestor(
    root: Option<Rc<RefCell<TreeNode>>>,
    p: Option<Rc<RefCell<TreeNode>>>,
    q: Option<Rc<RefCell<TreeNode>>>,
) -> Option<Rc<RefCell<TreeNode>>> {
    lowest_common_ancestor_optimal(root, p, q)
}

#[cfg(test)]
mod tests {
    use super::*;

    type NodeRef = Rc<RefCell<TreeNode>>;

    // Helper to create a leaf node
    fn leaf(val: i32) -> NodeRef {
        TreeNode::new(val)
    }

    fn create_test_tree() -> (NodeRef, NodeRef, NodeRef) {
        //      3
        //     / \
        //    5   1
        //   / \ / \
        //  6  2 0  8
        //    / \
        //   7   4
        let root = TreeNode::new(3);
        let n5 = leaf(5);
        let n1 = leaf(1);
        let n6 = leaf(6);
        let n2 = leaf(2);
        let n0 = leaf(0);
        let n8 = leaf(8);
        let n7 = leaf(7);
        let n4 = leaf(4);

        // Connections
        n2.borrow_mut().left = Some(Rc::clone(&n7));
        n2.borrow_mut().right = Some(Rc::clone(&n4));

        n5.borrow_mut().left = Some(Rc::clone(&n6));
        n5.borrow_mut().right = Some(Rc::clone(&n2));

        n1.borrow_mut().left = Some(Rc::clone(&n0));
        n1.borrow_mut().right = Some(Rc::clone(&n8));

        root.borrow_mut().left = Some(Rc::clone(&n5));
        root.borrow_mut().right = Some(Rc::clone(&n1));

        (root, n5, n1)
    }

    #[test]
    fn test_brute_force_simple() {
        //      3
        //     / \
        //    5   1
        let root = TreeNode::new(3);
        let n5 = leaf(5);
        let n1 = leaf(1);

        root.borrow_mut().left = Some(Rc::clone(&n5));
        root.borrow_mut().right = Some(Rc::clone(&n1));

        let lca = lowest_common_ancestor_brute_force(
            Some(Rc::clone(&root)),
            Some(Rc::clone(&n5)),
            Some(Rc::clone(&n1)),
        );
        assert_eq!(lca.unwrap().borrow().val, 3);
    }

    #[test]
    fn test_all_approaches_lca_is_root() {
        let (root, n5, n1) = create_test_tree();

        // LCA of 5 and 1 is 3 (root)
        let lca_bf = lowest_common_ancestor_brute_force(
            Some(Rc::clone(&root)),
            Some(Rc::clone(&n5)),
            Some(Rc::clone(&n1)),
        );
        let lca_opt = lowest_common_ancestor_optimized(
            Some(Rc::clone(&root)),
            Some(Rc::clone(&n5)),
            Some(Rc::clone(&n1)),
        );
        let lca_iter = lowest_common_ancestor_optimal(
            Some(Rc::clone(&root)),
            Some(Rc::clone(&n5)),
            Some(Rc::clone(&n1)),
        );

        assert_eq!(lca_bf.as_ref().unwrap().borrow().val, 3);
        assert_eq!(lca_opt.as_ref().unwrap().borrow().val, 3);
        assert_eq!(lca_iter.as_ref().unwrap().borrow().val, 3);
    }

    #[test]
    fn test_all_approaches_lca_is_intermediate() {
        let (root, n5, _n1) = create_test_tree();
        // LCA of 5 and 4 is 5
        // We need to fetch n4 reference.
        // Traversing to get it:
        let n4 = root
            .borrow()
            .left
            .as_ref()
            .unwrap()
            .borrow()
            .right
            .as_ref()
            .unwrap()
            .borrow()
            .right
            .as_ref()
            .unwrap()
            .clone();

        let lca_bf = lowest_common_ancestor_brute_force(
            Some(Rc::clone(&root)),
            Some(Rc::clone(&n5)),
            Some(Rc::clone(&n4)),
        );
        let lca_opt = lowest_common_ancestor_optimized(
            Some(Rc::clone(&root)),
            Some(Rc::clone(&n5)),
            Some(Rc::clone(&n4)),
        );
        let lca_iter = lowest_common_ancestor_optimal(
            Some(Rc::clone(&root)),
            Some(Rc::clone(&n5)),
            Some(Rc::clone(&n4)),
        );

        assert_eq!(lca_bf.as_ref().unwrap().borrow().val, 5);
        assert_eq!(lca_opt.as_ref().unwrap().borrow().val, 5);
        assert_eq!(lca_iter.as_ref().unwrap().borrow().val, 5);
    }

    #[test]
    fn test_one_is_descendant_of_other() {
        let (root, n5, _n1) = create_test_tree();
        // LCA of 5 and 6 is 5.
        let n6 = root
            .borrow()
            .left
            .as_ref()
            .unwrap()
            .borrow()
            .left
            .as_ref()
            .unwrap()
            .clone();

        let lca_bf = lowest_common_ancestor_brute_force(
            Some(Rc::clone(&root)),
            Some(Rc::clone(&n5)),
            Some(Rc::clone(&n6)),
        );
        let lca_opt = lowest_common_ancestor_optimized(
            Some(Rc::clone(&root)),
            Some(Rc::clone(&n5)),
            Some(Rc::clone(&n6)),
        );
        let lca_iter = lowest_common_ancestor_optimal(
            Some(Rc::clone(&root)),
            Some(Rc::clone(&n5)),
            Some(Rc::clone(&n6)),
        );

        assert_eq!(lca_bf.as_ref().unwrap().borrow().val, 5);
        assert_eq!(lca_opt.as_ref().unwrap().borrow().val, 5);
        assert_eq!(lca_iter.as_ref().unwrap().borrow().val, 5);
    }
}
