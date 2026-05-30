//! # 143. Reorder List
//!
//! Difficulty: Medium
//! Link: https://leetcode.com/problems/reorder-list/
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
//! manipulation. It demonstrates how to safely split a linked list, reverse a portion
//! of it in-place, and interweave two lists together while satisfying the borrow checker
//! using `Option::take()`.
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

    /// Helper to create a list from a vector (useful for tests and brute force)
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

/// Brute force approach: Collect all nodes into a `VecDeque` and rebuild
/// Time: O(N) - Two passes: one to extract all nodes, one to rebuild
/// Space: O(N) - Storing all nodes in a double-ended queue
///
/// This approach sidesteps advanced pointer manipulation. By extracting all nodes
/// into a `VecDeque`, we can trivially pop from the front and back to rebuild the list.
///
/// RUST INSIGHT: `Option::take()` is used to disconnect each node from the list,
/// allowing us to move owned `Box<ListNode>` instances into the `VecDeque` without
/// cloning.
pub fn reorder_list_brute_force(head: &mut Option<Box<ListNode>>) {
    if head.is_none() || head.as_ref().unwrap().next.is_none() {
        return;
    }

    // Extract all nodes
    let mut deque: VecDeque<Box<ListNode>> = VecDeque::new();
    let mut curr = head.take();
    while let Some(mut node) = curr {
        curr = node.next.take();
        deque.push_back(node);
    }

    // Rebuild from ends
    let mut dummy = ListNode::new(0);
    let mut tail = &mut dummy.next;
    let mut take_front = true;

    while !deque.is_empty() {
        let node = if take_front {
            deque.pop_front()
        } else {
            deque.pop_back()
        };
        take_front = !take_front;

        *tail = node;
        if let Some(n) = tail {
            tail = &mut n.next;
        }
    }

    *head = dummy.next;
}

/// Helper to reverse a linked list
fn reverse_list(head: Option<Box<ListNode>>) -> Option<Box<ListNode>> {
    let mut prev = None;
    let mut current = head;

    while let Some(mut node) = current {
        let next = node.next.take();
        node.next = prev;
        prev = Some(node);
        current = next;
    }

    prev
}

/// Optimal approach: Split, reverse second half, and merge in-place
/// Time: O(N) - Finding middle is O(N), reversing is O(N), merging is O(N)
/// Space: O(1) - Only a few pointers are used
///
/// This approach modifies the linked list in-place without any extra allocations.
/// It consists of 3 steps:
/// 1. Find the middle of the linked list (using a length counter since slow/fast pointers are tricky in Rust)
/// 2. Split the list and reverse the second half
/// 3. Interweave the nodes of the first half and the reversed second half
///
/// GOTCHA: In Rust, getting a mutable reference to the middle of a linked list while
/// keeping ownership of the head is complex. Instead, we can calculate the length,
/// traverse again to the split point, and use `Option::take()` to sever the list.
pub fn reorder_list_optimal(head: &mut Option<Box<ListNode>>) {
    if head.is_none() || head.as_ref().unwrap().next.is_none() {
        return;
    }

    // Step 1: Find length
    let mut len = 0;
    let mut curr = head.as_ref();
    while let Some(node) = curr {
        len += 1;
        curr = node.next.as_ref();
    }

    // Find split point
    let split_idx = (len + 1) / 2;

    // Step 2: Split the list
    let mut curr_mut = head.as_mut();
    for _ in 0..split_idx - 1 {
        if let Some(node) = curr_mut {
            curr_mut = node.next.as_mut();
        }
    }

    let l2 = if let Some(node) = curr_mut {
        node.next.take()
    } else {
        None
    };

    // Step 3: Reverse the second half
    let mut l2 = reverse_list(l2);

    // Step 4: Merge l1 and l2
    // We rebuild the list using a dummy head to easily append to the tail.
    // We extract the entire l1 from `head` temporarily to avoid borrowing issues.
    let mut l1 = head.take();
    let mut dummy = ListNode::new(0);
    let mut tail = &mut dummy.next;

    // We process l1, then l2, interweaving them.
    while l1.is_some() || l2.is_some() {
        if let Some(mut node1) = l1 {
            l1 = node1.next.take();
            *tail = Some(node1);
            if let Some(t) = tail {
                tail = &mut t.next;
            }
        }

        if let Some(mut node2) = l2 {
            l2 = node2.next.take();
            *tail = Some(node2);
            if let Some(t) = tail {
                tail = &mut t.next;
            }
        }
    }

    *head = dummy.next;
}

/// Main entry point - uses optimal solution
pub fn reorder_list(head: &mut Option<Box<ListNode>>) {
    reorder_list_optimal(head);
}

/// ## Alternative Approaches
///
/// 1. **Recursion/Call Stack**: You could theoretically use recursion to reach the end of the list and then perform the reordering as the stack unwinds. However, this is prone to stack overflows on large lists and is less idiomatic than the iterative approach.
/// 2. **Vec of Mutable References**: Instead of a VecDeque of owned values, one could try to collect mutable references to the nodes. However, Rust's borrow checker explicitly prevents multiple mutable references to overlapping data structures, making this approach extremely complex or requiring `unsafe` code. The in-place optimal approach or the safe extraction brute-force approach are preferred.

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
    fn test_empty_list() {
        let mut list = None;
        reorder_list(&mut list);
        assert_eq!(list, None);
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
    fn test_stress() {
        // Stress test with many nodes
        let n = 1000;
        let mut vals = Vec::with_capacity(n);
        for i in 1..=n {
            vals.push(i as i32);
        }

        let mut list = ListNode::from_vec(vals);
        reorder_list(&mut list);

        let mut expected = Vec::with_capacity(n);
        let mut left = 1;
        let mut right = n as i32;
        while left <= right {
            expected.push(left);
            left += 1;
            if left <= right {
                expected.push(right);
                right -= 1;
            }
        }

        assert_eq!(list.unwrap().to_vec(), expected);
    }
}
