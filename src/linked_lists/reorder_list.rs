//! # 143. Reorder List
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/reorder-list/>
//!
//! You are given the head of a singly linked-list. The list can be represented as:
//! `L0 → L1 → … → Ln-1 → Ln`
//!
//! Reorder the list to be on the following form:
//! `L0 → Ln → L1 → Ln-1 → L2 → Ln-2 → …`
//!
//! You may not modify the values in the list's nodes. Only nodes themselves may be changed.
//!
//! This problem perfectly demonstrates Rust's ownership model, specifically `Option<Box<ListNode>>` manipulation,
//! `Option::take()`, and borrowing rules during in-place list modification, contrasting a brute-force `VecDeque`
//! approach with an optimal slow/fast pointer in-place approach.
//!
//! ## Approach

Unlike Python or Java where linked lists are often modified casually with garbage collection cleaning up unreferenced nodes, Rust requires explicit ownership transfer (`Option::take()`). This forces us to sever links cleanly and rebuild them without aliasing mutable pointers.
//!
//! We present two approaches:
//! 1. **Brute Force (`VecDeque`)**: Takes ownership of the list, puts all nodes into a `VecDeque`, and reconstructs
//!    the list by alternately popping from the front and back. Time: O(n), Space: O(n).
//! 2. **Optimal (In-place pointers)**:
//!    - Find the middle of the linked list.
//!    - Reverse the second half of the linked list.
//!    - Merge the two halves.
//!    Time: O(n), Space: O(1). This is significantly more idiomatic and optimal, showcasing how to safely navigate
//!    mutable aliases and borrow checker constraints.
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

/// Brute Force approach: Collect to VecDeque and reconstruct
///
/// Time: O(n)
/// Space: O(n)
pub fn reorder_list_brute_force(head: &mut Option<Box<ListNode>>) {
    if head.is_none() || head.as_ref().unwrap().next.is_none() {
        return;
    }

    // RUST INSIGHT: We use `take()` to move the entire list out of the mutable reference,
    // leaving `None` in its place. This avoids borrow checker issues while we dismantle it.
    let mut deque = VecDeque::new();
    let mut current = head.take();

    while let Some(mut node) = current {
        current = node.next.take();
        deque.push_back(node);
    }

    let mut dummy = ListNode::new(0);
    let mut tail = &mut dummy;
    let mut toggle = true;

    while !deque.is_empty() {
        // GOTCHA: By conditionally taking from front or back, we avoid complex index math,
        // but we spend O(N) memory storing the nodes.
        let node = if toggle {
            deque.pop_front().unwrap()
        } else {
            deque.pop_back().unwrap()
        };
        tail.next = Some(node);
        tail = tail.next.as_mut().unwrap();
        toggle = !toggle;
    }

    *head = dummy.next.take();
}

/// Optimal approach: In-place middle split, reverse, and merge
///
/// Time: O(n)
/// Space: O(1)
pub fn reorder_list_optimal(head: &mut Option<Box<ListNode>>) {
    let mut len = 0;
    {
        let mut curr = head.as_ref();
        while let Some(node) = curr {
            len += 1;
            curr = node.next.as_ref();
        }
    }

    if len <= 2 {
        return;
    }

    // Step 1: Find the middle and split the list
    let mut curr = head.as_mut().unwrap();
    for _ in 0..((len - 1) / 2) {
        curr = curr.next.as_mut().unwrap();
    }

    // RUST INSIGHT: `take()` gracefully severs the list into two distinct, un-aliased halves
    let second_half = curr.next.take();

    // Step 2: Reverse the second half
    let mut reversed = None;
    let mut curr2 = second_half;
    while let Some(mut node) = curr2 {
        let next = node.next.take();
        node.next = reversed;
        reversed = Some(node);
        curr2 = next;
    }

    // Step 3: Merge the two halves
    let mut curr1 = head.as_mut();
    let mut curr2 = reversed;

    while curr1.is_some() && curr2.is_some() {
        // GOTCHA: Merging requires careful unwrapping and `.take()` to weave the pointers
        // without violating single-ownership.
        let c1 = curr1.unwrap();
        let c1_next = c1.next.take();

        let mut c2 = curr2.unwrap();
        let c2_next = c2.next.take();

        c2.next = c1_next;
        c1.next = Some(c2);

        curr1 = c1.next.as_mut().unwrap().next.as_mut();
        curr2 = c2_next;
    }
}

/// Main entry point - uses optimal solution
pub fn reorder_list(head: &mut Option<Box<ListNode>>) {
    reorder_list_optimal(head);
}

// Alternative Approaches:
// 1. Recursive: One could try to solve this recursively, but it would require an O(N) call stack
//    and potentially complicated return types to weave the list back together. The iterative
//    approach here is both clearer and strictly O(1) space.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force_happy_path() {
        let mut list = ListNode::from_vec(vec![1, 2, 3, 4]);
        reorder_list_brute_force(&mut list);
        assert_eq!(list.unwrap().to_vec(), vec![1, 4, 2, 3]);

        let mut list2 = ListNode::from_vec(vec![1, 2, 3, 4, 5]);
        reorder_list_brute_force(&mut list2);
        assert_eq!(list2.unwrap().to_vec(), vec![1, 5, 2, 4, 3]);
    }

    #[test]
    fn test_optimal_happy_path() {
        let mut list = ListNode::from_vec(vec![1, 2, 3, 4]);
        reorder_list_optimal(&mut list);
        assert_eq!(list.unwrap().to_vec(), vec![1, 4, 2, 3]);

        let mut list2 = ListNode::from_vec(vec![1, 2, 3, 4, 5]);
        reorder_list_optimal(&mut list2);
        assert_eq!(list2.unwrap().to_vec(), vec![1, 5, 2, 4, 3]);
    }

    #[test]
    fn test_edge_cases() {
        // Empty list
        let mut list: Option<Box<ListNode>> = None;
        reorder_list(&mut list);
        assert_eq!(list, None);

        // Single node
        let mut list = ListNode::from_vec(vec![1]);
        reorder_list(&mut list);
        assert_eq!(list.unwrap().to_vec(), vec![1]);

        // Two nodes
        let mut list = ListNode::from_vec(vec![1, 2]);
        reorder_list(&mut list);
        assert_eq!(list.unwrap().to_vec(), vec![1, 2]);
    }

    #[test]
    fn test_stress_boundary_case() {
        let mut vec = Vec::new();
        for i in 1..=1000 {
            vec.push(i);
        }
        let mut list = ListNode::from_vec(vec.clone());
        reorder_list(&mut list);

        let mut expected = Vec::new();
        let mut left = 0;
        let mut right = vec.len() - 1;
        while left <= right {
            expected.push(vec[left]);
            if left != right {
                expected.push(vec[right]);
            }
            left += 1;
            if right == 0 { break; } // prevent underflow
            right -= 1;
        }

        assert_eq!(list.unwrap().to_vec(), expected);
    }
}
