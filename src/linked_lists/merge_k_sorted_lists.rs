//! # 23. Merge k Sorted Lists
//!
//! Difficulty: Hard
//! Link: https://leetcode.com/problems/merge-k-sorted-lists/
//!
//! You are given an array of `k` linked-lists `lists`, each linked-list is sorted in ascending order.
//!
//! Merge all the linked-lists into one sorted linked-list and return it.
//!
//! This problem is a classic application of the Heap (Priority Queue) data structure.
//! In Rust, it demonstrates how to define custom ordering for complex types (`Box<ListNode>`)
//! to work with `BinaryHeap`, and how to manage ownership when moving nodes between
//! containers (Vector -> Heap -> Result List).
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::linked_lists::merge_k_sorted_lists::{merge_k_lists, ListNode};
//!
//! let l1 = ListNode::from_vec(vec![1, 4, 5]);
//! let l2 = ListNode::from_vec(vec![1, 3, 4]);
//! let l3 = ListNode::from_vec(vec![2, 6]);
//!
//! let lists = vec![l1, l2, l3];
//! let merged = merge_k_lists(lists);
//! assert_eq!(merged.unwrap().to_vec(), vec![1, 1, 2, 3, 4, 4, 5, 6]);
//! ```
//!
//! ## Constraints
//!
//! - `k == lists.length`
//! - `0 <= k <= 10^4`
//! - `0 <= lists[i].length <= 500`
//! - `-10^4 <= lists[i][j] <= 10^4`
//! - `lists[i]` is sorted in ascending order.
//! - The sum of `lists[i].length` will not exceed `10^4`.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

// Definition for singly-linked list.
// Standard definition used in LeetCode problems.
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

    /// Helper to create a list from a vector (useful for tests and initialization)
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

/// Wrapper struct to implement `Ord` for `Box<ListNode>`.
/// Rust's `BinaryHeap` is a Max-Heap. To use it as a Min-Heap, we need to reverse the ordering.
struct HeapNode {
    node: Box<ListNode>,
}

impl PartialEq for HeapNode {
    fn eq(&self, other: &Self) -> bool {
        self.node.val == other.node.val
    }
}

impl Eq for HeapNode {}

// RUST INSIGHT: We implement `PartialOrd` and `Ord` to define custom ordering.
// `BinaryHeap` pops the largest element. We want the smallest value to be "largest".
// So we compare `other.val` to `self.val`.
impl PartialOrd for HeapNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for HeapNode {
    fn cmp(&self, other: &Self) -> Ordering {
        other.node.val.cmp(&self.node.val)
    }
}

/// Brute force approach: Collect all values, sort, rebuild
/// Time: O(N log N) - N is the total number of nodes; sorting dominates.
/// Space: O(N) - We store every value in a vector before rebuilding.
///
/// This approach ignores the fact that each individual list is already sorted.
/// It's the most obvious solution: flatten everything, sort, and rebuild a single list.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn merge_k_lists_brute_force(lists: Vec<Option<Box<ListNode>>>) -> Option<Box<ListNode>> {
    let mut values = Vec::new();

    // Flatten every list into a single vector of values.
    // `into_iter().flatten()` yields the `Some` heads and consumes the outer vector.
    for list in lists.into_iter().flatten() {
        let mut current = Some(list);
        while let Some(node) = current {
            values.push(node.val);
            current = node.next;
        }
    }

    values.sort_unstable();

    ListNode::from_vec(values)
}

/// Optimal approach: Min-Heap (Priority Queue)
/// Time: O(N log k) where k is the number of linked lists.
/// - The heap size is at most k.
/// - Every node is pushed and popped exactly once.
/// Space: O(k)
/// - The heap stores at most k nodes at any time.
/// - The result list simply relinks existing nodes (plus a dummy head).
///
/// idiomatic Rust features:
/// - `BinaryHeap` for efficient minimum retrieval.
/// - `Option<Box<T>>` manipulation with `take()`.
/// - Wrapper struct (`HeapNode`) to bypass the orphan rule or implement custom trait behavior locally.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn merge_k_lists_optimal(lists: Vec<Option<Box<ListNode>>>) -> Option<Box<ListNode>> {
    let mut min_heap = BinaryHeap::new();

    // Initial population of the heap
    // We only push the head of each non-empty list.
    // RUST INSIGHT: `into_iter` consumes the vector, giving us ownership of the Boxes.
    // `flatten` automatically handles the `Option`s, yielding only `Some` variants.
    for node in lists.into_iter().flatten() {
        min_heap.push(HeapNode { node });
    }

    let mut dummy = ListNode::new(0);
    let mut tail = &mut dummy;

    // While the heap is not empty, extract the minimum node
    while let Some(HeapNode { mut node }) = min_heap.pop() {
        // If the extracted node has a next node, push it to the heap
        // RUST INSIGHT: `node.next.take()` moves the next node out of the current node,
        // leaving `None` in its place. We then push that next node (if it exists) into the heap.
        if let Some(next) = node.next.take() {
            min_heap.push(HeapNode { node: next });
        }

        // Append the current node to the result list
        // GOTCHA: `tail` is a mutable reference. We update `tail.next` to point to the new node.
        // Then we move `tail` to point to the new last node.
        tail.next = Some(node);
        tail = tail.next.as_mut().unwrap();
    }

    dummy.next
}

/// Main entry point - uses the optimal min-heap solution.
#[must_use]
pub fn merge_k_lists(lists: Vec<Option<Box<ListNode>>>) -> Option<Box<ListNode>> {
    merge_k_lists_optimal(lists)
}

// Alternative approach (not implemented here): Divide and Conquer.
// Merge lists pairwise recursively for O(N log k) time and O(log k) stack space.
// It avoids the heap's overhead but is recursive.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_k_lists_basic() {
        let l1 = ListNode::from_vec(vec![1, 4, 5]);
        let l2 = ListNode::from_vec(vec![1, 3, 4]);
        let l3 = ListNode::from_vec(vec![2, 6]);

        let lists = vec![l1, l2, l3];
        let merged = merge_k_lists(lists);
        assert_eq!(merged.unwrap().to_vec(), vec![1, 1, 2, 3, 4, 4, 5, 6]);
    }

    #[test]
    fn test_merge_k_lists_empty_input() {
        let lists: Vec<Option<Box<ListNode>>> = vec![];
        let merged = merge_k_lists(lists);
        assert_eq!(merged, None);
    }

    #[test]
    fn test_merge_k_lists_empty_lists_inside() {
        let lists = vec![None, None, ListNode::from_vec(vec![1])];
        let merged = merge_k_lists(lists);
        assert_eq!(merged.unwrap().to_vec(), vec![1]);
    }

    #[test]
    fn test_merge_k_lists_single_list() {
        let l1 = ListNode::from_vec(vec![1, 2, 3]);
        let lists = vec![l1];
        let merged = merge_k_lists(lists);
        assert_eq!(merged.unwrap().to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn test_merge_k_lists_different_lengths() {
        let l1 = ListNode::from_vec(vec![1]);
        let l2 = ListNode::from_vec(vec![2, 5, 10, 12]);
        let l3 = ListNode::from_vec(vec![3, 4]);

        let lists = vec![l1, l2, l3];
        let merged = merge_k_lists(lists);
        assert_eq!(merged.unwrap().to_vec(), vec![1, 2, 3, 4, 5, 10, 12]);
    }

    #[test]
    fn test_brute_force_basic() {
        let l1 = ListNode::from_vec(vec![1, 4, 5]);
        let l2 = ListNode::from_vec(vec![1, 3, 4]);
        let l3 = ListNode::from_vec(vec![2, 6]);

        let merged = merge_k_lists_brute_force(vec![l1, l2, l3]);
        assert_eq!(merged.unwrap().to_vec(), vec![1, 1, 2, 3, 4, 4, 5, 6]);
    }

    #[test]
    fn test_brute_force_edge_cases() {
        let empty: Vec<Option<Box<ListNode>>> = vec![];
        assert_eq!(merge_k_lists_brute_force(empty), None);

        let with_gaps = vec![None, None, ListNode::from_vec(vec![1])];
        assert_eq!(
            merge_k_lists_brute_force(with_gaps).unwrap().to_vec(),
            vec![1]
        );
    }

    #[test]
    fn test_both_approaches_agree() {
        let build = || {
            vec![
                ListNode::from_vec(vec![1, 4, 5]),
                ListNode::from_vec(vec![1, 3, 4]),
                ListNode::from_vec(vec![2, 6]),
            ]
        };
        let brute = merge_k_lists_brute_force(build());
        let optimal = merge_k_lists_optimal(build());
        assert_eq!(brute, optimal);
    }
}
