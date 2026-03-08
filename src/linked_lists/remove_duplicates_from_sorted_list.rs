//! # 83. Remove Duplicates from Sorted List
//!
//! Difficulty: Easy
//! Link: <https://leetcode.com/problems/remove-duplicates-from-sorted-list/>
//!
//! Given the head of a sorted linked list, delete all duplicates such that each element appears only once.
//! Return the linked list sorted as well.
//!
//! ## Why this matters in Rust
//! This problem perfectly demonstrates how to navigate and modify a linked list concurrently. In Rust,
//! mutably traversing a recursive data structure like `Option<Box<ListNode>>` requires precise lifetime
//! management. Specifically, it highlights the use of `as_mut()` to get a mutable reference inside an `Option`,
//! and `unwrap()` or `take()` to bypass borrow checker restrictions when modifying adjacent nodes.
//!
//! ## Approaches
//!
//! ### Approach 1: Brute Force (Collect and Rebuild)
//! Similar to reverse linked list, we can sidestep pointer manipulation by collecting all values into a `Vec`,
//! deduplicating them (since the list is sorted, `dedup()` works natively), and then rebuilding the list.
//! - Time: O(n) - One pass to collect, one pass to rebuild.
//! - Space: O(n) - Stores all unique elements in a vector.
//!
//! ### Approach 2: Optimal (In-Place Pointer Manipulation)
//! We iterate through the list with a mutable pointer `current`. For each node, we check if its `next` node
//! has the same value. If it does, we skip the `next` node by re-wiring `current.next` to `current.next.next`.
//! If not, we simply advance `current`.
//! - Time: O(n) - Single pass through the list.
//! - Space: O(1) - In-place modification with a single pointer.
//!
//! ## Constraints
//! - The number of nodes in the list is in the range `[0, 300]`.
//! - `-100 <= Node.val <= 100`
//! - The list is guaranteed to be sorted in ascending order.

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

/// Brute force approach: Collect to Vec, dedup, rebuild
///
/// Time: O(n)
/// Space: O(n)
///
/// This avoids pointer manipulation by taking advantage of standard library utilities.
#[must_use]
pub fn delete_duplicates_brute_force(head: Option<Box<ListNode>>) -> Option<Box<ListNode>> {
    let mut vec = Vec::new();
    let mut current = head;

    // Collect values
    while let Some(node) = current {
        vec.push(node.val);
        current = node.next;
    }

    // Since the input list is sorted, `dedup` removes consecutive duplicates.
    vec.dedup();

    // Rebuild the list from the deduplicated vector.
    // Our `from_vec` implementation builds from tail to head, but iterates backwards
    // using `rev()`, so we don't need to manually reverse the vector here.
    ListNode::from_vec(vec)
}

/// Optimal approach: In-place iteration and modification
///
/// Time: O(n)
/// Space: O(1)
///
/// # Rust Insight
/// We use `&mut head` and `as_mut()` to maintain a mutable reference to the `current` node as we traverse.
/// Using nested `while let` loops allows us to safely peek ahead at `node.next` without taking
/// ownership unless we intend to modify it.
///
/// # Gotcha
/// When bypassing a node (e.g., `node.next = next_node.next`), we take ownership of `next_node` and its
/// `next` pointer by taking the option (implicit in the pattern match). Rust's memory management automatically
/// drops the bypassed node when `next_node` goes out of scope.
#[must_use]
pub fn delete_duplicates_optimal(mut head: Option<Box<ListNode>>) -> Option<Box<ListNode>> {
    // We use a mutable reference to track our current position in the list.
    let mut current = head.as_mut();

    while let Some(node) = current {
        // Look ahead to the next node. If it exists and matches our current value...
        while let Some(next_node) = node.next.as_mut() {
            if next_node.val == node.val {
                // ...bypass it. We take the `next` pointer from the `next_node` and assign it
                // directly to `node.next`.
                //
                // RUST INSIGHT: `take()` removes the value from the Option, replacing it with None.
                // This gives us ownership of the rest of the list so we can attach it to `node`.
                let next_next = next_node.next.take();
                node.next = next_next;
            } else {
                // If the values don't match, we've found a new unique value.
                // Break out of the look-ahead loop so we can advance `current`.
                break;
            }
        }
        // Advance the `current` pointer to the next unique node.
        current = node.next.as_mut();
    }

    head
}

/// Main entry point - uses optimal solution
#[must_use]
pub fn delete_duplicates(head: Option<Box<ListNode>>) -> Option<Box<ListNode>> {
    delete_duplicates_optimal(head)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(v: &[i32]) -> Option<Box<ListNode>> {
        ListNode::from_vec(v.to_vec())
    }

    #[test]
    fn test_brute_force_example_1() {
        // Input: head = [1,1,2]
        // Output: [1,2]
        let input = list(&[1, 1, 2]);
        let result = delete_duplicates_brute_force(input);
        assert_eq!(result.unwrap().to_vec(), vec![1, 2]);
    }

    #[test]
    fn test_brute_force_example_2() {
        // Input: head = [1,1,2,3,3]
        // Output: [1,2,3]
        let input = list(&[1, 1, 2, 3, 3]);
        let result = delete_duplicates_brute_force(input);
        assert_eq!(result.unwrap().to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn test_optimal_example_1() {
        let input = list(&[1, 1, 2]);
        let result = delete_duplicates_optimal(input);
        assert_eq!(result.unwrap().to_vec(), vec![1, 2]);
    }

    #[test]
    fn test_optimal_example_2() {
        let input = list(&[1, 1, 2, 3, 3]);
        let result = delete_duplicates_optimal(input);
        assert_eq!(result.unwrap().to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn test_all_duplicates() {
        let input = list(&[1, 1, 1, 1, 1]);
        let result = delete_duplicates(input);
        assert_eq!(result.unwrap().to_vec(), vec![1]);
    }

    #[test]
    fn test_no_duplicates() {
        let input = list(&[1, 2, 3, 4, 5]);
        let result = delete_duplicates(input);
        assert_eq!(result.unwrap().to_vec(), vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_empty_list() {
        let result = delete_duplicates(None);
        assert_eq!(result, None);
    }

    #[test]
    fn test_single_node() {
        let input = list(&[42]);
        let result = delete_duplicates(input);
        assert_eq!(result.unwrap().to_vec(), vec![42]);
    }

    #[test]
    fn test_multiple_groups() {
        let input = list(&[1, 1, 2, 2, 2, 3, 4, 4, 5, 5, 5]);
        let result = delete_duplicates(input);
        assert_eq!(result.unwrap().to_vec(), vec![1, 2, 3, 4, 5]);
    }
}
