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
//! ### Approach 1: HashSet (Visited Nodes)
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
    pub next: Option<Rc<RefCell<ListNode>>>,
}

impl ListNode {
    #[inline]
    pub fn new(val: i32) -> Self {
        ListNode { val, next: None }
    }
}

// =========================================================================================
// Optimal Approach: Fast and Slow Pointers (Floyd's Algorithm)
// =========================================================================================

/// Determines if the linked list has a cycle using O(1) memory.
pub fn has_cycle(head: Option<Rc<RefCell<ListNode>>>) -> bool {
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
}
