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
//! This problem perfectly demonstrates how to combine three fundamental linked list operations:
//! finding the middle (tortoise and hare), reversing a linked list, and merging two linked lists.
//! In Rust, it's an excellent exercise in carefully managing ownership (`Option<Box<ListNode>>`)
//! and mutable references when splitting and weaving a recursive data structure.
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
//!
//! ## Constraints
//!
//! - The number of nodes in the list is in the range `[1, 5 * 10^4]`.
//! - `1 <= Node.val <= 1000`

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

/// Brute force approach: Collect to Vec, build new list
///
/// Time: O(N) - One pass to collect, one pass to rebuild.
/// Space: O(N) - Stores all elements in a vector.
///
/// This avoids pointer manipulation by taking advantage of standard library utilities.
/// We extract the values into a vector, and then rebuild the list using two pointers
/// (start and end) moving towards the middle.
#[allow(clippy::ptr_arg)]
pub fn reorder_list_brute_force(head: &mut Option<Box<ListNode>>) {
    if head.is_none() {
        return;
    }

    // Step 1: Collect values
    let mut vec = Vec::new();
    let mut current = head.take();
    while let Some(node) = current {
        vec.push(node.val);
        current = node.next;
    }

    // Step 2: Rebuild list
    let len = vec.len();
    let mut result_vec = Vec::with_capacity(len);
    let mut left = 0;
    let mut right = len - 1;

    while left < right {
        result_vec.push(vec[left]);
        result_vec.push(vec[right]);
        left += 1;
        right -= 1;
    }

    // If length is odd, the middle element is left
    if left == right {
        result_vec.push(vec[left]);
    }

    *head = ListNode::from_vec(result_vec);
}

/// Optimal approach: In-place pointer manipulation
///
/// Time: O(N)
/// Space: O(1)
///
/// # Algorithm
/// 1. Find the middle of the list using slow/fast pointers.
/// 2. Split the list into two halves and reverse the second half.
/// 3. Merge the two halves node-by-node.
///
/// # Rust Insight
/// We use `Option::take()` to temporarily steal ownership of nodes while we re-wire their
/// `next` pointers, thus satisfying the borrow checker without needing `unsafe` or multiple
/// mutable references. We must handle the case where the problem signature takes a `&mut Option<Box<ListNode>>`
/// instead of taking ownership directly, by using `.take()` on the head to gain full ownership
/// during the transformation, then assigning the result back.
///
/// # Gotcha
/// When finding the middle, ensure the first half terminates properly (i.e., its tail points to None).
/// Otherwise, you might create a cycle or infinitely loop during the merge step.
///
/// # Panics
///
/// Does not panic in practice: `unwrap` calls are safe because they are only executed when
/// the presence of a node is guaranteed by prior checks or loop invariants.
#[allow(clippy::ptr_arg)] // Standard LeetCode signature uses &mut
pub fn reorder_list_optimal(head: &mut Option<Box<ListNode>>) {
    if head.is_none() || head.as_ref().unwrap().next.is_none() {
        return;
    }

    // Step 1: Find the middle
    let mut len = 0;
    let mut current = head.as_ref();
    while let Some(node) = current {
        len += 1;
        current = node.next.as_ref();
    }

    // For length N, the first half gets ceil(N/2) nodes.
    let mid_idx = (len + 1) / 2;

    // Take ownership of the list to perform the split and merge
    let mut first_half = head.take();

    // Advance a mutable reference to the end of the first half
    let mut current_mut = first_half.as_mut();
    for _ in 1..mid_idx {
        if let Some(node) = current_mut {
            current_mut = node.next.as_mut();
        }
    }

    // Split the list
    let second_half = current_mut.unwrap().next.take();

    // Step 2: Reverse the second half
    let mut reversed_second = None;
    let mut current = second_half;
    while let Some(mut node) = current {
        let next = node.next.take();
        node.next = reversed_second;
        reversed_second = Some(node);
        current = next;
    }

    // Step 3: Merge the two halves
    let mut dummy = ListNode::new(0);
    let mut tail = &mut dummy;

    let mut l1 = first_half;
    let mut l2 = reversed_second;

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

    // Assign the merged result back to head
    *head = dummy.next;
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
    fn test_all_approaches_consistency() {
        let data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9];

        let mut list1 = ListNode::from_vec(data.clone());
        reorder_list_brute_force(&mut list1);

        let mut list2 = ListNode::from_vec(data);
        reorder_list_optimal(&mut list2);

        assert_eq!(list1.unwrap().to_vec(), list2.unwrap().to_vec());
    }
}
