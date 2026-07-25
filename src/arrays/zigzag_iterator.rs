//! # 281. Zigzag Iterator
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/zigzag-iterator/>
//!
//! This problem is a natural fit for Rust's iterator system and demonstrates how custom
//! structs can orchestrate multiple data streams. It highlights why implementing traits
//! is a core part of idiomatic Rust, allowing custom types to seamlessly integrate with
//! the standard library's `Iterator` combinators.
//!
//! ## Approach
//!
//! Instead of allocating a new vector and pre-computing the interleaved elements (brute force),
//! we store the underlying iterators and alternate between them dynamically. This yields a
//! zero-allocation, O(1) space optimal solution. In C++ or Java, managing multiple
//! iterators can be error-prone and requires careful boundary checking. In Rust, we leverage
//! the `Iterator` trait and the `Option` enum to safely consume values.
//!
//! We provide two implementations:
//! 1. `GenericZigzagIter`: A purely idiomatic, generic implementation using Rust's `Iterator` trait.
//! 2. `ZigzagIterator`: A specific wrapper to satisfy the `LeetCode` platform's required `next()`
//!    and `has_next()` methods, utilizing `std::vec::IntoIter` which implements `ExactSizeIterator`.
//!
//! ## Alternative Approaches
//! 1. **Brute Force (Pre-computation)**: Iterate through both arrays and push elements
//!    into a new `Vec` alternately during initialization. This uses O(n + m) extra space.
//! 2. **Queue of Iterators**: Store iterators in a `VecDeque` and pop/push them from the front
//!    to back. This is more scalable for *k* arrays, though slightly overkill for just two.

/// A generic Zigzag iterator that yields elements from two underlying iterators alternately.
///
/// RUST INSIGHT: By defining a generic struct and applying trait bounds in the `impl` block,
/// we ensure this works for *any* type that implements `Iterator`. This zero-cost abstraction
/// allows seamless integration with standard iterator combinators (like `.map()` or `.filter()`).
pub struct GenericZigzagIter<I, J> {
    iter1: I,
    iter2: J,
    turn_first: bool,
}

impl<I, J> GenericZigzagIter<I, J> {
    /// Creates a new `GenericZigzagIter`.
    pub const fn new(iter1: I, iter2: J) -> Self {
        Self {
            iter1,
            iter2,
            turn_first: true,
        }
    }
}

// GOTCHA: We place the trait bounds `where I: Iterator<Item = T>, J: Iterator<Item = T>`
// on the `impl` block rather than the struct definition itself. This is idiomatic Rust,
// as it avoids cluttering the struct with bounds that are only necessary for the methods.
impl<I, J, T> Iterator for GenericZigzagIter<I, J>
where
    I: Iterator<Item = T>,
    J: Iterator<Item = T>,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.turn_first {
            self.turn_first = false;
            // RUST INSIGHT: `Option::or_else` beautifully handles the fallback logic
            // without explicit `match` statements or null checks.
            self.iter1.next().or_else(|| self.iter2.next())
        } else {
            self.turn_first = true;
            self.iter2.next().or_else(|| self.iter1.next())
        }
    }
}

/// The specific struct for `LeetCode` #281 compatibility.
pub struct ZigzagIterator {
    iter1: std::vec::IntoIter<i32>,
    iter2: std::vec::IntoIter<i32>,
    turn_first: bool,
}

impl ZigzagIterator {
    /// Creates a new `ZigzagIterator` from two vectors.
    #[must_use]
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(v1: Vec<i32>, v2: Vec<i32>) -> Self {
        Self {
            // GOTCHA: We use `into_iter()` to take ownership of the vector's heap allocation,
            // avoiding unnecessary clones. `.iter()` would yield references `&i32`.
            iter1: v1.into_iter(),
            iter2: v2.into_iter(),
            turn_first: true,
        }
    }

    /// Returns the next element.
    ///
    /// # Panics
    ///
    /// Panics if called when there is no next element.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> i32 {
        // RUST INSIGHT: `unwrap()` is safe here because LeetCode guarantees `next()`
        // is only called if `has_next()` is true. However, in production Rust, we would
        // return an `Option<i32>` instead to enforce exhaustive handling by the caller.
        if self.turn_first {
            if self.iter1.len() > 0 {
                self.turn_first = false;
                self.iter1.next().unwrap()
            } else {
                self.iter2.next().unwrap()
            }
        } else {
            if self.iter2.len() > 0 {
                self.turn_first = true;
                self.iter2.next().unwrap()
            } else {
                self.iter1.next().unwrap()
            }
        }
    }

    /// Returns true if the iterator has more elements.
    #[must_use]
    pub fn has_next(&self) -> bool {
        self.iter1.len() > 0 || self.iter2.len() > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generic_zigzag_happy_path() {
        let v1 = vec![1, 2];
        let v2 = vec![3, 4, 5, 6];

        let iter = GenericZigzagIter::new(v1.into_iter(), v2.into_iter());
        let result: Vec<i32> = iter.collect();
        assert_eq!(result, vec![1, 3, 2, 4, 5, 6]);
    }

    #[test]
    fn test_generic_zigzag_edge_case_empty_first() {
        let v1: Vec<i32> = Vec::new();
        let v2 = vec![1, 2, 3];

        let iter = GenericZigzagIter::new(v1.into_iter(), v2.into_iter());
        let result: Vec<i32> = iter.collect();
        assert_eq!(result, vec![1, 2, 3]);
    }

    #[test]
    fn test_generic_zigzag_stress_both_empty() {
        let v1 = Vec::<i32>::new();
        let v2 = Vec::<i32>::new();

        let iter = GenericZigzagIter::new(v1.into_iter(), v2.into_iter());
        let result: Vec<i32> = iter.collect();
        assert_eq!(result, Vec::<i32>::new());
    }

    #[test]
    fn test_leetcode_struct_happy_path() {
        let v1 = vec![1, 2];
        let v2 = vec![3, 4, 5, 6];

        let mut iterator = ZigzagIterator::new(v1, v2);
        let mut result = Vec::new();

        while iterator.has_next() {
            result.push(iterator.next());
        }

        assert_eq!(result, vec![1, 3, 2, 4, 5, 6]);
    }

    #[test]
    fn test_leetcode_struct_edge_case_empty_second() {
        let v1 = vec![1, 2, 3];
        let v2 = Vec::new();

        let mut iterator = ZigzagIterator::new(v1, v2);
        let mut result = Vec::new();

        while iterator.has_next() {
            result.push(iterator.next());
        }

        assert_eq!(result, vec![1, 2, 3]);
    }

    #[test]
    fn test_leetcode_struct_boundary_single_elements() {
        let v1 = vec![1];
        let v2 = vec![2];

        let mut iterator = ZigzagIterator::new(v1, v2);
        let mut result = Vec::new();

        while iterator.has_next() {
            result.push(iterator.next());
        }

        assert_eq!(result, vec![1, 2]);
    }
}
