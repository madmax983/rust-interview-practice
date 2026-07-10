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
        for &val in &nums[i..] {
            current_sum += val;
            max_sum = cmp::max(max_sum, current_sum);
        }
    }

    max_sum
}

/// Optimized Approach: Divide and Conquer
///
/// **Strategy**:
/// Split the array in half. The maximum subarray is either entirely in the left half,
/// entirely in the right half, or it crosses the midpoint. We recursively solve the two
/// halves and compute the best crossing sum by expanding outward from the midpoint.
///
/// **Time**: O(N log N) - We split the array log N times, doing O(N) work (the crossing scan) per level.
/// **Space**: O(log N) - Recursion stack depth.
///
/// # RUST INSIGHT
/// Slices (`&[i32]`) let us recurse over sub-ranges without copying the underlying data,
/// so each recursive call is a cheap fat pointer rather than a new allocation.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn max_sub_array_optimized(nums: Vec<i32>) -> i32 {
    fn solve(nums: &[i32]) -> i32 {
        // Base case: a single element is its own maximum subarray.
        if nums.len() == 1 {
            return nums[0];
        }

        let mid = nums.len() / 2;
        let left_best = solve(&nums[..mid]);
        let right_best = solve(&nums[mid..]);

        // Best sum of a subarray that must include the element at `mid - 1` (expanding left).
        let mut sum = 0;
        let mut cross_left = i32::MIN;
        for &v in nums[..mid].iter().rev() {
            sum += v;
            cross_left = cmp::max(cross_left, sum);
        }

        // Best sum of a subarray that must include the element at `mid` (expanding right).
        sum = 0;
        let mut cross_right = i32::MIN;
        for &v in &nums[mid..] {
            sum += v;
            cross_right = cmp::max(cross_right, sum);
        }

        let cross_best = cross_left + cross_right;
        cmp::max(cmp::max(left_best, right_best), cross_best)
    }

    solve(&nums)
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
pub fn max_sub_array_optimal(nums: Vec<i32>) -> i32 {
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

/// Functional-style variant of the optimal Kadane approach using `fold`.
///
/// NOTE: This shares the same O(N) time / O(1) space profile as `max_sub_array_optimal`; it is kept
/// under a clearly-named suffix to demonstrate carrying complex state (`current_max`, `global_max`)
/// through an iterator chain, which is a core idiom this repository practices.
///
/// **Time**: O(N) - Single pass.
/// **Space**: O(1) - Only the accumulator tuple.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn max_sub_array_functional(nums: Vec<i32>) -> i32 {
    // We init with the first element, so we skip(1).
    // State tuple: (current_sum, max_sum)
    // RUST INSIGHT: `fold` accumulates state. Here the accumulator is `(i32, i32)`.
    // Extract max_sum

    nums.iter()
        .skip(1)
        .fold((nums[0], nums[0]), |(curr, max_so_far), &num| {
            let new_curr = cmp::max(num, curr + num);
            let new_max = cmp::max(max_so_far, new_curr);
            (new_curr, new_max)
        })
        .1
}

/// Main entry point - uses optimal solution
#[must_use]
pub fn max_sub_array(nums: Vec<i32>) -> i32 {
    max_sub_array_optimal(nums)
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
        assert_eq!(max_sub_array_optimized(nums.clone()), 30);
        assert_eq!(max_sub_array_optimal(nums.clone()), 30);
        assert_eq!(max_sub_array_functional(nums), 30);
    }

    #[test]
    fn test_optimized_divide_and_conquer() {
        let nums = vec![-2, 1, -3, 4, -1, 2, 1, -5, 4];
        assert_eq!(max_sub_array_optimized(nums), 6);
        // Single element (base case) and all-negative behaviour.
        assert_eq!(max_sub_array_optimized(vec![-3]), -3);
        assert_eq!(max_sub_array_optimized(vec![-5, -2, -9, -1, -8]), -1);
    }

    #[test]
    fn test_optimal_kadane() {
        let nums = vec![-2, 1, -3, 4, -1, 2, 1, -5, 4];
        assert_eq!(max_sub_array_optimal(nums), 6);
        assert_eq!(max_sub_array_optimal(vec![1]), 1);
    }

    #[test]
    fn test_all_approaches_agreement() {
        let cases = vec![
            vec![-2, 1, -3, 4, -1, 2, 1, -5, 4],
            vec![1],
            vec![5, 4, -1, 7, 8],
            vec![-5, -2, -9, -1, -8],
            vec![10, 20, -100, 5, 5],
            vec![3, -1, -1, 3, -2, 5],
            vec![-1, -2, -3, -4],
        ];

        for nums in cases {
            let expected = max_sub_array_optimal(nums.clone());
            assert_eq!(
                max_sub_array_brute_force(nums.clone()),
                expected,
                "brute force mismatch for {nums:?}"
            );
            assert_eq!(
                max_sub_array_optimized(nums.clone()),
                expected,
                "divide-and-conquer mismatch for {nums:?}"
            );
            assert_eq!(
                max_sub_array_functional(nums.clone()),
                expected,
                "functional mismatch for {nums:?}"
            );
        }
    }
}
