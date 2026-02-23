//! # 234. Palindrome Linked List
//!
//! Link: <https://leetcode.com/problems/palindrome-linked-list/>
//!
//! Given the head of a singly linked list, return `true` if it is a palindrome.
//!
//! This problem is a classic "hard in Rust" exercise because it requires you to navigate the
//! borrow checker's strict rules around aliasing and mutation. In a language like C++ or Java,
//! you might use a fast/slow pointer approach to find the middle, then reverse the second half
//! in-place. In Rust, holding a reference to the "slow" pointer while advancing the "fast"
//! pointer often leads to lifetime issues if you try to mutate the structure simultaneously.
//!
//! This solution demonstrates two approaches:
//! 1.  **Brute Force**: Convert the list to a `Vec`, which simplifies the problem to an array check but costs O(N) space.
//! 2.  **Optimal**: Perform the in-place reversal strategy safely by using a two-pass approach (count length -> split -> reverse -> compare)
//!     or by carefully managing ownership with `Option::take`.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::linked_lists::palindrome_linked_list::{is_palindrome, ListNode};
//!
//! let list = ListNode::from_vec(vec![1, 2, 2, 1]);
//! assert!(is_palindrome(list));
//!
//! let list = ListNode::from_vec(vec![1, 2]);
//! assert!(!is_palindrome(list));
//! ```
//!
//! ## Constraints
//!
//! - The number of nodes in the list is in the range `[1, 10^5]`.
//! - `0 <= Node.val <= 9`
//!
//! ## Follow up
//! Could you do it in O(n) time and O(1) space?

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

/// Brute Force Approach: Convert to `Vec` and Check
///
/// Time: O(N) - Two passes (one to collect, one to check palindrome).
/// Space: O(N) - Stores all elements in a vector.
///
/// # Why this is idiomatic in Rust
/// Often, the "dumb" solution of collecting into a `Vec` is preferred in Rust because
/// it avoids complex pointer manipulation and `unsafe` blocks. If N is small or memory
/// is not a constraint, this is the most readable and maintainable solution.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_palindrome_brute_force(head: Option<Box<ListNode>>) -> bool {
    let mut vec = Vec::new();
    let mut current = head;

    while let Some(node) = current {
        vec.push(node.val);
        current = node.next;
    }

    // Check palindrome property
    let len = vec.len();
    for i in 0..len / 2 {
        if vec[i] != vec[len - 1 - i] {
            return false;
        }
    }

    true
}

/// Optimized Approach: Reverse Second Half In-Place
///
/// Time: O(N) - Count length (N), advance to middle (N/2), reverse (N/2), compare (N/2).
/// Space: O(1) - Constant extra space (ignoring recursion stack if recursive reverse is used, but we use iterative).
///
/// # Algorithm
/// 1. Count the length of the list.
/// 2. Traverse to the node just before the second half starts.
/// 3. Split the list into two halves using `Option::take`.
/// 4. Reverse the second half.
/// 5. Compare the two halves node-by-node.
/// 6. (Optional) Restore the list, though usually not required for LeetCode.
///
/// # Rust Insight
/// We use `Option::take()` to split the list. This avoids the need for raw pointers
/// or `unsafe` code to handle the "middle" node's `next` pointer. We temporarily
/// own the second half, process it, and then drop it (since we don't restore).
///
/// # Gotcha
/// Be careful with odd vs even lengths. If length is odd (e.g., 5), we skip the middle
/// element (index 2) and compare `[0, 1]` with `reverse([3, 4])`.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_palindrome_optimal(head: Option<Box<ListNode>>) -> bool {
    if head.is_none() {
        return true;
    }

    // Step 1: Count length
    let mut len = 0;
    let mut current = head.as_ref();
    while let Some(node) = current {
        len += 1;
        current = node.next.as_ref();
    }

    if len <= 1 {
        return true;
    }

    // Step 2: Traverse to the split point.
    // For even length (e.g., 4), we want to split after index 1 (start of second half is index 2).
    // For odd length (e.g., 5), we want to split after index 2 (start of second half is index 3).
    // Target index is `(len - 1) / 2`.
    let split_idx = (len - 1) / 2;

    // We need ownership of `head` to mutate it (split it).
    // But we also need to traverse it.
    // We can use a mutable reference to iterate.
    let mut current_mut = head;
    let mut ptr = current_mut.as_mut();

    for _ in 0..split_idx {
        // We know this unwrap is safe because split_idx < len
        ptr = ptr.unwrap().next.as_mut();
    }

    // Step 3: Split the list.
    // `ptr` now points to the node BEFORE the second half.
    // For len=4, split_idx=1. `ptr` is at index 1. `ptr.next` is index 2.
    // For len=5, split_idx=2. `ptr` is at index 2. `ptr.next` is index 3.
    let second_half_head = ptr.unwrap().next.take();

    // Step 4: Reverse the second half.
    let reversed_second_half = reverse_list(second_half_head);

    // Step 5: Compare the two halves.
    // `current_mut` is the head of the first half (now truncated).
    // `reversed_second_half` is the head of the reversed second half.
    let mut p1 = current_mut.as_ref();
    let mut p2 = reversed_second_half.as_ref();

    while let (Some(n1), Some(n2)) = (p1, p2) {
        if n1.val != n2.val {
            return false;
        }
        p1 = n1.next.as_ref();
        p2 = n2.next.as_ref();
    }

    true
}

/// Helper function to reverse a linked list iteratively.
/// (Copied/Adapted from `reverse_linked_list.rs` to keep this file self-contained).
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

/// Main entry point - uses optimal solution.
#[must_use]
pub fn is_palindrome(head: Option<Box<ListNode>>) -> bool {
    is_palindrome_optimal(head)
}

// =========================================================================================
// Tests
// =========================================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force_example_1() {
        let list = ListNode::from_vec(vec![1, 2, 2, 1]);
        assert!(is_palindrome_brute_force(list));
    }

    #[test]
    fn test_brute_force_example_2() {
        let list = ListNode::from_vec(vec![1, 2]);
        assert!(!is_palindrome_brute_force(list));
    }

    #[test]
    fn test_optimal_example_1() {
        let list = ListNode::from_vec(vec![1, 2, 2, 1]);
        assert!(is_palindrome_optimal(list));
    }

    #[test]
    fn test_optimal_example_2() {
        let list = ListNode::from_vec(vec![1, 2]);
        assert!(!is_palindrome_optimal(list));
    }

    #[test]
    fn test_empty_list() {
        assert!(is_palindrome(None));
    }

    #[test]
    fn test_single_node() {
        let list = ListNode::from_vec(vec![1]);
        assert!(is_palindrome(list));
    }

    #[test]
    fn test_odd_palindrome() {
        let list = ListNode::from_vec(vec![1, 2, 3, 2, 1]);
        assert!(is_palindrome(list));
    }

    #[test]
    fn test_odd_not_palindrome() {
        let list = ListNode::from_vec(vec![1, 2, 3, 4, 1]);
        assert!(!is_palindrome(list));
    }

    #[test]
    fn test_stress_consistency() {
        // Construct a large palindrome
        let mut v: Vec<i32> = (0..1000).collect();
        let mut reversed = v.clone();
        reversed.reverse();
        v.extend(reversed);

        let list1 = ListNode::from_vec(v.clone());
        let list2 = ListNode::from_vec(v.clone());

        assert_eq!(
            is_palindrome_brute_force(list1),
            is_palindrome_optimal(list2)
        );
    }
}
