//! # 141. Linked List Cycle
//!
//! Difficulty: Easy
//!
//! Link: <https://leetcode.com/problems/linked-list-cycle/>
//!
//! Given `head`, the head of a linked list, determine if the linked list has a cycle in it.
//!
//! There is a cycle in a linked list if there is some node in the list that can be reached again
//! by continuously following the `next` pointer.
//!
//! ## Why this matters in Rust
//!
//! This problem highlights a fundamental limitation (and safety feature) of safe Rust:
//! You cannot create a cyclic data structure using standard `Box<T>` single-ownership pointers.
//! In C++ or Java, a cycle is just a pointer pointing back to a previous node. In Rust, a `Box`
//! *owns* its contents, and a node cannot be owned by two different nodes at once.
//!
//! To model a cycle in safe Rust, you must use **Shared Ownership** (`Rc<T>`) combined with
//! **Interior Mutability** (`RefCell<T>`). Alternatively, and more idiomatically for performance,
//! you can use an **Arena / Index-based approach** where nodes live in a `Vec` and point to each
//! other via indices.
//!
//! This file demonstrates the `Rc<RefCell<Node>>` approach to show how Rust handles true shared
//! reference cycles, and then solves the algorithm using the classic Tortoise and Hare approach.
//!
//! ## Approaches
//!
//! ### Approach 1: `HashSet` (Visited Nodes)
//! We can traverse the list, putting the pointer (or memory address) of each node into a `HashSet`.
//! If we ever see a pointer we've already stored, there's a cycle.
//! - **Time Complexity:** O(N)
//! - **Space Complexity:** O(N) to store the pointers.
//!
//! ### Approach 2: Fast and Slow Pointers (Floyd's Cycle-Finding Algorithm) - Optimal
//! We use two pointers, `slow` and `fast`. `slow` moves one step at a time, `fast` moves two steps.
//! If there is a cycle, the `fast` pointer will eventually lap the `slow` pointer, and they will
//! point to the exact same node in memory. If `fast` reaches the end (`None`), there is no cycle.
//! - **Time Complexity:** O(N)
//! - **Space Complexity:** O(1)
//!
//! ## Rust Insight: Pointer Equality
//!
//! In Rust, to check if two `Rc` pointers point to the exact same allocation in memory, you use
//! `Rc::ptr_eq(&a, &b)`. Comparing them with `a == b` would instead check if their *inner values*
//! are equal, which is typically not what you want for cycle detection (and would infinite-loop
//! if a cycle existed and the inner values were checked!).
//!

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

// =========================================================================================
// Definition for singly-linked list with shared ownership and interior mutability.
//
// RUST INSIGHT: Notice how this differs from standard LeetCode Rust signatures.
// Standard LeetCode avoids cyclic inputs entirely by flattening or altering problem signatures.
// Here, we model it truthfully:
// - `Rc` allows multiple nodes to own a reference to the next node.
// - `RefCell` allows us to mutate the `next` pointer of a node after it has been created,
//   which is strictly necessary to form a cycle.
// =========================================================================================

#[derive(Debug)]
pub struct ListNode {
    pub val: i32,
    pub next: Option<Rc<RefCell<Self>>>,
}

impl ListNode {
    #[inline]
    #[must_use]
    pub const fn new(val: i32) -> Self {
        Self { val, next: None }
    }
}

// =========================================================================================
// Brute Force Approach: HashSet of Visited Nodes
// =========================================================================================

/// Brute force approach: track visited nodes in a `HashSet`.
///
/// We traverse the list, recording the memory address (`Rc::as_ptr`) of each node we visit.
/// If we ever encounter an address we've already seen, the list contains a cycle.
///
/// Time: O(N) - We visit each node at most once before detecting a repeat.
/// Space: O(N) - The `HashSet` stores up to N node addresses.
///
/// RUST INSIGHT: `Rc::as_ptr` gives us the raw pointer to the underlying allocation,
/// which uniquely identifies a node. We store `*const RefCell<ListNode>` (an address),
/// so the `HashSet` compares identities, never the (potentially cyclic) inner values.
#[must_use]
pub fn has_cycle_brute_force(head: Option<Rc<RefCell<ListNode>>>) -> bool {
    let mut visited: HashSet<*const RefCell<ListNode>> = HashSet::new();
    let mut current = head;

    while let Some(node) = current {
        // `insert` returns false if the address was already present -> cycle.
        if !visited.insert(Rc::as_ptr(&node)) {
            return true;
        }
        // Advance to the next node (cheap: bumps the refcount).
        current = node.borrow().next.as_ref().map(Rc::clone);
    }

    false
}

// =========================================================================================
// Optimal Approach: Fast and Slow Pointers (Floyd's Algorithm)
// =========================================================================================

/// Optimal approach: fast and slow pointers (Floyd's cycle-finding), O(1) memory.
///
/// # Panics
///
/// Does not panic in practice: the `unwrap` on `slow`'s `next` runs only after the
/// fast pointer has already proven a further node exists, so a successor is guaranteed.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
pub fn has_cycle_optimal(head: Option<Rc<RefCell<ListNode>>>) -> bool {
    // GOTCHA: We must handle the empty list case gracefully.
    let Some(ref head_node) = head else {
        return false;
    };

    // We clone the `Rc` pointers to create our slow and fast runners.
    // Cloning an `Rc` is cheap; it only increments the reference count.
    let mut slow = Rc::clone(head_node);

    // Attempt to advance fast pointer once initially to set up the loop
    let fast_initial = {
        let borrowed = head_node.borrow();
        if let Some(ref next_node) = borrowed.next {
            Rc::clone(next_node)
        } else {
            return false; // List has only one node and no cycle
        }
    };
    let mut fast = fast_initial;

    // Loop until we find a cycle or reach the end of the list
    loop {
        // RUST INSIGHT: We use `Rc::ptr_eq` to check if both Rc smart pointers
        // point to the EXACT same memory address (same underlying allocation).
        if Rc::ptr_eq(&slow, &fast) {
            return true;
        }

        // Advance slow by 1 step
        let next_slow = {
            let borrowed_slow = slow.borrow();
            // We know it has a next, otherwise fast would have hit None first
            Rc::clone(borrowed_slow.next.as_ref().unwrap())
        };
        slow = next_slow;

        // Advance fast by 2 steps
        let next_fast = {
            let borrowed_fast = fast.borrow();
            if let Some(ref first_step) = borrowed_fast.next {
                let borrowed_first_step = first_step.borrow();
                if let Some(ref second_step) = borrowed_first_step.next {
                    Rc::clone(second_step)
                } else {
                    return false; // Fast hit the end
                }
            } else {
                return false; // Fast hit the end
            }
        };
        fast = next_fast;
    }
}

/// Main entry point - uses the optimal (Floyd's) solution.
#[must_use]
pub fn has_cycle(head: Option<Rc<RefCell<ListNode>>>) -> bool {
    has_cycle_optimal(head)
}

// =========================================================================================
// Tests
// =========================================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to create a new node
    fn create_node(val: i32) -> Rc<RefCell<ListNode>> {
        Rc::new(RefCell::new(ListNode::new(val)))
    }

    #[test]
    fn test_happy_path_with_cycle() {
        // Create 3 -> 2 -> 0 -> -4
        let n1 = create_node(3);
        let n2 = create_node(2);
        let n3 = create_node(0);
        let n4 = create_node(-4);

        // Link nodes
        n1.borrow_mut().next = Some(Rc::clone(&n2));
        n2.borrow_mut().next = Some(Rc::clone(&n3));
        n3.borrow_mut().next = Some(Rc::clone(&n4));

        // Create cycle: -4 points back to 2
        n4.borrow_mut().next = Some(Rc::clone(&n2));

        assert!(has_cycle(Some(n1)));
    }

    #[test]
    fn test_happy_path_no_cycle() {
        // Create 1 -> 2 -> 3
        let n1 = create_node(1);
        let n2 = create_node(2);
        let n3 = create_node(3);

        // Link nodes
        n1.borrow_mut().next = Some(Rc::clone(&n2));
        n2.borrow_mut().next = Some(Rc::clone(&n3));
        // No cycle

        assert!(!has_cycle(Some(n1)));
    }

    #[test]
    fn test_edge_case_empty_list() {
        assert!(!has_cycle(None));
    }

    #[test]
    fn test_edge_case_single_node_no_cycle() {
        let n1 = create_node(1);
        assert!(!has_cycle(Some(n1)));
    }

    #[test]
    fn test_stress_boundary_single_node_cycle() {
        let n1 = create_node(1);
        // Node points to itself
        n1.borrow_mut().next = Some(Rc::clone(&n1));

        assert!(has_cycle(Some(n1)));
    }

    /// Builds a chain of nodes from `vals`. If `cycle_to` is `Some(i)`, the tail's
    /// `next` is linked back to node index `i` to form a cycle. Returns the head.
    fn build(vals: &[i32], cycle_to: Option<usize>) -> Option<Rc<RefCell<ListNode>>> {
        let nodes: Vec<_> = vals.iter().map(|&v| create_node(v)).collect();
        for i in 0..nodes.len().saturating_sub(1) {
            nodes[i].borrow_mut().next = Some(Rc::clone(&nodes[i + 1]));
        }
        if let (Some(target), Some(last)) = (cycle_to, nodes.last()) {
            last.borrow_mut().next = Some(Rc::clone(&nodes[target]));
        }
        nodes.into_iter().next()
    }

    #[test]
    fn test_brute_force_with_cycle() {
        let head = build(&[3, 2, 0, -4], Some(1));
        assert!(has_cycle_brute_force(head));
    }

    #[test]
    fn test_brute_force_no_cycle() {
        let head = build(&[1, 2, 3], None);
        assert!(!has_cycle_brute_force(head));
    }

    #[test]
    fn test_brute_force_edge_cases() {
        assert!(!has_cycle_brute_force(None));
        let single = create_node(1);
        assert!(!has_cycle_brute_force(Some(single)));

        let self_cycle = create_node(7);
        self_cycle.borrow_mut().next = Some(Rc::clone(&self_cycle));
        assert!(has_cycle_brute_force(Some(self_cycle)));
    }

    #[test]
    fn test_both_approaches_agree() {
        // Cyclic and acyclic inputs must yield identical answers from both impls.
        assert_eq!(
            has_cycle_brute_force(build(&[1, 2, 3, 4], Some(1))),
            has_cycle_optimal(build(&[1, 2, 3, 4], Some(1)))
        );
        assert_eq!(
            has_cycle_brute_force(build(&[1, 2, 3, 4], None)),
            has_cycle_optimal(build(&[1, 2, 3, 4], None))
        );
    }
}
