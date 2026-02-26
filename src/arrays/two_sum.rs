//! # 1. Two Sum
//!
//! Given an array of integers `nums` and an integer `target`, return indices of the
//! two numbers such that they add up to `target`.
//!
//! You may assume that each input would have exactly one solution, and you may not
//! use the same element twice.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::arrays::two_sum::two_sum;
//!
//! assert_eq!(two_sum(vec![2, 7, 11, 15], 9), vec![0, 1]);
//! assert_eq!(two_sum(vec![3, 2, 4], 6), vec![1, 2]);
//! ```
//!
//! ## Constraints
//!
//! - 2 <= nums.length <= 10^4
//! - -10^9 <= nums[i] <= 10^9
//! - -10^9 <= target <= 10^9
//! - Only one valid answer exists

use std::collections::HashMap;

/// Brute force approach: Check all pairs
/// Time: O(n²) - nested loops check every pair
/// Space: O(1) - no extra space needed
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
pub fn two_sum_brute_force(nums: Vec<i32>, target: i32) -> Vec<i32> {
    let n = nums.len();

    // Strategy: Try every possible pair (i, j) where i < j
    for i in 0..n {
        for j in (i + 1)..n {
            // Check if this pair sums to target
            if nums[i] + nums[j] == target {
                return vec![i as i32, j as i32]; // Found the answer
            }
        }
    }

    vec![] // Should never reach here given constraints
}

/// Optimized approach: Hash map (two pass)
/// Time: O(n) - two passes through array
/// Space: O(n) - hash map stores all elements
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
pub fn two_sum_optimized(nums: Vec<i32>, target: i32) -> Vec<i32> {
    let mut map = HashMap::new(); // Maps value -> index

    // First pass: Build the hash map
    for (i, &num) in nums.iter().enumerate() {
        map.insert(num, i); // Store each number and its index
    }

    // Second pass: Look for complement
    for (i, &num) in nums.iter().enumerate() {
        let complement = target - num; // What we need to find

        // Check if complement exists and it's not the same element
        if let Some(&j) = map.get(&complement)
            && i != j
        {
            // Found a pair!
            return vec![i as i32, j as i32];
        }
    }

    vec![] // Should never reach here
}

/// Optimal approach: Hash map (one pass)
/// Time: O(n) - single pass through array
/// Space: O(n) - hash map stores elements as we go
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::cast_possible_wrap)]
pub fn two_sum_optimal(nums: Vec<i32>, target: i32) -> Vec<i32> {
    let mut map = HashMap::new(); // Maps value -> index

    // Strategy: Build map and search simultaneously
    // As we add each element, check if its complement was already seen
    for (i, &num) in nums.iter().enumerate() {
        let complement = target - num; // What we're looking for

        // Check if we've already seen the complement
        if let Some(&j) = map.get(&complement) {
            // Found it! j is before i (already in map)
            return vec![j as i32, i as i32];
        }

        // Haven't found complement yet, add current number to map
        map.insert(num, i);
    }

    vec![] // Should never reach here
}

/// Main entry point - uses optimal solution
#[must_use]
pub fn two_sum(nums: Vec<i32>, target: i32) -> Vec<i32> {
    two_sum_optimal(nums, target)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to check if result is valid (handles different valid orderings)
    fn is_valid_result(nums: &[i32], target: i32, result: &[i32]) -> bool {
        if result.len() != 2 {
            return false;
        }
        let i = result[0] as usize;
        let j = result[1] as usize;
        i < nums.len() && j < nums.len() && i != j && nums[i] + nums[j] == target
    }

    // Brute force tests
    #[test]
    fn test_brute_force_example_1() {
        let result = two_sum_brute_force(vec![2, 7, 11, 15], 9);
        assert!(is_valid_result(&[2, 7, 11, 15], 9, &result));
    }

    #[test]
    fn test_brute_force_example_2() {
        let result = two_sum_brute_force(vec![3, 2, 4], 6);
        assert!(is_valid_result(&[3, 2, 4], 6, &result));
    }

    #[test]
    fn test_brute_force_example_3() {
        let result = two_sum_brute_force(vec![3, 3], 6);
        assert!(is_valid_result(&[3, 3], 6, &result));
    }

    // Optimized tests
    #[test]
    fn test_optimized_example_1() {
        let result = two_sum_optimized(vec![2, 7, 11, 15], 9);
        assert!(is_valid_result(&[2, 7, 11, 15], 9, &result));
    }

    #[test]
    fn test_optimized_example_2() {
        let result = two_sum_optimized(vec![3, 2, 4], 6);
        assert!(is_valid_result(&[3, 2, 4], 6, &result));
    }

    #[test]
    fn test_optimized_example_3() {
        let result = two_sum_optimized(vec![3, 3], 6);
        assert!(is_valid_result(&[3, 3], 6, &result));
    }

    // Optimal tests
    #[test]
    fn test_optimal_example_1() {
        let result = two_sum_optimal(vec![2, 7, 11, 15], 9);
        assert!(is_valid_result(&[2, 7, 11, 15], 9, &result));
    }

    #[test]
    fn test_optimal_example_2() {
        let result = two_sum_optimal(vec![3, 2, 4], 6);
        assert!(is_valid_result(&[3, 2, 4], 6, &result));
    }

    #[test]
    fn test_optimal_example_3() {
        let result = two_sum_optimal(vec![3, 3], 6);
        assert!(is_valid_result(&[3, 3], 6, &result));
    }

    // Cross-implementation tests
    #[test]
    fn test_all_approaches_negative_numbers() {
        let nums = vec![-1, -2, -3, -4, -5];
        let target = -8;

        let result1 = two_sum_brute_force(nums.clone(), target);
        let result2 = two_sum_optimized(nums.clone(), target);
        let result3 = two_sum_optimal(nums.clone(), target);

        assert!(is_valid_result(&nums, target, &result1));
        assert!(is_valid_result(&nums, target, &result2));
        assert!(is_valid_result(&nums, target, &result3));
    }

    #[test]
    fn test_all_approaches_large_numbers() {
        let nums = vec![1000000000, -1000000000, 999999999];
        let target = 0;

        let result1 = two_sum_brute_force(nums.clone(), target);
        let result2 = two_sum_optimized(nums.clone(), target);
        let result3 = two_sum_optimal(nums.clone(), target);

        assert!(is_valid_result(&nums, target, &result1));
        assert!(is_valid_result(&nums, target, &result2));
        assert!(is_valid_result(&nums, target, &result3));
    }

    // Main function test
    #[test]
    fn test_main_example_1() {
        let result = two_sum(vec![2, 7, 11, 15], 9);
        assert!(is_valid_result(&[2, 7, 11, 15], 9, &result));
    }

    #[test]
    fn test_main_example_2() {
        let result = two_sum(vec![3, 2, 4], 6);
        assert!(is_valid_result(&[3, 2, 4], 6, &result));
    }
}
