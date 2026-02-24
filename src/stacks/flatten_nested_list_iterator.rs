//! # 341. Flatten Nested List Iterator
//!
//! Given a nested list of integers, implement an iterator to flatten it.
//! Each element is either an integer, or a list -- whose elements may also be integers or other lists.
//!
//! [LeetCode Problem 341](https://leetcode.com/problems/flatten-nested-list-iterator/)
//!
//! ## Why This Matters in Rust
//!
//! This problem is an excellent exercise for understanding:
//!
//! 1.  **Custom Iterators**: Implementing the `Iterator` trait manually is a core Rust skill. It forces you to think about
//!     state management (`next()` vs internal state) and termination conditions.
//! 2.  **Enums and Recursive Data Structures**: `NestedInteger` is a recursive enum, a common pattern in Rust for trees,
//!     JSON, or ASTs.
//! 3.  **Ownership vs. Borrowing**: We can implement this by consuming the data structure (using `IntoIter`) or by
//!     borrowing it. The consuming approach is often cleaner for this specific problem as it avoids complex lifetime
//!     management with self-referential stacks, but it demonstrates how `Vec::into_iter()` works.
//! 4.  **Stack-based Recursion**: Flattening a nested structure is inherently recursive. Using an explicit stack allows
//!     us to pause and resume the "recursion" inside `next()`, which a simple recursive function cannot do.
//!
//! ## Approach
//!
//! We need to flatten a tree-like structure into a linear sequence.
//! Since `Iterator::next()` returns one item at a time, we cannot use a simple recursive function that yields all items.
//! Instead, we must maintain the state of our traversal.
//!
//! We use a **Stack of Iterators**:
//! - The stack holds `vec::IntoIter<NestedInteger>`.
//! - When `next()` is called:
//!     1.  Peek at the top iterator on the stack.
//!     2.  Get its next item.
//!     3.  If the item is an Integer, return it.
//!     4.  If the item is a List, push its iterator onto the stack and repeat.
//!     5.  If the iterator is exhausted (`None`), pop it from the stack and repeat.
//!
//! This ensures we process elements in the correct Depth-First Search (DFS) order.
//!
//! Time Complexity: O(N) total for iterating through all N integers. Each integer and list is pushed and popped exactly once.
//! Space Complexity: O(D), where D is the maximum depth of nesting (stack size).

/// Represents a nested integer, which can be a single integer or a list of NestedIntegers.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum NestedInteger {
    Int(i32),
    List(Vec<NestedInteger>),
}

/// Iterator that flattens a `NestedInteger` structure.
pub struct NestedIterator {
    // Stack of iterators.
    // We use `IntoIter` to consume the nested structure.
    // RUST INSIGHT: Storing iterators in a stack allows us to pause/resume traversal.
    // `IntoIter` owns the data, avoiding lifetime complexity for this exercise.
    stack: Vec<std::vec::IntoIter<NestedInteger>>,
}

impl NestedIterator {
    /// Creates a new iterator from a vector of `NestedInteger`.
    pub fn new(nested_list: Vec<NestedInteger>) -> Self {
        let mut stack = Vec::new();
        // Push the main list's iterator onto the stack.
        // If the list is empty, we don't strictly need to push it, but pushing it handles
        // the logic uniformly (it will just return None immediately).
        // However, optimizing out empty initial lists is fine.
        // Let's push it so the loop in `next()` handles everything.
        stack.push(nested_list.into_iter());

        Self { stack }
    }
}

impl Iterator for NestedIterator {
    type Item = i32;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            // If stack is empty, we are done.
            if self.stack.is_empty() {
                return None;
            }

            // Get the next item from the top iterator.
            // We isolate the borrow of `self.stack` to this statement.
            // `last_mut()` returns `Option<&mut IntoIter>`, `next()` advances it.
            // The result `next_item` owns the `NestedInteger` (moved out of the iterator).
            let next_item = self.stack.last_mut()?.next();

            match next_item {
                Some(NestedInteger::Int(val)) => {
                    // Found an integer. Return it.
                    return Some(val);
                }
                Some(NestedInteger::List(list)) => {
                    // Found a list. We need to iterate into it.
                    // Push its iterator onto the stack.
                    // The borrow of `self.stack` has ended, so we can mutate it here.
                    self.stack.push(list.into_iter());
                    // Loop again to process the first item of this new list.
                    // GOTCHA: It's important NOT to eagerly flatten the list here (e.g., collecting into a single Vec).
                    // Doing so would defeat the purpose of an iterator (lazy evaluation) and spike memory usage.
                }
                None => {
                    // The top iterator is exhausted. Pop it.
                    self.stack.pop();
                    // Loop again to continue with the iterator below it.
                }
            }
        }
    }
}

// -----------------------------------------------------------------------------------------
// Alternative Approaches
// -----------------------------------------------------------------------------------------
//
// 1. **Reference-Based Iterator**:
//    Instead of `IntoIter` (consuming), we could store `std::slice::Iter<'a, NestedInteger>`.
//    - Pros: Does not consume the original data structure.
//    - Cons: Requires lifetime annotations (`NestedIterator<'a>`), making the code slightly more verbose
//      and harder to adapt if the input is temporary.
//
// 2. **Pre-flattening (Brute Force)**:
//    Traverse the entire structure in `new()` and store all integers in a `Vec<i32>`.
//    Then, `next()` just iterates this vector.
//    - Pros: Very simple to implement.
//    - Cons: O(N) memory upfront. Fails the "iterator" spirit of lazy evaluation.
//      If the nested list is massive, this crashes.
//
// 3. **Generator/Coroutine (Future Rust)**:
//    Using `yield` syntax (unstable in Rust as of 2024) would allow writing a simple recursive function
//    that behaves like an iterator. This is how Python's `yield` works.

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to construct List variant easily
    fn list(v: Vec<NestedInteger>) -> NestedInteger {
        NestedInteger::List(v)
    }

    // Helper to construct Int variant easily
    fn int(i: i32) -> NestedInteger {
        NestedInteger::Int(i)
    }

    #[test]
    fn test_simple_flat_list() {
        // [1, 2, 3]
        let input = vec![int(1), int(2), int(3)];
        let iter = NestedIterator::new(input);
        let result: Vec<i32> = iter.collect();
        assert_eq!(result, vec![1, 2, 3]);
    }

    #[test]
    fn test_nested_structure() {
        // [[1,1], 2, [1,1]]
        let input = vec![
            list(vec![int(1), int(1)]),
            int(2),
            list(vec![int(1), int(1)]),
        ];
        let iter = NestedIterator::new(input);
        let result: Vec<i32> = iter.collect();
        assert_eq!(result, vec![1, 1, 2, 1, 1]);
    }

    #[test]
    fn test_deeply_nested() {
        // [1, [4, [6]]]
        let input = vec![int(1), list(vec![int(4), list(vec![int(6)])])];
        let iter = NestedIterator::new(input);
        let result: Vec<i32> = iter.collect();
        assert_eq!(result, vec![1, 4, 6]);
    }

    #[test]
    fn test_empty_lists() {
        // [1, [], [2, []], 3] -> [1, 2, 3]
        let input = vec![
            int(1),
            list(vec![]),
            list(vec![int(2), list(vec![])]),
            int(3),
        ];
        let iter = NestedIterator::new(input);
        let result: Vec<i32> = iter.collect();
        assert_eq!(result, vec![1, 2, 3]);
    }

    #[test]
    fn test_total_empty() {
        // []
        let input: Vec<NestedInteger> = vec![];
        let iter = NestedIterator::new(input);
        let result: Vec<i32> = iter.collect();
        assert_eq!(result, Vec::<i32>::new());
    }

    #[test]
    fn test_nested_empty() {
        // [[]]
        let input = vec![list(vec![])];
        let iter = NestedIterator::new(input);
        let result: Vec<i32> = iter.collect();
        assert_eq!(result, Vec::<i32>::new());
    }
}
