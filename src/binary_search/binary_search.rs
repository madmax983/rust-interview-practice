//! # 704. Binary Search
//!
//! Difficulty: Easy
//! Link: <https://leetcode.com/problems/binary-search>/
//!
//! Given an array of integers `nums` which is sorted in ascending order, and an integer `target`,
//! write a function to search `target` in `nums`. If `target` exists, then return its index.
//! Otherwise, return `-1`.
//!
//! You must write an algorithm with `O(log n)` runtime complexity.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::binary_search::binary_search::search;
//!
//! assert_eq!(search(vec![-1, 0, 3, 5, 9, 12], 9), 4);
//! assert_eq!(search(vec![-1, 0, 3, 5, 9, 12], 2), -1);
//! ```
//!
//! ## Constraints
//!
//! - `1 <= nums.length <= 10^4`
//! - `-10^4 < nums[i], target < 10^4`
//! - All the integers in `nums` are unique.
//! - `nums` is sorted in ascending order.
//!
//! ## Why this matters in Rust
//! This problem emphasizes safe array indexing and basic control flow in Rust.
//! The `std::cmp::Ordering` enum makes binary search comparisons explicit and exhaustive,
//! avoiding nested if-else blocks. Rust's strict typing and index bounds checking prevent
//! out-of-bounds array access that is common in binary search implementations in C++ or C.
//!
//! Note: beyond the standard `brute_force`/`optimized`/`optimal` triad, this file also keeps a
//! bonus `search_std` alternative that delegates to the standard-library `slice::binary_search`.

use std::cmp::Ordering;

/// Brute force approach: Linear Scan
///
/// We just iterate over the array and look for the target.
/// This doesn't satisfy the O(log N) requirement but is perfectly idiomatic Rust for an O(N) search.
///
/// Time: O(N) - we examine every element once in the worst case.
/// Space: O(1) - no extra memory is allocated.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
#[allow(clippy::cast_possible_truncation)] // LeetCode constraints guarantee it fits
#[allow(clippy::cast_possible_wrap)]
pub fn search_brute_force(nums: Vec<i32>, target: i32) -> i32 {
    // RUST INSIGHT:
    // Iterator `.enumerate()` gives us `(index, item)`. We can combine this with `.find()`
    // which short-circuits as soon as the condition is met.
    if let Some((idx, _)) = nums.iter().enumerate().find(|&(_, &val)| val == target) {
        idx as i32
    } else {
        -1
    }
}

/// Optimized approach: Standard Iterative Binary Search using `<` and `>`
///
/// We use two pointers, `left` and `right`, to repeatedly halve the search space.
///
/// Time: O(log N) - halving the search space on each step.
/// Space: O(1) - constant extra space for pointers.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
#[allow(clippy::cast_possible_truncation)] // LeetCode constraints guarantee it fits
#[allow(clippy::cast_possible_wrap)]
pub fn search_optimized(nums: Vec<i32>, target: i32) -> i32 {
    if nums.is_empty() {
        return -1;
    }

    let mut left = 0;
    // GOTCHA:
    // We use `nums.len() - 1` for `right`, which means if `nums` is empty,
    // this would underflow and panic. We guard against this with `is_empty()`.
    let mut right = nums.len() - 1;

    while left <= right {
        // RUST INSIGHT:
        // Using `left + (right - left) / 2` avoids integer overflow that can occur with `(left + right) / 2`.
        let mid = left + (right - left) / 2;

        match nums[mid].cmp(&target) {
            Ordering::Equal => return mid as i32,
            Ordering::Less => left = mid + 1,
            Ordering::Greater => {
                // Check to avoid underflow
                if mid == 0 {
                    break;
                }
                right = mid - 1;
            }
        }
    }

    -1
}

/// Optimal approach: Idiomatic Binary Search with `std::cmp::Ordering`
///
/// This approach uses Rust's `cmp` function which returns an `Ordering` enum.
/// Pattern matching on `Ordering` makes the logic extremely clean and exhaustive.
///
/// Time: O(log N) - binary search.
/// Space: O(1) - iterative approach.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
#[allow(clippy::cast_possible_truncation)] // LeetCode constraints guarantee it fits
#[allow(clippy::cast_possible_wrap)]
pub fn search_optimal(nums: Vec<i32>, target: i32) -> i32 {
    // Instead of `right = len - 1`, we can use `right = len`.
    // This represents an exclusive upper bound: `[left, right)`.
    // This avoids all underflow issues with `- 1` when `mid == 0` or empty array.
    let mut left = 0;
    let mut right = nums.len();

    while left < right {
        let mid = left + (right - left) / 2;

        // RUST INSIGHT:
        // `cmp` is a method available on all types implementing `Ord`.
        // It returns an `Ordering` enum: `Less`, `Equal`, or `Greater`.
        // The `match` statement enforces exhaustiveness, ensuring we handle all 3 cases.
        match nums[mid].cmp(&target) {
            Ordering::Equal => return mid as i32,
            Ordering::Less => left = mid + 1, // Target is greater, search right half
            Ordering::Greater => right = mid, // Target is smaller, search left half (exclusive bound)
        }
    }

    -1
}

/// Alternative approach: Using Standard Library
/// Rust provides a built-in `binary_search` for slices.
/// In a real-world scenario, you would just use `nums.binary_search(&target)`.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
#[allow(clippy::cast_possible_truncation)] // LeetCode constraints guarantee it fits
#[allow(clippy::cast_possible_wrap)]
pub fn search_std(nums: Vec<i32>, target: i32) -> i32 {
    // RUST INSIGHT:
    // `binary_search` returns `Result<usize, usize>`.
    // `Ok(idx)` means it found the element at `idx`.
    // `Err(idx)` means the element was not found, but it should be inserted at `idx`.
    // `map_or` converts `Ok(val)` to `val as i32` and `Err(_)` to `-1`.
    nums.binary_search(&target).map_or(-1, |idx| idx as i32)
}

/// Main entry point - uses optimal solution
#[must_use]
pub fn search(nums: Vec<i32>, target: i32) -> i32 {
    search_optimal(nums, target)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Happy Path
    #[test]
    fn test_brute_force_happy_path() {
        assert_eq!(search_brute_force(vec![-1, 0, 3, 5, 9, 12], 9), 4);
    }

    #[test]
    fn test_optimized_happy_path() {
        assert_eq!(search_optimized(vec![-1, 0, 3, 5, 9, 12], 9), 4);
    }

    #[test]
    fn test_optimal_happy_path() {
        assert_eq!(search_optimal(vec![-1, 0, 3, 5, 9, 12], 9), 4);
    }

    // Edge Cases
    #[test]
    fn test_target_not_found() {
        let nums = vec![-1, 0, 3, 5, 9, 12];
        let target = 2;
        assert_eq!(search_brute_force(nums.clone(), target), -1);
        assert_eq!(search_optimized(nums.clone(), target), -1);
        assert_eq!(search_optimal(nums, target), -1);
    }

    #[test]
    fn test_empty_array() {
        let nums = vec![];
        let target = 5;
        assert_eq!(search_brute_force(nums.clone(), target), -1);
        assert_eq!(search_optimized(nums.clone(), target), -1);
        assert_eq!(search_optimal(nums, target), -1);
    }

    #[test]
    fn test_single_element_found() {
        let nums = vec![5];
        let target = 5;
        assert_eq!(search_brute_force(nums.clone(), target), 0);
        assert_eq!(search_optimized(nums.clone(), target), 0);
        assert_eq!(search_optimal(nums, target), 0);
    }

    #[test]
    fn test_single_element_not_found() {
        let nums = vec![5];
        let target = 2;
        assert_eq!(search_brute_force(nums.clone(), target), -1);
        assert_eq!(search_optimized(nums.clone(), target), -1);
        assert_eq!(search_optimal(nums, target), -1);
    }

    // Stress / Boundary Cases
    #[test]
    fn test_target_is_first_element() {
        let nums = vec![1, 2, 3, 4, 5];
        let target = 1;
        assert_eq!(search_brute_force(nums.clone(), target), 0);
        assert_eq!(search_optimized(nums.clone(), target), 0);
        assert_eq!(search_optimal(nums, target), 0);
    }

    #[test]
    fn test_target_is_last_element() {
        let nums = vec![1, 2, 3, 4, 5];
        let target = 5;
        assert_eq!(search_brute_force(nums.clone(), target), 4);
        assert_eq!(search_optimized(nums.clone(), target), 4);
        assert_eq!(search_optimal(nums, target), 4);
    }

    #[test]
    fn test_target_out_of_bounds_left() {
        let nums = vec![1, 2, 3, 4, 5];
        let target = 0;
        assert_eq!(search_brute_force(nums.clone(), target), -1);
        assert_eq!(search_optimized(nums.clone(), target), -1);
        assert_eq!(search_optimal(nums, target), -1);
    }

    #[test]
    fn test_target_out_of_bounds_right() {
        let nums = vec![1, 2, 3, 4, 5];
        let target = 6;
        assert_eq!(search_brute_force(nums.clone(), target), -1);
        assert_eq!(search_optimized(nums.clone(), target), -1);
        assert_eq!(search_optimal(nums, target), -1);
    }

    #[test]
    fn test_all_approaches_equality() {
        let nums = vec![2, 4, 6, 8, 10, 12, 14, 16];
        let targets = vec![1, 2, 4, 7, 10, 15, 16, 20];

        for t in targets {
            let res1 = search_brute_force(nums.clone(), t);
            let res2 = search_optimized(nums.clone(), t);
            let res3 = search_optimal(nums.clone(), t);
            let res4 = search_std(nums.clone(), t);
            let res5 = search(nums.clone(), t);

            assert_eq!(res1, res2);
            assert_eq!(res2, res3);
            assert_eq!(res3, res4);
            assert_eq!(res4, res5);
        }
    }
}
