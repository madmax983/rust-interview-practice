//! # 53. Maximum Subarray
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/maximum-subarray/>
//!
//! Given an integer array `nums`, find the subarray with the largest sum, and return its sum.
//!
//! This problem is famous for Kadane's Algorithm. In Rust, it is a perfect candidate to
//! demonstrate how to replace imperative loops containing multiple mutable variables
//! with a purely functional `fold` operation using a custom state struct.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::arrays::maximum_subarray::max_sub_array;
//!
//! assert_eq!(max_sub_array(vec![-2, 1, -3, 4, -1, 2, 1, -5, 4]), 6); // [4, -1, 2, 1]
//! assert_eq!(max_sub_array(vec![1]), 1);
//! assert_eq!(max_sub_array(vec![5, 4, -1, 7, 8]), 23);
//! ```
//!
//! ## Constraints
//!
//! - `1 <= nums.length <= 10^5`
//! - `-10^4 <= nums[i] <= 10^4`

use std::cmp;

/// Brute force approach: Check all subarrays.
/// Time: O(n²) - nested loops to check every possible start and end.
/// Space: O(1) - constant extra space.
///
/// While easy to understand, this approach is too slow for large inputs
/// and times out on `LeetCode`.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
pub fn max_sub_array_brute_force(nums: Vec<i32>) -> i32 {
    if nums.is_empty() {
        return 0;
    }

    let mut max_sum = i32::MIN;
    let n = nums.len();

    for i in 0..n {
        let mut current_sum = 0;
        for &val in &nums[i..] {
            current_sum += val;
            max_sum = cmp::max(max_sum, current_sum);
        }
    }

    max_sum
}

/// Optimized approach: Divide and Conquer.
///
/// Time: O(n log n) - The array is split in half at each level (log n levels), and each level
///       does O(n) total work computing the maximum crossing sum.
/// Space: O(log n) - Recursion stack depth.
///
/// The maximum subarray either lies entirely in the left half, entirely in the right half, or
/// crosses the midpoint. We recurse on the halves and compute the best crossing sum directly.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
pub fn max_sub_array_optimized(nums: Vec<i32>) -> i32 {
    fn helper(nums: &[i32], lo: usize, hi: usize) -> i32 {
        if lo == hi {
            return nums[lo];
        }

        let mid = lo + (hi - lo) / 2;
        let left_best = helper(nums, lo, mid);
        let right_best = helper(nums, mid + 1, hi);

        // Best sum ending at mid, extending leftward.
        let mut sum = 0;
        let mut cross_left = i32::MIN;
        for &val in nums[lo..=mid].iter().rev() {
            sum += val;
            cross_left = cmp::max(cross_left, sum);
        }

        // Best sum starting at mid+1, extending rightward.
        sum = 0;
        let mut cross_right = i32::MIN;
        for &val in &nums[mid + 1..=hi] {
            sum += val;
            cross_right = cmp::max(cross_right, sum);
        }

        let cross_best = cross_left + cross_right;
        cmp::max(cmp::max(left_best, right_best), cross_best)
    }

    if nums.is_empty() {
        return 0;
    }

    let n = nums.len();
    helper(&nums, 0, n - 1)
}

/// Custom struct to model the state in our functional approach.
///
/// By using a struct, our `fold` accumulator gains semantic meaning instead of
/// just being a generic `(i32, i32)` tuple. This prevents bugs from swapping tuple
/// elements and makes the transition logic self-documenting.
struct KadaneState {
    current_sum: i32,
    max_sum: i32,
}

/// Optimal approach: Kadane's Algorithm via functional `fold`.
/// Time: O(n) - single pass through the array.
/// Space: O(1) - the state struct is compiled down to CPU registers.
///
/// In imperative languages, Kadane's is written with a loop updating two mutable
/// variables (`current_sum` and `max_sum`). In Rust, we can model this functionally
/// using iterators and `fold`.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
pub fn max_sub_array_optimal(nums: Vec<i32>) -> i32 {
    // Handle empty case. The constraints say `1 <= nums.length`, but robust code checks.
    if nums.is_empty() {
        return 0;
    }

    // We can't start our fold from 0 because the array might contain all negative numbers.
    // Instead, we initialize the state with the first element.
    let first = nums[0];

    // RUST INSIGHT: `fold` takes an initial state and a closure. We use `.into_iter().skip(1)`
    // because we already consumed the first element for our initial state.
    let final_state = nums.into_iter().skip(1).fold(
        KadaneState {
            current_sum: first,
            max_sum: first,
        },
        |state, num| {
            // Is it better to extend the previous subarray, or start a new one here?
            let current_sum = cmp::max(num, state.current_sum + num);

            // Does this new current_sum beat our all-time best?
            let max_sum = cmp::max(state.max_sum, current_sum);

            KadaneState {
                current_sum,
                max_sum,
            }
        },
    );

    final_state.max_sum
}

/// Main entry point - uses optimal solution
#[must_use]
pub fn max_sub_array(nums: Vec<i32>) -> i32 {
    max_sub_array_optimal(nums)
}

// Alternative approaches footer:
// - Divide and Conquer: The array can be split in half, recursively finding the max subarray
//   in the left half, right half, and the max subarray crossing the midpoint. This is O(N log N)
//   and useful as a building block for segment trees, but overkill for this specific problem.

#[cfg(test)]
mod tests {
    use super::*;

    // Happy Path
    #[test]
    fn test_happy_path() {
        let nums = vec![-2, 1, -3, 4, -1, 2, 1, -5, 4];
        assert_eq!(max_sub_array_brute_force(nums.clone()), 6);
        assert_eq!(max_sub_array_optimized(nums.clone()), 6);
        assert_eq!(max_sub_array_optimal(nums), 6);
    }

    // Edge Case: Single element
    #[test]
    fn test_single_element() {
        let nums = vec![1];
        assert_eq!(max_sub_array_brute_force(nums.clone()), 1);
        assert_eq!(max_sub_array_optimized(nums.clone()), 1);
        assert_eq!(max_sub_array_optimal(nums), 1);
    }

    // Edge Case: All negative numbers
    #[test]
    fn test_all_negative() {
        let nums = vec![-5, -2, -9, -1, -3];
        assert_eq!(max_sub_array_brute_force(nums.clone()), -1);
        assert_eq!(max_sub_array_optimized(nums.clone()), -1);
        assert_eq!(max_sub_array_optimal(nums), -1);
    }

    // Stress/Boundary case: All positive numbers
    #[test]
    fn test_all_positive() {
        let nums = vec![5, 4, 1, 7, 8];
        assert_eq!(max_sub_array_brute_force(nums.clone()), 25);
        assert_eq!(max_sub_array_optimized(nums.clone()), 25);
        assert_eq!(max_sub_array_optimal(nums), 25);
    }

    // Cross-implementation agreement across shared inputs
    #[test]
    fn test_all_approaches_agree() {
        let cases = vec![
            vec![-2, 1, -3, 4, -1, 2, 1, -5, 4],
            vec![1],
            vec![5, 4, -1, 7, 8],
            vec![-5, -2, -9, -1, -3],
            vec![0],
            vec![-1, -2],
            vec![3, -2, 5, -1],
        ];
        for case in cases {
            let expected = max_sub_array_brute_force(case.clone());
            assert_eq!(max_sub_array_optimized(case.clone()), expected);
            assert_eq!(max_sub_array_optimal(case.clone()), expected);
        }
    }
}
