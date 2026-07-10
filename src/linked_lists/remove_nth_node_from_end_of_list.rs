//! # 19. Remove Nth Node From End of List
//!
//! Given the head of a linked list, remove the `n`-th node from the end of the list and return its head.
//!
//! This problem highlights the importance of the "two-pointer" technique and dealing with edge cases in linked lists,
//! specifically removing the head of the list. In Rust, it also challenges us to manage ownership and mutable references
//! correctly, especially when we need to modify a node deep in the list.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::linked_lists::remove_nth_node_from_end_of_list::{remove_nth_from_end, ListNode};
//!
//! let list = ListNode::from_vec(vec![1, 2, 3, 4, 5]);
//! let result = remove_nth_from_end(list, 2);
//! assert_eq!(result.unwrap().to_vec(), vec![1, 2, 3, 5]);
//! ```
//!
//! ## Constraints
//!
//! - The number of nodes in the list is `sz`.
//! - `1 <= sz <= 30`
//! - `0 <= Node.val <= 100`
//! - `1 <= n <= sz`

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

/// Brute force approach: Collect to Vec, remove, rebuild
/// Time: O(N) - Two passes (one to collect, one to rebuild)
/// Space: O(N) - Storing all elements in a vector
///
/// This approach simplifies the problem by converting the linked list into an array-like structure
/// where random access is O(1). We calculate the index to remove (`len - n`) and then rebuild the list.
///
/// GOTCHA: Be careful with 0-based indexing. The problem says "n-th from end", so if length is 5 and n is 2,
/// we remove the node at index 3 (5-2).
#[must_use]
#[allow(clippy::cast_sign_loss)] // n is guaranteed positive by constraints/checks
pub fn remove_nth_from_end_brute_force(
    head: Option<Box<ListNode>>,
    n: i32,
) -> Option<Box<ListNode>> {
    let mut vec = Vec::new();
    let mut current = head;

    // Collect values
    while let Some(node) = current {
        vec.push(node.val);
        current = node.next;
    }

    // Remove the element
    let len = vec.len();
    if n > 0 && (n as usize) <= len {
        let index_to_remove = len - (n as usize);
        vec.remove(index_to_remove);
    }

    // Rebuild list
    ListNode::from_vec(vec)
}

/// Optimal approach: iterative two passes (calculate length, then traverse)
/// Time: O(N) - Two passes (one to count, one to remove)
/// Space: O(1) - Constant extra space
///
/// This approach avoids the O(N) space of the vector by first calculating the length of the list,
/// then traversing again to the node *before* the one we want to remove.
///
/// RUST INSIGHT: We use a "dummy" node (sentinel) to handle the edge case where we need to remove
/// the head of the list. Without a dummy node, we'd need a special check `if index_to_remove == 0`.
#[must_use]
#[allow(clippy::cast_sign_loss)] // n is guaranteed positive
#[allow(clippy::missing_panics_doc)] // Unwraps are safe due to loop invariants
pub fn remove_nth_from_end_optimal(head: Option<Box<ListNode>>, n: i32) -> Option<Box<ListNode>> {
    // Pass 1: Calculate length
    let mut len = 0;
    let mut current = &head;
    while let Some(node) = current {
        len += 1;
        current = &node.next;
    }

    let index_to_remove = len - n;

    // Create a dummy node pointing to head
    let mut dummy = Box::new(ListNode { val: 0, next: head });
    let mut current = &mut dummy;

    // Pass 2: Traverse to the node *before* the one to remove
    for _ in 0..index_to_remove {
        // We know this unwrap is safe because index_to_remove < len
        current = current.next.as_mut().unwrap();
    }

    // Remove the node
    // current is now the node before the target.
    // We want: current.next = current.next.next
    let target = current.next.take();
    if let Some(node) = target {
        current.next = node.next;
    }

    dummy.next
}

/// Optimized approach: recursive one pass
/// Time: O(N) - Visit each node once
/// Space: O(N) - Stack recursion depth
///
/// This approach traverses to the end, and then counts up from the end as the recursion unwinds.
/// When the count equals `n`, we remove the node. This is conceptually "one pass" but uses O(N) stack space.
///
/// RUST INSIGHT: This demonstrates how recursion can simplify state management (the "index from end" logic)
/// at the cost of stack space.
#[must_use]
#[allow(clippy::option_if_let_else)] // Explicit match is cleaner for recursion here
pub fn remove_nth_from_end_optimized(head: Option<Box<ListNode>>, n: i32) -> Option<Box<ListNode>> {
    fn helper(node: Option<Box<ListNode>>, n: i32) -> (Option<Box<ListNode>>, i32) {
        if let Some(mut boxed_node) = node {
            let (next_node, index) = helper(boxed_node.next.take(), n);
            boxed_node.next = next_node;
            if index == n {
                // Remove current node, return its next
                (boxed_node.next, index + 1)
            } else {
                // Keep current node
                (Some(boxed_node), index + 1)
            }
        } else {
            (None, 1) // 1-based index from end
        }
    }

    helper(head, n).0
}

/// Main entry point - uses the optimal iterative two-pass solution (O(1) space, more robust than recursion).
#[must_use]
pub fn remove_nth_from_end(head: Option<Box<ListNode>>, n: i32) -> Option<Box<ListNode>> {
    remove_nth_from_end_optimal(head, n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force_example_1() {
        let list = ListNode::from_vec(vec![1, 2, 3, 4, 5]);
        let result = remove_nth_from_end_brute_force(list, 2);
        assert_eq!(result.unwrap().to_vec(), vec![1, 2, 3, 5]);
    }

    #[test]
    fn test_optimal_example_1() {
        let list = ListNode::from_vec(vec![1, 2, 3, 4, 5]);
        let result = remove_nth_from_end_optimal(list, 2);
        assert_eq!(result.unwrap().to_vec(), vec![1, 2, 3, 5]);
    }

    #[test]
    fn test_optimized_example_1() {
        let list = ListNode::from_vec(vec![1, 2, 3, 4, 5]);
        let result = remove_nth_from_end_optimized(list, 2);
        assert_eq!(result.unwrap().to_vec(), vec![1, 2, 3, 5]);
    }

    #[test]
    fn test_remove_head() {
        let list = ListNode::from_vec(vec![1, 2]);
        let result = remove_nth_from_end(list, 2); // Remove 2nd from end (which is head: 1)
        assert_eq!(result.unwrap().to_vec(), vec![2]);
    }

    #[test]
    fn test_remove_tail() {
        let list = ListNode::from_vec(vec![1, 2]);
        let result = remove_nth_from_end(list, 1); // Remove 1st from end (which is tail: 2)
        assert_eq!(result.unwrap().to_vec(), vec![1]);
    }

    #[test]
    fn test_single_node() {
        let list = ListNode::from_vec(vec![1]);
        let result = remove_nth_from_end(list, 1);
        assert_eq!(result, None);
    }

    #[test]
    fn test_all_approaches_consistency() {
        let v = vec![1, 2, 3, 4, 5, 6, 7];
        let n = 3;

        let res1 = remove_nth_from_end_brute_force(ListNode::from_vec(v.clone()), n);
        let res2 = remove_nth_from_end_optimized(ListNode::from_vec(v.clone()), n);
        let res3 = remove_nth_from_end_optimal(ListNode::from_vec(v), n);

        assert_eq!(res1, res2);
        assert_eq!(res2, res3);
    }
}
