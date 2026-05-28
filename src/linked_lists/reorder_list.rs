//! # 143. Reorder List
//!
//! **Difficulty:** Medium
//! **Link:** <https://leetcode.com/problems/reorder-list/>
//!
//! You are given the head of a singly linked-list. The list can be represented as:
//!
//! L0 → L1 → … → Ln - 1 → Ln
//!
//! Reorder the list to be on the following form:
//!
//! L0 → Ln → L1 → Ln - 1 → L2 → Ln - 2 → …
//!
//! You may not modify the values in the list's nodes. Only nodes themselves may be changed.
//!
//! This problem is a comprehensive test of linked list manipulation in Rust. It perfectly
//! demonstrates Rust's ownership model, specifically `Option<Box<ListNode>>` manipulation,
//! `Option::take()`, and borrowing rules during in-place list modification. It requires
//! splitting a list, reversing a list, and merging two lists.
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
    pub next: Option<Box<ListNode>>,
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

/// Brute force approach: Extract nodes to a `VecDeque`, pop alternately, and relink.
/// Time: O(N) - Two passes (one to extract, one to rebuild).
/// Space: O(N) - Auxiliary space for the `VecDeque`.
///
/// This approach uses a double-ended queue to cleanly pop from the front and back.
/// It avoids complex pointer arithmetic by breaking the list apart completely
/// and rebuilding it.
///
/// RUST INSIGHT: We use `Option::take()` repeatedly to detach nodes from the list
/// while satisfying the borrow checker, avoiding `clone()`.
pub fn reorder_list_brute_force(head: &mut Option<Box<ListNode>>) {
    if head.is_none() || head.as_ref().unwrap().next.is_none() {
        return;
    }

    // Step 1: Extract all nodes into a VecDeque
    let mut deque = VecDeque::new();
    let mut current = head.take();
    while let Some(mut node) = current {
        current = node.next.take(); // Detach the rest of the list
        deque.push_back(node);
    }

    // Step 2: Rebuild the list by popping alternately
    let mut dummy = ListNode::new(0);
    let mut tail = &mut dummy;

    let mut turn = true; // true = pop_front, false = pop_back
    while !deque.is_empty() {
        let node = if turn {
            deque.pop_front().unwrap()
        } else {
            deque.pop_back().unwrap()
        };

        tail.next = Some(node);
        tail = tail.next.as_mut().unwrap();
        turn = !turn;
    }

    *head = dummy.next;
}

/// Optimal approach: Slow/fast pointers to find middle, reverse second half, merge in-place.
/// Time: O(N) - Three passes (find middle, reverse, merge), but O(1) space.
/// Space: O(1) - Only a few pointers are used.
///
/// This is the idiomatic, in-place solution. It combines three classic linked list
/// techniques into one algorithm.
///
/// RUST INSIGHT: In C++, you'd just use raw pointers. In Rust, we have to carefully
/// manage ownership. We split the list into two distinct `Option<Box<ListNode>>`
/// sub-lists so we can independently traverse and merge them without aliasing issues.
///
/// GOTCHA: Finding the middle with safe Rust requires either counting length or
/// traversing and then splitting. Counting length is often easier to appease the borrow
/// checker than a slow/fast `&mut` traversal. Here, we'll use length counting for
/// simplicity and robust borrow checking.
pub fn reorder_list_optimal(head: &mut Option<Box<ListNode>>) {
    if head.is_none() || head.as_ref().unwrap().next.is_none() {
        return;
    }

    // Step 1: Find the length of the list
    let mut len = 0;
    let mut curr_ref = head.as_ref();
    while let Some(node) = curr_ref {
        len += 1;
        curr_ref = node.next.as_ref();
    }

    // The first half gets `len - len / 2` nodes.
    let first_half_len = len - len / 2;

    // Step 2: Split the list into two halves
    let mut curr_mut = head.as_mut();
    for _ in 0..(first_half_len - 1) {
        curr_mut = curr_mut.unwrap().next.as_mut();
    }
    // `second_half` now owns the rest of the list.
    let mut second_half = curr_mut.unwrap().next.take();

    // Step 3: Reverse the second half
    let mut reversed_second = None;
    while let Some(mut node) = second_half {
        let next = node.next.take();
        node.next = reversed_second;
        reversed_second = Some(node);
        second_half = next;
    }

    // Step 4: Merge the two halves alternately
    // RUST INSIGHT: We use a dummy head to easily build the merged list,
    // and `Option::take()` to safely extract the actual `head` we passed in,
    // swapping it with `None` temporarily, and putting the result back at the end.
    let first_half = head.take();
    let mut dummy = ListNode::new(0);
    let mut tail = &mut dummy;

    let mut l1 = first_half;
    let mut l2 = reversed_second;

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

    // Put the merged list back into `head`
    *head = dummy.next;
}

/// Main entry point - uses optimal solution
pub fn reorder_list(head: &mut Option<Box<ListNode>>) {
    reorder_list_optimal(head);
}

// Alternative Approaches
// ----------------------
// 1. **VecDeque (Brute Force):** Collect all nodes into a `VecDeque`, then alternately pop from the front and back to rebuild the list.
//    - *Pros:* Very simple to conceptualize and implement.
//    - *Cons:* O(N) extra space required, which violates the spirit of "in-place" modification typical for linked list problems.
//
// 2. **Recursive approach:** Use recursion to traverse to the end, modifying pointers on the way back up the call stack.
//    - *Pros:* Can be elegant in purely functional languages.
//    - *Cons:* Causes an O(N) call stack size overhead, risking stack overflow in Rust since TCO isn't guaranteed, and pointer state management is quite messy to express correctly with safe Rust.

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
}
