//! # 281. Zigzag Iterator
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/zigzag-iterator/>
//!
//! Why this matters in Rust: This problem is a classic exercise in building custom stateful
//! iterators. It demonstrates how to implement the `Iterator` trait, manage state across
//! multiple underlying iterators, and apply trait bounds, which provides a great educational
//! contrast between brute force allocation and zero-allocation optimal solutions.

/// **Approach 1: Brute Force (Pre-computation)**
///
/// **Algorithm:**
/// In the constructor, iterate through both lists alternately and push the elements into
/// a single queue (or vector). Then `next()` just pops from the queue.
///
/// **Complexity:**
/// - **Time:** O(N) where N is the total number of elements in both vectors, for initialization.
///   O(1) per `next()` or `has_next()` call.
/// - **Space:** O(N) to store all elements in a new data structure.
///
/// **Why it's not idiomatic Rust:**
/// Rust's iterators are designed to be lazy and zero-allocation when possible. Collecting
/// everything upfront defeats the purpose of an iterator.
pub struct ZigzagIteratorBruteForce {
    queue: std::collections::VecDeque<i32>,
}

impl ZigzagIteratorBruteForce {
    #[must_use]
    pub fn new(v1: Vec<i32>, v2: Vec<i32>) -> Self {
        let mut queue = std::collections::VecDeque::with_capacity(v1.len() + v2.len());
        let mut it1 = v1.into_iter();
        let mut it2 = v2.into_iter();

        loop {
            let val1 = it1.next();
            let val2 = it2.next();

            if val1.is_none() && val2.is_none() {
                break;
            }
            if let Some(v) = val1 {
                queue.push_back(v);
            }
            if let Some(v) = val2 {
                queue.push_back(v);
            }
        }

        Self { queue }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> i32 {
        // RUST INSIGHT: The problem signature expects us to return an i32 and panic if empty,
        // or assumes has_next() is called first. In a true Rust Iterator, this would return Option<i32>.
        // Using clippy::should_implement_trait to silence the warning while matching LeetCode's API.
        self.queue.pop_front().unwrap_or(0)
    }

    #[must_use]
    pub fn has_next(&self) -> bool {
        !self.queue.is_empty()
    }
}

/// **Approach 2: Optimal (Lazy Iteration with `IntoIter`)**
///
/// **Algorithm:**
/// Instead of storing elements, we store the iterators themselves. We maintain a boolean flag
/// (or just use a queue of iterators) to know which iterator to pull from next.
/// This approach scales nicely if we wanted to generalize to `k` vectors by keeping a
/// queue of active iterators.
///
/// **Complexity:**
/// - **Time:** O(1) for initialization, O(1) for `next()` and `has_next()`.
/// - **Space:** O(1) or O(k) where k is the number of iterators (2 in this case).
///
/// **Idiomatic Rust:**
/// This leverages `std::vec::IntoIter` to take ownership of the vectors without allocating
/// new collections. This is exactly how one should build custom lazy iterators in Rust.
pub struct ZigzagIterator {
    iters: std::collections::VecDeque<std::vec::IntoIter<i32>>,
}

impl ZigzagIterator {
    #[must_use]
    pub fn new(v1: Vec<i32>, v2: Vec<i32>) -> Self {
        let mut iters = std::collections::VecDeque::with_capacity(2);

        let it1 = v1.into_iter();
        // GOTCHA: We must only add iterators that actually have elements left.
        // Otherwise has_next() logic becomes more complex.
        // RUST INSIGHT: ExactSizeIterator allows us to check length efficiently.
        if it1.len() > 0 {
            iters.push_back(it1);
        }

        let it2 = v2.into_iter();
        if it2.len() > 0 {
            iters.push_back(it2);
        }

        Self { iters }
    }

    /// # Panics
    ///
    /// Panics if called when there are no elements left to iterate.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> i32 {
        if let Some(mut it) = self.iters.pop_front() {
            // RUST INSIGHT: We unwrap here because our invariant guarantees
            // that any iterator in the queue has at least one element.
            let val = it.next().unwrap();

            // If the iterator still has elements, put it at the back of the queue
            if it.len() > 0 {
                self.iters.push_back(it);
            }

            val
        } else {
            // Panic or return default. LeetCode implies has_next is checked.
            panic!("Called next() on empty iterator");
        }
    }

    #[must_use]
    pub fn has_next(&self) -> bool {
        // Because of our invariant (only non-empty iterators are in the queue),
        // we just need to check if the queue is empty.
        !self.iters.is_empty()
    }
}

/// **Alternative Approaches:**
/// 1. **Index-based:** Keep references `&Vec<i32>` and two indices `i`, `j`. Useful if you
///    don't want to consume the vectors, but requires lifetimes `&'a Vec<i32>`.
/// 2. **`std::iter::Chain` and `zip`:** You can theoretically construct a complex built-in
///    iterator chain, but handling vectors of different lengths (which `zip` truncates) makes
///    the standard combinators tricky without `itertools::Itertools::interleave`.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force_example_1() {
        let v1 = vec![1, 2];
        let v2 = vec![3, 4, 5, 6];
        let mut i = ZigzagIteratorBruteForce::new(v1, v2);
        let mut res = Vec::new();
        while i.has_next() {
            res.push(i.next());
        }
        assert_eq!(res, vec![1, 3, 2, 4, 5, 6]);
    }

    #[test]
    fn test_optimal_example_1() {
        let v1 = vec![1, 2];
        let v2 = vec![3, 4, 5, 6];
        let mut i = ZigzagIterator::new(v1, v2);
        let mut res = Vec::new();
        while i.has_next() {
            res.push(i.next());
        }
        assert_eq!(res, vec![1, 3, 2, 4, 5, 6]);
    }

    #[test]
    fn test_optimal_empty_first() {
        let v1 = vec![];
        let v2 = vec![1];
        let mut i = ZigzagIterator::new(v1, v2);
        let mut res = Vec::new();
        while i.has_next() {
            res.push(i.next());
        }
        assert_eq!(res, vec![1]);
    }

    #[test]
    fn test_optimal_both_empty() {
        let v1 = Vec::<i32>::new();
        let v2 = Vec::<i32>::new();
        let i = ZigzagIterator::new(v1, v2);
        assert!(!i.has_next());
    }

    // Cross-implementation verification
    #[test]
    fn test_all_approaches_edge_case() {
        let v1 = vec![1, 1, 1, 1];
        let v2 = vec![2, 2];

        let mut i_brute = ZigzagIteratorBruteForce::new(v1.clone(), v2.clone());
        let mut res_brute = Vec::new();
        while i_brute.has_next() {
            res_brute.push(i_brute.next());
        }

        let mut i_opt = ZigzagIterator::new(v1, v2);
        let mut res_opt = Vec::new();
        while i_opt.has_next() {
            res_opt.push(i_opt.next());
        }

        assert_eq!(res_brute, vec![1, 2, 1, 2, 1, 1]);
        assert_eq!(res_brute, res_opt);
    }
}
