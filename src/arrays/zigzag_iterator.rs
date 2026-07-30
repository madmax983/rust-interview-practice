//! # 281. Zigzag Iterator
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/zigzag-iterator/>
//!
//! This problem is a natural fit for Rust's `Iterator` trait and state management, demonstrating how to interleave multiple data streams without allocation.
//!
//! ## Approach
//!
//! The brute force approach eagerly collects elements from both vectors into a single interleaved vector, resulting in O(N+M) time and space complexity upon initialization.
//!
//! The optimal approach consumes the vectors into their respective iterators (`std::vec::IntoIter`) and dynamically switches between them. By using `ExactSizeIterator` (via `.len()`), we can implement `has_next(&self)` without needing `Peekable` (which requires mutable access). This gives O(1) time complexity for `next()` and `has_next()`, with O(1) extra space.
//!
//! ## Alternative approaches
//!
//! An alternative optimal approach is to store a `VecDeque<std::vec::IntoIter<i32>>` to generalize to K iterators (e.g., for the K-dimensional zigzag iterator). This is slightly more complex but more scalable than boolean toggling.

/// Brute force approach: Eager evaluation
/// Time: O(N + M) for initialization, O(1) for `next`
/// Space: O(N + M) for the allocated vector
pub struct ZigzagIteratorBruteForce {
    data: std::vec::IntoIter<i32>,
}

impl ZigzagIteratorBruteForce {
    #[must_use]
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(v1: Vec<i32>, v2: Vec<i32>) -> Self {
        let mut data = Vec::with_capacity(v1.len() + v2.len());
        let mut i1 = v1.into_iter();
        let mut i2 = v2.into_iter();

        loop {
            match (i1.next(), i2.next()) {
                (Some(v), Some(w)) => {
                    data.push(v);
                    data.push(w);
                }
                (Some(v), None) => {
                    data.push(v);
                    // RUST INSIGHT: Extend consumes the rest of the iterator in one go
                    data.extend(i1);
                    break;
                }
                (None, Some(w)) => {
                    data.push(w);
                    data.extend(i2);
                    break;
                }
                (None, None) => break,
            }
        }

        Self {
            data: data.into_iter(),
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> i32 {
        self.data.next().unwrap()
    }

    #[must_use]
    pub fn has_next(&self) -> bool {
        self.data.len() > 0
    }
}

/// Optimal approach: Lazy evaluation
/// Time: O(1) for `new`, `next`, and `has_next`
/// Space: O(1) (excluding input data which is consumed)
pub struct ZigzagIteratorOptimal {
    i1: std::vec::IntoIter<i32>,
    i2: std::vec::IntoIter<i32>,
    turn: bool,
}

impl ZigzagIteratorOptimal {
    #[must_use]
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(v1: Vec<i32>, v2: Vec<i32>) -> Self {
        // RUST INSIGHT: Taking ownership of Vec<i32> and converting to into_iter()
        // avoids heap allocations during iteration while maintaining safe ownership rules.
        Self {
            i1: v1.into_iter(),
            i2: v2.into_iter(),
            turn: true,
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> i32 {
        if self.turn {
            if self.i1.len() > 0 {
                self.turn = false;
                self.i1.next().unwrap()
            } else {
                self.i2.next().unwrap()
            }
        } else if self.i2.len() > 0 {
            self.turn = true;
            self.i2.next().unwrap()
        } else {
            self.i1.next().unwrap()
        }
    }

    // GOTCHA: `has_next(&self)` takes an immutable reference on LeetCode. If you try to use `std::iter::Peekable`, you will get a borrow checker error because `Peekable::peek()` requires a mutable reference (`&mut self`).
    #[must_use]
    pub fn has_next(&self) -> bool {
        self.i1.len() > 0 || self.i2.len() > 0
    }
}

/// Main implementation alias
pub type ZigzagIterator = ZigzagIteratorOptimal;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force_happy_path() {
        let v1 = vec![1, 2];
        let v2 = vec![3, 4, 5, 6];
        let mut iter = ZigzagIteratorBruteForce::new(v1, v2);

        let mut result = Vec::new();
        while iter.has_next() {
            result.push(iter.next());
        }

        assert_eq!(result, vec![1, 3, 2, 4, 5, 6]);
    }

    #[test]
    fn test_brute_force_edge_case() {
        let v1 = Vec::<i32>::new();
        let v2 = Vec::<i32>::new();
        let iter = ZigzagIteratorBruteForce::new(v1, v2);
        assert!(!iter.has_next());
    }

    #[test]
    fn test_optimal_happy_path() {
        let v1 = vec![1, 2];
        let v2 = vec![3, 4, 5, 6];
        let mut iter = ZigzagIteratorOptimal::new(v1, v2);

        let mut result = Vec::new();
        while iter.has_next() {
            result.push(iter.next());
        }

        assert_eq!(result, vec![1, 3, 2, 4, 5, 6]);
    }

    #[test]
    fn test_optimal_edge_case_empty_first() {
        let v1 = Vec::<i32>::new();
        let v2 = vec![1];
        let mut iter = ZigzagIteratorOptimal::new(v1, v2);

        assert!(iter.has_next());
        assert_eq!(iter.next(), 1);
        assert!(!iter.has_next());
    }

    #[test]
    fn test_optimal_edge_case_empty_second() {
        let v1 = vec![1];
        let v2 = Vec::<i32>::new();
        let mut iter = ZigzagIteratorOptimal::new(v1, v2);

        assert!(iter.has_next());
        assert_eq!(iter.next(), 1);
        assert!(!iter.has_next());
    }

    #[test]
    fn test_optimal_stress_case() {
        let v1 = vec![1, 3, 5, 7, 9];
        let v2 = vec![2, 4, 6];
        let mut iter = ZigzagIterator::new(v1, v2);

        let mut result = Vec::new();
        while iter.has_next() {
            result.push(iter.next());
        }

        assert_eq!(result, vec![1, 2, 3, 4, 5, 6, 7, 9]);
    }
}
