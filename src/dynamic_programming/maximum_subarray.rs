//! # 53. Maximum Subarray
//!
//! **Difficulty: Medium**
//!
//! [LeetCode Problem 53](https://leetcode.com/problems/maximum-subarray/)
//!
//! Given an integer array `nums`, find the subarray which has the largest sum and return its sum.
//!
//! ## Why this matters in Rust
//! This problem is the textbook example of **Kadane's Algorithm** (Dynamic Programming).
//! It highlights:
//! -   **Iterators**: Using `iter().fold` to express state transitions concisely.
//! -   **State Management**: Tracking local vs global maximums.
//! -   **Efficiency**: Achieving O(N) time with O(1) space, avoiding unnecessary allocations.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::dynamic_programming::maximum_subarray::max_sub_array;
//!
//! let nums = vec![-2,1,-3,4,-1,2,1,-5,4];
//! assert_eq!(max_sub_array(nums), 6);
//! // The subarray [4,-1,2,1] has the largest sum = 6.
//! ```
//!
//! ## Constraints
//!
//! - `1 <= nums.length <= 10^5`
//! - `-10^4 <= nums[i] <= 10^4`

use std::cmp;

/// Brute Force Approach: Check All Subarrays
///
/// **Strategy**:
/// Iterate through all possible start `i` and end `j` indices.
/// Compute the sum for each subarray `nums[i..=j]` and track the maximum.
///
/// **Time**: O(N^2) - Nested loops. Sum calculation is incremental, so inner work is O(1).
/// **Space**: O(1) - Constant extra space.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn max_sub_array_brute_force(nums: Vec<i32>) -> i32 {
    let n = nums.len();
    let mut max_sum = i32::MIN;

    for i in 0..n {
        let mut current_sum = 0;
        for j in i..n {
            current_sum += nums[j];
            max_sum = cmp::max(max_sum, current_sum);
        }
    }

    max_sum
}

/// Optimal Approach: Kadane's Algorithm
///
/// **Strategy**:
/// Iterate through the array, maintaining a `current_sum`.
/// At each step `i`, we have a choice:
/// 1. Extend the existing subarray: `current_sum + nums[i]`
/// 2. Start a new subarray at `i`: `nums[i]`
///
/// Logic: If `current_sum` becomes negative, adding it to the next element will only decrease the sum. So we should discard it and start fresh.
/// `current_sum = max(nums[i], current_sum + nums[i])`
/// `global_max = max(global_max, current_sum)`
///
/// **Time**: O(N) - Single pass.
/// **Space**: O(1) - Only two variables.
///
/// # RUST INSIGHT
/// We can implement this imperatively with a loop or functionally with `fold`.
/// The imperative version is often clearer for this specific logic because `fold` accumulates a single state,
/// but here we need to track two values (`current_max` and `global_max`).
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn max_sub_array_kadane(nums: Vec<i32>) -> i32 {
    // Constraint: 1 <= nums.length. Safe to access index 0.
    let mut current_sum = nums[0];
    let mut max_sum = nums[0];

    // Start from the second element
    for &num in nums.iter().skip(1) {
        // Should we start new at `num` or extend `current_sum`?
        // If `current_sum` is negative, `num` > `current_sum + num` (assuming num could be anything).
        // Actually simpler: max(num, current_sum + num) covers both cases.
        current_sum = cmp::max(num, current_sum + num);

        // Update global max
        max_sum = cmp::max(max_sum, current_sum);
    }

    max_sum
}

/// Functional approach using `fold`
///
/// Demonstrates how to carry complex state (current_max, global_max) through an iterator chain.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn max_sub_array_functional(nums: Vec<i32>) -> i32 {
    // We init with the first element, so we skip(1).
    // State tuple: (current_sum, max_sum)
    // RUST INSIGHT: `fold` accumulates state. Here the accumulator is `(i32, i32)`.
    let max_sum = nums
        .iter()
        .skip(1)
        .fold((nums[0], nums[0]), |(curr, max_so_far), &num| {
            let new_curr = cmp::max(num, curr + num);
            let new_max = cmp::max(max_so_far, new_curr);
            (new_curr, new_max)
        })
        .1; // Extract max_sum

    max_sum
}

/// Main entry point - uses optimal solution
#[must_use]
pub fn max_sub_array(nums: Vec<i32>) -> i32 {
    max_sub_array_kadane(nums)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_example_1() {
        let nums = vec![-2, 1, -3, 4, -1, 2, 1, -5, 4];
        // [4, -1, 2, 1] sum = 6
        assert_eq!(max_sub_array(nums.clone()), 6);
        assert_eq!(max_sub_array_brute_force(nums.clone()), 6);
        assert_eq!(max_sub_array_functional(nums), 6);
    }

    #[test]
    fn test_example_2() {
        let nums = vec![1];
        assert_eq!(max_sub_array(nums.clone()), 1);
        assert_eq!(max_sub_array_brute_force(nums.clone()), 1);
        assert_eq!(max_sub_array_functional(nums), 1);
    }

    #[test]
    fn test_example_3() {
        let nums = vec![5, 4, -1, 7, 8];
        // All positive except -1. Entire array sum: 5+4-1+7+8 = 23
        assert_eq!(max_sub_array(nums.clone()), 23);
        assert_eq!(max_sub_array_brute_force(nums.clone()), 23);
        assert_eq!(max_sub_array_functional(nums), 23);
    }

    #[test]
    fn test_all_negative() {
        // Max subarray is the single largest element (least negative)
        let nums = vec![-5, -2, -9, -1, -8];
        assert_eq!(max_sub_array(nums.clone()), -1);
        assert_eq!(max_sub_array_brute_force(nums.clone()), -1);
        assert_eq!(max_sub_array_functional(nums), -1);
    }

    #[test]
    fn test_mixed_with_large_negative() {
        // Large negative breaks the chain
        // [10, 20, -100, 5, 5] -> max is 30 (10+20) or 10 (5+5). Should be 30.
        let nums = vec![10, 20, -100, 5, 5];
        assert_eq!(max_sub_array(nums.clone()), 30);
        assert_eq!(max_sub_array_brute_force(nums.clone()), 30);
        assert_eq!(max_sub_array_functional(nums), 30);
    }
}
