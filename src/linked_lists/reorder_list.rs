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
//! This problem is a phenomenal exercise in Rust because it forces you to orchestrate
//! three distinct linked list operations (find middle, reverse, merge) while strictly
//! adhering to ownership and borrowing rules. It demonstrates why safely maneuvering
//! `Option<Box<ListNode>>` prevents the dangling pointers common in C++ implementations.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::linked_lists::reorder_list::{reorder_list, ListNode};
//!
//! let list = ListNode::from_vec(vec![1, 2, 3, 4]);
//! let reordered = reorder_list(list);
//! assert_eq!(reordered.unwrap().to_vec(), vec![1, 4, 2, 3]);
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

/// Brute force approach: Collect into `VecDeque`, rebuild list.
///
/// Time: O(N) - One pass to collect, one pass to rebuild.
/// Space: O(N) - Storing all nodes in a queue.
///
/// This avoids complex pointer manipulation by transferring ownership of each node
/// into a `VecDeque`, and then popping alternately from the front and back to
/// rebuild the connections.
#[must_use]
pub fn reorder_list_brute_force(mut head: Option<Box<ListNode>>) -> Option<Box<ListNode>> {
    if head.is_none() || head.as_ref().unwrap().next.is_none() {
        return head;
    }

    let mut deque = VecDeque::new();
    while let Some(mut node) = head {
        // RUST INSIGHT: `take()` allows us to detach the tail and transfer ownership
        // of `node` into the deque without cloning.
        head = node.next.take();
        deque.push_back(node);
    }

    let mut dummy = ListNode::new(0);
    let mut tail = &mut dummy.next;
    let mut pop_front = true;

    while !deque.is_empty() {
        let next_node = if pop_front {
            deque.pop_front()
        } else {
            deque.pop_back()
        };

        *tail = next_node;
        tail = &mut tail.as_mut().unwrap().next;
        pop_front = !pop_front;
    }

    dummy.next
}

/// Optimal approach: Find middle, reverse second half, merge two halves.
///
/// Time: O(N) - Three linear passes (find mid, reverse, merge).
/// Space: O(1) - Constant auxiliary space, modifying the list in-place.
///
/// # Idiomatic Rust
/// In C/C++, we'd use slow and fast pointers to find the middle. In Rust,
/// multiple mutable references to a linked list are difficult to maintain safely.
/// The functional and idiomatic Rust approach involves taking ownership, counting
/// the length, advancing to the midpoint, and physically splitting the list into two owned halves.
#[must_use]
pub fn reorder_list_optimal(mut head: Option<Box<ListNode>>) -> Option<Box<ListNode>> {
    if head.is_none() || head.as_ref().unwrap().next.is_none() {
        return head;
    }

    // 1. Find the length
    let mut len = 0;
    let mut curr = &head;
    while let Some(node) = curr {
        len += 1;
        curr = &node.next;
    }

    // 2. Split the list in half
    let mid = (len + 1) / 2;
    let mut curr_mut = &mut head;
    for _ in 0..mid {
        if let Some(node) = curr_mut {
            curr_mut = &mut node.next;
        }
    }

    // RUST INSIGHT: `take()` physically severs the list, giving us full ownership of the second half.
    // The first half (in `head`) now implicitly terminates with `None`.
    let second_half = curr_mut.take();

    // 3. Reverse the second half
    let mut reversed_second = None;
    let mut curr2 = second_half;
    while let Some(mut node) = curr2 {
        let next = node.next.take();
        node.next = reversed_second;
        reversed_second = Some(node);
        curr2 = next;
    }

    // 4. Merge the two halves
    // GOTCHA: We must re-bind `head` iteratively, or build from a dummy node.
    let mut dummy = ListNode::new(0);
    let mut tail = &mut dummy.next;
    let mut l1 = head;
    let mut l2 = reversed_second;

    while l1.is_some() || l2.is_some() {
        if let Some(mut n1) = l1 {
            l1 = n1.next.take();
            *tail = Some(n1);
            tail = &mut tail.as_mut().unwrap().next;
        }
        if let Some(mut n2) = l2 {
            l2 = n2.next.take();
            *tail = Some(n2);
            tail = &mut tail.as_mut().unwrap().next;
        }
    }

    dummy.next
}

/// Main entry point - uses the optimal solution
#[must_use]
pub fn reorder_list(head: Option<Box<ListNode>>) -> Option<Box<ListNode>> {
    reorder_list_optimal(head)
}

// Alternative Approaches:
// 1. **Recursive Array Indexing**: Store the values or nodes in a `Vec`, then manipulate pointers.
//    Similar to the brute-force `VecDeque` approach but uses integer indices. Space O(N).
// 2. **Unsafe Fast/Slow Pointers**: One could use `*mut ListNode` to implement standard
//    Tortoise and Hare pointer manipulation, avoiding length counting. However, standard
//    safe Rust splits and length counting is preferred as it relies entirely on the type system
//    for safety while retaining O(N) performance.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force_even() {
        let list = ListNode::from_vec(vec![1, 2, 3, 4]);
        let result = reorder_list_brute_force(list);
        assert_eq!(result.unwrap().to_vec(), vec![1, 4, 2, 3]);
    }

    #[test]
    fn test_brute_force_odd() {
        let list = ListNode::from_vec(vec![1, 2, 3, 4, 5]);
        let result = reorder_list_brute_force(list);
        assert_eq!(result.unwrap().to_vec(), vec![1, 5, 2, 4, 3]);
    }

    #[test]
    fn test_optimal_even() {
        let list = ListNode::from_vec(vec![1, 2, 3, 4]);
        let result = reorder_list_optimal(list);
        assert_eq!(result.unwrap().to_vec(), vec![1, 4, 2, 3]);
    }

    #[test]
    fn test_optimal_odd() {
        let list = ListNode::from_vec(vec![1, 2, 3, 4, 5]);
        let result = reorder_list_optimal(list);
        assert_eq!(result.unwrap().to_vec(), vec![1, 5, 2, 4, 3]);
    }

    #[test]
    fn test_empty_list() {
        assert_eq!(reorder_list(None), None);
    }

    #[test]
    fn test_single_node() {
        let list = ListNode::from_vec(vec![1]);
        let result = reorder_list(list);
        assert_eq!(result.unwrap().to_vec(), vec![1]);
    }

    #[test]
    fn test_two_nodes() {
        let list = ListNode::from_vec(vec![1, 2]);
        let result = reorder_list(list);
        assert_eq!(result.unwrap().to_vec(), vec![1, 2]);
    }

    #[test]
    fn test_all_approaches_consistency() {
        let vectors = vec![vec![1, 2, 3, 4, 5, 6, 7], vec![10, 20], vec![1, 2, 3]];

        for v in vectors {
            let l1 = ListNode::from_vec(v.clone());
            let l2 = ListNode::from_vec(v);

            let res1 = reorder_list_brute_force(l1);
            let res2 = reorder_list_optimal(l2);

            assert_eq!(res1, res2);
        }
    }
}
