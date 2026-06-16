//! # 143. Reorder List
//!
//! Difficulty: Medium
//!
//! Link: <https://leetcode.com/problems/reorder-list/>
//!
//! Given the head of a singly linked-list. The list can be represented as:
//! L0 → L1 → … → Ln - 1 → Ln
//!
//! Reorder the list to be on the following form:
//! L0 → Ln → L1 → Ln - 1 → L2 → Ln - 2 → …
//!
//! You may not modify the values in the list's nodes. Only nodes themselves may be changed.
//!
//! This problem is a brilliant illustration of Rust's ownership model, specifically `Option<Box<ListNode>>`
//! manipulation, `Option::take()`, and borrowing rules during in-place list modification. It highlights why
//! safe Rust prevents simultaneous slow/fast mutable pointer traversal, steering us towards a more explicit
//! "two-pass length-counting" approach for safe, in-place mutation.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::linked_lists::reorder_list::{ListNode, reorder_list};
//!
//! // 1 -> 2 -> 3 -> 4
//! let mut head = Some(Box::new(ListNode::new(1)));
//! head.as_mut().unwrap().next = Some(Box::new(ListNode::new(2)));
//! head.as_mut().unwrap().next.as_mut().unwrap().next = Some(Box::new(ListNode::new(3)));
//! head.as_mut().unwrap().next.as_mut().unwrap().next.as_mut().unwrap().next = Some(Box::new(ListNode::new(4)));
//!
//! reorder_list(&mut head);
//!
//! assert_eq!(head.as_ref().unwrap().val, 1);
//! assert_eq!(head.as_ref().unwrap().next.as_ref().unwrap().val, 4);
//! assert_eq!(head.as_ref().unwrap().next.as_ref().unwrap().next.as_ref().unwrap().val, 2);
//! assert_eq!(head.as_ref().unwrap().next.as_ref().unwrap().next.as_ref().unwrap().next.as_ref().unwrap().val, 3);
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
    pub const fn new(val: i32) -> Self {
        ListNode { next: None, val }
    }
}

// =========================================================================================
// Approach 1: VecDeque Collection (Brute Force)
// =========================================================================================

/// Brute Force Approach: VecDeque Collection
///
/// Iterates through the list, taking ownership of the nodes and storing them in a `VecDeque`.
/// Then, pops nodes alternately from the front and back of the deque to rebuild the list.
///
/// Time: O(N) - One pass to collect, one pass to rebuild.
/// Space: O(N) - Storing all nodes in a `VecDeque`.
///
/// **Rust Insight:**
/// This sidesteps complex pointer manipulation entirely by leveraging Rust's ownership model
/// and standard collections. It completely severs the list into isolated owned nodes in the deque,
/// making it trivial to rebuild without borrow checker complaints.
pub fn reorder_list_brute_force(head: &mut Option<Box<ListNode>>) {
    if head.is_none() || head.as_ref().unwrap().next.is_none() {
        return;
    }

    // Take ownership of the list, leaving None in head temporarily
    let mut current = head.take();
    let mut deque = VecDeque::new();

    // Collect all nodes into the deque
    while let Some(mut node) = current {
        current = node.next.take();
        deque.push_back(node);
    }

    // Rebuild the list by popping front and back alternately
    let mut dummy = ListNode::new(0);
    let mut tail = &mut dummy;

    let mut take_front = true;
    while !deque.is_empty() {
        let node = if take_front {
            deque.pop_front().unwrap()
        } else {
            deque.pop_back().unwrap()
        };
        tail.next = Some(node);
        tail = tail.next.as_mut().unwrap();
        take_front = !take_front;
    }

    // Put the rebuilt list back into the original head
    *head = dummy.next;
}

// =========================================================================================
// Approach 2: Optimal In-Place Two-Pass Length-Counting (Optimal)
// =========================================================================================

/// Optimal Approach: Two-Pass Length-Counting In-Place
///
/// In languages like C++ or Java, the standard approach is to use slow/fast pointers to find the middle.
/// In safe Rust, mutable slow/fast pointers violate strict aliasing (multiple mutable references).
/// Therefore, the idiomatic safe Rust approach is:
/// 1. Count the length of the list (one pass).
/// 2. Traverse to the middle node using the count.
/// 3. Split the list into two halves.
/// 4. Reverse the second half.
/// 5. Merge the two halves alternately.
///
/// Time: O(N) - Two passes (one for length, one to split, reverse, and merge).
/// Space: O(1) - In-place pointer manipulation.
///
/// **Rust Insight:**
/// We use `Option::take()` to extract the list temporarily as an owned value to avoid borrow checker
/// conflicts when splitting and merging. The two-pass length counting avoids the need for simultaneous
/// mutable references.
pub fn reorder_list_optimal(head: &mut Option<Box<ListNode>>) {
    if head.is_none() || head.as_ref().unwrap().next.is_none() {
        return;
    }

    // Step 1: Count length
    let mut len = 0;
    let mut curr = head.as_ref();
    while let Some(node) = curr {
        len += 1;
        curr = node.next.as_ref();
    }

    let mid = (len + 1) / 2;

    // Step 2: Traverse to the split point
    let mut first_half = head.take();
    let mut curr_mut = &mut first_half;
    for _ in 0..mid {
        if let Some(node) = curr_mut {
            curr_mut = &mut node.next;
        }
    }

    // GOTCHA: `curr_mut.take()` merely takes the reference, not the underlying node.
    // To sever the list or extract the tail, explicitly take the `next` field.
    // We already traversed to the split point, so `curr_mut` points to the `next` link
    // that connects the first half to the second half.
    let second_half = curr_mut.take();

    // Step 3: Reverse the second half
    let reversed_second = reverse_list(second_half);

    // Step 4: Merge alternately
    let mut dummy = ListNode::new(0);
    let mut tail = &mut dummy;

    let mut first_ptr = first_half;
    let mut second_ptr = reversed_second;

    while first_ptr.is_some() || second_ptr.is_some() {
        if let Some(mut first_node) = first_ptr {
            first_ptr = first_node.next.take();
            tail.next = Some(first_node);
            tail = tail.next.as_mut().unwrap();
        }
        if let Some(mut second_node) = second_ptr {
            second_ptr = second_node.next.take();
            tail.next = Some(second_node);
            tail = tail.next.as_mut().unwrap();
        }
    }

    *head = dummy.next;
}

// Helper function to reverse a linked list in-place
fn reverse_list(head: Option<Box<ListNode>>) -> Option<Box<ListNode>> {
    let mut prev = None;
    let mut current = head;

    while let Some(mut node) = current {
        let next = node.next.take();
        node.next = prev;
        prev = Some(node);
        current = next;
    }

    prev
}

// =========================================================================================
// Main Entry Point
// =========================================================================================

pub fn reorder_list(head: &mut Option<Box<ListNode>>) {
    reorder_list_optimal(head);
}

// =========================================================================================
// Alternative Approaches
// =========================================================================================
// 1. **Unsafe Fast/Slow Pointers**: Using raw pointers to replicate the C++ approach.
//    Not idiomatic, as it circumvents Rust's safety guarantees just to avoid a simple counting pass.

#[cfg(test)]
mod tests {
    use super::*;

    fn to_list(vec: &[i32]) -> Option<Box<ListNode>> {
        let mut head = None;
        for &val in vec.iter().rev() {
            let mut node = Box::new(ListNode::new(val));
            node.next = head;
            head = Some(node);
        }
        head
    }

    fn to_vec(mut node: Option<Box<ListNode>>) -> Vec<i32> {
        let mut vec = Vec::new();
        while let Some(n) = node {
            vec.push(n.val);
            node = n.next;
        }
        vec
    }

    #[test]
    fn test_reorder_list_even() {
        let mut list = to_list(&[1, 2, 3, 4]);
        reorder_list(&mut list);
        assert_eq!(to_vec(list), vec![1, 4, 2, 3]);
    }

    #[test]
    fn test_reorder_list_odd() {
        let mut list = to_list(&[1, 2, 3, 4, 5]);
        reorder_list(&mut list);
        assert_eq!(to_vec(list), vec![1, 5, 2, 4, 3]);
    }

    #[test]
    fn test_reorder_list_single() {
        let mut list = to_list(&[1]);
        reorder_list(&mut list);
        assert_eq!(to_vec(list), vec![1]);
    }

    #[test]
    fn test_reorder_list_empty() {
        let mut list = None;
        reorder_list(&mut list);
        assert_eq!(to_vec(list), vec![]);
    }

    #[test]
    fn test_reorder_list_brute_force_consistency() {
        let mut list1 = to_list(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
        let mut list2 = to_list(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);

        reorder_list_brute_force(&mut list1);
        reorder_list_optimal(&mut list2);

        assert_eq!(to_vec(list1), to_vec(list2));
    }
}
