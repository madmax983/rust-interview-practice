//! # 2. Add Two Numbers
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/add-two-numbers/>
//!
//! You are given two **non-empty** linked lists representing two non-negative integers.
//! The digits are stored in **reverse order**, and each of their nodes contains a single digit.
//! Add the two numbers and return the sum as a linked list.
//!
//! You may assume the two numbers do not contain any leading zero, except the number 0 itself.
//!
//! This problem is a fundamental exercise in linked list manipulation and arbitrary-precision arithmetic.
//! In Rust, it highlights how to safely traverse and construct recursive data structures (`Option<Box<ListNode>>`)
//! while managing ownership and mutable references. It also teaches manual memory management patterns (like
//! dummy heads) that are idiomatic in systems programming.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::linked_lists::add_two_numbers::{add_two_numbers, ListNode};
//!
//! let l1 = ListNode::from_vec(vec![2, 4, 3]); // Represents 342
//! let l2 = ListNode::from_vec(vec![5, 6, 4]); // Represents 465
//! let result = add_two_numbers(l1, l2);
//! assert_eq!(result.unwrap().to_vec(), vec![7, 0, 8]); // Represents 807
//! ```
//!
//! ## Constraints
//!
//! - The number of nodes in each linked list is in the range `[1, 100]`.
//! - `0 <= Node.val <= 9`
//! - It is guaranteed that the list represents a number that does not have leading zeros.

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

    /// Helper to create a list from a vector (useful for tests and initialization)
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

    /// Helper to convert list to vector (useful for verification)
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

/// Brute Force Approach: Vector Arithmetic
///
/// Convert both linked lists to vectors, perform digit-by-digit addition with carry,
/// and then convert the result back to a linked list.
///
/// Time: O(N + M) - We traverse both lists once to build vectors, then iterate again to sum.
/// Space: O(N + M) - We allocate vectors to store the digits of both numbers.
///
/// Why this matters:
/// This approach separates data extraction from logic. It's easier to reason about
/// vector indexing than pointer manipulation, but it incurs extra allocation overhead.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn add_two_numbers_brute_force(
    l1: Option<Box<ListNode>>,
    l2: Option<Box<ListNode>>,
) -> Option<Box<ListNode>> {
    // Helper to convert list to vec
    let to_vec = |mut node: Option<Box<ListNode>>| -> Vec<i32> {
        let mut vec = Vec::new();
        while let Some(n) = node {
            vec.push(n.val);
            node = n.next;
        }
        vec
    };

    let v1 = to_vec(l1);
    let v2 = to_vec(l2);

    let mut result_vec = Vec::new();
    let mut carry = 0;
    let mut i = 0;
    let mut j = 0;

    // RUST INSIGHT: We can use `while let` or just loop with indices.
    // Since vectors are random access, indices work fine.
    // However, direct iteration is usually preferred if we don't need random access.
    // Here, we need to handle potentially different lengths, so indices are clear.

    while i < v1.len() || j < v2.len() || carry > 0 {
        let val1 = if i < v1.len() { v1[i] } else { 0 };
        let val2 = if j < v2.len() { v2[j] } else { 0 };

        let sum = val1 + val2 + carry;
        carry = sum / 10;
        result_vec.push(sum % 10);

        i += 1;
        j += 1;
    }

    // Convert result vector back to list
    ListNode::from_vec(result_vec)
}

/// Optimal Approach: Iterative In-Place Construction
///
/// Traverse both lists simultaneously, maintaining a carry, and construct the result list
/// node by node. We use a dummy head to simplify list construction.
///
/// Time: O(max(N, M)) - We iterate through the lists once.
/// Space: O(max(N, M)) - The result list itself (required output), plus O(1) auxiliary space.
///
/// RUST INSIGHT:
/// - We use `&Option<Box<ListNode>>` to traverse without taking ownership of the input lists.
/// - We build the new list using `Box::new` directly.
/// - A `dummy` head simplifies the logic by removing the need to special-case the first node.
/// - `tail` is a mutable reference `&mut ListNode` that moves as we append nodes.
///
/// GOTCHA:
/// Be careful when updating `tail`. Since `tail` borrows from `dummy` (or the previous node),
/// we need to ensure we don't violate borrow rules. We use `tail.next.as_mut().unwrap()`
/// to advance the mutable borrow safely.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
#[allow(clippy::missing_panics_doc)] // Safe because we just assigned `tail.next`
pub fn add_two_numbers_optimal(
    l1: Option<Box<ListNode>>,
    l2: Option<Box<ListNode>>,
) -> Option<Box<ListNode>> {
    let mut dummy = ListNode::new(0);
    let mut tail = &mut dummy;

    // We create local mutable references to the input lists so we can advance them.
    // Note: We don't need ownership of the input nodes, just references to them.
    // However, the function signature takes ownership `Option<Box<ListNode>>`.
    // So we can just move the owned boxes.
    let mut p1 = l1;
    let mut p2 = l2;
    let mut carry = 0;

    // RUST INSIGHT: `while let` is powerful, but here a simple `while` with boolean logic
    // is cleaner because we have three conditions (list1, list2, carry).
    while p1.is_some() || p2.is_some() || carry > 0 {
        let mut sum = carry;

        if let Some(node) = p1 {
            sum += node.val;
            p1 = node.next; // Move ownership of the rest of the list to p1
        }

        if let Some(node) = p2 {
            sum += node.val;
            p2 = node.next; // Move ownership of the rest of the list to p2
        }

        carry = sum / 10;

        // Append new node to result
        tail.next = Some(Box::new(ListNode::new(sum % 10)));
        tail = tail.next.as_mut().unwrap();
    }

    dummy.next
}

/// Main entry point
#[must_use]
pub fn add_two_numbers(
    l1: Option<Box<ListNode>>,
    l2: Option<Box<ListNode>>,
) -> Option<Box<ListNode>> {
    add_two_numbers_optimal(l1, l2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force_example() {
        let l1 = ListNode::from_vec(vec![2, 4, 3]);
        let l2 = ListNode::from_vec(vec![5, 6, 4]);
        let result = add_two_numbers_brute_force(l1, l2);
        assert_eq!(result.unwrap().to_vec(), vec![7, 0, 8]);
    }

    #[test]
    fn test_optimal_example() {
        let l1 = ListNode::from_vec(vec![2, 4, 3]);
        let l2 = ListNode::from_vec(vec![5, 6, 4]);
        let result = add_two_numbers_optimal(l1, l2);
        assert_eq!(result.unwrap().to_vec(), vec![7, 0, 8]);
    }

    #[test]
    fn test_zeros() {
        let l1 = ListNode::from_vec(vec![0]);
        let l2 = ListNode::from_vec(vec![0]);
        assert_eq!(add_two_numbers(l1, l2).unwrap().to_vec(), vec![0]);
    }

    #[test]
    fn test_different_lengths() {
        let l1 = ListNode::from_vec(vec![9, 9, 9, 9, 9, 9, 9]);
        let l2 = ListNode::from_vec(vec![9, 9, 9, 9]);
        // 9999999 + 9999 = 10009998
        // Reversed: [8, 9, 9, 9, 0, 0, 0, 1]
        let result = add_two_numbers(l1, l2);
        assert_eq!(result.unwrap().to_vec(), vec![8, 9, 9, 9, 0, 0, 0, 1]);
    }

    #[test]
    fn test_carry_at_end() {
        let l1 = ListNode::from_vec(vec![5]);
        let l2 = ListNode::from_vec(vec![5]);
        // 5 + 5 = 10 -> [0, 1]
        assert_eq!(add_two_numbers(l1, l2).unwrap().to_vec(), vec![0, 1]);
    }

    #[test]
    fn test_all_approaches_consistency() {
        let l1_data = vec![2, 4, 9]; // 942
        let l2_data = vec![5, 6, 4, 9]; // 9465
        // 942 + 9465 = 10407 -> [7, 0, 4, 0, 1]

        let l1 = ListNode::from_vec(l1_data.clone());
        let l2 = ListNode::from_vec(l2_data.clone());
        let res_brute = add_two_numbers_brute_force(l1, l2);

        let l1 = ListNode::from_vec(l1_data);
        let l2 = ListNode::from_vec(l2_data);
        let res_opt = add_two_numbers_optimal(l1, l2);

        assert_eq!(res_brute.unwrap().to_vec(), res_opt.unwrap().to_vec());
    }
}
