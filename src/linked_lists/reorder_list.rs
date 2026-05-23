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
//! ## Why this matters in Rust
//!
//! This problem is a brilliant exercise in Rust's ownership model, specifically `Option<Box<ListNode>>`
//! manipulation and borrowing rules. In languages like C++ or Java, reordering nodes is just
//! moving pointers around. In Rust, you must respect the single-ownership rule. Splitting a list,
//! reversing it, and merging it back together requires careful use of `Option::take()` and
//! mutable references to avoid borrow checker errors and dropping nodes prematurely.
//!
//! ## Approach
//!
//! We provide two approaches:
//!
//! 1. **Brute Force (VecDeque)**: We iterate through the list, collect all the nodes into a
//!    `VecDeque` (or just collect values), and then rebuild the list by popping alternately from
//!    the front and the back. This avoids complex pointer manipulation but uses O(N) auxiliary space.
//!    Time: O(N), Space: O(N).
//!
//! 2. **Optimal (In-place)**:
//!    - **Find the middle**: We count the nodes and advance a pointer to the middle to split the list.
//!    - **Split and reverse**: We disconnect the second half using `Option::take()` and reverse it.
//!    - **Merge**: We iterate through both halves, interleaving the nodes by swapping their `next` pointers.
//!    Time: O(N), Space: O(1).
//!
//! In Python or Java, this is straightforward. In Rust, we need to be extremely careful about
//! when we take ownership of a node's `next` pointer versus when we borrow it mutably.

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

/// Brute force approach: Collect to VecDeque and rebuild
/// Time: O(N) - Visit each node to collect, then rebuild
/// Space: O(N) - Store all nodes in a VecDeque
///
/// This approach sidesteps the complexity of in-place pointer manipulation.
#[allow(clippy::needless_pass_by_value)]
pub fn reorder_list_brute_force(head: &mut Option<Box<ListNode>>) {
    // If list is empty or has only one element, nothing to do
    if head.is_none() || head.as_ref().unwrap().next.is_none() {
        return;
    }

    // Step 1: Extract all nodes into a VecDeque
    // RUST INSIGHT: We use `take()` to safely move the entire list out of `head`
    // leaving `None` in its place while we work with it.
    let mut current = head.take();
    let mut deque = VecDeque::new();

    while let Some(mut node) = current {
        current = node.next.take(); // Disconnect node from the rest of the list
        deque.push_back(node);
    }

    // Step 2: Rebuild the list alternately from front and back
    let mut dummy = Box::new(ListNode::new(0));
    let mut tail = &mut dummy;

    let mut toggle = true;
    while !deque.is_empty() {
        let node = if toggle {
            deque.pop_front()
        } else {
            deque.pop_back()
        };

        tail.next = node;
        // GOTCHA: We must advance `tail` safely by borrowing `tail.next` mutably
        tail = tail.next.as_mut().unwrap();
        toggle = !toggle;
    }

    // Put the rebuilt list back into head
    *head = dummy.next;
}

/// Optimal approach: In-place O(1) space
/// Time: O(N) - Visit nodes to find middle, reverse, and merge
/// Space: O(1) - Only a few pointers
///
/// This approach splits the list, reverses the second half, and then merges them.
pub fn reorder_list_optimal(head: &mut Option<Box<ListNode>>) {
    if head.is_none() || head.as_ref().unwrap().next.is_none() {
        return;
    }

    // Step 1: Find the middle
    // In Rust, because we need to eventually split the list, we can calculate the length,
    // and then advance a mutable pointer to the middle to split it.
    let mut len = 0;
    let mut current = head.as_ref();
    while let Some(node) = current {
        len += 1;
        current = node.next.as_ref();
    }

    // `div_ceil` or integer math `(len + 1) / 2` to safely round up.
    let middle = (len + 1) / 2;

    let mut current_mut = head.as_mut();
    for _ in 1..middle {
        if let Some(node) = current_mut {
            current_mut = node.next.as_mut();
        }
    }

    // Step 2: Split and reverse the second half
    // `current_mut` now points to the last node of the first half.
    // RUST INSIGHT: `take()` allows us to detach the second half from the first half safely.
    let l2 = if let Some(node) = current_mut {
        node.next.take() // Split! First half ends here.
    } else {
        None
    };

    let l2 = reverse_list(l2);

    // Step 3: Merge the two halves
    merge_lists(head, l2);
}

// Helper function to reverse a linked list
#[must_use]
fn reverse_list(mut head: Option<Box<ListNode>>) -> Option<Box<ListNode>> {
    let mut prev = None;
    while let Some(mut node) = head {
        let next = node.next.take();
        node.next = prev;
        prev = Some(node);
        head = next;
    }
    prev
}

// Helper function to merge two lists alternately
fn merge_lists(l1: &mut Option<Box<ListNode>>, mut l2: Option<Box<ListNode>>) {
    let mut p1 = l1.as_mut();

    while let Some(node1) = p1 {
        // If l2 is empty, we are done (since l1 is always >= l2 in length)
        if l2.is_none() {
            break;
        }

        // Take the next node from l2
        let mut node2 = l2.unwrap();
        l2 = node2.next.take(); // Advance l2

        // node1 points to node2, and node2 points to the old node1.next
        let next1 = node1.next.take();
        node2.next = next1;
        node1.next = Some(node2);

        // Advance p1 by 2 steps (to the original node1.next)
        // GOTCHA: We need to carefully traverse `next.next` to satisfy the borrow checker.
        p1 = node1.next.as_mut().unwrap().next.as_mut();
    }
}

/// Main entry point - uses optimal solution
pub fn reorder_list(head: &mut Option<Box<ListNode>>) {
    reorder_list_optimal(head);
}

// Alternative Approaches:
// 1. Recursive Unwinding: We could recurse to the end of the list and merge on the way back up
//    while passing a mutable reference to the front of the list. This avoids explicit finding of
//    the middle, but is trickier to implement safely in Rust and uses O(N) space on the call stack.

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
    fn test_large_list() {
        let v: Vec<i32> = (1..=100).collect();
        let mut list1 = ListNode::from_vec(v.clone());
        let mut list2 = ListNode::from_vec(v);

        reorder_list_brute_force(&mut list1);
        reorder_list_optimal(&mut list2);

        // Expected: 1, 100, 2, 99, 3, 98...
        let res1 = list1.unwrap().to_vec();
        let res2 = list2.unwrap().to_vec();

        assert_eq!(res1, res2);
        assert_eq!(res1[0], 1);
        assert_eq!(res1[1], 100);
        assert_eq!(res1[2], 2);
        assert_eq!(res1[3], 99);
    }
}
