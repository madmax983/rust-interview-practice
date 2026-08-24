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
//! This problem matters in Rust because it forces you to understand **ownership transfer**,
//! **in-place mutation**, and **splitting mutable borrows**. Unlike garbage-collected languages
//! where you can keep multiple aliases to different nodes, Rust's `Box<T>` requires explicit
//! detaching (`Option::take`) and reattaching to manipulate nodes.
//!
//! ## Approach
//!
//! We provide three approaches to illustrate tradeoffs in Rust:
//!
//! 1.  **Brute Force**: Collect values into a `Vec`, then build a completely new list.
//!     *   Time: O(N), Space: O(N).
//!     *   *Why this is idiomatic*: Sometimes allocating a new list is perfectly fine and avoids complex pointer math, yielding clean, safe code.
//!
//! 2.  **Optimized**: Detach nodes from the original list and store them in a `VecDeque`. Then, pop alternatingly from the front and back to rebuild the list in-place.
//!     *   Time: O(N), Space: O(N) pointers (not full nodes).
//!     *   *Why this is idiomatic*: Shows how to move *ownership* of nodes into a temporary collection and re-stitch them without allocating new nodes.
//!
//! 3.  **Optimal**: Find the middle (via length counting), split the list, reverse the second half, and interleave the two halves.
//!     *   Time: O(N), Space: O(1).
//!     *   *Why this is idiomatic*: Achieves the in-place O(1) space requirement. It avoids the classic "slow/fast pointer" alias issues by counting length and mutating `&mut Option<Box<ListNode>>`.
//!
//! ## Alternative approaches
//!
//! -   Instead of counting the length in the optimal approach, one could use recursion, but this would increase space complexity to O(N) due to the call stack.

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

/// Brute force approach: Collect values to Vec, create a new list.
/// Time: O(N)
/// Space: O(N)
#[must_use]
pub fn reorder_list_brute_force(head: Option<Box<ListNode>>) -> Option<Box<ListNode>> {
    let mut vec = Vec::new();
    let mut current = head;
    while let Some(node) = current {
        vec.push(node.val);
        current = node.next;
    }

    let mut reordered = Vec::with_capacity(vec.len());
    let n = vec.len();
    if n > 0 {
        let mut left = 0;
        let mut right = n.saturating_sub(1);
        while left <= right {
            reordered.push(vec[left]);
            if left != right {
                reordered.push(vec[right]);
            }
            left += 1;
            if right == 0 {
                break;
            }
            right -= 1;
        }
    }

    ListNode::from_vec(reordered)
}

/// Optimized approach: Store detached nodes in a `VecDeque`, then re-link.
/// Time: O(N)
/// Space: O(N) for pointers in `VecDeque`
///
/// # Panics
/// Panics if the `VecDeque` yields a node but the tail cannot be mutably dereferenced.
#[must_use]
pub fn reorder_list_optimized(mut head: Option<Box<ListNode>>) -> Option<Box<ListNode>> {
    let mut deque = VecDeque::new();

    // RUST INSIGHT: `head` takes ownership of the Box. `node.next.take()` leaves
    // `None` in the current node, allowing us to safely push the detached node into the deque.
    while let Some(mut node) = head {
        head = node.next.take();
        deque.push_back(node);
    }

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
        // GOTCHA: `.unwrap()` is safe here because we just assigned `tail.next`.
        tail = tail.next.as_mut().unwrap();
        toggle = !toggle;
    }

    dummy.next
}

/// Optimal approach: O(1) space.
/// 1. Find middle by length.
/// 2. Split list.
/// 3. Reverse second half.
/// 4. Merge alternating.
///
/// # Panics
/// Panics if a node cannot be mutably dereferenced during the merge step, or if unwrapping
/// an unexpectedly `None` node.
#[must_use]
pub fn reorder_list_optimal(head: Option<Box<ListNode>>) -> Option<Box<ListNode>> {
    let mut head = head;
    if head.is_none() || head.as_ref().unwrap().next.is_none() {
        return head;
    }

    // 1. Find the middle
    let mut len = 0;
    let mut current = &head;
    while let Some(node) = current {
        len += 1;
        current = &node.next;
    }

    let mid = (len + 1) / 2;

    // 2. Split the list
    let mut current_mut = &mut head;
    for _ in 0..mid {
        if let Some(node) = current_mut {
            current_mut = &mut node.next;
        }
    }

    // RUST INSIGHT: `take()` leaves `None` at the midpoint, effectively severing the list.
    // `second_half` now owns the remainder of the nodes.
    let second_half = current_mut.take();

    // 3. Reverse the second half
    let mut prev = None;
    let mut current = second_half;
    while let Some(mut node) = current {
        let next = node.next.take();
        node.next = prev;
        prev = Some(node);
        current = next;
    }
    let mut second_half = prev;

    // 4. Merge the two halves
    let mut first_half = head;
    let mut dummy = Box::new(ListNode::new(0));
    let mut tail = &mut dummy;

    while first_half.is_some() || second_half.is_some() {
        if let Some(mut first_node) = first_half {
            first_half = first_node.next.take();
            tail.next = Some(first_node);
            tail = tail.next.as_mut().unwrap();
        }
        if let Some(mut second_node) = second_half {
            second_half = second_node.next.take();
            tail.next = Some(second_node);
            tail = tail.next.as_mut().unwrap();
        }
    }

    dummy.next
}

/// Main entry point - uses optimal solution
#[must_use]
pub fn reorder_list(head: Option<Box<ListNode>>) -> Option<Box<ListNode>> {
    reorder_list_optimal(head)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force() {
        let list = ListNode::from_vec(vec![1, 2, 3, 4]);
        let result = reorder_list_brute_force(list);
        assert_eq!(result.unwrap().to_vec(), vec![1, 4, 2, 3]);

        let list2 = ListNode::from_vec(vec![1, 2, 3, 4, 5]);
        let result2 = reorder_list_brute_force(list2);
        assert_eq!(result2.unwrap().to_vec(), vec![1, 5, 2, 4, 3]);
    }

    #[test]
    fn test_optimized() {
        let list = ListNode::from_vec(vec![1, 2, 3, 4]);
        let result = reorder_list_optimized(list);
        assert_eq!(result.unwrap().to_vec(), vec![1, 4, 2, 3]);
    }

    #[test]
    fn test_optimal() {
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
        let v = vec![1, 2, 3, 4, 5, 6, 7];

        let l1 = ListNode::from_vec(v.clone());
        let l2 = ListNode::from_vec(v.clone());
        let l3 = ListNode::from_vec(v);

        let res1 = reorder_list_brute_force(l1);
        let res2 = reorder_list_optimized(l2);
        let res3 = reorder_list_optimal(l3);

        assert_eq!(res1, res2);
        assert_eq!(res2, res3);
    }
}
