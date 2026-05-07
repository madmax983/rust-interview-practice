//! # 704. Binary Search
//!
//! Difficulty: Easy
//! Link: https://leetcode.com/problems/binary-search/
//!
//! Given an array of integers `nums` which is sorted in ascending order, and an integer `target`,
//! write a function to search `target` in `nums`. If `target` exists, then return its index.
//! Otherwise, return `-1`.
//!
//! You must write an algorithm with `O(log n)` runtime complexity.
//!
//! ## Why this matters in Rust
//! This problem demonstrates safe array indexing, basic control flow, and Rust's `std::cmp::Ordering`
//! for match exhaustiveness. It highlights how pattern matching can eliminate entire classes of
//! logical errors (like using `<` when you meant `<=`) by forcing you to handle `Less`, `Greater`,
//! and `Equal` explicitly.
//!
//! ## Approach
//! We implement three solutions:
//! 1. **Brute Force:** A simple O(N) linear scan to demonstrate the baseline.
//! 2. **Iterative Binary Search:** The standard O(log N) approach. We maintain `left` and `right`
//!    pointers, calculating `mid = left + (right - left) / 2` to avoid integer overflow.
//! 3. **Optimal:** Using the idiomatic standard library `slice::binary_search` which provides
//!    zero-cost abstractions and is heavily optimized.

use std::cmp::Ordering;

/// Brute force approach: Linear scan
/// Time: O(n) - examines each element
/// Space: O(1) - no extra space used
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
#[allow(clippy::cast_possible_wrap)] // Safe because array indices for this problem fit in i32
pub fn search_brute_force(nums: Vec<i32>, target: i32) -> i32 {
    // RUST INSIGHT: We can use `.iter().enumerate()` to safely access both the index and value
    // without manual bounds checking or loop counters.
    for (i, &num) in nums.iter().enumerate() {
        if num == target {
            return i as i32;
        }
    }
    -1
}

/// Iterative approach: Standard binary search
/// Time: O(log n) - halves search space each iteration
/// Space: O(1) - only uses a few pointers
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
#[allow(clippy::cast_possible_wrap)]
pub fn search_iterative(nums: Vec<i32>, target: i32) -> i32 {
    if nums.is_empty() {
        return -1;
    }

    let mut left = 0;
    // GOTCHA: Pay attention to `right`. If it's initialized to `nums.len() - 1` and `nums` is empty,
    // this would panic due to underflow. We guard against this with the `is_empty()` check above.
    let mut right = nums.len() - 1;

    while left <= right {
        // Calculate mid safely to avoid integer overflow.
        // Even though standard `(left + right) / 2` is fine for normal vectors in Rust (since
        // max capacity is well below usize::MAX / 2), this is the universally safe pattern.
        let mid = left + (right - left) / 2;

        // RUST INSIGHT: This match is exhaustive. The compiler guarantees we handle all cases
        // (Less, Greater, Equal) explicitly, unlike an `if-else` chain which might miss a branch.
        match nums[mid].cmp(&target) {
            Ordering::Equal => return mid as i32,
            Ordering::Less => {
                // If mid is less than target, target must be in the right half
                left = mid + 1;
            }
            Ordering::Greater => {
                // If mid is greater than target, target must be in the left half
                // If right was 0, subtracting 1 would underflow usize.
                // We handle this gracefully by checking if mid is 0.
                if mid == 0 {
                    break;
                }
                right = mid - 1;
            }
        }
    }

    -1
}

/// Optimal approach: Idiomatic standard library usage
/// Time: O(log n) - internally implements binary search
/// Space: O(1) - zero allocation abstraction
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
#[allow(clippy::cast_possible_wrap)]
pub fn search_optimal(nums: Vec<i32>, target: i32) -> i32 {
    // RUST INSIGHT: `binary_search` returns a Result.
    // - Ok(index) means the element was found.
    // - Err(index) means the element was not found, but indicates where it *could* be inserted.
    match nums.binary_search(&target) {
        Ok(idx) => idx as i32,
        Err(_) => -1,
    }
}

/// Main entry point - uses optimal solution
#[must_use]
pub fn search(nums: Vec<i32>, target: i32) -> i32 {
    search_optimal(nums, target)
}

// Alternative Approaches
// ----------------------
// 1. **Recursive Binary Search**: It's elegant, but standard recursion in Rust (without TCO)
//    will consume O(log n) stack space. In performance-critical scenarios, iterative or
//    `slice::binary_search` is preferred.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force() {
        // Happy path
        assert_eq!(search_brute_force(vec![-1, 0, 3, 5, 9, 12], 9), 4);
        assert_eq!(search_brute_force(vec![-1, 0, 3, 5, 9, 12], 2), -1);

        // Edge cases
        assert_eq!(search_brute_force(vec![], 5), -1);
        assert_eq!(search_brute_force(vec![5], 5), 0);
        assert_eq!(search_brute_force(vec![5], 2), -1);
    }

    #[test]
    fn test_iterative() {
        // Happy path
        assert_eq!(search_iterative(vec![-1, 0, 3, 5, 9, 12], 9), 4);
        assert_eq!(search_iterative(vec![-1, 0, 3, 5, 9, 12], 2), -1);

        // Edge cases
        assert_eq!(search_iterative(vec![], 5), -1);
        assert_eq!(search_iterative(vec![5], 5), 0);
        assert_eq!(search_iterative(vec![5], 2), -1);

        // Boundary cases (first and last elements)
        assert_eq!(search_iterative(vec![1, 2, 3, 4, 5], 1), 0);
        assert_eq!(search_iterative(vec![1, 2, 3, 4, 5], 5), 4);
        assert_eq!(search_iterative(vec![1, 2, 3, 4, 5], 0), -1); // smaller than all
        assert_eq!(search_iterative(vec![1, 2, 3, 4, 5], 6), -1); // larger than all
    }

    #[test]
    fn test_optimal() {
        // Happy path
        assert_eq!(search_optimal(vec![-1, 0, 3, 5, 9, 12], 9), 4);
        assert_eq!(search_optimal(vec![-1, 0, 3, 5, 9, 12], 2), -1);

        // Edge cases
        assert_eq!(search_optimal(vec![], 5), -1);
        assert_eq!(search_optimal(vec![5], 5), 0);
        assert_eq!(search_optimal(vec![5], 2), -1);

        // Stress case
        let large_nums: Vec<i32> = (0..10_000).collect();
        assert_eq!(search_optimal(large_nums.clone(), 5000), 5000);
        assert_eq!(search_optimal(large_nums, 10_001), -1);
    }
}
