//! # 143. Reorder List
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/reorder-list/>
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
//! This problem matters in Rust because it teaches in-place structural mutation of a linked list.
//! You must handle mutable references safely, specifically managing `Option::take()` for ownership
//! transfer while splitting, reversing, and merging linked lists without causing data races or memory leaks.
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

/// Brute Force Approach: Convert to `VecDeque`, interleave, and rebuild.
///
/// Time: O(N) - Two passes (one to collect, one to rebuild).
/// Space: O(N) - Stores all elements in a `VecDeque`.
///
/// # Why this is idiomatic in Rust
/// Converting the list to a collection to sidestep tricky pointer manipulation
/// is a perfectly valid and often preferred approach if memory isn't a strict constraint.
///
/// # Panics
///
/// Panics if the internal tail pointer is unexpectedly `None` while rebuilding the list,
/// though the loop invariants guarantee this is impossible.
#[allow(clippy::needless_pass_by_value)]
pub fn reorder_list_brute_force(head: &mut Option<Box<ListNode>>) {
    if head.is_none() {
        return;
    }

    let mut deque = VecDeque::new();
    let mut current = head.take();

    // Collect all nodes into the deque
    while let Some(mut node) = current {
        current = node.next.take();
        deque.push_back(node);
    }

    // Rebuild the list
    let mut new_head = None;
    let mut tail = &mut new_head;

    let mut toggle = true;
    while !deque.is_empty() {
        let node = if toggle {
            deque.pop_front()
        } else {
            deque.pop_back()
        };

        if let Some(n) = node {
            *tail = Some(n);
            tail = &mut tail.as_mut().unwrap().next;
        }
        toggle = !toggle;
    }

    *head = new_head;
}

/// Optimal Approach: Split in half, reverse second half, merge two halves.
///
/// Time: O(N) - Finding the middle (O(N)), reversing the second half (O(N)), merging (O(N)).
/// Space: O(1) - Constant auxiliary space, modifying the nodes in place.
///
/// # Algorithm
/// 1. Find the middle of the list. We use two pointers (slow/fast logic via counting length).
/// 2. Split the list into two halves. `Option::take()` allows safe structural disconnection.
/// 3. Reverse the second half.
/// 4. Merge the first half and the reversed second half node by node.
///
/// # Rust Insight
/// We rely heavily on `head.take()` to temporarily gain ownership of nodes out of their
/// Options, manipulate their `next` references, and then place them into their new destination.
///
/// # Gotcha
/// When merging `l1` and `l2`, be careful to maintain the correct interleaving order
/// and strictly manage the `tail` pointer to avoid dropping nodes or creating cycles.
///
/// # Panics
///
/// Panics if `unwrap()` fails during traversal or merging. The bounds and structure
/// guarantee the nodes exist at these exact required points.
pub fn reorder_list_optimal(head: &mut Option<Box<ListNode>>) {
    if head.is_none() || head.as_ref().unwrap().next.is_none() {
        return;
    }

    // Step 1: Count length to find the midpoint
    let mut len = 0;
    let mut current = head.as_ref();
    while let Some(node) = current {
        len += 1;
        current = node.next.as_ref();
    }

    // Step 2: Traverse to the node before the split point
    // For odd length (e.g., 5), we keep 3 nodes in the first half, 2 in the second.
    // For even length (e.g., 4), we keep 2 nodes in the first half, 2 in the second.
    let split_idx = (len + 1) / 2;
    let mut current_mut = head.as_mut();
    for _ in 0..split_idx - 1 {
        // Safe to unwrap because split_idx - 1 < len
        current_mut = current_mut.unwrap().next.as_mut();
    }

    // `current_mut` is the last node of the first half. We take its next to form the second half.
    let second_half = current_mut.unwrap().next.take();

    // Step 3: Reverse the second half
    let mut prev = None;
    let mut current_rev = second_half;
    while let Some(mut node) = current_rev {
        let next = node.next.take();
        node.next = prev;
        prev = Some(node);
        current_rev = next;
    }

    let mut l2 = prev; // Head of the reversed second half

    // Step 4: Merge the two halves
    let mut l1 = head.take(); // Take the first half

    let mut merged = None;
    let mut tail = &mut merged;

    // RUST INSIGHT: We use `Option::take()` within the loop to move the current node
    // out of l1/l2 without moving the entire Box, leaving l1/l2 pointing to the rest of the list.
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

    *head = merged;
}

/// Main entry point - uses optimal in-place algorithm.
pub fn reorder_list(head: &mut Option<Box<ListNode>>) {
    reorder_list_optimal(head);
}

// =========================================================================================
// Alternative Approaches
// =========================================================================================
// 1. Recursive Approach: Reorder using a recursive helper that maintains a pointer
//    to the left side of the list while the recursion unwinds from the right side.
//    This is O(N) space due to the call stack and can be difficult to implement
//    without interior mutability (`Rc<RefCell<T>>`) or very careful lifetime bounds.
// 2. HashMap/Vector indexing: Map indices to node references to directly link `i`
//    to `N - i`. Less idiomatic and requires auxiliary space, but conceptually simpler
//    if working with `Rc<RefCell<Node>>` rather than `Box`.

// =========================================================================================
// Tests
// =========================================================================================

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
        assert!(list.is_none());
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
        let v: Vec<i32> = (1..1000).collect();
        let mut list1 = ListNode::from_vec(v.clone());
        let mut list2 = ListNode::from_vec(v);

        reorder_list_brute_force(&mut list1);
        reorder_list_optimal(&mut list2);

        assert_eq!(list1.unwrap().to_vec(), list2.unwrap().to_vec());
    }
}
