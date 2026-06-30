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
//! This problem is an excellent study in Rust's ownership model, specifically `Option<Box<ListNode>>`
//! manipulation, `Option::take()`, and borrowing rules during in-place list modification. It contrasts
//! a brute-force `VecDeque` approach with an optimal two-pass length-counting approach.
//!
//! ## Approaches
//!
//! 1.  **VecDeque (Brute Force / Additional Space)**:
//!     -   Traverse the list and push all nodes into a `VecDeque`.
//!     -   Pop from the front and back of the deque alternately to rebuild the list.
//!     -   Time: O(N).
//!     -   Space: O(N) auxiliary space.
//! 2.  **Two-Pass Length-Counting (Optimal / In-Place)**:
//!     -   Pass 1: Count the length of the list.
//!     -   Pass 2: Traverse to the middle, split the list, reverse the second half, and merge the two halves.
//!     -   Time: O(N).
//!     -   Space: O(1) auxiliary space.
//!     -   *Why not fast/slow pointers?* In safe Rust, traversing a linked list simultaneously with a slow
//!         mutable pointer and a fast immutable/mutable pointer violates strict aliasing rules unless we use
//!         raw pointers or unsafe blocks. The length-counting method is 100% safe and just as fast in practice.

use std::collections::VecDeque;

// =========================================================================================
// Data Structures
// =========================================================================================

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

    /// Helper to create a list from a slice (useful for tests)
    #[must_use]
    pub fn from_slice(values: &[i32]) -> Option<Box<Self>> {
        let mut current = None;
        for &val in values.iter().rev() {
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

// =========================================================================================
// Approach 1: VecDeque
// =========================================================================================

/// Time: O(N)
/// Space: O(N)
pub fn reorder_list_deque(head: &mut Option<Box<ListNode>>) {
    if head.is_none() || head.as_ref().unwrap().next.is_none() {
        return;
    }

    // Step 1: Collect all nodes into a VecDeque
    let mut deque = VecDeque::new();
    let mut current = head.take();

    while let Some(mut node) = current {
        current = node.next.take(); // Detach the rest of the list
        deque.push_back(node);
    }

    // Step 2: Rebuild the list
    let mut dummy = ListNode::new(0);
    let mut tail = &mut dummy.next;

    let mut toggle = true;
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

// =========================================================================================
// Approach 2: Optimal In-Place
// =========================================================================================

/// Helper to reverse a linked list in-place
fn reverse_list(mut head: Option<Box<ListNode>>) -> Option<Box<ListNode>> {
    let mut prev = None;
    while let Some(mut node) = head {
        head = node.next.take();
        node.next = prev;
        prev = Some(node);
    }
    prev
}

/// Time: O(N)
/// Space: O(1)
pub fn reorder_list_optimal(head: &mut Option<Box<ListNode>>) {
    if head.is_none() || head.as_ref().unwrap().next.is_none() {
        return;
    }

    // Step 1: Count the length of the list
    let mut len = 0;
    let mut current = head.as_ref();
    while let Some(node) = current {
        len += 1;
        current = node.next.as_ref();
    }

    // Step 2: Find the middle node to split the list
    // If length is 5, middle is 3. If length is 4, middle is 2.
    let mid = (len + 1) / 2;
    let mut current_mut = head.as_mut();
    for _ in 1..mid {
        if let Some(node) = current_mut {
            current_mut = node.next.as_mut();
        }
    }

    // Step 3: Split the list and reverse the second half
    let second_half = current_mut.unwrap().next.take();
    let mut reversed_second = reverse_list(second_half);

    // Step 4: Merge the two halves
    // RUST INSIGHT: Merging two owned lists safely without violating strict aliasing
    let mut first_half = head.take();
    let mut dummy = ListNode::new(0);
    let mut tail = &mut dummy.next;

    while first_half.is_some() || reversed_second.is_some() {
        if let Some(mut first_node) = first_half {
            first_half = first_node.next.take();
            *tail = Some(first_node);
            tail = &mut tail.as_mut().unwrap().next;
        }

        if let Some(mut second_node) = reversed_second {
            reversed_second = second_node.next.take();
            *tail = Some(second_node);
            tail = &mut tail.as_mut().unwrap().next;
        }
    }

    *head = dummy.next;
}

/// Main entry point - uses optimal solution
pub fn reorder_list(head: &mut Option<Box<ListNode>>) {
    reorder_list_optimal(head);
}

// =========================================================================================
// Tests
// =========================================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_happy_path_even() {
        // [1,2,3,4] -> [1,4,2,3]
        let mut list1 = ListNode::from_slice(&[1, 2, 3, 4]);
        reorder_list_deque(&mut list1);
        assert_eq!(list1.unwrap().to_vec(), vec![1, 4, 2, 3]);

        let mut list2 = ListNode::from_slice(&[1, 2, 3, 4]);
        reorder_list_optimal(&mut list2);
        assert_eq!(list2.unwrap().to_vec(), vec![1, 4, 2, 3]);
    }

    #[test]
    fn test_happy_path_odd() {
        // [1,2,3,4,5] -> [1,5,2,4,3]
        let mut list1 = ListNode::from_slice(&[1, 2, 3, 4, 5]);
        reorder_list_deque(&mut list1);
        assert_eq!(list1.unwrap().to_vec(), vec![1, 5, 2, 4, 3]);

        let mut list2 = ListNode::from_slice(&[1, 2, 3, 4, 5]);
        reorder_list_optimal(&mut list2);
        assert_eq!(list2.unwrap().to_vec(), vec![1, 5, 2, 4, 3]);
    }

    #[test]
    fn test_edge_case_empty() {
        let mut list1 = None;
        reorder_list_deque(&mut list1);
        assert_eq!(list1, None);

        let mut list2 = None;
        reorder_list_optimal(&mut list2);
        assert_eq!(list2, None);
    }

    #[test]
    fn test_edge_case_single() {
        let mut list1 = ListNode::from_slice(&[1]);
        reorder_list_deque(&mut list1);
        assert_eq!(list1.unwrap().to_vec(), vec![1]);

        let mut list2 = ListNode::from_slice(&[1]);
        reorder_list_optimal(&mut list2);
        assert_eq!(list2.unwrap().to_vec(), vec![1]);
    }

    #[test]
    fn test_edge_case_two() {
        let mut list1 = ListNode::from_slice(&[1, 2]);
        reorder_list_deque(&mut list1);
        assert_eq!(list1.unwrap().to_vec(), vec![1, 2]);

        let mut list2 = ListNode::from_slice(&[1, 2]);
        reorder_list_optimal(&mut list2);
        assert_eq!(list2.unwrap().to_vec(), vec![1, 2]);
    }
}
