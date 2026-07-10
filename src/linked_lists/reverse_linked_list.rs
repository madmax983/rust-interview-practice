//! # 206. Reverse Linked List
//!
//! Given the head of a singly linked list, reverse the list, and return the reversed list.
//!
//! This problem is the "Hello World" of pointer manipulation. In Rust, it forces you to
//! confront the borrow checker's rules about multiple mutable references and ownership transfer.
//! It's an excellent exercise to understand `Option<Box<T>>`, `mem::replace`, and `Option::take`.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::linked_lists::reverse_linked_list::{reverse_list, ListNode};
//!
//! let list = ListNode::from_vec(vec![1, 2, 3, 4, 5]);
//! let reversed = reverse_list(list);
//! assert_eq!(reversed.unwrap().to_vec(), vec![5, 4, 3, 2, 1]);
//! ```
//!
//! ## Constraints
//!
//! - The number of nodes in the list is the range `[0, 5000]`.
//! - `-5000 <= Node.val <= 5000`

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

    /// Helper to create a list from a vector (useful for tests and brute force)
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

/// Brute force approach: Collect to Vec, reverse, rebuild
/// Time: O(N) - Two passes (one to collect, one to rebuild)
/// Space: O(N) - Storing all elements in a vector
///
/// This approach sidesteps pointer manipulation by using a high-level data structure.
/// It's simple to implement but uses O(N) auxiliary space.
#[must_use]
pub fn reverse_list_brute_force(head: Option<Box<ListNode>>) -> Option<Box<ListNode>> {
    let mut vec = Vec::new();
    let mut current = head;

    // Collect values
    while let Some(node) = current {
        vec.push(node.val);
        current = node.next;
    }

    // Rebuild reversed list.
    // We collected values in order (e.g., [1, 2, 3]). We want the new list to be 3->2->1.
    // We reverse the vector to [3, 2, 1]. `from_vec` will iterate this in reverse (1, then 2, then 3)
    // building the list from tail to head: 1->None, then 2->1, then 3->2->1.
    vec.reverse();
    ListNode::from_vec(vec)
}

/// Optimized approach: recursive with a tail-recursive helper
/// Time: O(N) - Visit each node once
/// Space: O(N) - Stack frames for recursion (one frame per node)
///
/// This uses a helper function to accumulate the reversed list.
///
/// RUST INSIGHT: Tail call optimization is not guaranteed in Rust yet,
/// so this could stack overflow for very large lists (though N=5000 might be fine).
#[must_use]
pub fn reverse_list_optimized(head: Option<Box<ListNode>>) -> Option<Box<ListNode>> {
    fn helper(head: Option<Box<ListNode>>, prev: Option<Box<ListNode>>) -> Option<Box<ListNode>> {
        match head {
            None => prev,
            Some(mut node) => {
                let next = node.next.take();
                node.next = prev;
                helper(next, Some(node))
            }
        }
    }
    helper(head, None)
}

/// Optimal approach: Iterative in-place
/// Time: O(N) - Visit each node once
/// Space: O(1) - Only a few pointers
///
/// This is the standard idiomatic solution. We maintain `prev` and `current` pointers
/// and reverse the links as we traverse.
///
/// RUST INSIGHT: `Option::take()` is crucial here. It allows us to move the value out
/// of the `Option` (leaving `None` in its place), which satisfies the borrow checker
/// when we need to modify the structure we are traversing.
///
/// GOTCHA: Be careful not to lose the reference to `next` before overwriting `current.next`.
#[must_use]
pub fn reverse_list_optimal(head: Option<Box<ListNode>>) -> Option<Box<ListNode>> {
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

/// Main entry point - uses optimal solution
#[must_use]
pub fn reverse_list(head: Option<Box<ListNode>>) -> Option<Box<ListNode>> {
    reverse_list_optimal(head)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force_example_1() {
        let list = ListNode::from_vec(vec![1, 2, 3, 4, 5]);
        let result = reverse_list_brute_force(list);
        assert_eq!(result.unwrap().to_vec(), vec![5, 4, 3, 2, 1]);
    }

    #[test]
    fn test_optimized_example_1() {
        let list = ListNode::from_vec(vec![1, 2, 3, 4, 5]);
        let result = reverse_list_optimized(list);
        assert_eq!(result.unwrap().to_vec(), vec![5, 4, 3, 2, 1]);
    }

    #[test]
    fn test_optimal_example_1() {
        let list = ListNode::from_vec(vec![1, 2, 3, 4, 5]);
        let result = reverse_list_optimal(list);
        assert_eq!(result.unwrap().to_vec(), vec![5, 4, 3, 2, 1]);
    }

    #[test]
    fn test_empty_list() {
        assert_eq!(reverse_list(None), None);
    }

    #[test]
    fn test_single_node() {
        let list = ListNode::from_vec(vec![1]);
        let result = reverse_list(list);
        assert_eq!(result.unwrap().to_vec(), vec![1]);
    }

    #[test]
    fn test_two_nodes() {
        let list = ListNode::from_vec(vec![1, 2]);
        let result = reverse_list(list);
        assert_eq!(result.unwrap().to_vec(), vec![2, 1]);
    }

    #[test]
    fn test_all_approaches_consistency() {
        let v = vec![1, 2, 3, 4, 5];

        let l1 = ListNode::from_vec(v.clone());
        let l2 = ListNode::from_vec(v.clone());
        let l3 = ListNode::from_vec(v);

        let res1 = reverse_list_brute_force(l1);
        let res2 = reverse_list_optimized(l2);
        let res3 = reverse_list_optimal(l3);

        assert_eq!(res1, res2);
        assert_eq!(res2, res3);
    }
}
