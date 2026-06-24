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
//! This problem is a phenomenal exercise in understanding Rust's strict aliasing rules and ownership model.
//! In languages like C++ or Python, you would use simultaneous slow and fast pointers to find the middle.
//! In Rust, having multiple mutable pointers traversing the same structure violates borrow rules. Thus, we learn
//! to use `Option::take()` to extract ownership, split the list, reverse the second half, and merge them back together.

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
}

/// Brute Force / VecDeque Approach
///
/// Time: O(n) - We iterate through the list to fill the deque, then iterate again to rebuild.
/// Space: O(n) - We allocate a `VecDeque` to store all node values.
///
/// This approach simply pulls all the values out of the linked list into a double-ended queue.
/// We then rebuild the list by alternating pops from the front and the back. While easy,
/// it violates the spirit of the question (which asks to reorder *in place* without extra space).
#[allow(clippy::needless_pass_by_value)]
pub fn reorder_list_deque(head: &mut Option<Box<ListNode>>) {
    if head.is_none() || head.as_ref().unwrap().next.is_none() {
        return;
    }

    let mut deque = VecDeque::new();
    let mut current = head.take(); // Take ownership out of the mutable reference

    // Extract all nodes
    while let Some(mut node) = current {
        current = node.next.take(); // Sever the rest of the list
        deque.push_back(node);
    }

    // Rebuild alternating front and back
    let mut dummy = ListNode::new(0);
    let mut tail = &mut dummy.next;
    let mut toggle = true;

    while !deque.is_empty() {
        let node = if toggle {
            deque.pop_front().unwrap()
        } else {
            deque.pop_back().unwrap()
        };

        *tail = Some(node);
        tail = &mut tail.as_mut().unwrap().next;
        toggle = !toggle;
    }

    *head = dummy.next;
}

/// Optimized Approach: Two-Pass Length Counting In-Place
///
/// Time: O(n) - Pass 1: count length. Pass 2: split. Pass 3: reverse half. Pass 4: merge.
/// Space: O(1) - We only manipulate pointers.
///
/// Because Rust prevents simultaneous mutable slow and fast pointers, we first count the length
/// to find the midpoint. We then split the list into two halves, reverse the second half, and merge them.
pub fn reorder_list_optimized(head: &mut Option<Box<ListNode>>) {
    if head.is_none() || head.as_ref().unwrap().next.is_none() {
        return;
    }

    // 1. Count the length of the list
    let mut len = 0;
    let mut curr = head.as_ref();
    while let Some(node) = curr {
        len += 1;
        curr = node.next.as_ref();
    }

    // 2. Advance to the midpoint to split the list
    let mid = (len - 1) / 2;
    let mut curr_mut = head.as_mut();
    for _ in 0..mid {
        if let Some(node) = curr_mut {
            curr_mut = node.next.as_mut();
        }
    }

    // GOTCHA: We must use `.take()` to literally cut the list into two distinct owned pieces.
    // `.take()` replaces the `next` pointer of the midpoint with `None` and gives us the tail.
    let mut l2 = if let Some(node) = curr_mut {
        node.next.take()
    } else {
        None
    };

    // 3. Reverse the second half (l2)
    let mut prev = None;
    let mut curr = l2;
    while let Some(mut node) = curr {
        // Grab the rest of the list
        let next = node.next.take();
        // Point the current node backwards
        node.next = prev;
        // Move prev and curr forward
        prev = Some(node);
        curr = next;
    }
    l2 = prev; // l2 is now the head of the reversed second half

    // 4. Merge l1 (which is `head`) and l2 alternatingly
    // RUST INSIGHT: We use a dummy head to simplify list reconstruction without edge cases.
    let mut dummy = ListNode::new(0);
    let mut tail = &mut dummy.next;

    // Take ownership of the first half
    let mut l1 = head.take();

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

    // Put the merged list back into the original head reference
    *head = dummy.next;
}

/// Main entry point delegating to the optimized O(1) space approach.
pub fn reorder_list(head: &mut Option<Box<ListNode>>) {
    reorder_list_optimized(head);
}

// Alternative Approaches:
// 1. Recursive Stack: You can use recursion to reach the end of the list, using the call stack to
//    effectively give you a "backwards" traversal while iterating forwards. This takes O(n) space
//    on the call stack and can cause stack overflow for very long lists.

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to convert Vec to Linked List
    fn vec_to_list(vec: Vec<i32>) -> Option<Box<ListNode>> {
        let mut dummy = ListNode::new(0);
        let mut tail = &mut dummy.next;
        for val in vec {
            *tail = Some(Box::new(ListNode::new(val)));
            tail = &mut tail.as_mut().unwrap().next;
        }
        dummy.next
    }

    // Helper to convert Linked List to Vec
    fn list_to_vec(mut head: Option<Box<ListNode>>) -> Vec<i32> {
        let mut vec = Vec::new();
        while let Some(node) = head {
            vec.push(node.val);
            head = node.next;
        }
        vec
    }

    #[test]
    fn test_happy_path_even_length() {
        // 1 -> 2 -> 3 -> 4 becomes 1 -> 4 -> 2 -> 3
        let mut list = vec_to_list(vec![1, 2, 3, 4]);
        reorder_list(&mut list);
        assert_eq!(list_to_vec(list), vec![1, 4, 2, 3]);
    }

    #[test]
    fn test_happy_path_odd_length() {
        // 1 -> 2 -> 3 -> 4 -> 5 becomes 1 -> 5 -> 2 -> 4 -> 3
        let mut list = vec_to_list(vec![1, 2, 3, 4, 5]);
        reorder_list(&mut list);
        assert_eq!(list_to_vec(list), vec![1, 5, 2, 4, 3]);
    }

    #[test]
    fn test_edge_case_single_node() {
        // 1 becomes 1
        let mut list = vec_to_list(vec![1]);
        reorder_list(&mut list);
        assert_eq!(list_to_vec(list), vec![1]);
    }

    #[test]
    fn test_edge_case_empty() {
        let mut list = None;
        reorder_list(&mut list);
        assert_eq!(list_to_vec(list), vec![]);
    }

    #[test]
    fn test_stress_boundary_two_nodes() {
        // 1 -> 2 becomes 1 -> 2
        let mut list = vec_to_list(vec![1, 2]);
        reorder_list(&mut list);
        assert_eq!(list_to_vec(list), vec![1, 2]);
    }
}
