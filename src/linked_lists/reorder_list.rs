//! # 143. Reorder List
//!
//! Difficulty: Medium
//! Link: https://leetcode.com/problems/reorder-list/
//!
//! You are given the head of a singly linked-list. The list can be represented as:
//! L0 → L1 → … → Ln - 1 → Ln
//!
//! Reorder the list to be on the following form:
//! L0 → Ln → L1 → Ln - 1 → L2 → Ln - 2 → …
//!
//! You may not modify the values in the list's nodes. Only nodes themselves may be changed.
//!
//! This problem is an excellent study of Rust's ownership model when manipulating recursive data structures.
//! It demonstrates why "slow/fast pointer" techniques often require two passes in safe Rust (one to count,
//! one to split) due to aliasing rules that prevent holding a mutable and immutable reference simultaneously.
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

/// Brute Force approach: Collect into a `VecDeque`, then reconstruct.
///
/// We collect all the nodes into a `VecDeque` (taking ownership),
/// and then we pop from the front and the back alternately to build
/// the reordered list.
///
/// Time: O(N)
/// Space: O(N) - We store all nodes in a `VecDeque`.
///
/// # Rust Insight
/// This approach completely sidesteps the tricky borrow checker rules around
/// linked list manipulation by temporarily transferring ownership of the nodes
/// into a data structure designed for double-ended access (`VecDeque`),
/// and then transferring ownership back into a new linked list.
pub fn reorder_list_brute_force(head: &mut Option<Box<ListNode>>) {
    if head.is_none() || head.as_ref().unwrap().next.is_none() {
        return;
    }

    // GOTCHA: We must take the list out of the mutable reference to consume it.
    // We replace `head` with `None` temporarily.
    let mut current = head.take();
    let mut deque = VecDeque::new();

    while let Some(mut node) = current {
        current = node.next.take(); // Detach the rest
        deque.push_back(node);
    }

    // Reconstruct the list
    let mut dummy = ListNode::new(0);
    let mut tail = &mut dummy;

    let mut take_front = true;
    while !deque.is_empty() {
        let next_node = if take_front {
            deque.pop_front()
        } else {
            deque.pop_back()
        };

        tail.next = next_node;
        tail = tail.next.as_mut().unwrap();
        take_front = !take_front;
    }

    *head = dummy.next;
}

/// Optimal approach: Split, Reverse, Merge (In-place).
///
/// 1. Find the middle of the list.
/// 2. Split the list into two halves.
/// 3. Reverse the second half.
/// 4. Merge the two halves alternately.
///
/// Time: O(N)
/// Space: O(1) - Only a few pointers are used.
///
/// # Gotcha
/// The classic C++/Java "slow and fast pointers" to find the middle in one pass
/// is difficult in safe Rust because the slow pointer would need a mutable reference
/// to split the list, while the fast pointer has an immutable reference to the same list.
/// This violates Rust's aliasing rules (no mutable and immutable references at the same time).
///
/// # Rust Insight
/// Instead, we do two passes: one to count the length, and one to traverse to the middle
/// mutably. This is still O(N) and perfectly safe and idiomatic in Rust.
pub fn reorder_list_optimal(head: &mut Option<Box<ListNode>>) {
    if head.is_none() || head.as_ref().unwrap().next.is_none() {
        return;
    }

    // 1. Count the length
    let mut len = 0;
    let mut current = head.as_ref();
    while let Some(node) = current {
        len += 1;
        current = node.next.as_ref();
    }

    // 2. Traverse to the middle mutably and split
    let mid = (len + 1) / 2; // e.g., len=4 -> mid=2. len=5 -> mid=3.
    let mut current_mut = head.as_mut();
    for _ in 0..mid - 1 {
        // Move forward mid - 1 times
        if let Some(node) = current_mut {
            current_mut = node.next.as_mut();
        }
    }

    // Split the list: `l2` is the second half, `current_mut`'s next becomes `None`
    let mut l2 = current_mut.and_then(|node| node.next.take());

    // 3. Reverse the second half
    let mut prev = None;
    let mut current_l2 = l2;
    while let Some(mut node) = current_l2 {
        let next = node.next.take();
        node.next = prev;
        prev = Some(node);
        current_l2 = next;
    }
    l2 = prev; // `l2` is now the head of the reversed second half

    // 4. Merge `head` (which is now just the first half) and `l2`
    let mut l1 = head.take(); // Take l1 entirely to consume it
    let mut dummy = ListNode::new(0);
    let mut tail = &mut dummy;

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

/// Main entry point - uses optimal solution
pub fn reorder_list(head: &mut Option<Box<ListNode>>) {
    reorder_list_optimal(head);
}

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
    fn test_all_approaches_consistency() {
        let v = vec![1, 2, 3, 4, 5, 6, 7];

        let mut l1 = ListNode::from_vec(v.clone());
        let mut l2 = ListNode::from_vec(v.clone());

        reorder_list_brute_force(&mut l1);
        reorder_list_optimal(&mut l2);

        assert_eq!(l1, l2);
    }

    #[test]
    fn test_stress() {
        // Stress test with 10,000 nodes to ensure no stack overflow or massive slowdowns
        let mut v = Vec::with_capacity(10_000);
        for i in 1..=10_000 {
            v.push(i);
        }

        let mut l1 = ListNode::from_vec(v.clone());
        reorder_list_optimal(&mut l1);

        // We just ensure it doesn't panic and actually runs fast.
        assert!(l1.is_some());
    }
    // Note: Stress test is included
}

// Alternative approaches:
// 1. Recursion: You can technically solve this recursively by going all the way to the end
//    and holding the front pointer in a mutable state. However, it requires O(N) stack space
//    and is very difficult to model cleanly in Safe Rust due to mutable aliasing.
// 2. `Vec` of References: You can collect `&mut Option<Box<ListNode>>` into a Vec, but this
//    is impossible in Safe Rust because it creates overlapping mutable references to the list.
//    The `VecDeque` approach (taking ownership) or Optimal approach (splitting) are the
//    preferred idiomatic Rust solutions.
