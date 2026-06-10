//! # 143. Reorder List
//!
//! Difficulty: Medium
//!
//! Link: <https://leetcode.com/problems/reorder-list/>
//!
//! You are given the head of a singly linked-list. The list can be represented as:
//! `L0 → L1 → … → Ln-1 → Ln`
//!
//! Reorder the list to be on the following form:
//! `L0 → Ln → L1 → Ln-1 → L2 → Ln-2 → …`
//!
//! You may not modify the values in the list's nodes. Only nodes themselves may be changed.
//!
//! This problem perfectly highlights Rust's strict aliasing rules. Unlike other languages where
//! you can easily keep a "slow" and "fast" pointer while mutably splitting the list in a single pass,
//! Rust's borrow checker prevents having multiple mutable references (or mixing mutable and immutable
//! references) to the same linked list structure. This forces us to either use a safe two-pass
//! length-counting approach for O(1) space, or use an O(N) space data structure like a `VecDeque`.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::linked_lists::reorder_list::{ListNode, reorder_list};
//!
//! // 1 -> 2 -> 3 -> 4
//! let mut list = ListNode::from_vec(vec![1, 2, 3, 4]);
//! reorder_list(&mut list);
//! assert_eq!(list.unwrap().to_vec(), vec![1, 4, 2, 3]);
//! ```
//!
//! ## Constraints
//!
//! - The number of nodes in the list is in the range `[1, 5 * 10^4]`.
//! - `1 <= Node.val <= 1000`
//! - `1 <= Node.val <= 1000`

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

/// Brute Force / Optimized approach: VecDeque
/// Time: O(N) - One pass to collect nodes, one pass to re-link.
/// Space: O(N) - We store all nodes in a `VecDeque`.
///
/// We detach all nodes from the list and place them into a double-ended queue (`VecDeque`).
/// Then we alternately pop from the front and the back to rebuild the reordered list.
/// This completely bypasses complex pointer manipulation.
#[allow(clippy::ptr_arg)]
pub fn reorder_list_brute_force(head: &mut Option<Box<ListNode>>) {
    if head.is_none() || head.as_ref().unwrap().next.is_none() {
        return;
    }

    // RUST INSIGHT: We use `Option::take()` to take ownership of the entire list,
    // leaving `None` in `head`. This satisfies the borrow checker.
    let mut current = head.take();
    let mut deque = std::collections::VecDeque::new();

    // Collect all nodes into the deque
    while let Some(mut node) = current {
        current = node.next.take();
        deque.push_back(node);
    }

    // Rebuild the list alternately from front and back
    let mut dummy = ListNode::new(0);
    let mut tail = &mut dummy.next;

    let mut turn_front = true;
    while !deque.is_empty() {
        let node = if turn_front {
            deque.pop_front()
        } else {
            deque.pop_back()
        };

        *tail = node;
        tail = &mut tail.as_mut().unwrap().next;
        turn_front = !turn_front;
    }

    *head = dummy.next;
}

/// Optimal approach: Two-Pass Length-Counting (In-Place)
/// Time: O(N) - We traverse the list to find length, traverse to split, reverse, and merge.
/// Space: O(1) - Only a few pointers are used.
///
/// In this approach we:
/// 1. Count the length of the list to find the middle.
/// 2. Split the list into two halves.
/// 3. Reverse the second half.
/// 4. Merge the two halves by taking one node from each alternately.
///
/// GOTCHA: We use length-counting instead of a fast/slow pointer because Rust's strict aliasing
/// rules forbid holding a mutable and immutable reference to the list simultaneously, making the
/// fast/slow pointer split tricky without `unsafe`. Length counting provides a safe, O(1) space alternative.
#[allow(clippy::ptr_arg)]
pub fn reorder_list_optimal(head: &mut Option<Box<ListNode>>) {
    if head.is_none() || head.as_ref().unwrap().next.is_none() {
        return;
    }

    // 1. Find the length
    let mut len = 0;
    let mut current = head.as_ref();
    while let Some(node) = current {
        len += 1;
        current = node.next.as_ref();
    }

    // 2. Split the list at the middle
    let mid = (len + 1) / 2;
    let mut current_mut = head.as_mut();
    for _ in 0..mid - 1 {
        if let Some(node) = current_mut {
            current_mut = node.next.as_mut();
        }
    }

    // RUST INSIGHT: To sever the list, we explicitly take the `next` field of the inner node.
    let mut second_half = if let Some(node) = current_mut {
        node.next.take()
    } else {
        None
    };

    // 3. Reverse the second half
    let mut prev = None;
    let mut curr_rev = second_half;
    while let Some(mut node) = curr_rev {
        let next = node.next.take();
        node.next = prev;
        prev = Some(node);
        curr_rev = next;
    }
    second_half = prev;

    // 4. Merge the two halves
    let first_half = head.take();
    let mut dummy = ListNode::new(0);
    let mut tail = &mut dummy.next;

    let mut l1 = first_half;
    let mut l2 = second_half;

    while l1.is_some() || l2.is_some() {
        if let Some(mut node1) = l1 {
            l1 = node1.next.take();
            *tail = Some(node1);
            tail = &mut tail.as_mut().unwrap().next;
        }
        if let Some(mut node2) = l2 {
            l2 = node2.next.take();
            *tail = Some(node2);
            tail = &mut tail.as_mut().unwrap().next;
        }
    }

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
    fn test_single_element() {
        let mut list = ListNode::from_vec(vec![1]);
        reorder_list(&mut list);
        assert_eq!(list.unwrap().to_vec(), vec![1]);
    }

    #[test]
    fn test_all_approaches_consistency() {
        let v = vec![1, 2, 3, 4, 5, 6, 7];

        let mut l1 = ListNode::from_vec(v.clone());
        let mut l2 = ListNode::from_vec(v.clone());

        reorder_list_brute_force(&mut l1);
        reorder_list_optimal(&mut l2);

        assert_eq!(l1.unwrap().to_vec(), l2.unwrap().to_vec());
    }
}

// =========================================================================================
// Alternative Approaches
// =========================================================================================
// 1. **VecDeque Conversion (Brute Force)**: Detailed above, takes O(N) memory but is
//    significantly simpler by sidestepping complex pointer traversals. Useful when
//    memory is plentiful and developer time is scarce.
// 2. **Recursive Reordering**: In languages like Java or Python, recursion can implicitly
//    manage the "fast/slow" traversal via the call stack. However, in Rust, a purely
//    recursive approach in-place encounters the same mutable aliasing issues when trying
//    to modify pointers from two ends of the call stack simultaneously without `Rc<RefCell<T>>`.
