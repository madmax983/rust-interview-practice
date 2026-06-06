//! # 143. Reorder List
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/reorder-list/>
//!
//! You are given the head of a singly linked-list. The list can be represented as:
//! `L0 → L1 → … → Ln - 1 → Ln`
//!
//! Reorder the list to be on the following form:
//! `L0 → Ln → L1 → Ln - 1 → L2 → Ln - 2 → …`
//!
//! You may not modify the values in the list's nodes. Only nodes themselves may be changed.
//!
//! This problem is a masterclass in Rust's ownership model and `Option<Box<ListNode>>`
//! manipulation. It forces you to safely borrow, extract (`Option::take`), and rebuild
//! linked structures without violating aliasing rules, demonstrating why Rust's strict
//! compiler guarantees prevent dangling pointers during complex list surgeries.
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
//!
//! ## Constraints
//!
//! - The number of nodes in the list is in the range `[1, 5 * 10^4]`.
//! - `1 <= Node.val <= 1000`

use std::collections::VecDeque;

// Definition for singly-linked list.
#[derive(PartialEq, Eq, Clone, Debug)]
pub struct ListNode {
    pub val: i32,
    pub next: Option<Box<Self>>,
}

impl ListNode {
    #[inline]
    #[must_use]
    pub const fn new(val: i32) -> Self {
        Self { next: None, val }
    }

    /// Helper to create a list from a vector
    #[must_use]
    #[allow(clippy::needless_pass_by_value)]
    pub fn from_vec(vec: Vec<i32>) -> Option<Box<Self>> {
        let mut current = None;
        for &val in vec.iter().rev() {
            let mut node = Self::new(val);
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

/// Brute force approach: Collect to VecDeque and Rebuild
/// Time: O(N) - Two passes (one to collect, one to rebuild)
/// Space: O(N) - Storing all elements in a VecDeque
///
/// This approach simplifies the problem by putting all nodes into a `VecDeque`.
/// We then rebuild the list by taking nodes alternately from the front and back.
pub fn reorder_list_brute_force(head: &mut Option<Box<ListNode>>) {
    if head.is_none() || head.as_ref().unwrap().next.is_none() {
        return;
    }

    let mut deque = VecDeque::new();

    // GOTCHA: We must take the list out of `head` to take ownership of the nodes.
    // If we just iterated with `&mut`, we couldn't easily re-link them.
    let mut current = head.take();
    while let Some(mut node) = current {
        current = node.next.take();
        deque.push_back(node);
    }

    let mut dummy = ListNode::new(0);
    let mut tail = &mut dummy;
    let mut take_from_front = true;

    while !deque.is_empty() {
        let node = if take_from_front {
            deque.pop_front().unwrap()
        } else {
            deque.pop_back().unwrap()
        };
        take_from_front = !take_from_front;

        tail.next = Some(node);
        tail = tail.next.as_deref_mut().unwrap();
    }

    *head = dummy.next;
}

/// Optimal approach: Find Middle, Reverse Second Half, Merge In-Place
/// Time: O(N) - Visit each node a constant number of times
/// Space: O(1) - Only a few pointers used, modifying the list in-place
///
/// This is the standard idiomatic solution without O(N) memory.
/// Step 1: Find the middle of the list (using two-pass length counting or slow/fast pointers).
/// Step 2: Split the list into two halves.
/// Step 3: Reverse the second half.
/// Step 4: Merge the two halves alternately.
pub fn reorder_list_optimal(head: &mut Option<Box<ListNode>>) {
    if head.is_none() || head.as_ref().unwrap().next.is_none() {
        return;
    }

    // Step 1: Count length to find the middle.
    // RUST INSIGHT: A slow/fast pointer approach is tricky in safe Rust with single mutable references.
    // Counting the length and advancing a pointer is much safer and easier to appease the borrow checker,
    // while remaining O(N) time and O(1) space.
    let mut len = 0;
    let mut current = head.as_ref();
    while let Some(node) = current {
        len += 1;
        current = node.next.as_ref();
    }

    // Advance to the middle node.
    let mid = len / 2;
    let mut current_mut = head.as_mut();
    for _ in 0..mid {
        if let Some(node) = current_mut {
            current_mut = node.next.as_mut();
        }
    }

    // Step 2: Split the list
    // RUST INSIGHT: `take()` replaces the `next` field with `None`, effectively severing the list,
    // and giving us owned access to the second half.
    let mut second_half = if let Some(node) = current_mut {
        node.next.take()
    } else {
        None
    };

    // Step 3: Reverse the second half
    let mut prev = None;
    let mut curr = second_half;
    while let Some(mut node) = curr {
        let next = node.next.take();
        node.next = prev;
        prev = Some(node);
        curr = next;
    }
    second_half = prev;

    // Step 4: Merge the two halves
    let mut first_curr = head.as_mut();
    let mut second_curr = second_half;

    while let (Some(first), Some(mut second)) = (first_curr.take(), second_curr.take()) {
        let first_next = first.next.take();
        let second_next = second.next.take();

        // first -> second
        second.next = first_next;
        let second_box = Some(second);
        first.next = second_box;

        // Advance pointers
        first_curr = first.next.as_mut().unwrap().next.as_mut();
        second_curr = second_next;
    }
}

/// Main entry point - uses optimal solution
pub fn reorder_list(head: &mut Option<Box<ListNode>>) {
    reorder_list_optimal(head);
}

// Alternative Approaches:
// 1. **Array/Vector of References**: Collect `&mut Box<ListNode>` into a `Vec`, then manipulate them.
//    This is extremely difficult in safe Rust due to aliasing rules. The `VecDeque` approach of taking
//    ownership is the "Rust way" to do the brute force method safely.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force_even() {
        let mut list = ListNode::from_vec(vec![1, 2, 3, 4]);
        reorder_list_brute_force(&mut list);
        assert_eq!(list.unwrap().to_vec(), vec![1, 4, 2, 3]);
    }

    #[test]
    fn test_brute_force_odd() {
        let mut list = ListNode::from_vec(vec![1, 2, 3, 4, 5]);
        reorder_list_brute_force(&mut list);
        assert_eq!(list.unwrap().to_vec(), vec![1, 5, 2, 4, 3]);
    }

    #[test]
    fn test_optimal_even() {
        let mut list = ListNode::from_vec(vec![1, 2, 3, 4]);
        reorder_list_optimal(&mut list);
        assert_eq!(list.unwrap().to_vec(), vec![1, 4, 2, 3]);
    }

    #[test]
    fn test_optimal_odd() {
        let mut list = ListNode::from_vec(vec![1, 2, 3, 4, 5]);
        reorder_list_optimal(&mut list);
        assert_eq!(list.unwrap().to_vec(), vec![1, 5, 2, 4, 3]);
    }

    #[test]
    fn test_single_node() {
        let mut list = ListNode::from_vec(vec![1]);
        reorder_list(&mut list);
        assert_eq!(list.unwrap().to_vec(), vec![1]);
    }

    #[test]
    fn test_two_nodes() {
        let mut list = ListNode::from_vec(vec![1, 2]);
        reorder_list(&mut list);
        assert_eq!(list.unwrap().to_vec(), vec![1, 2]);
    }

    #[test]
    fn test_empty_list() {
        let mut list = None;
        reorder_list(&mut list);
        assert_eq!(list, None);
    }
}
