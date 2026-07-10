//! # 155. Min Stack
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/min-stack/>
//!
//! Design a stack that supports push, pop, top, and retrieving the minimum element in constant time.
//!
//! This problem matters in Rust because it perfectly demonstrates how `Vec` is the idiomatic way
//! to implement a stack, and how zero-cost abstractions like tuples can be used to neatly bundle
//! data without allocating on the heap.
//!
//! ## Approach
//!
//! We provide two approaches:
//! 1. **Tuple Approach (`MinStack`)**: We store pairs of `(value, current_min)` in a single `Vec`.
//!    This is very clean and leverages cache locality well, but repeats the minimum value for each element.
//!    Time: O(1) for all operations. Space: O(N).
//!
//! 2. **Two-Stack Approach (`MinStackOptimized`)**: We maintain a primary stack for values and an
//!    auxiliary stack for the minimum values. We only push to the min stack when the new value is
//!    less than or equal to the current minimum. This saves space when there are many duplicate
//!    minimums or values greater than the current minimum.
//!    Time: O(1) for all operations. Space: O(N) worst case, but often less in practice.
//!
//! In Python or Java, you might be tempted to use linked lists or custom node classes. In Rust,
//! `Vec` provides exactly what we need with excellent performance due to contiguous memory allocation.
//!
//! Note: design problem with two idiomatic variants (`MinStack`, `MinStackOptimized`); the brute/optimized/optimal progression does not apply here.

/// A Min Stack using a single `Vec` storing `(value, current_min)` tuples.
#[derive(Default, Debug)]
pub struct MinStack {
    // RUST INSIGHT: A Vec of tuples `(i32, i32)` is stored contiguously in memory.
    // There's no extra heap allocation per node like there would be with `Box<Node>`.
    stack: Vec<(i32, i32)>,
}

impl MinStack {
    #[must_use]
    pub fn new() -> Self {
        Self { stack: Vec::new() }
    }

    pub fn push(&mut self, val: i32) {
        let min = if let Some(&(_, current_min)) = self.stack.last() {
            val.min(current_min)
        } else {
            val
        };
        self.stack.push((val, min));
    }

    pub fn pop(&mut self) {
        // GOTCHA: `pop` on an empty Vec returns `None`. We can ignore it if we
        // assume valid usage, but for LeetCode we just call it.
        self.stack.pop();
    }

    #[must_use]
    pub fn top(&self) -> i32 {
        // RUST INSIGHT: `unwrap` is used here because the problem description usually
        // guarantees that `top` is only called on non-empty stacks.
        self.stack.last().unwrap().0
    }

    #[must_use]
    pub fn get_min(&self) -> i32 {
        self.stack.last().unwrap().1
    }
}

/// An optimized Min Stack using two separate `Vec`s.
#[derive(Default, Debug)]
pub struct MinStackOptimized {
    stack: Vec<i32>,
    min_stack: Vec<i32>,
}

impl MinStackOptimized {
    #[must_use]
    pub fn new() -> Self {
        Self {
            stack: Vec::new(),
            min_stack: Vec::new(),
        }
    }

    pub fn push(&mut self, val: i32) {
        self.stack.push(val);
        // Push to min_stack if it's empty or the value is <= current min
        if self.min_stack.is_empty() || val <= *self.min_stack.last().unwrap() {
            self.min_stack.push(val);
        }
    }

    pub fn pop(&mut self) {
        if let Some(val) = self.stack.pop() {
            // If the popped value is the current min, pop it from the min_stack too
            if val == *self.min_stack.last().unwrap() {
                self.min_stack.pop();
            }
        }
    }

    #[must_use]
    pub fn top(&self) -> i32 {
        *self.stack.last().unwrap()
    }

    #[must_use]
    pub fn get_min(&self) -> i32 {
        *self.min_stack.last().unwrap()
    }
}

// Alternative approach: One could define a custom Node struct instead of a tuple.
// struct Node { val: i32, min: i32 }
// However, tuples are more concise and idiomatic for simple pairs like this.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_min_stack_happy_path() {
        let mut min_stack = MinStack::new();
        min_stack.push(-2);
        min_stack.push(0);
        min_stack.push(-3);
        assert_eq!(min_stack.get_min(), -3); // return -3
        min_stack.pop();
        assert_eq!(min_stack.top(), 0); // return 0
        assert_eq!(min_stack.get_min(), -2); // return -2
    }

    #[test]
    fn test_min_stack_optimized_happy_path() {
        let mut min_stack = MinStackOptimized::new();
        min_stack.push(-2);
        min_stack.push(0);
        min_stack.push(-3);
        assert_eq!(min_stack.get_min(), -3);
        min_stack.pop();
        assert_eq!(min_stack.top(), 0);
        assert_eq!(min_stack.get_min(), -2);
    }

    #[test]
    fn test_min_stack_edge_cases() {
        let mut min_stack = MinStack::new();
        min_stack.push(5);
        assert_eq!(min_stack.top(), 5);
        assert_eq!(min_stack.get_min(), 5);

        min_stack.push(5);
        assert_eq!(min_stack.get_min(), 5);
        min_stack.pop();
        assert_eq!(min_stack.get_min(), 5);
    }

    #[test]
    fn test_min_stack_optimized_edge_cases() {
        let mut min_stack = MinStackOptimized::new();
        min_stack.push(5);
        assert_eq!(min_stack.top(), 5);
        assert_eq!(min_stack.get_min(), 5);

        min_stack.push(5);
        assert_eq!(min_stack.get_min(), 5);
        min_stack.pop();
        assert_eq!(min_stack.get_min(), 5);
    }

    #[test]
    fn test_min_stack_stress() {
        let mut min_stack = MinStack::new();
        for i in (0..1000).rev() {
            min_stack.push(i);
            assert_eq!(min_stack.get_min(), i);
        }
        for i in 0..999 {
            assert_eq!(min_stack.get_min(), i);
            min_stack.pop();
        }
    }
}
