//! # 21. Merge Two Sorted Lists
//!
//! You are given the heads of two sorted linked lists `list1` and `list2`.
//!
//! Merge the two lists in a one sorted list. The list should be made by splicing
//! together the nodes of the first two lists.
//!
//! Return the head of the merged linked list.
//!
//! This problem is a classic introduction to linked list manipulation, focusing on
//! pointer management and handling edge cases like empty lists. In Rust, it's an
//! excellent way to understand `Option<Box<T>>` and ownership transfer.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::linked_lists::merge_two_sorted_lists::{merge_two_lists, ListNode};
//!
//! let list1 = ListNode::from_vec(vec![1, 2, 4]);
//! let list2 = ListNode::from_vec(vec![1, 3, 4]);
//! let merged = merge_two_lists(list1, list2);
//! assert_eq!(merged.unwrap().to_vec(), vec![1, 1, 2, 3, 4, 4]);
//! ```
//!
//! ## Constraints
//!
//! - The number of nodes in both lists is in the range `[0, 50]`.
//! - `-100 <= Node.val <= 100`
//! - Both `list1` and `list2` are sorted in non-decreasing order.

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

    /// Helper to create a list from a vector (useful for tests and brute force)
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

/// Brute force approach: Collect to Vec, sort, rebuild
/// Time: O((N+M) log (N+M)) - sorting dominates
/// Space: O(N+M) - storing all elements in a vector
///
/// This approach completely ignores the existing sorted property of the lists.
/// It's "brute force" in the sense that it does the most obvious work without
/// leveraging the problem structure.
#[must_use]
pub fn merge_two_lists_brute_force(
    list1: Option<Box<ListNode>>,
    list2: Option<Box<ListNode>>,
) -> Option<Box<ListNode>> {
    let mut vec = Vec::new();

    // Helper to collect nodes
    let mut collect = |head: Option<Box<ListNode>>| {
        let mut current = head;
        while let Some(node) = current {
            vec.push(node.val);
            current = node.next;
        }
    };

    collect(list1);
    collect(list2);

    vec.sort_unstable();

    ListNode::from_vec(vec)
}

/// Optimized approach: Recursive merge
/// Time: O(N + M) - visit each node once
/// Space: O(N + M) - stack frames for recursion
///
/// This is the functional approach. It's elegant and readable.
///
/// RUST INSIGHT: We use `match` to handle the `Option` cases cleanly.
/// The ownership of nodes is moved into the result structure naturally.
///
/// GOTCHA: In languages without tail-call optimization (like Rust currently),
/// this can overflow the stack for very large lists. The constraint N <= 50
/// makes this safe here.
#[must_use]
pub fn merge_two_lists_optimized(
    list1: Option<Box<ListNode>>,
    list2: Option<Box<ListNode>>,
) -> Option<Box<ListNode>> {
    match (list1, list2) {
        (None, None) => None,
        (Some(l), None) => Some(l),
        (None, Some(r)) => Some(r),
        (Some(mut l), Some(mut r)) => {
            if l.val <= r.val {
                l.next = merge_two_lists_optimized(l.next, Some(r));
                Some(l)
            } else {
                r.next = merge_two_lists_optimized(Some(l), r.next);
                Some(r)
            }
        }
    }
}

/// Optimal approach: Iterative merge with dummy head
/// Time: O(N + M) - visit each node once
/// Space: O(1) - only a few pointers
///
/// This is the idiomatic systems programming approach. It uses constant stack space.
///
/// RUST INSIGHT: We use a `dummy` node to simplify handling the head of the list.
/// We use `&mut` references to build the list without taking ownership until the end.
/// `ref mut` in pattern matching helps us get a mutable reference to the `Box`.
#[must_use]
#[allow(clippy::missing_panics_doc)] // Unwraps are safe due to loop invariants
pub fn merge_two_lists_optimal(
    list1: Option<Box<ListNode>>,
    list2: Option<Box<ListNode>>,
) -> Option<Box<ListNode>> {
    let mut dummy = ListNode::new(0);
    let mut tail = &mut dummy;
    let mut l1 = list1;
    let mut l2 = list2;

    while l1.is_some() && l2.is_some() {
        let val1 = l1.as_ref().unwrap().val;
        let val2 = l2.as_ref().unwrap().val;

        if val1 <= val2 {
            // Take the head of l1
            let mut head = l1.take().unwrap();
            // Move l1 to next
            l1 = head.next.take();
            // Append head to tail
            tail.next = Some(head);
        } else {
            // Take the head of l2
            let mut head = l2.take().unwrap();
            // Move l2 to next
            l2 = head.next.take();
            // Append head to tail
            tail.next = Some(head);
        }
        // Advance tail
        tail = tail.next.as_mut().unwrap();
    }

    // Attach the remaining part
    if l1.is_some() {
        tail.next = l1;
    } else {
        tail.next = l2;
    }

    dummy.next
}

/// Main entry point - uses optimal solution
#[must_use]
pub fn merge_two_lists(
    list1: Option<Box<ListNode>>,
    list2: Option<Box<ListNode>>,
) -> Option<Box<ListNode>> {
    merge_two_lists_optimal(list1, list2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force_example_1() {
        let l1 = ListNode::from_vec(vec![1, 2, 4]);
        let l2 = ListNode::from_vec(vec![1, 3, 4]);
        let result = merge_two_lists_brute_force(l1, l2);
        assert_eq!(result.unwrap().to_vec(), vec![1, 1, 2, 3, 4, 4]);
    }

    #[test]
    fn test_optimized_example_1() {
        let l1 = ListNode::from_vec(vec![1, 2, 4]);
        let l2 = ListNode::from_vec(vec![1, 3, 4]);
        let result = merge_two_lists_optimized(l1, l2);
        assert_eq!(result.unwrap().to_vec(), vec![1, 1, 2, 3, 4, 4]);
    }

    #[test]
    fn test_optimal_example_1() {
        let l1 = ListNode::from_vec(vec![1, 2, 4]);
        let l2 = ListNode::from_vec(vec![1, 3, 4]);
        let result = merge_two_lists_optimal(l1, l2);
        assert_eq!(result.unwrap().to_vec(), vec![1, 1, 2, 3, 4, 4]);
    }

    #[test]
    fn test_empty_lists() {
        assert_eq!(merge_two_lists(None, None), None);
        let l1 = ListNode::from_vec(vec![1]);
        assert_eq!(merge_two_lists(l1, None).unwrap().to_vec(), vec![1]);
        let l2 = ListNode::from_vec(vec![1]);
        assert_eq!(merge_two_lists(None, l2).unwrap().to_vec(), vec![1]);
    }

    #[test]
    fn test_all_approaches_duplicates() {
        let v1 = vec![1, 1, 1];
        let v2 = vec![1, 1];

        let l1 = ListNode::from_vec(v1.clone());
        let l2 = ListNode::from_vec(v2.clone());
        let res_brute = merge_two_lists_brute_force(l1, l2);
        assert_eq!(res_brute.unwrap().to_vec(), vec![1, 1, 1, 1, 1]);

        let l1 = ListNode::from_vec(v1.clone());
        let l2 = ListNode::from_vec(v2.clone());
        let res_rec = merge_two_lists_optimized(l1, l2);
        assert_eq!(res_rec.unwrap().to_vec(), vec![1, 1, 1, 1, 1]);

        let l1 = ListNode::from_vec(v1.clone());
        let l2 = ListNode::from_vec(v2.clone());
        let res_opt = merge_two_lists_optimal(l1, l2);
        assert_eq!(res_opt.unwrap().to_vec(), vec![1, 1, 1, 1, 1]);
    }
}
