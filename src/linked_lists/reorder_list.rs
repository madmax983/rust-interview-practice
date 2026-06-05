//! # 143. Reorder List
//!
//! Difficulty: Medium
//! Link: https://leetcode.com/problems/reorder-list/
//!
//! You are given the head of a singly linked-list. The list can be represented as:
//!
//! `L0 → L1 → … → Ln - 1 → Ln`
//!
//! Reorder the list to be on the following form:
//!
//! `L0 → Ln → L1 → Ln - 1 → L2 → Ln - 2 → …`
//!
//! You may not modify the values in the list's nodes. Only nodes themselves may be changed.
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
//!
//! This problem matters in Rust because modifying a linked list in-place touches upon core ownership
//! and borrowing principles. Rust's borrow checker strictly prevents simultaneous mutable references
//! and forces us to be explicit when restructuring a graph-like data structure. It demonstrates
//! why `Option::take()` is critical for ownership transfer and how temporary detachment allows safe
//! mutable operations without violating aliasing rules.

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

/// Brute force approach: Array of Pointers / Nodes
///
/// Time: O(N) - Two passes over the list
/// Space: O(N) - Storing pointers/nodes in a VecDeque
///
/// In languages like Java or Python, you might just collect references to all nodes
/// in a list and then stitch them together. In safe Rust, we can't easily hold multiple
/// mutable references to different nodes in the list.
///
/// Instead, we extract ownership of all the nodes sequentially into a `VecDeque`,
/// which naturally supports popping from both ends. We then rebuild the list
/// from the deque, re-using the original nodes without modifying their values.
/// This respects the constraint of the problem by only modifying pointers, not values.
#[allow(clippy::needless_pass_by_value)]
pub fn reorder_list_brute_force(head: &mut Option<Box<ListNode>>) {
    if head.is_none() || head.as_ref().unwrap().next.is_none() {
        return;
    }

    // Step 1: Collect nodes into a VecDeque, severing their next pointers
    let mut deque = std::collections::VecDeque::new();
    let mut current = head.take();
    while let Some(mut node) = current {
        current = node.next.take();
        deque.push_back(node);
    }

    // Step 2: Rebuild the list from the deque by popping from both ends
    let mut dummy = Box::new(ListNode::new(0));
    let mut tail = &mut dummy;

    let mut toggle = true;
    while !deque.is_empty() {
        let node = if toggle {
            deque.pop_front().unwrap()
        } else {
            deque.pop_back().unwrap()
        };
        tail.next = Some(node);
        tail = tail.next.as_mut().unwrap();
        toggle = !toggle;
    }

    *head = dummy.next;
}

/// Optimal approach: Slow/Fast Pointers + Reversal + Merge
///
/// Time: O(N) - Finding mid, reversing, and merging each take linear time.
/// Space: O(1) - In-place manipulation with constant number of extra variables.
///
/// Algorithm:
/// 1. Find the middle of the linked list using a slow/fast pointer approach.
/// 2. Split the list into two halves.
/// 3. Reverse the second half in-place.
/// 4. Merge the two halves by alternating nodes.
///
/// In Python/Java/C++, we might use multiple pointers directly (`slow`, `fast`, `prev`).
/// In safe Rust, this implementation demonstrates how to use mutable references
/// and `Option::take()` to manage ownership carefully without triggering the borrow checker.
pub fn reorder_list_optimal(head: &mut Option<Box<ListNode>>) {
    if head.is_none() || head.as_ref().unwrap().next.is_none() {
        return;
    }

    // 1. Find the middle node using slow and fast pointers.
    // In Rust, using raw pointers makes this exactly like C++, but we want safe Rust.
    // We can't have `fast` run ahead while `slow` mutates the list.
    // So we first find the length, then advance a mutable pointer.
    let mut len = 0;
    let mut curr = head.as_ref();
    while let Some(node) = curr {
        len += 1;
        curr = node.next.as_ref();
    }

    let mid = (len - 1) / 2;
    let mut curr_mut = head.as_mut();
    for _ in 0..mid {
        if let Some(node) = curr_mut {
            curr_mut = node.next.as_mut();
        }
    }

    // 2. Sever the list and get the second half
    // RUST INSIGHT: We call `take()` on the *field* `next` of the node, not on the `Option` itself.
    // Calling `take()` on `curr_mut` directly would merely take ownership of the mutable reference,
    // not the underlying data.
    let second_half = if let Some(node) = curr_mut {
        node.next.take()
    } else {
        None
    };

    // 3. Reverse the second half
    let mut prev = None;
    let mut curr = second_half;
    while let Some(mut node) = curr {
        let next = node.next.take();
        node.next = prev;
        prev = Some(node);
        curr = next;
    }
    let mut second_half = prev;

    // 4. Merge the two halves
    let mut first_half = head.take();
    let mut dummy = Box::new(ListNode::new(0));
    let mut tail = &mut dummy;

    let mut toggle = true;
    while first_half.is_some() || second_half.is_some() {
        if toggle {
            if let Some(mut first_node) = first_half {
                // GOTCHA: We must take the rest of the list before mutating `next`
                first_half = first_node.next.take();
                tail.next = Some(first_node);
                tail = tail.next.as_mut().unwrap();
            }
        } else if let Some(mut second_node) = second_half {
            second_half = second_node.next.take();
            tail.next = Some(second_node);
            tail = tail.next.as_mut().unwrap();
        }
        toggle = !toggle;
    }

    *head = dummy.next;
}

/// Main entry point - uses optimal solution
pub fn reorder_list(head: &mut Option<Box<ListNode>>) {
    reorder_list_optimal(head);
}

// Alternative Approaches Footer:
// - A recursive approach could also be used to navigate to the end of the list and merge backwards,
//   but this would incur O(N) space complexity due to the call stack and can be trickier to satisfy
//   the borrow checker.
// - An array of raw pointers or indices could simulate an array-based modification, but `VecDeque`
//   provides a cleaner abstraction for the brute force method in safe Rust.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force_even_nodes() {
        let mut list = ListNode::from_vec(vec![1, 2, 3, 4]);
        reorder_list_brute_force(&mut list);
        assert_eq!(list.unwrap().to_vec(), vec![1, 4, 2, 3]);
    }

    #[test]
    fn test_brute_force_odd_nodes() {
        let mut list = ListNode::from_vec(vec![1, 2, 3, 4, 5]);
        reorder_list_brute_force(&mut list);
        assert_eq!(list.unwrap().to_vec(), vec![1, 5, 2, 4, 3]);
    }

    #[test]
    fn test_optimal_even_nodes() {
        let mut list = ListNode::from_vec(vec![1, 2, 3, 4]);
        reorder_list_optimal(&mut list);
        assert_eq!(list.unwrap().to_vec(), vec![1, 4, 2, 3]);
    }

    #[test]
    fn test_optimal_odd_nodes() {
        let mut list = ListNode::from_vec(vec![1, 2, 3, 4, 5]);
        reorder_list_optimal(&mut list);
        assert_eq!(list.unwrap().to_vec(), vec![1, 5, 2, 4, 3]);
    }

    #[test]
    fn test_edge_cases() {
        // Empty list
        let mut list: Option<Box<ListNode>> = None;
        reorder_list(&mut list);
        assert_eq!(list, None);

        // Single node
        let mut list = ListNode::from_vec(vec![1]);
        reorder_list(&mut list);
        assert_eq!(list.unwrap().to_vec(), vec![1]);

        // Two nodes
        let mut list = ListNode::from_vec(vec![1, 2]);
        reorder_list(&mut list);
        assert_eq!(list.unwrap().to_vec(), vec![1, 2]);
    }
}
