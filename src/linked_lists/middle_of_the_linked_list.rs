//! # 876. Middle of the Linked List
//!
//! Difficulty: Easy
//!
//! Link: <https://leetcode.com/problems/middle-of-the-linked-list/>
//!
//! Given the `head` of a singly linked list, return the middle node of the linked list.
//! If there are two middle nodes, return the second middle node.
//!
//! This problem is an excellent case study in Rust's ownership and borrowing model. It demonstrates
//! the "Fast and Slow Pointer" technique (Tortoise and Hare), highlighting how to safely maintain
//! two independent, read-only references traversing the same data structure without violating
//! the borrow checker's rules.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::linked_lists::middle_of_the_linked_list::{ListNode, middle_node};
//!
//! // 1 -> 2 -> 3 -> 4 -> 5
//! let mut head = Some(Box::new(ListNode::new(1)));
//! head.as_mut().unwrap().next = Some(Box::new(ListNode::new(2)));
//! head.as_mut().unwrap().next.as_mut().unwrap().next = Some(Box::new(ListNode::new(3)));
//!
//! let middle = middle_node(head);
//! assert_eq!(middle.unwrap().val, 2);
//! ```

// =========================================================================================
// Definition for singly-linked list.
// =========================================================================================

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
}

// =========================================================================================
// Approach 1: Vector Collection (Brute Force)
// =========================================================================================

/// Brute Force Approach: Vector Collection
///
/// Iterates through the entire linked list and stores a clone of each node's value (or the node itself)
/// into a vector. Then, accesses the middle element directly using index `len / 2`.
///
/// Time: O(N) - Single pass through the list.
/// Space: O(N) - We store all elements in a `Vec`.
///
/// **Rust Insight:**
/// While easy to write, this approach fundamentally subverts the linked list by turning it into
/// an array, incurring an O(N) heap allocation overhead.
#[must_use]
pub fn middle_node_brute_force(mut head: Option<Box<ListNode>>) -> Option<Box<ListNode>> {
    let mut nodes = Vec::new();

    // Consume the list to build the vector
    while let Some(node) = head {
        nodes.push(node.val);
        head = node.next;
    }

    // Reconstruct the remaining list from the middle point.
    // GOTCHA: Since LeetCode requires returning the actual sublist,
    // collecting into a Vec consumes the original list. We either have to rebuild it,
    // or we must collect references `&Option<Box<ListNode>>` which creates lifetime complexities
    // when returning owned values. Here we rebuild for simplicity.
    if nodes.is_empty() {
        return None;
    }

    let mid = nodes.len() / 2;
    let mut dummy = Box::new(ListNode::new(0));
    let mut current = &mut dummy;

    for &val in nodes.iter().skip(mid) {
        current.next = Some(Box::new(ListNode::new(val)));
        current = current.next.as_mut().unwrap();
    }

    dummy.next
}

// =========================================================================================
// Approach 2: Fast and Slow Pointers (Optimal)
// =========================================================================================

/// Optimal Approach: Fast and Slow Pointers (Tortoise and Hare)
///
/// We use two pointers: a `slow` pointer that advances one node at a time, and a `fast` pointer
/// that advances two nodes at a time. When the `fast` pointer reaches the end, the `slow` pointer
/// will be precisely at the middle.
///
/// Time: O(N) - Single pass through the list.
/// Space: O(1) - Only two references are maintained regardless of list size.
///
/// **Rust Insight:**
/// In Rust, maintaining two mutable references (`&mut`) to the same data structure is forbidden.
/// However, multiple immutable references (`&`) are perfectly fine. By taking `&head`, we can
/// traverse the list with both pointers simultaneously. Since the function signature requires
/// returning an owned `Option<Box<ListNode>>`, we use `.clone()` on the `slow` reference at the end.
#[must_use]
#[allow(clippy::missing_panics_doc)] // Unwrap is safe due to loop invariants
pub fn middle_node_optimal(head: Option<Box<ListNode>>) -> Option<Box<ListNode>> {
    // RUST INSIGHT: We take immutable references to the Option wrapper itself.
    // This allows both `slow` and `fast` to safely inspect the linked list simultaneously.
    let mut slow = &head;
    let mut fast = &head;

    // Advance `fast` by two steps, and `slow` by one step.
    // We use a `while let` loop to elegantly handle the nested Option unwrapping.
    while let Some(fast_node) = fast {
        if let Some(fast_next) = &fast_node.next {
            // fast advanced 1, can safely advance 2
            fast = &fast_next.next;
            // slow advances 1
            slow = &slow.as_ref().unwrap().next;
        } else {
            // fast is at the last node, cannot advance 2
            break;
        }
    }

    // GOTCHA: Our pointers are references (`&Option<Box<ListNode>>`).
    // The function signature demands an owned `Option<Box<ListNode>>`.
    // Because `Box<T>` owns its data, we must clone the remaining sublist.
    // If the signature was `&Option<Box<ListNode>>`, this clone would be a zero-cost abstraction.
    slow.clone()
}

/// Main entry point - uses the optimal fast/slow pointer solution.
#[must_use]
pub fn middle_node(head: Option<Box<ListNode>>) -> Option<Box<ListNode>> {
    middle_node_optimal(head)
}

// =========================================================================================
// Alternative Approaches
// =========================================================================================
// 1. **Two Pass Count**: Iterate once to count nodes (N). Iterate again to node N/2.
//    Time: O(N) (2 passes), Space: O(1).
//    In many scenarios without memory cache considerations, this is just as fast as the two-pointer approach,
//    and avoids evaluating two references at once.

#[cfg(test)]
mod tests {
    use super::*;

    // Helper function to create a linked list from a slice
    fn to_list(vec: &[i32]) -> Option<Box<ListNode>> {
        let mut head = None;
        for &val in vec.iter().rev() {
            let mut node = Box::new(ListNode::new(val));
            node.next = head;
            head = Some(node);
        }
        head
    }

    // Helper function to convert a linked list back to a Vec
    fn to_vec(mut node: Option<Box<ListNode>>) -> Vec<i32> {
        let mut vec = Vec::new();
        while let Some(n) = node {
            vec.push(n.val);
            node = n.next;
        }
        vec
    }

    #[test]
    fn test_middle_node_odd() {
        let list = to_list(&[1, 2, 3, 4, 5]);
        let expected = vec![3, 4, 5];

        assert_eq!(to_vec(middle_node_optimal(list.clone())), expected);
        assert_eq!(to_vec(middle_node_brute_force(list)), expected);
    }

    #[test]
    fn test_middle_node_even() {
        let list = to_list(&[1, 2, 3, 4, 5, 6]);
        // When even, it should return the second middle node (4)
        let expected = vec![4, 5, 6];

        assert_eq!(to_vec(middle_node_optimal(list.clone())), expected);
        assert_eq!(to_vec(middle_node_brute_force(list)), expected);
    }

    #[test]
    fn test_middle_node_single_element() {
        let list = to_list(&[1]);
        let expected = vec![1];

        assert_eq!(to_vec(middle_node_optimal(list.clone())), expected);
        assert_eq!(to_vec(middle_node_brute_force(list)), expected);
    }

    #[test]
    fn test_middle_node_two_elements() {
        let list = to_list(&[1, 2]);
        let expected = vec![2];

        assert_eq!(to_vec(middle_node_optimal(list.clone())), expected);
        assert_eq!(to_vec(middle_node_brute_force(list)), expected);
    }

    #[test]
    fn test_middle_node_empty() {
        // Wrapper (optimal) and brute force both handle the empty list.
        assert_eq!(middle_node(None), None);
        assert_eq!(middle_node_brute_force(None), None);
    }

    #[test]
    fn test_both_approaches_agree() {
        for len in 1..=8 {
            let list: Vec<i32> = (0..len).collect();
            let optimal = middle_node_optimal(to_list(&list));
            let brute = middle_node_brute_force(to_list(&list));
            assert_eq!(to_vec(optimal), to_vec(brute));
        }
    }
}
