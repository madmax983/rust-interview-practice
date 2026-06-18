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
//! This problem perfectly demonstrates Rust's strict aliasing and ownership rules.
//! In languages like C++ or Java, the standard approach uses a slow/fast pointer to find the middle,
//! reverse the second half, and interleave. However, safe Rust disallows multiple mutable pointers
//! traversing a linked list simultaneously. Here we use an idiomatic "two-pass length-counting" approach
//! and safely re-build the list using `Option::take`.
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

/// Brute force approach: Collect into `VecDeque`, rebuild
/// Time: O(N)
/// Space: O(N) auxiliary space
///
/// This approach takes ownership of all nodes, puts them in a `VecDeque`,
/// and reconstructs the list by alternating popping from the front and back.
/// It's simple but allocates O(N) space, violating the spirit of in-place reordering.
pub fn reorder_list_brute_force(head: &mut Option<Box<ListNode>>) {
    // We cannot proceed if list is empty or has 1 element
    if head.is_none() || head.as_ref().unwrap().next.is_none() {
        return;
    }

    // GOTCHA: We must take ownership of the list to put nodes into our Deque.
    // If we only take references, we couldn't rebuild the ownership chain.
    let mut current = head.take();
    let mut deque = VecDeque::new();

    while let Some(mut node) = current {
        current = node.next.take(); // Sever the rest of the list
        deque.push_back(node);
    }

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

/// Optimal approach: Two-pass length-counting in-place
/// Time: O(N)
/// Space: O(1) auxiliary space (if we ignore recursion stack, though we do it iteratively here where possible)
///
/// Finding the middle of a linked list with a slow/fast pointer in *safe* Rust while mutating is practically impossible
/// because it violates the "one mutable reference at a time" rule.
///
/// Instead, the idiomatic way in Rust to achieve O(1) space is:
/// 1. First pass: count the total number of nodes.
/// 2. Second pass: iterate to the middle node, sever the second half.
/// 3. Reverse the second half.
/// 4. Merge the two halves.
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

    let mid = len / 2 + len % 2;

    // Step 2: Traverse to the middle and split the list.
    // RUST INSIGHT: We temporarily `take` the head to safely iterate and split, avoiding borrow checker errors.
    let mut first_half = head.take();
    let mut curr_mut = &mut first_half;

    for _ in 0..mid {
        if let Some(node) = curr_mut {
            curr_mut = &mut node.next;
        }
    }

    // `curr_mut` now points to the `next` field of the middle node.
    // We `take()` it to sever the list and get ownership of the second half.
    // GOTCHA: `.take()` on the option itself just takes the reference, not the underlying node.
    // By taking `node.next`, we extract the tail. Here, `curr_mut` IS `node.next`.
    let mut second_half = curr_mut.take();

    // Step 3: Reverse the second half
    let mut reversed_second = None;
    while let Some(mut node) = second_half {
        let next = node.next.take();
        node.next = reversed_second;
        reversed_second = Some(node);
        second_half = next;
    }

    // Step 4: Merge the two halves
    let mut dummy = Box::new(ListNode::new(0));
    let mut tail = &mut dummy;

    let mut h1 = first_half;
    let mut h2 = reversed_second;

    while h1.is_some() || h2.is_some() {
        if let Some(mut node1) = h1 {
            h1 = node1.next.take();
            tail.next = Some(node1);
            tail = tail.next.as_mut().unwrap();
        }
        if let Some(mut node2) = h2 {
            h2 = node2.next.take();
            tail.next = Some(node2);
            tail = tail.next.as_mut().unwrap();
        }
    }

    // Put the merged list back into head
    *head = dummy.next;
}

/// Main entry point - delegates to the optimal O(1) space implementation
pub fn reorder_list(head: &mut Option<Box<ListNode>>) {
    reorder_list_optimal(head);
}

// Alternative Approaches:
// 1. **Vec of pointers/references (Unsafe)**: Using unsafe Rust, one could create a `Vec<*mut ListNode>`
//    to allow random access and quick middle-finding/tail-swapping, similar to C++. However, this requires
//    careful raw pointer management and defeats the safety guarantees of Rust.
// 2. **Recursion (Call Stack Space)**: A recursive approach can keep track of the front of the list while
//    returning from the end of the list to swap links. This uses O(N) space on the call stack.

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
