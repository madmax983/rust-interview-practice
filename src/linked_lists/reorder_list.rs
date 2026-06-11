//! # 143. Reorder List
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/reorder-list/>
//!
//! You are given the head of a singly linked-list. The list can be represented as:
//! L0 → L1 → … → Ln - 1 → Ln
//!
//! Reorder the list to be on the following form:
//! L0 → Ln → L1 → Ln - 1 → L2 → Ln - 2 → …
//!
//! You may not modify the values in the list's nodes. Only nodes themselves may be changed.
//!
//! This problem is an excellent exercise for mastering Rust's ownership model, specifically `Option<Box<ListNode>>`
//! manipulation, `Option::take()`, and borrowing rules during in-place list modification.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::linked_lists::reorder_list::{reorder_list, ListNode};
//!
//! let mut list = ListNode::from_vec(vec![1, 2, 3, 4]);
//! reorder_list(&mut list);
//! assert_eq!(list.unwrap().to_vec(), vec![1, 4, 2, 3]);
//! ```

use std::collections::VecDeque;

// Definition for singly-linked list.
#[derive(PartialEq, Eq, Clone, Debug)]
pub struct ListNode {
    pub val: i32,
    pub next: Option<Box<ListNode>>,
}

impl ListNode {
    #[inline]
    #[must_use]
    pub const fn new(val: i32) -> Self {
        Self { next: None, val }
    }

    /// Helper to create a list from a vector (useful for tests)
    #[must_use]
    pub fn from_vec(vec: Vec<i32>) -> Option<Box<ListNode>> {
        let mut current = None;
        for &val in vec.iter().rev() {
            let mut node = ListNode::new(val);
            node.next = current;
            current = Some(Box::new(node));
        }
        current
    }

    /// Helper to convert list to vector
    #[must_use]
    pub fn to_vec(&self) -> Vec<i32> {
        let mut vec = Vec::new();
        let mut current = self;
        vec.push(current.val);
        while let Some(node) = &current.next {
            current = node;
            vec.push(current.val);
        }
        vec
    }
}

/// Brute force approach: Using `VecDeque`
///
/// Time: O(N) - One pass to collect, one pass to rebuild.
/// Space: O(N) - Storing all nodes in a double-ended queue.
///
/// This approach sidesteps the complexity of in-place linked list manipulation by
/// moving ownership of all nodes into a `VecDeque`. We can then easily pop from the
/// front and back to reconstruct the list in the desired order.
pub fn reorder_list_brute_force(head: &mut Option<Box<ListNode>>) {
    if head.is_none() || head.as_ref().unwrap().next.is_none() {
        return;
    }

    let mut deque = VecDeque::new();
    // We take ownership of the list out of `head` temporarily
    let mut current = head.take();

    while let Some(mut node) = current {
        current = node.next.take();
        deque.push_back(node);
    }

    // RUST INSIGHT: We use a dummy head to simplify appending.
    let mut dummy = Box::new(ListNode::new(0));
    let mut tail = &mut dummy;

    let mut turn = true; // true = pop_front, false = pop_back
    while !deque.is_empty() {
        let node = if turn {
            deque.pop_front()
        } else {
            deque.pop_back()
        };

        if let Some(n) = node {
            tail.next = Some(n);
            tail = tail.next.as_mut().unwrap();
        }
        turn = !turn;
    }

    *head = dummy.next;
}

/// Optimal approach: Two-pass length-counting in-place
///
/// Time: O(N) - Multiple passes (count length, traverse to middle, reverse half, interleave), but overall linear.
/// Space: O(1) - Only a few pointers are used.
///
/// This approach splits the problem into three phases:
/// 1. Find the middle of the list.
/// 2. Reverse the second half.
/// 3. Interleave the first half and the reversed second half.
///
/// RUST INSIGHT: Safe Rust's strict aliasing rules make simultaneous slow/fast mutable pointer
/// traversal complex, as you cannot have two mutable references to the same list.
/// Thus, calculating the length first and advancing a single pointer is the idiomatic,
/// safe Rust approach.
#[allow(clippy::missing_panics_doc)]
pub fn reorder_list_optimal(head: &mut Option<Box<ListNode>>) {
    if head.is_none() || head.as_ref().unwrap().next.is_none() {
        return;
    }

    // Step 1: Find the length to locate the middle
    let mut len = 0;
    {
        let mut curr = head.as_ref();
        while let Some(node) = curr {
            len += 1;
            curr = node.next.as_ref();
        }
    }

    // Step 2: Traverse to the middle and split the list
    let mid = (len - 1) / 2;
    let mut curr_mut = head.as_mut().unwrap();
    for _ in 0..mid {
        curr_mut = curr_mut.next.as_mut().unwrap();
    }

    // Split the list into two parts. `second_half` takes ownership of the rest of the nodes.
    // GOTCHA: We must `take()` the next pointer to sever the list. If we just reassigned,
    // we would drop the rest of the list.
    let mut second_half = curr_mut.next.take();

    // Step 3: Reverse the second half
    let mut prev = None;
    let mut current = second_half;
    while let Some(mut node) = current {
        let next = node.next.take();
        node.next = prev;
        prev = Some(node);
        current = next;
    }
    second_half = prev;

    // Step 4: Interleave the two halves
    let mut first_half_curr = head.as_mut();
    let mut second_half_curr = second_half;

    while second_half_curr.is_some() {
        // RUST INSIGHT: `Option::take()` temporarily takes ownership to avoid multiple mutable borrows.
        let mut second_node = second_half_curr.take().unwrap();
        let second_next = second_node.next.take();

        if let Some(first_node) = first_half_curr {
            let first_next = first_node.next.take();

            // Interleave: first -> second -> first_next
            second_node.next = first_next;
            first_node.next = Some(second_node);

            // Advance pointers
            first_half_curr = first_node.next.as_mut().unwrap().next.as_mut();
            second_half_curr = second_next;
        } else {
            break;
        }
    }
}

/// Main entry point
pub fn reorder_list(head: &mut Option<Box<ListNode>>) {
    reorder_list_optimal(head);
}

// Alternative Approaches:
// 1. Recursive approach: It's possible but cumbersome to maintain the paired nodes
//    from front and back in recursion without extra state passing. It's rarely used
//    due to O(N) space and complexity in Rust.

#[cfg(test)]
mod tests {
    use super::*;

    // Happy path tests
    #[test]
    fn test_reorder_list_even() {
        let mut list = ListNode::from_vec(vec![1, 2, 3, 4]);
        reorder_list(&mut list);
        assert_eq!(list.unwrap().to_vec(), vec![1, 4, 2, 3]);
    }

    #[test]
    fn test_reorder_list_odd() {
        let mut list = ListNode::from_vec(vec![1, 2, 3, 4, 5]);
        reorder_list(&mut list);
        assert_eq!(list.unwrap().to_vec(), vec![1, 5, 2, 4, 3]);
    }

    // Edge case tests
    #[test]
    fn test_reorder_list_empty() {
        let mut list = None;
        reorder_list(&mut list);
        assert_eq!(list, None);
    }

    #[test]
    fn test_reorder_list_single() {
        let mut list = ListNode::from_vec(vec![1]);
        reorder_list(&mut list);
        assert_eq!(list.unwrap().to_vec(), vec![1]);
    }

    #[test]
    fn test_reorder_list_two() {
        let mut list = ListNode::from_vec(vec![1, 2]);
        reorder_list(&mut list);
        assert_eq!(list.unwrap().to_vec(), vec![1, 2]);
    }

    // Stress/boundary case test
    #[test]
    fn test_all_approaches_consistency() {
        let v = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

        let mut l1 = ListNode::from_vec(v.clone());
        let mut l2 = ListNode::from_vec(v.clone());

        reorder_list_brute_force(&mut l1);
        reorder_list_optimal(&mut l2);

        assert_eq!(l1.unwrap().to_vec(), l2.unwrap().to_vec());
    }
}
