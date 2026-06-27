//! # 143. Reorder List
//!
//! Difficulty: Medium
//! Link: https://leetcode.com/problems/reorder-list/
//!
//! Why this matters in Rust: This problem perfectly demonstrates Rust's strict aliasing rules.
//! In languages like C++ or Java, the standard approach uses a slow and fast pointer to find
//! the middle of the list while simultaneously modifying it. In Rust, holding a mutable reference
//! to traverse while holding another mutable reference to modify violates the single mutable
//! borrow rule. This leads us to explore alternative, idiomatic Rust approaches like a two-pass
//! length-counting method or utilizing `VecDeque` for a more straightforward (but space-heavy) solution.
//!
//! ## Approach
//!
//! The goal is to reorder a singly-linked list `L0 -> L1 -> ... -> Ln-1 -> Ln` into
//! `L0 -> Ln -> L1 -> Ln-1 -> L2 -> Ln-2 -> ...` in place.
//!
//! Two solutions are provided:
//!
//! 1. `reorder_list_deque` (Brute-force / Straightforward):
//!    - Takes ownership of all nodes, puts them into a `VecDeque`.
//!    - Pops from the front and back alternately to rebuild the list.
//!    - Very easy to reason about and implement, but requires O(N) extra space.
//!
//! 2. `reorder_list_optimal` (Optimal In-Place):
//!    - **Step 1: Count length.** Perform a first pass to count the number of nodes `N`.
//!    - **Step 2: Split the list.** Traverse to the `N/2`th node. Use `take()` to sever the list into two halves.
//!    - **Step 3: Reverse the second half.** Standard iterative linked list reversal.
//!    - **Step 4: Merge.** Interleave the nodes from the first half and the reversed second half.
//!
//! Time Complexity: O(N) for both approaches.
//! Space Complexity: O(N) for `reorder_list_deque`, O(1) for `reorder_list_optimal`.
//!
//! ## Alternative Approaches
//!
//! * Unsafe Rust could bypass the borrow checker to implement the classic slow/fast pointer
//!   approach directly, but idiomatic safe Rust prefers the two-pass approach or standard
//!   collections, ensuring memory safety guarantees are maintained.

use std::collections::VecDeque;

// Definition for singly-linked list.
#[derive(PartialEq, Eq, Clone, Debug)]
pub struct ListNode {
    pub val: i32,
    pub next: Option<Box<ListNode>>,
}

impl ListNode {
    #[inline]
    fn new(val: i32) -> Self {
        ListNode { next: None, val }
    }
}

/// Brute-force O(N) space approach using VecDeque
pub fn reorder_list_deque(head: &mut Option<Box<ListNode>>) {
    if head.is_none() || head.as_ref().unwrap().next.is_none() {
        return;
    }

    // Extract all nodes into a double-ended queue
    let mut deque = VecDeque::new();
    let mut current = head.take();
    while let Some(mut node) = current {
        current = node.next.take();
        deque.push_back(node);
    }

    // Rebuild the list
    let mut dummy = ListNode::new(0);
    let mut tail = &mut dummy;

    let mut turn_front = true;
    while !deque.is_empty() {
        let node = if turn_front {
            deque.pop_front()
        } else {
            deque.pop_back()
        };

        tail.next = node;
        tail = tail.next.as_mut().unwrap();
        turn_front = !turn_front;
    }

    *head = dummy.next;
}

/// Optimal O(1) auxiliary space approach using two passes
pub fn reorder_list_optimal(head: &mut Option<Box<ListNode>>) {
    if head.is_none() || head.as_ref().unwrap().next.is_none() {
        return;
    }

    // Step 1: Count length
    let mut len = 0;
    let mut curr = head.as_ref();
    while let Some(node) = curr {
        len += 1;
        curr = node.next.as_ref();
    }

    // Step 2: Traverse to the middle and split the list
    let mid = len / 2 + len % 2;
    let mut curr_mut = head.as_mut();
    for _ in 1..mid {
        if let Some(node) = curr_mut {
            curr_mut = node.next.as_mut();
        }
    }

    // RUST INSIGHT: `node.next.take()` is crucial here. It extracts the `Option` value,
    // leaving `None` in its place, effectively severing the list safely.
    let mut second_half = if let Some(node) = curr_mut {
        node.next.take()
    } else {
        None
    };

    // Step 3: Reverse the second half
    let mut reversed_second = None;
    while let Some(mut node) = second_half {
        second_half = node.next.take();
        node.next = reversed_second;
        reversed_second = Some(node);
    }

    // Step 4: Merge the two halves
    let mut first_half = head.take();
    let mut dummy = ListNode::new(0);
    let mut tail = &mut dummy;

    // RUST INSIGHT: Using `take()` again to safely move ownership node by node
    // without conflicting mutable borrows.
    while first_half.is_some() {
        if let Some(mut first_node) = first_half {
            first_half = first_node.next.take();
            tail.next = Some(first_node);
            tail = tail.next.as_mut().unwrap();
        }

        if let Some(mut second_node) = reversed_second {
            reversed_second = second_node.next.take();
            tail.next = Some(second_node);
            tail = tail.next.as_mut().unwrap();
        }
    }

    *head = dummy.next;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn to_list(vec: Vec<i32>) -> Option<Box<ListNode>> {
        let mut head = None;
        for &val in vec.iter().rev() {
            let mut node = Box::new(ListNode::new(val));
            node.next = head;
            head = Some(node);
        }
        head
    }

    fn to_vec(head: Option<Box<ListNode>>) -> Vec<i32> {
        let mut vec = Vec::new();
        let mut curr = head;
        while let Some(node) = curr {
            vec.push(node.val);
            curr = node.next;
        }
        vec
    }

    #[test]
    fn test_reorder_list_happy_path_even() {
        let mut list1 = to_list(vec![1, 2, 3, 4]);
        reorder_list_deque(&mut list1);
        assert_eq!(to_vec(list1), vec![1, 4, 2, 3]);

        let mut list2 = to_list(vec![1, 2, 3, 4]);
        reorder_list_optimal(&mut list2);
        assert_eq!(to_vec(list2), vec![1, 4, 2, 3]);
    }

    #[test]
    fn test_reorder_list_happy_path_odd() {
        let mut list1 = to_list(vec![1, 2, 3, 4, 5]);
        reorder_list_deque(&mut list1);
        assert_eq!(to_vec(list1), vec![1, 5, 2, 4, 3]);

        let mut list2 = to_list(vec![1, 2, 3, 4, 5]);
        reorder_list_optimal(&mut list2);
        assert_eq!(to_vec(list2), vec![1, 5, 2, 4, 3]);
    }

    #[test]
    fn test_reorder_list_edge_cases() {
        // Empty list
        let mut list1 = None;
        reorder_list_optimal(&mut list1);
        assert_eq!(to_vec(list1), vec![]);

        // Single element
        let mut list2 = to_list(vec![1]);
        reorder_list_optimal(&mut list2);
        assert_eq!(to_vec(list2), vec![1]);

        // Two elements
        let mut list3 = to_list(vec![1, 2]);
        reorder_list_optimal(&mut list3);
        assert_eq!(to_vec(list3), vec![1, 2]);
    }
}
