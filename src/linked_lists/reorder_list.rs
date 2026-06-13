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
//! This problem emphasizes the strictness of Rust's ownership model. In languages like C++
//! or Python, finding the middle with a slow/fast pointer and then mutating the list
//! structure simultaneously is trivial. In safe Rust, mutable aliasing rules prevent
//! maintaining two mutable pointers into the same list. This pushes us toward safer
//! functional patterns (like extracting nodes into a collection) or careful, phase-separated
//! in-place mutation (count length, sever middle, reverse second half, merge).
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::linked_lists::reorder_list::{reorder_list, ListNode};
//!
//! let mut head = ListNode::from_vec(vec![1, 2, 3, 4]);
//! reorder_list(&mut head);
//! assert_eq!(head.unwrap().to_vec(), vec![1, 4, 2, 3]);
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

    /// Helper to create a list from a vector
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

/// Brute Force / Extra Space approach: VecDeque
/// Time: O(N) - to traverse and build the list.
/// Space: O(N) - stores all nodes in a VecDeque.
///
/// This approach bypasses Rust's aliasing restrictions by extracting all nodes
/// into a double-ended queue. We completely consume the original list, store
/// the `Box`es in the queue, and then rebuild a new list by alternatively
/// popping from the front and back.
#[allow(clippy::needless_pass_by_value)]
pub fn reorder_list_brute_force(head: &mut Option<Box<ListNode>>) {
    if head.is_none() || head.as_ref().unwrap().next.is_none() {
        return;
    }

    // RUST INSIGHT: `Option::take()` leaves `None` in the original variable and returns the `Some` value.
    // This allows us to take ownership of the entire list out of the mutable reference.
    let mut current = head.take();
    let mut deque = VecDeque::new();

    while let Some(mut node) = current {
        current = node.next.take(); // Sever the rest of the list
        deque.push_back(node);
    }

    // Dummy node to simplify rebuilding
    let mut dummy = ListNode::new(0);
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

    // Restore back to head
    *head = dummy.next;
}

/// Optimal approach: Two-Pass Length-Counting In-Place
/// Time: O(N)
/// Space: O(1)
///
/// In languages without borrow checking, we would use a slow and fast pointer to find
/// the middle, reverse the second half, and merge.
/// In Safe Rust, having a mutable `slow` and `fast` pointer traversing the same list
/// simultaneously is forbidden (mutable aliasing).
/// Instead, we perform multiple distinct passes:
/// 1. Count the length.
/// 2. Traverse to the middle node.
/// 3. Sever the list into two halves.
/// 4. Reverse the second half.
/// 5. Merge the two halves.
pub fn reorder_list_optimal(head: &mut Option<Box<ListNode>>) {
    if head.is_none() || head.as_ref().unwrap().next.is_none() {
        return;
    }

    // 1. Count length
    let mut len = 0;
    let mut curr = head.as_ref();
    while let Some(node) = curr {
        len += 1;
        curr = node.next.as_ref();
    }

    // 2. Traverse to the middle node
    let mid = (len + 1) / 2;
    let mut curr_mut = head.as_mut();
    for _ in 0..mid - 1 {
        if let Some(node) = curr_mut {
            curr_mut = node.next.as_mut();
        }
    }

    // 3. Sever the list into two halves
    // GOTCHA: We must take the `next` field of the node, not `.take()` on the option itself!
    let second_half = if let Some(node) = curr_mut {
        node.next.take()
    } else {
        None
    };

    // 4. Reverse the second half
    let mut prev = None;
    let mut curr_rev = second_half;
    while let Some(mut node) = curr_rev {
        let next_temp = node.next.take();
        node.next = prev;
        prev = Some(node);
        curr_rev = next_temp;
    }
    let mut reversed_second = prev;

    // 5. Merge the two halves
    // We take ownership of the first half out of `head` temporarily
    let mut first_half = head.take();

    let mut dummy = ListNode::new(0);
    let mut tail = &mut dummy;

    // RUST INSIGHT: Rebuilding the list using a dummy head and a tail pointer
    // is a common, safe idiom for linked list construction in Rust without unsafe code.
    while first_half.is_some() || reversed_second.is_some() {
        if let Some(mut node1) = first_half {
            first_half = node1.next.take();
            tail.next = Some(node1);
            tail = tail.next.as_mut().unwrap();
        }
        if let Some(mut node2) = reversed_second {
            reversed_second = node2.next.take();
            tail.next = Some(node2);
            tail = tail.next.as_mut().unwrap();
        }
    }

    // Restore the merged list back to head
    *head = dummy.next;
}

/// Main entry point - uses optimal solution
pub fn reorder_list(head: &mut Option<Box<ListNode>>) {
    reorder_list_optimal(head);
}

// Alternative Approaches:
// 1. **Vec of Mut Refs / Unsafe**: One could use `unsafe` to get raw pointers to the nodes,
//    allowing simultaneous traversal and mutation like in C. This is highly discouraged in Rust
//    unless performance is absolutely critical and the safe abstraction is proven to be the bottleneck.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force_even() {
        let mut head = ListNode::from_vec(vec![1, 2, 3, 4]);
        reorder_list_brute_force(&mut head);
        assert_eq!(head.unwrap().to_vec(), vec![1, 4, 2, 3]);
    }

    #[test]
    fn test_optimal_even() {
        let mut head = ListNode::from_vec(vec![1, 2, 3, 4]);
        reorder_list_optimal(&mut head);
        assert_eq!(head.unwrap().to_vec(), vec![1, 4, 2, 3]);
    }

    #[test]
    fn test_brute_force_odd() {
        let mut head = ListNode::from_vec(vec![1, 2, 3, 4, 5]);
        reorder_list_brute_force(&mut head);
        assert_eq!(head.unwrap().to_vec(), vec![1, 5, 2, 4, 3]);
    }

    #[test]
    fn test_optimal_odd() {
        let mut head = ListNode::from_vec(vec![1, 2, 3, 4, 5]);
        reorder_list_optimal(&mut head);
        assert_eq!(head.unwrap().to_vec(), vec![1, 5, 2, 4, 3]);
    }

    #[test]
    fn test_single_element() {
        let mut head = ListNode::from_vec(vec![1]);
        reorder_list(&mut head);
        assert_eq!(head.unwrap().to_vec(), vec![1]);
    }

    #[test]
    fn test_two_elements() {
        let mut head = ListNode::from_vec(vec![1, 2]);
        reorder_list(&mut head);
        assert_eq!(head.unwrap().to_vec(), vec![1, 2]);
    }

    #[test]
    fn test_empty() {
        let mut head = None;
        reorder_list(&mut head);
        assert_eq!(head, None);
    }
}
