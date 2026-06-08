//! # 143. Reorder List
//!
//! You are given the head of a singly linked-list. The list can be represented as:
//! `L0 → L1 → … → Ln - 1 → Ln`
//!
//! Reorder the list to be on the following form:
//! `L0 → Ln → L1 → Ln - 1 → L2 → Ln - 2 → …`
//!
//! You may not modify the values in the list's nodes. Only nodes themselves may be changed.
//!
//! - Difficulty: Medium
//! - LeetCode: <https://leetcode.com/problems/reorder-list/>
//!
//! ## Why this matters in Rust
//! This problem perfectly highlights the contrast between other languages where everything is a shared reference, and Rust, where ownership and mutable borrowing strictly control who can modify what. Reordering a linked list in-place requires severing the list, reversing a portion of it, and weaving two disconnected lists together—all operations that typically anger the borrow checker. Mastering `Option::take()` and `Option::replace()` to temporarily take ownership of nodes while rebuilding a list is a core pattern in idiomatic Rust linked list manipulation.
//!
//! ## Approach
//!
//! We will explore two implementations:
//! 1.  **Brute Force / Space Tradeoff (`VecDeque`)**: Convert the linked list into a double-ended queue. We can then easily pop from the front and back to build the reordered list. This is simpler to write but uses O(N) extra space.
//! 2.  **Optimal In-Place (`Slow/Fast Pointer + Reverse + Merge`)**: The canonical O(1) space solution. It requires three steps:
//!     *   Find the middle of the list using the slow/fast pointer technique.
//!     *   Sever the second half and reverse it.
//!     *   Merge the two halves by alternating nodes.
//!
//! ## Types
//! We assume the standard LeetCode definition for `ListNode`.

use std::collections::VecDeque;

// Definition for singly-linked list.
#[derive(PartialEq, Eq, Clone, Debug)]
pub struct ListNode {
    pub val: i32,
    pub next: Option<Box<ListNode>>,
}

impl ListNode {
    #[inline]
    pub fn new(val: i32) -> Self {
        ListNode { next: None, val }
    }
}

/// Brute Force / Space Tradeoff Approach using `VecDeque`
///
/// We iterate through the list, popping all nodes and pushing them into a `VecDeque`.
/// Then, we rebuild the list by taking nodes alternately from the front and the back of the deque.
///
/// - **Time Complexity**: O(N) to traverse the list and populate the deque, and O(N) to rebuild it.
/// - **Space Complexity**: O(N) to store the nodes in the `VecDeque`.
pub fn reorder_list_deque(head: &mut Option<Box<ListNode>>) {
    if head.is_none() || head.as_ref().unwrap().next.is_none() {
        return;
    }

    let mut deque = VecDeque::new();
    // GOTCHA: We must use `.take()` here to move the list out of `head` without violating
    // the borrow checker. We leave `None` in `head` temporarily.
    let mut current = head.take();

    // Populate the deque
    while let Some(mut node) = current {
        current = node.next.take(); // Sever the connection to avoid deep recursive drops or loops later
        deque.push_back(node);
    }

    let mut dummy = ListNode::new(0);
    let mut tail = &mut dummy.next;
    let mut toggle = true;

    // Rebuild the list
    while !deque.is_empty() {
        let node = if toggle {
            deque.pop_front()
        } else {
            deque.pop_back()
        };
        *tail = node;
        tail = &mut tail.as_mut().unwrap().next;
        toggle = !toggle;
    }

    *head = dummy.next;
}

/// Optimal In-Place Approach: Slow/Fast Pointer + Reverse + Merge
///
/// This approach achieves O(1) extra space by manipulating the pointers directly.
/// 1. Find the middle of the list.
/// 2. Reverse the second half.
/// 3. Merge the first and second halves.
///
/// - **Time Complexity**: O(N). Finding the middle is O(N), reversing is O(N), and merging is O(N).
/// - **Space Complexity**: O(1). We only use a few pointers.
pub fn reorder_list(head: &mut Option<Box<ListNode>>) {
    // Edge case: empty list or single node
    if head.is_none() || head.as_ref().unwrap().next.is_none() {
        return;
    }

    // Step 1: Find the middle using a fast/slow pointer approach, but we need
    // to do it in a way that allows us to sever the list. A simple way in Rust
    // without multiple mutable borrows is to count the length first, or use a two-pass approach.
    // Let's use the two-pass length-counting approach as it's cleaner in safe Rust.

    let mut len = 0;
    let mut current = head.as_ref();
    while let Some(node) = current {
        len += 1;
        current = node.next.as_ref();
    }

    // RUST INSIGHT: To mutate a linked list in place, we often need to trace a `&mut` reference
    // to the split point.
    let mid = (len + 1) / 2;
    let mut first_half_tail = head.as_mut();
    for _ in 0..(mid - 1) {
        if let Some(node) = first_half_tail {
            first_half_tail = node.next.as_mut();
        }
    }

    // Step 2: Sever the second half and reverse it.
    // RUST INSIGHT: `.take()` takes the value out of the Option, leaving None in its place.
    // This effectively severs the list exactly where we want it!
    let mut second_half = if let Some(node) = first_half_tail {
        node.next.take()
    } else {
        None
    };

    let mut prev = None;
    let mut curr = second_half;
    while let Some(mut node) = curr {
        let next = node.next.take();
        node.next = prev;
        prev = Some(node);
        curr = next;
    }
    second_half = prev;

    // Step 3: Merge the two halves
    // GOTCHA: We can't easily iterate over `head` and modify it at the same time if we
    // just use `while let`. We need to extract `head` to a temporary variable and rebuild it.
    let mut first_half = head.take();
    let mut dummy = ListNode::new(0);
    let mut tail = &mut dummy.next;

    let mut l1 = first_half;
    let mut l2 = second_half;

    while l1.is_some() || l2.is_some() {
        if let Some(mut node) = l1 {
            l1 = node.next.take();
            *tail = Some(node);
            tail = &mut tail.as_mut().unwrap().next;
        }
        if let Some(mut node) = l2 {
            l2 = node.next.take();
            *tail = Some(node);
            tail = &mut tail.as_mut().unwrap().next;
        }
    }

    // Restore the reordered list to the original head pointer.
    *head = dummy.next;
}

// Alternative Approaches:
// 1. **Recursion**: You could use the call stack to reach the end of the list and work backwards, effectively merging the ends. However, this is O(N) space due to the call stack and can cause stack overflows on very large lists, making it less practical than the in-place iterative method.

#[cfg(test)]
mod tests {
    use super::*;

    // Helper function to build a list from a vec
    fn build_list(vec: Vec<i32>) -> Option<Box<ListNode>> {
        let mut dummy = ListNode::new(0);
        let mut tail = &mut dummy.next;
        for val in vec {
            *tail = Some(Box::new(ListNode::new(val)));
            tail = &mut tail.as_mut().unwrap().next;
        }
        dummy.next
    }

    // Helper function to convert a list to a vec for easy assertions
    fn list_to_vec(head: &Option<Box<ListNode>>) -> Vec<i32> {
        let mut vec = Vec::new();
        let mut current = head;
        while let Some(node) = current {
            vec.push(node.val);
            current = &node.next;
        }
        vec
    }

    #[test]
    fn test_reorder_list_even() {
        let mut head = build_list(vec![1, 2, 3, 4]);
        reorder_list(&mut head);
        assert_eq!(list_to_vec(&head), vec![1, 4, 2, 3]);
    }

    #[test]
    fn test_reorder_list_odd() {
        let mut head = build_list(vec![1, 2, 3, 4, 5]);
        reorder_list(&mut head);
        assert_eq!(list_to_vec(&head), vec![1, 5, 2, 4, 3]);
    }

    #[test]
    fn test_reorder_list_edge_cases() {
        // Empty list
        let mut head = build_list(vec![]);
        reorder_list(&mut head);
        assert_eq!(list_to_vec(&head), vec![]);

        // Single node
        let mut head = build_list(vec![1]);
        reorder_list(&mut head);
        assert_eq!(list_to_vec(&head), vec![1]);

        // Two nodes
        let mut head = build_list(vec![1, 2]);
        reorder_list(&mut head);
        assert_eq!(list_to_vec(&head), vec![1, 2]);
    }

    #[test]
    fn test_reorder_list_deque() {
        let mut head = build_list(vec![1, 2, 3, 4, 5]);
        reorder_list_deque(&mut head);
        assert_eq!(list_to_vec(&head), vec![1, 5, 2, 4, 3]);
    }
}
