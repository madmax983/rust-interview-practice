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
//! This problem is a phenomenal showcase of Rust's ownership model. Modifying a linked list
//! in place requires mutable references, but Rust's strict aliasing rules prevent simultaneous
//! "slow/fast pointer" traversal with mutation. This artifact demonstrates how safe Rust handles
//! this via structural decomposition (`Option::take()`) and a two-pass length-counting approach.
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

/// Brute Force approach: VecDeque
///
/// Time: O(N) - We traverse the list to collect nodes, then process them.
/// Space: O(N) - We store all nodes in a VecDeque.
///
/// This avoids tricky pointer manipulation by taking ownership of all nodes into a
/// double-ended queue, then popping from the front and back to interleave them.
#[allow(clippy::needless_pass_by_value)]
pub fn reorder_list_brute_force(head: &mut Option<Box<ListNode>>) {
    // GOTCHA: We must `take()` the head so we own the list and satisfy the borrow checker.
    let mut current = head.take();
    if current.is_none() {
        return;
    }

    // Collect all nodes into a Deque
    let mut deque = VecDeque::new();
    while let Some(mut node) = current {
        current = node.next.take(); // Sever the connection
        deque.push_back(node);
    }

    // Rebuild the list by alternating front and back
    let mut dummy = ListNode::new(0);
    let mut tail = &mut dummy;
    let mut toggle = true;

    while !deque.is_empty() {
        let next_node = if toggle {
            deque.pop_front()
        } else {
            deque.pop_back()
        };
        tail.next = next_node;
        tail = tail.next.as_mut().unwrap();
        toggle = !toggle;
    }

    // Put the rebuilt list back into the original head
    *head = dummy.next;
}

/// Helper function to reverse a linked list
fn reverse_list(mut head: Option<Box<ListNode>>) -> Option<Box<ListNode>> {
    let mut prev = None;
    while let Some(mut current) = head {
        head = current.next.take();
        current.next = prev;
        prev = Some(current);
    }
    prev
}

/// Optimal approach: Two-pass Length-Counting In-Place
///
/// Time: O(N) - Pass 1 (count length), Pass 2 (split), Pass 3 (reverse), Pass 4 (merge). All O(N).
/// Space: O(1) - Only a few pointers are used; modifies the list completely in-place.
///
/// In C/C++, you find the middle using "slow/fast pointers". In safe Rust, doing that mutably
/// causes a "multiple mutable borrows" error. The idiomatic safe Rust solution is to:
/// 1. Count the length.
/// 2. Traverse halfway to split the list.
/// 3. Reverse the second half.
/// 4. Merge the two halves.
pub fn reorder_list_optimal(head: &mut Option<Box<ListNode>>) {
    if head.is_none() || head.as_ref().unwrap().next.is_none() {
        return;
    }

    // 1. Count the length of the list
    let mut len = 0;
    let mut current = head.as_ref();
    while let Some(node) = current {
        len += 1;
        current = node.next.as_ref();
    }

    // 2. Traverse to the middle node to split the list
    let mid = (len + 1) / 2;
    let mut current_mut = head.as_mut();
    for _ in 1..mid {
        if let Some(node) = current_mut {
            current_mut = node.next.as_mut();
        }
    }

    // RUST INSIGHT: To sever the list safely, we take() the `next` pointer of the midpoint node.
    let second_half = if let Some(node) = current_mut {
        node.next.take()
    } else {
        None
    };

    // 3. Reverse the second half
    let mut l2 = reverse_list(second_half);

    // 4. Merge the two halves
    let mut l1 = head.take();
    let mut dummy = ListNode::new(0);
    let mut tail = &mut dummy;

    // Zip l1 and l2 together
    while l1.is_some() || l2.is_some() {
        if let Some(mut node1) = l1 {
            l1 = node1.next.take();
            tail.next = Some(node1);
            tail = tail.next.as_mut().unwrap();
        }
        if let Some(mut node2) = l2 {
            l2 = node2.next.take();
            tail.next = Some(node2);
            tail = tail.next.as_mut().unwrap();
        }
    }

    *head = dummy.next;
}

/// Main entry point
pub fn reorder_list(head: &mut Option<Box<ListNode>>) {
    reorder_list_optimal(head);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force_even_length() {
        let mut list = ListNode::from_vec(vec![1, 2, 3, 4]);
        reorder_list_brute_force(&mut list);
        assert_eq!(list.unwrap().to_vec(), vec![1, 4, 2, 3]);
    }

    #[test]
    fn test_brute_force_odd_length() {
        let mut list = ListNode::from_vec(vec![1, 2, 3, 4, 5]);
        reorder_list_brute_force(&mut list);
        assert_eq!(list.unwrap().to_vec(), vec![1, 5, 2, 4, 3]);
    }

    #[test]
    fn test_optimal_even_length() {
        let mut list = ListNode::from_vec(vec![1, 2, 3, 4]);
        reorder_list_optimal(&mut list);
        assert_eq!(list.unwrap().to_vec(), vec![1, 4, 2, 3]);
    }

    #[test]
    fn test_optimal_odd_length() {
        let mut list = ListNode::from_vec(vec![1, 2, 3, 4, 5]);
        reorder_list_optimal(&mut list);
        assert_eq!(list.unwrap().to_vec(), vec![1, 5, 2, 4, 3]);
    }

    #[test]
    fn test_edge_case_empty() {
        let mut list = None;
        reorder_list(&mut list);
        assert_eq!(list, None);
    }

    #[test]
    fn test_edge_case_single_element() {
        let mut list = ListNode::from_vec(vec![1]);
        reorder_list(&mut list);
        assert_eq!(list.unwrap().to_vec(), vec![1]);
    }
}
