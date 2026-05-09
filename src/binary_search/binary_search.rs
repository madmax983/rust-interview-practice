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
//! This problem matters in Rust because it perfectly demonstrates safe array indexing, handling integer types (e.g. `i32` vs `usize`),
//! and the power of Rust's `std::cmp::Ordering` enum within a `match` expression to guarantee exhaustive control flow compared to `if/else` chains.
//!
//! ## Approach
//!
//! - **Brute Force:** Iterate over the array to find the target. Time: `O(N)`, Space: `O(1)`.
//! - **Iterative (Standard):** Maintain `left` and `right` pointers and narrow down the search space by half each iteration.
//!   Time: `O(log N)`, Space: `O(1)`.
//!   In Rust, using `std::cmp::Ordering` in a `match` expression ensures we handle `<, ==, >` safely and clearly.
//! - **Optimal (Idiomatic):** Using Rust's standard library `binary_search` method. Time: `O(log N)`, Space: `O(1)`.
//!
//! ## Alternative Approaches
//!
//! - **Recursion:** A recursive binary search could be used, but iteration avoids stack depth issues and is more common.

use std::cmp::Ordering;

/// Brute Force Approach: Linear Search
/// Iterates through the vector from start to finish looking for the target.
/// Time: O(N)
/// Space: O(1)
#[must_use]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::needless_pass_by_value
)]
pub fn search_brute_force(nums: Vec<i32>, target: i32) -> i32 {
    // RUST INSIGHT: `.iter().position()` is an idiomatic way to find an index instead of a manual loop.
    match nums.iter().position(|&x| x == target) {
        Some(idx) => idx as i32,
        None => -1,
    }
}

/// Iterative Approach: Two Pointers
/// Narrows the search space by half each step.
/// Time: O(log N)
/// Space: O(1)
#[must_use]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::needless_pass_by_value
)]
pub fn search_iterative(nums: Vec<i32>, target: i32) -> i32 {
    if nums.is_empty() {
        return -1;
    }

    let mut left = 0;
    // GOTCHA: Right pointer needs to be `nums.len() - 1`, but `nums.len()` is `usize`.
    // If the array is empty, `0 - 1` would underflow and panic in Rust. We checked `is_empty` above!
    let mut right = nums.len() - 1;

    while left <= right {
        // Prevent overflow by doing `left + (right - left) / 2` instead of `(left + right) / 2`.
        let mid = left + (right - left) / 2;

        // RUST INSIGHT: `std::cmp::Ordering` and `cmp` guarantee exhaustiveness.
        // It forces us to explicitly handle Less, Equal, and Greater.
        match nums[mid].cmp(&target) {
            Ordering::Equal => return mid as i32,
            Ordering::Less => {
                // target is greater, so search right half
                left = mid + 1;
            }
            Ordering::Greater => {
                // target is less, so search left half
                // GOTCHA: We must be careful about `usize` underflow if `mid == 0`.
                if mid == 0 {
                    break;
                }
                right = mid - 1;
            }
        }
    }

    -1
}

/// Optimal Approach: Standard Library
/// Uses the built-in slice `binary_search` which is highly optimized.
/// Time: O(log N)
/// Space: O(1)
#[must_use]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::needless_pass_by_value
)]
pub fn search_optimal(nums: Vec<i32>, target: i32) -> i32 {
    // RUST INSIGHT: `binary_search` returns a `Result`.
    // `Ok(idx)` if found, `Err(idx)` where it could be inserted.
    match nums.binary_search(&target) {
        Ok(idx) => idx as i32,
        Err(_) => -1,
    }
}

/// Main entry point
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn search(nums: Vec<i32>, target: i32) -> i32 {
    search_optimal(nums, target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force() {
        assert_eq!(search_brute_force(vec![-1, 0, 3, 5, 9, 12], 9), 4); // Happy path
        assert_eq!(search_brute_force(vec![-1, 0, 3, 5, 9, 12], 2), -1); // Not found
        assert_eq!(search_brute_force(vec![], 5), -1); // Edge case: empty
        assert_eq!(search_brute_force(vec![5], 5), 0); // Edge case: single element
    }

    #[test]
    fn test_iterative() {
        assert_eq!(search_iterative(vec![-1, 0, 3, 5, 9, 12], 9), 4);
        assert_eq!(search_iterative(vec![-1, 0, 3, 5, 9, 12], 2), -1);
        assert_eq!(search_iterative(vec![], 5), -1);
        assert_eq!(search_iterative(vec![5], 5), 0);
        // Boundary case to trigger the mid == 0 check on `Greater`
        assert_eq!(search_iterative(vec![5], 2), -1);
    }

    #[test]
    fn test_optimal() {
        assert_eq!(search_optimal(vec![-1, 0, 3, 5, 9, 12], 9), 4);
        assert_eq!(search_optimal(vec![-1, 0, 3, 5, 9, 12], 2), -1);
        assert_eq!(search_optimal(vec![], 5), -1);
        assert_eq!(search_optimal(vec![5], 5), 0);
    }
}
