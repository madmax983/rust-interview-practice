//! # 143. Reorder List
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
//! This problem is a masterclass in Rust's ownership model. It requires you to safely dismantle
//! and reconstruct a recursive data structure without triggering borrow checker errors or memory leaks.
//! It showcases `Option::take()`, `VecDeque` for brute-force tracking, and an optimal in-place approach.
//!
//! ## Approach
//!
//! There are two main ways to solve this in Rust:
//!
//! 1.  **Brute Force (`VecDeque`)**: We consume the entire list, place every node into a `VecDeque`,
//!     and then pop from the front and back to rebuild the list. This takes O(N) space but makes
//!     the pointer manipulation trivial.
//! 2.  **Optimal (Split, Reverse, Merge)**: We split the list in half, reverse the second half,
//!     and then merge the two lists alternately. In languages like C++, you'd use a mutable slow/fast
//!     pointer to split the list in-place. In safe Rust, since we cannot have two mutable pointers to the
//!     same list, we perform an immutable slow/fast pass to find the length to the middle, then do
//!     a targeted mutable traversal to split the list using `Option::take()`.
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
    pub next: Option<Box<ListNode>>,
}

impl ListNode {
    #[inline]
    #[must_use]
    pub fn new(val: i32) -> Self {
        ListNode { next: None, val }
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

/// Brute force approach: `VecDeque` auxiliary storage
/// Time: O(N) - Single pass to collect, single pass to rebuild
/// Space: O(N) - Storing all nodes in a double-ended queue
///
/// This approach sidesteps advanced pointer manipulation by moving ownership of all nodes
/// into a `VecDeque`. We then pop from the front and back to rebuild the list.
pub fn reorder_list_brute_force(head: &mut Option<Box<ListNode>>) {
    let mut deque = VecDeque::new();

    // RUST INSIGHT: `take()` replaces `head` with `None`, transferring ownership
    // to `current` without violating mutable borrow rules.
    let mut current = head.take();

    while let Some(mut node) = current {
        current = node.next.take(); // Sever the rest of the list
        deque.push_back(Some(node));
    }

    let mut dummy = ListNode::new(0);
    let mut tail = &mut dummy;
    let mut flip = true;

    while !deque.is_empty() {
        let node = if flip {
            deque.pop_front().flatten()
        } else {
            deque.pop_back().flatten()
        };

        tail.next = node;
        if let Some(ref mut added) = tail.next {
            tail = added.as_mut();
        }
        flip = !flip;
    }

    *head = dummy.next;
}

/// Optimal approach: Split, Reverse, Merge
/// Time: O(N) - Slow/fast pointer to find middle, reverse second half, merge
/// Space: O(1) - In-place pointer manipulation
///
/// This approach avoids O(N) auxiliary space.
///
/// RUST INSIGHT: In C++, we'd use a mutable slow/fast pointer to split the list.
/// Rust's borrow checker prevents multiple mutable references into the same structure.
/// Instead, we perform an immutable slow/fast pass to find the split index, then a targeted
/// mutable pass to split the list using `Option::take()`.
pub fn reorder_list_optimal(head: &mut Option<Box<ListNode>>) {
    // 1. Find the split point using an immutable fast/slow pointer approach
    let mut fast = head.as_ref();
    let mut slow = head.as_ref();
    let mut split_len = 0;

    while let Some(f) = fast {
        if let Some(f_next) = f.next.as_ref() {
            fast = f_next.next.as_ref();
            if let Some(s) = slow {
                slow = s.next.as_ref();
            }
            split_len += 1;
        } else {
            break;
        }
    }

    // If fast is not None (i.e. we broke out of the loop because f.next was None),
    // the list has an odd number of elements. We increment split_len to include the
    // middle element in the first half.
    if fast.is_some() {
        split_len += 1;
    }

    if split_len == 0 {
        return;
    }

    // 2. Split the list
    let mut current_mut = head.as_mut();
    for _ in 1..split_len {
        if let Some(node) = current_mut {
            current_mut = node.next.as_mut();
        }
    }

    // GOTCHA: `take()` replaces the `next` pointer of the mid node with `None`,
    // effectively severing the list in two. `second_half` now owns the rest.
    let mut second_half = if let Some(node) = current_mut {
        node.next.take()
    } else {
        None
    };

    // 3. Reverse the second half
    let mut prev = None;
    while let Some(mut node) = second_half {
        let next = node.next.take();
        node.next = prev;
        prev = Some(node);
        second_half = next;
    }
    let mut reversed_second = prev;

    // 4. Merge alternating nodes
    let mut first_curr = head.as_mut();
    let mut second_curr = reversed_second;

    while let Some(mut second_node) = second_curr {
        if let Some(first_node) = first_curr.take() {
            // Detach the next nodes for both halves
            let first_next = first_node.next.take();
            let second_next = second_node.next.take();

            // Link first to second
            first_node.next = Some(second_node);

            // Link second to the rest of the first half
            if let Some(ref mut newly_added) = first_node.next {
                newly_added.next = first_next;
                // Move first_curr to the rest of the first half
                first_curr = newly_added.next.as_mut();
            }

            second_curr = second_next;
        } else {
            break;
        }
    }
}

/// Main entry point - uses optimal solution
pub fn reorder_list(head: &mut Option<Box<ListNode>>) {
    reorder_list_optimal(head);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force_example_1() {
        let mut list = ListNode::from_vec(vec![1, 2, 3, 4]);
        reorder_list_brute_force(&mut list);
        assert_eq!(list.unwrap().to_vec(), vec![1, 4, 2, 3]);
    }

    #[test]
    fn test_brute_force_example_2() {
        let mut list = ListNode::from_vec(vec![1, 2, 3, 4, 5]);
        reorder_list_brute_force(&mut list);
        assert_eq!(list.unwrap().to_vec(), vec![1, 5, 2, 4, 3]);
    }

    #[test]
    fn test_optimal_example_1() {
        let mut list = ListNode::from_vec(vec![1, 2, 3, 4]);
        reorder_list_optimal(&mut list);
        assert_eq!(list.unwrap().to_vec(), vec![1, 4, 2, 3]);
    }

    #[test]
    fn test_optimal_example_2() {
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
    fn test_three_nodes() {
        let mut list = ListNode::from_vec(vec![1, 2, 3]);
        reorder_list(&mut list);
        assert_eq!(list.unwrap().to_vec(), vec![1, 3, 2]);
    }
}
