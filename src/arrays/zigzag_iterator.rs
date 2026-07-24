//! # 281. Zigzag Iterator
//!
//! Medium
//!
//! <https://leetcode.com/problems/zigzag-iterator/>
//!
//! This problem is a natural fit for Rust's iterator system and demonstrates how custom Iterator implementations and state management with multiple underlying iterators eliminate allocations and provide zero-cost abstractions.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::arrays::zigzag_iterator::ZigzagIterator;
//!
//! let v1 = vec![1, 2];
//! let v2 = vec![3, 4, 5, 6];
//! let mut iter = ZigzagIterator::new(v1, v2);
//!
//! let mut res = Vec::new();
//! while iter.has_next() {
//!     res.push(iter.next());
//! }
//! assert_eq!(res, vec![1, 3, 2, 4, 5, 6]);
//! ```

/// Approach: Brute Force
///
/// In the brute force approach, we pre-compute the merged list by eagerly iterating both arrays and collecting elements into a single flat vector.
/// Time: O(n + m) - To process both arrays
/// Space: O(n + m) - For the flattened vector
pub struct ZigzagIteratorBruteForce {
    data: std::vec::IntoIter<i32>,
}

impl ZigzagIteratorBruteForce {
    #[must_use]
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(v1: Vec<i32>, v2: Vec<i32>) -> Self {
        let mut data = Vec::with_capacity(v1.len() + v2.len());
        let mut i = 0;
        let mut j = 0;

        while i < v1.len() || j < v2.len() {
            if i < v1.len() {
                data.push(v1[i]);
                i += 1;
            }
            if j < v2.len() {
                data.push(v2[j]);
                j += 1;
            }
        }

        Self {
            data: data.into_iter(),
        }
    }

    /// Returns the next element in the zigzag iteration.
    ///
    /// # Panics
    ///
    /// Panics if the iterator is already exhausted (i.e., `has_next()` returns `false`).
    #[must_use]
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> i32 {
        self.data.next().unwrap()
    }

    #[must_use]
    pub fn has_next(&self) -> bool {
        self.data.len() > 0
    }
}

/// Approach: Optimal (Zero-Allocation Iterator)
///
/// We store the two vectors' iterators directly and maintain a boolean state indicating whose turn it is.
/// We do not allocate new lists, and we only advance iterators when requested.
/// Time: O(1) for `next` and `has_next`
/// Space: O(1) (or O(1) auxiliary space beyond the inputs themselves since we consume them)
pub struct ZigzagIteratorOptimal {
    iter1: std::vec::IntoIter<i32>,
    iter2: std::vec::IntoIter<i32>,
    turn: bool,
}

impl ZigzagIteratorOptimal {
    #[must_use]
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(v1: Vec<i32>, v2: Vec<i32>) -> Self {
        Self {
            iter1: v1.into_iter(),
            iter2: v2.into_iter(),
            turn: true,
        }
    }

    /// Returns the next element in the zigzag iteration.
    ///
    /// # Panics
    ///
    /// Panics if the iterator is already exhausted (i.e., `has_next()` returns `false`).
    // RUST INSIGHT: The required LeetCode method is `next`, but idiomatic Rust uses the `Iterator` trait.
    // We suppress the warning for `next` to match LeetCode's API expectations.
    #[must_use]
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> i32 {
        // GOTCHA: We must check `has_next` before calling `next`, as per standard iterator contract,
        // though `unwrap()` is used here assuming the caller checks `has_next()` first.
        if self.turn {
            if self.iter1.len() > 0 {
                self.turn = false;
                self.iter1.next().unwrap()
            } else {
                self.iter2.next().unwrap()
            }
        } else {
            if self.iter2.len() > 0 {
                self.turn = true;
                self.iter2.next().unwrap()
            } else {
                self.iter1.next().unwrap()
            }
        }
    }

    #[must_use]
    pub fn has_next(&self) -> bool {
        self.iter1.len() > 0 || self.iter2.len() > 0
    }
}

pub type ZigzagIterator = ZigzagIteratorOptimal;

// Alternative approaches footer:
// We could also build a generic `ZigzagIterator<I: Iterator>` instead of taking `Vec`s specifically,
// which is more idiomatic. A `VecDeque` of iterators could also extend this to `k` vectors seamlessly.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force_example_1() {
        let v1 = vec![1, 2];
        let v2 = vec![3, 4, 5, 6];
        let mut iter = ZigzagIteratorBruteForce::new(v1, v2);
        let mut res = Vec::new();
        while iter.has_next() {
            res.push(iter.next());
        }
        assert_eq!(res, vec![1, 3, 2, 4, 5, 6]);
    }

    #[test]
    fn test_optimal_example_1() {
        let v1 = vec![1, 2];
        let v2 = vec![3, 4, 5, 6];
        let mut iter = ZigzagIteratorOptimal::new(v1, v2);
        let mut res = Vec::new();
        while iter.has_next() {
            res.push(iter.next());
        }
        assert_eq!(res, vec![1, 3, 2, 4, 5, 6]);
    }

    #[test]
    fn test_optimal_edge_case_empty() {
        let v1 = Vec::<i32>::new();
        let v2 = vec![1];
        let mut iter = ZigzagIteratorOptimal::new(v1, v2);
        let mut res = Vec::new();
        while iter.has_next() {
            res.push(iter.next());
        }
        assert_eq!(res, vec![1]);
    }

    #[test]
    fn test_optimal_stress_boundary() {
        let v1 = vec![1];
        let v2 = Vec::<i32>::new();
        let mut iter = ZigzagIteratorOptimal::new(v1, v2);
        let mut res = Vec::new();
        while iter.has_next() {
            res.push(iter.next());
        }
        assert_eq!(res, vec![1]);
    }
}
