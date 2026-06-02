//! # 143. Reorder List
//!
//! You are given the head of a singly linked-list. The list can be represented as:
//!
//! L0 → L1 → … → Ln - 1 → Ln
//!
//! Reorder the list to be on the following form:
//!
//! L0 → Ln → L1 → Ln - 1 → L2 → Ln - 2 → …
//!
//! You may not modify the values in the list's nodes. Only nodes themselves may be changed.
//!
//! - Difficulty: Medium
//! - LeetCode: <https://leetcode.com/problems/reorder-list/>
//!
//! ## Why this matters in Rust
//! This problem is a brilliant exercise in Rust's ownership model, specifically manipulating
//! `Option<Box<ListNode>>`. Unlike in Python or C++ where you just casually assign pointer values,
//! Rust's borrow checker forces you to explicitly `Option::take()` nodes to temporarily claim ownership
//! of a subtree, avoiding aliasing rules. We will see how to carefully detach and re-attach nodes.
//!
//! ## Approach
//!
//! We explore two implementations:
//! 1.  **Brute Force (VecDeque)**: We can push all the nodes into a `VecDeque`. Then we can iteratively pop from the front and back to rebuild the reordered list. This requires O(N) auxiliary space.
//! 2.  **Optimal (In-place pointers)**: We find the middle of the list (using length counting), reverse the second half, and interleave the two halves. To satisfy Rust's strict mutability rules, we use length counting instead of the slow/fast pointer technique, as advancing two mutable pointers into the same linked list simultaneously requires `unsafe` or `Rc<RefCell<T>>`.

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

/// Brute Force approach: Collect to VecDeque and Rebuild
///
/// Time: O(N) - We iterate through the list to collect it, then rebuild it.
/// Space: O(N) - We allocate a `VecDeque` holding all nodes.
///
/// This approach avoids complex pointer manipulation by taking ownership of the list
/// into a data structure that supports fast removal from both ends.
#[allow(clippy::needless_pass_by_value)]
pub fn reorder_list_brute_force(head: &mut Option<Box<ListNode>>) {
    // RUST INSIGHT: We use `Option::take()` to completely extract the linked list
    // from the `head` mutable reference, leaving `None` behind. This gives us
    // full ownership to dismantle the list safely.
    let mut current = head.take();
    let mut deque = VecDeque::new();

    // Collect all nodes into the deque, severing their `next` pointers.
    while let Some(mut node) = current {
        current = node.next.take(); // Sever the connection
        deque.push_back(node);
    }

    if deque.is_empty() {
        return;
    }

    // A dummy node simplifies list reconstruction.
    let mut dummy = ListNode::new(0);
    let mut tail = &mut dummy;

    // Build the list by alternating front and back
    let mut flip = true;
    while !deque.is_empty() {
        let node = if flip {
            deque.pop_front().unwrap()
        } else {
            deque.pop_back().unwrap()
        };
        flip = !flip;

        tail.next = Some(node);
        tail = tail.next.as_mut().unwrap();
    }

    // Put the reconstructed list back into `head`
    *head = dummy.next;
}

/// Optimal approach: Length Counting, Reverse Half, Interleave
///
/// Time: O(N) - Multiple passes (count length, find middle, reverse, merge)
/// Space: O(1) - Only modifying pointers in place.
///
/// To modify a linked list in place without `Rc<RefCell>`, we can't easily use the
/// slow/fast pointer trick because it requires having two mutable references active
/// in the same sequence. Instead, we can count the nodes to find the middle.
#[allow(clippy::needless_pass_by_value)]
pub fn reorder_list_optimal(head: &mut Option<Box<ListNode>>) {
    if head.is_none() || head.as_ref().unwrap().next.is_none() {
        return;
    }

    // 1. Find the length of the list
    let mut len = 0;
    let mut curr = head.as_ref();
    while let Some(node) = curr {
        len += 1;
        curr = node.next.as_ref();
    }

    // 2. Split the list into two halves.
    // The first half has `(len + 1) / 2` nodes.
    let first_half_len = (len + 1) / 2;

    let mut curr_mut = head.as_mut();
    for _ in 0..first_half_len - 1 {
        // Safe to unwrap because we know `first_half_len - 1` is within bounds
        curr_mut = curr_mut.unwrap().next.as_mut();
    }

    // Split the list: take the second half
    // GOTCHA: `curr_mut.take()` would just take the reference, we need to take the `next` field
    // of the inner node to sever the list.
    let mut l2 = if let Some(node) = curr_mut {
        node.next.take()
    } else {
        None
    };

    // 3. Reverse the second half (l2)
    let mut prev = None;
    let mut current = l2;
    while let Some(mut node) = current {
        let next = node.next.take();
        node.next = prev;
        prev = Some(node);
        current = next;
    }
    l2 = prev; // l2 is now the reversed second half

    // 4. Interleave l1 (which is currently inside `head`) and l2
    // We must temporarily take ownership of `head` to build the new list.
    let mut l1 = head.take();
    let mut dummy = ListNode::new(0);
    let mut tail = &mut dummy;

    while l1.is_some() || l2.is_some() {
        if let Some(mut n1) = l1 {
            l1 = n1.next.take();
            tail.next = Some(n1);
            tail = tail.next.as_mut().unwrap();
        }

        if let Some(mut n2) = l2 {
            l2 = n2.next.take();
            tail.next = Some(n2);
            tail = tail.next.as_mut().unwrap();
        }
    }

    // Restore back to `head`
    *head = dummy.next;
}

/// Main entry point
pub fn reorder_list(head: &mut Option<Box<ListNode>>) {
    reorder_list_optimal(head);
}

// Alternative Approaches:
// 1. Recursive: One could use recursion to access the tail while the call stack holds the head pointers,
//    but this uses O(N) stack space, which could lead to stack overflows for large lists and isn't truly O(1) space.

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
    fn test_empty_and_single() {
        let mut empty = None;
        reorder_list(&mut empty);
        assert_eq!(empty, None);

        let mut single = ListNode::from_vec(vec![1]);
        reorder_list(&mut single);
        assert_eq!(single.unwrap().to_vec(), vec![1]);
    }

    #[test]
    fn test_two_nodes() {
        let mut list = ListNode::from_vec(vec![1, 2]);
        reorder_list(&mut list);
        assert_eq!(list.unwrap().to_vec(), vec![1, 2]);
    }
}
