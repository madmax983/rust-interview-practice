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
//! This problem is a masterclass in Rust's ownership model, specifically `Option<Box<ListNode>>`
//! manipulation and borrowing rules. It illustrates why the standard "slow and fast pointer"
//! technique—trivial in C++ or Java—is difficult in safe Rust due to strict aliasing. Instead,
//! it pushes us to adopt a two-pass length-counting approach for in-place modification.
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

/// Brute force approach: Collect nodes into a VecDeque, reconstruct list
///
/// Time: O(N) - One pass to collect, one pass to reconstruct.
/// Space: O(N) - Storing all nodes in a `VecDeque`.
///
/// This approach sidesteps the complex borrowing rules of in-place linked list
/// modification by temporarily taking ownership of all nodes into a `VecDeque`,
/// then alternating `pop_front` and `pop_back` to reconstruct the list.
#[allow(clippy::needless_pass_by_value)]
pub fn reorder_list_brute_force(head: &mut Option<Box<ListNode>>) {
    if head.is_none() || head.as_ref().unwrap().next.is_none() {
        return;
    }

    // Take the list out of head, leaving None in its place.
    // RUST INSIGHT: Option::take() is essential to gain ownership of the list
    // while maintaining a valid state in the mutable reference.
    let mut current = head.take();
    let mut deque = VecDeque::new();

    while let Some(mut node) = current {
        current = node.next.take(); // Sever the link
        deque.push_back(node);
    }

    let mut dummy = Box::new(ListNode::new(0));
    let mut tail = &mut dummy;

    let mut take_front = true;
    while !deque.is_empty() {
        let node = if take_front {
            deque.pop_front().unwrap()
        } else {
            deque.pop_back().unwrap()
        };
        take_front = !take_front;
        tail.next = Some(node);
        tail = tail.next.as_mut().unwrap();
    }

    *head = dummy.next;
}

/// Optimal approach: Two-pass length-counting, split, reverse, merge in-place
///
/// Time: O(N) - Two passes over the list.
/// Space: O(1) - Only a few pointers are used, modifying the list in-place.
///
/// In languages like C++, finding the middle of a linked list is often done using a
/// "slow and fast pointer". In safe Rust, having simultaneous mutable pointers to
/// different parts of the same linked list violates strict aliasing (you can't borrow
/// the same list twice mutably).
///
/// Instead, the idiomatic safe Rust way is a two-pass approach:
/// 1. Count the length (immutable pass).
/// 2. Traverse to the middle, split the list using `Option::take()` (mutable pass).
/// 3. Reverse the second half.
/// 4. Merge the two halves.
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

    // 2. Split the list in half
    let mut mid = len / 2;
    if len % 2 != 0 {
        mid += 1;
    }

    let mut current = head.as_mut();
    for _ in 0..mid - 1 {
        if let Some(node) = current {
            current = node.next.as_mut();
        }
    }

    // `current` is now at the last node of the first half.
    // We sever the list here by taking the `next` field.
    // GOTCHA: Don't call `.take()` on `current` itself (which is `Option<&mut Box<ListNode>>`),
    // because that just takes the reference out of the Option. We must take `node.next`.
    let mut second_half = None;
    if let Some(node) = current {
        second_half = node.next.take();
    }

    // 3. Reverse the second half
    let mut prev = None;
    let mut curr = second_half;
    while let Some(mut node) = curr {
        let next = node.next.take();
        node.next = prev;
        prev = Some(node);
        curr = next;
    }
    second_half = prev;

    // 4. Merge the two halves
    let first_half = head.take();

    // We'll rebuild the list into a dummy node
    let mut dummy = Box::new(ListNode::new(0));
    let mut tail = &mut dummy;

    let mut h1 = first_half;
    let mut h2 = second_half;

    while h1.is_some() || h2.is_some() {
        if let Some(mut n1) = h1 {
            h1 = n1.next.take();
            tail.next = Some(n1);
            tail = tail.next.as_mut().unwrap();
        }
        if let Some(mut n2) = h2 {
            h2 = n2.next.take();
            tail.next = Some(n2);
            tail = tail.next.as_mut().unwrap();
        }
    }

    *head = dummy.next;
}

/// Main entry point
pub fn reorder_list(head: &mut Option<Box<ListNode>>) {
    reorder_list_optimal(head);
}

// Alternative Approaches:
// 1. Recursive: Using recursion to reorder can be elegant but requires O(N) call stack space,
//    which is not ideal.
// 2. Unsafe pointers: We *could* implement a slow/fast pointer using `*mut ListNode` inside an
//    `unsafe` block. However, this repository prioritizes safe, idiomatic Rust. The O(N) two-pass
//    approach is just as asymptotically fast while maintaining the compiler's safety guarantees.

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
    fn test_stress_consistency() {
        let data: Vec<i32> = (1..=100).collect();
        let mut l1 = ListNode::from_vec(data.clone());
        let mut l2 = ListNode::from_vec(data.clone());

        reorder_list_brute_force(&mut l1);
        reorder_list_optimal(&mut l2);

        assert_eq!(l1.unwrap().to_vec(), l2.unwrap().to_vec());
    }
}
