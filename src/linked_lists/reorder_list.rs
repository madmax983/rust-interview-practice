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
//! This problem in Rust is notoriously difficult because it typically requires mutating pointers
//! in different parts of a linked list simultaneously. Rust's strict aliasing rules make
//! simultaneous slow/fast mutable pointer traversal complex. The idiomatic approach involves
//! breaking the problem down into distinct, sequentially safe operations: finding the middle,
//! splitting the list, reversing the second half, and merging them.
//!
//! ## Examples
//!
//! ```
//! // Omitted for brevity in docstrings due to boilerplate, see tests below.
//! ```
//!
//! ## Constraints
//!
//! - The number of nodes in the list is in the range `[1, 5 * 10^4]`.
//! - `1 <= Node.val <= 1000`

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

    /// Helper to create a list from a vector (useful for tests)
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

/// Brute force / VecDeque approach: Collect nodes and rebuild.
/// Time: O(N)
/// Space: O(N) auxiliary space.
///
/// While not "in-place" strictly, this is a very common approach in languages
/// with less strict pointer rules. In Rust, taking ownership of nodes into a `VecDeque`
/// and rebuilding the list is remarkably clean and safe.
#[allow(clippy::needless_pass_by_value)]
pub fn reorder_list_brute_force(head: &mut Option<Box<ListNode>>) {
    if head.is_none() || head.as_ref().unwrap().next.is_none() {
        return;
    }

    // Take ownership of the list
    let mut current = head.take();
    let mut deque = std::collections::VecDeque::new();

    // Collect all nodes into a double-ended queue
    while let Some(mut node) = current {
        current = node.next.take();
        deque.push_back(node);
    }

    // Rebuild the list taking from front and back
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

    *head = dummy.next;
}

/// Optimized in-place approach: Count length, split, reverse, merge.
/// Time: O(N) - two passes.
/// Space: O(1) - constant auxiliary space.
///
/// Finding the middle using the slow/fast pointer technique is hard in Rust
/// if you want to mutate (split) the list at the middle node, because you can't
/// easily keep a mutable reference to the middle while traversing with another mutable
/// pointer.
///
/// Instead, we count the length, then traverse to the middle to split the list.
///
/// RUST INSIGHT: Notice how we use `take()` to sever the list. We have to explicitly
/// take the `next` field of the node to split the list safely.
#[allow(clippy::needless_pass_by_value)]
pub fn reorder_list_optimized(head: &mut Option<Box<ListNode>>) {
    if head.is_none() || head.as_ref().unwrap().next.is_none() {
        return;
    }

    // 1. Count the length
    let mut len = 0;
    let mut curr = head.as_ref();
    while let Some(node) = curr {
        len += 1;
        curr = node.next.as_ref();
    }

    // 2. Find the middle and split
    let mid = len / 2;
    let mut curr_mut = head.as_mut();
    for _ in 0..mid {
        if let Some(node) = curr_mut {
            curr_mut = node.next.as_mut();
        }
    }

    // Sever the list and get the second half
    let mut second_half = if let Some(node) = curr_mut {
        node.next.take()
    } else {
        None
    };

    // 3. Reverse the second half
    let mut prev = None;
    let mut curr = second_half;
    while let Some(mut node) = curr {
        let next = node.next.take();
        node.next = prev;
        prev = Some(node);
        curr = next;
    }
    second_half = prev;

    // 4. Merge the two halves
    // We build the merged list into a dummy node to avoid complex borrow matching
    let mut dummy = ListNode::new(0);
    let mut tail = &mut dummy;

    let mut l1 = head.take();
    let mut l2 = second_half;

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

    *head = dummy.next;
}

/// Main entry point
pub fn reorder_list(head: &mut Option<Box<ListNode>>) {
    reorder_list_optimized(head);
}

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
    fn test_brute_force_odd() {
        let mut head = ListNode::from_vec(vec![1, 2, 3, 4, 5]);
        reorder_list_brute_force(&mut head);
        assert_eq!(head.unwrap().to_vec(), vec![1, 5, 2, 4, 3]);
    }

    #[test]
    fn test_optimized_even() {
        let mut head = ListNode::from_vec(vec![1, 2, 3, 4]);
        reorder_list_optimized(&mut head);
        assert_eq!(head.unwrap().to_vec(), vec![1, 4, 2, 3]);
    }

    #[test]
    fn test_optimized_odd() {
        let mut head = ListNode::from_vec(vec![1, 2, 3, 4, 5]);
        reorder_list_optimized(&mut head);
        assert_eq!(head.unwrap().to_vec(), vec![1, 5, 2, 4, 3]);
    }

    #[test]
    fn test_edge_cases() {
        let mut head = None;
        reorder_list(&mut head);
        assert_eq!(head, None);

        let mut head = ListNode::from_vec(vec![1]);
        reorder_list(&mut head);
        assert_eq!(head.unwrap().to_vec(), vec![1]);

        let mut head = ListNode::from_vec(vec![1, 2]);
        reorder_list(&mut head);
        assert_eq!(head.unwrap().to_vec(), vec![1, 2]);
    }
}
