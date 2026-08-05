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
//! ## Why This Matters in Rust
//!
//! This problem is an excellent exercise in understanding Rust's ownership model when manipulating
//! linked lists. It demonstrates how to safely dismantle and reconstruct recursive data structures
//! using `Option::take()` and `std::collections::VecDeque`. It also contrasts the ease of using
//! a collection (O(N) space) versus the strict pointer manipulation required for the optimal O(1) space solution.
//!
//! ## Approach
//!
//! We provide two implementations:
//! 1. **Optimized (Deque)**: We dismantle the list by taking ownership of each node and pushing it
//!    into a `VecDeque`. We then rebuild the list by alternately popping from the front and the back.
//!    This takes O(N) time and O(N) space.
//! 2. **Optimal (In-place)**: We achieve O(1) extra space by:
//!    - Counting the total number of nodes.
//!    - Splitting the list into two halves at the midpoint.
//!    - Reversing the second half in-place.
//!    - Weaving (merging) the two halves together node-by-node.
//!
//! In languages like C++ or Java, one might use slow/fast pointers to find the middle. In Rust,
//! iterating to count the length and then iterating again to the midpoint avoids multiple mutable
//! aliases and is highly idiomatic and equally O(N) time.

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

    /// Helper to create a list from a vector (useful for tests)
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

/// Optimized approach: Using a VecDeque
///
/// Time: O(N) - One pass to dismantle, one pass to rebuild.
/// Space: O(N) - Storing all nodes in a VecDeque.
///
/// This approach is very clean and avoids complex pointer manipulation by fully
/// dismantling the list into a double-ended queue, then rebuilding it.
#[allow(clippy::missing_panics_doc)]
pub fn reorder_list_deque(head: &mut Option<Box<ListNode>>) {
    // RUST INSIGHT: `take()` moves the value out of `head`, leaving `None` in its place.
    // We now have full ownership of the entire list.
    let mut current = head.take();
    let mut deque = VecDeque::new();

    // Dismantle the list into the deque
    while let Some(mut node) = current {
        current = node.next.take(); // Detach the rest of the list
        deque.push_back(node);
    }

    let mut dummy = ListNode::new(0);
    let mut tail = &mut dummy;
    let mut take_front = true;

    // Rebuild alternating between front and back
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

    // Put the rebuilt list back into the original head
    *head = dummy.next.take();
}

/// Optimal approach: O(1) space by splitting, reversing, and merging.
///
/// Time: O(N) - Finding length, splitting, reversing, and merging are all O(N) operations.
/// Space: O(1) - Only a few pointers are used.
///
/// This is the idiomatic systems programming approach. We use constant space.
#[allow(clippy::missing_panics_doc)]
pub fn reorder_list_optimal(head: &mut Option<Box<ListNode>>) {
    let mut len = 0;
    let mut current = head.as_ref();

    // 1. Count the number of nodes
    while let Some(node) = current {
        len += 1;
        current = node.next.as_ref();
    }

    if len <= 2 {
        return;
    }

    // 2. Find the midpoint and split the list
    let mid = (len + 1) / 2;
    let mut current_mut = head.as_mut();

    // Advance to the node just before the second half
    for _ in 0..(mid - 1) {
        if let Some(node) = current_mut {
            current_mut = node.next.as_mut();
        }
    }

    // `second_half` takes ownership of the rest of the list.
    // The first half is safely truncated because `take()` leaves `None`.
    let second_half = current_mut.unwrap().next.take();

    // 3. Reverse the second half
    let mut prev = None;
    let mut curr = second_half;

    while let Some(mut node) = curr {
        // GOTCHA: We must temporarily store `next` before overwriting it.
        let next = node.next.take();
        node.next = prev;
        prev = Some(node);
        curr = next;
    }
    let second_half = prev;

    // 4. Merge the two halves
    let first_half = head.take();
    let mut dummy = ListNode::new(0);
    let mut tail = &mut dummy;

    let mut l1 = first_half;
    let mut l2 = second_half;

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

    *head = dummy.next.take();
}

/// Main entry point - uses optimal solution
pub fn reorder_list(head: &mut Option<Box<ListNode>>) {
    reorder_list_optimal(head);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deque_approach_even() {
        let mut list = ListNode::from_vec(vec![1, 2, 3, 4]);
        reorder_list_deque(&mut list);
        assert_eq!(list.unwrap().to_vec(), vec![1, 4, 2, 3]);
    }

    #[test]
    fn test_deque_approach_odd() {
        let mut list = ListNode::from_vec(vec![1, 2, 3, 4, 5]);
        reorder_list_deque(&mut list);
        assert_eq!(list.unwrap().to_vec(), vec![1, 5, 2, 4, 3]);
    }

    #[test]
    fn test_optimal_approach_even() {
        let mut list = ListNode::from_vec(vec![1, 2, 3, 4]);
        reorder_list_optimal(&mut list);
        assert_eq!(list.unwrap().to_vec(), vec![1, 4, 2, 3]);
    }

    #[test]
    fn test_optimal_approach_odd() {
        let mut list = ListNode::from_vec(vec![1, 2, 3, 4, 5]);
        reorder_list_optimal(&mut list);
        assert_eq!(list.unwrap().to_vec(), vec![1, 5, 2, 4, 3]);
    }

    // Edge Case Tests
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

    // Stress/Boundary Tests
    #[test]
    fn test_large_list() {
        let size = 10_000;
        let mut values = Vec::with_capacity(size);
        for i in 1..=size {
            values.push(i as i32);
        }

        let mut list = ListNode::from_vec(values.clone());
        reorder_list(&mut list);

        let mut expected = Vec::with_capacity(size);
        let mut left = 0;
        let mut right = size - 1;
        while left <= right {
            expected.push(values[left]);
            if left != right {
                expected.push(values[right]);
            }
            left += 1;
            if left <= right {
                right -= 1;
            }
        }

        assert_eq!(list.unwrap().to_vec(), expected);
    }
}
