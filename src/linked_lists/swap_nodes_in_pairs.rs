//! # 24. Swap Nodes in Pairs
//!
//! Difficulty: Medium
//! Link: https://leetcode.com/problems/swap-nodes-in-pairs/
//!
//! Given a linked list, swap every two adjacent nodes and return its head. You must solve the problem
//! without modifying the values in the list's nodes (i.e., only nodes themselves may be changed.)
//!
//! ## Why this matters in Rust
//! This problem perfectly demonstrates the fundamental challenges and patterns of working with singly
//! linked lists in safe Rust. In languages like C++ or Java, swapping nodes is trivial pointer manipulation.
//! In Rust, however, `Option<Box<ListNode>>` implies strict, exclusive ownership. You cannot easily hold
//! multiple mutable aliases to different parts of the list. Thus, you are forced to creatively use `Option::take()`
//! and explicit ownership transfer to decouple and re-couple nodes while satisfying the borrow checker.

/// Definition for singly-linked list.
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
// Approach 1: Recursive (Elegant, but O(N) Space)
// =========================================================================================

/// Recursive Approach:
///
/// Recursion handles the state implicitly on the call stack. Instead of managing a complex chain
/// of pointers iteratively, we split the problem: swap the first two nodes, and then recursively
/// solve for the rest of the list.
///
/// Time: O(N) - visits each node once.
/// Space: O(N) - recursion stack depth could be N/2.
///
/// # Idiomatic Rust vs C++/Java
/// In C++/Java, recursion is fine but often an iterative pointer approach is preferred for O(1) space.
/// In Rust, recursion simplifies the problem dramatically because we don't need to juggle mutable
/// borrows across loop iterations. We just move ownership back and forth.
#[must_use]
pub fn swap_pairs_recursive(head: Option<Box<ListNode>>) -> Option<Box<ListNode>> {
    // RUST INSIGHT: We use pattern matching to elegantly destructure the first two nodes.
    // If we have at least two nodes...
    let mut node1 = head?; // equivalent to: if let Some(mut n1) = head { ... } else { return None; }

    if let Some(mut node2) = node1.next.take() {
        // We have a second node. We took it out using `.take()`, so `node1.next` is now None.

        // Recursively solve the rest of the list.
        // `node2.next` is taken out, moving ownership to the recursive call.
        let rest = swap_pairs_recursive(node2.next.take());

        // Re-link the nodes in swapped order
        node1.next = rest;
        node2.next = Some(node1);

        // node2 is the new head of this segment
        Some(node2)
    } else {
        // If there is only one node, just return it as is.
        Some(node1)
    }
}

// =========================================================================================
// Approach 2: Iterative (Optimal, O(1) Space)
// =========================================================================================

/// Iterative Approach:
///
/// To achieve O(1) space complexity, we use an iterative approach with a dummy node.
/// This avoids recursion stack overhead.
///
/// Time: O(N)
/// Space: O(1)
///
/// # Gotcha
/// In Rust, manipulating linked list pointers iteratively means you have to maintain a mutable reference
/// (`&mut Option<Box<ListNode>>`) that traverses down the list. Getting the lifetimes right for this
/// traversing pointer without running into "cannot borrow as mutable more than once" errors is the core challenge.
#[must_use]
pub fn swap_pairs_optimal(mut head: Option<Box<ListNode>>) -> Option<Box<ListNode>> {
    let mut dummy = Some(Box::new(ListNode { val: 0, next: None }));
    // We maintain a mutable reference to the "tail" of our newly formed list.
    let mut current = &mut dummy;

    // Loop as long as we can extract two nodes from the original list
    while let Some(mut node1) = head.take() {
        if let Some(mut node2) = node1.next.take() {
            // We have a pair!
            // node2 is the second node, node1 is the first node.

            // Advance `head` to the rest of the list
            head = node2.next.take();

            // Swap them: node2 points to node1
            node1.next = None;
            node2.next = Some(node1);

            // Attach the swapped pair to our new list
            if let Some(c) = current {
                c.next = Some(node2);
                // Advance current by 2 steps to the end of the newly attached pair
                // RUST INSIGHT: We re-borrow `current` to point to the end of the chain.
                // We unwrap safely because we just explicitly set c.next and node2.next (which is node1).
                current = &mut c.next.as_mut().unwrap().next;
            }
        } else {
            // Only one node left, no pair to swap.
            if let Some(c) = current {
                c.next = Some(node1);
            }
            break;
        }
    }

    // Dummy node's next points to the actual head of our swapped list
    dummy.unwrap().next
}

// =========================================================================================
// Main Wrapper
// =========================================================================================

/// Main entry point - uses optimal iterative solution
#[must_use]
pub fn swap_pairs(head: Option<Box<ListNode>>) -> Option<Box<ListNode>> {
    swap_pairs_optimal(head)
}

// =========================================================================================
// Alternative Approaches
// =========================================================================================
// 1. **Vec Transformation**: Convert the linked list to a `Vec`, swap pairs using slice `swap`,
//    and then rebuild the linked list. Easy but requires O(N) auxiliary space for the Vec,
//    defeating the purpose of an in-place linked list problem.

#[cfg(test)]
mod tests {
    use super::*;

    // Helper function to build a list from a slice
    fn build_list(vals: &[i32]) -> Option<Box<ListNode>> {
        let mut head = None;
        for &val in vals.iter().rev() {
            let mut node = Box::new(ListNode::new(val));
            node.next = head;
            head = Some(node);
        }
        head
    }

    // Helper function to convert a list back to a vec for easy assertions
    fn to_vec(mut head: Option<Box<ListNode>>) -> Vec<i32> {
        let mut res = Vec::new();
        while let Some(node) = head {
            res.push(node.val);
            head = node.next;
        }
        res
    }

    #[test]
    fn test_happy_path() {
        let input = build_list(&[1, 2, 3, 4]);
        let expected = vec![2, 1, 4, 3];

        let res_recursive = swap_pairs_recursive(input.clone());
        assert_eq!(to_vec(res_recursive), expected);

        let res_optimal = swap_pairs_optimal(input);
        assert_eq!(to_vec(res_optimal), expected);
    }

    #[test]
    fn test_edge_case_empty() {
        let input = build_list(&[]);
        let expected: Vec<i32> = vec![];

        let res_recursive = swap_pairs_recursive(input.clone());
        assert_eq!(to_vec(res_recursive), expected);

        let res_optimal = swap_pairs_optimal(input);
        assert_eq!(to_vec(res_optimal), expected);
    }

    #[test]
    fn test_edge_case_single() {
        let input = build_list(&[1]);
        let expected = vec![1];

        let res_recursive = swap_pairs_recursive(input.clone());
        assert_eq!(to_vec(res_recursive), expected);

        let res_optimal = swap_pairs_optimal(input);
        assert_eq!(to_vec(res_optimal), expected);
    }

    #[test]
    fn test_stress_test_odd_even() {
        let input = build_list(&[1, 2, 3, 4, 5]);
        let expected = vec![2, 1, 4, 3, 5]; // 5 remains in place

        let res_recursive = swap_pairs_recursive(input.clone());
        assert_eq!(to_vec(res_recursive), expected);

        let res_optimal = swap_pairs_optimal(input);
        assert_eq!(to_vec(res_optimal), expected);
    }
}
