//! # 167. Two Sum II - Input Array Is Sorted
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/two-sum-ii-input-array-is-sorted/>
//!
//! Given a **1-indexed** array of integers `numbers` that is already **sorted in non-decreasing order**,
//! find two numbers such that they add up to a specific `target` number. Let these two numbers be
//! `numbers[index1]` and `numbers[index2]` where `1 <= index1 < index2 <= numbers.length`.
//!
//! Return the indices of the two numbers, `index1` and `index2`, added by one as an integer array
//! `[index1, index2]` of length 2.
//!
//! The tests are generated such that there is exactly one solution. You may not use the same element twice.
//! Your solution must use only constant extra space.
//!
//! ## Why this matters in Rust
//!
//! This problem is a foundational introduction to the **Two Pointers** pattern.
//! While the original Two Sum uses a HashMap to achieve O(N) time complexity (costing O(N) space),
//! this problem exploits the *sorted* nature of the array to achieve O(N) time with O(1) space.
//! It's a great way to practice Rust's array indexing, loop invariants, and safely manipulating
//! multiple indices without running into bounds errors or borrow checker issues.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::arrays::two_sum_ii::two_sum;
//!
//! assert_eq!(two_sum(vec![2, 7, 11, 15], 9), vec![1, 2]);
//! assert_eq!(two_sum(vec![2, 3, 4], 6), vec![1, 3]);
//! assert_eq!(two_sum(vec![-1, 0], -1), vec![1, 2]);
//! ```
//!
//! ## Constraints
//!
//! - `2 <= numbers.length <= 3 * 10^4`
//! - `-1000 <= numbers[i] <= 1000`
//! - `numbers` is sorted in non-decreasing order.
//! - `-1000 <= target <= 1000`
//! - The tests are generated such that there is exactly one solution.

use std::cmp::Ordering;

/// Brute force approach: Nested loops.
///
/// Time: O(n²) - For every element, we check all subsequent elements.
/// Space: O(1) - No extra space allocated.
///
/// This approach tests every possible pair until it finds the target.
/// It completely ignores the fact that the array is sorted.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
#[allow(clippy::cast_possible_wrap)]
#[allow(clippy::cast_possible_truncation)]
pub fn two_sum_brute_force(numbers: Vec<i32>, target: i32) -> Vec<i32> {
    let n = numbers.len();

    for i in 0..n {
        for j in (i + 1)..n {
            if numbers[i] + numbers[j] == target {
                // GOTCHA: The problem requires a 1-indexed array result!
                return vec![(i + 1) as i32, (j + 1) as i32];
            }
        }
    }

    vec![]
}

/// Optimized approach: Binary Search.
///
/// Time: O(n log n) - For each element, we binary search the remainder of the array.
/// Space: O(1) - Iterative binary search takes constant space.
///
/// Since the array is sorted, instead of a linear scan for the complement
/// (target - numbers[i]), we can use binary search.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
#[allow(clippy::cast_possible_wrap)]
#[allow(clippy::cast_possible_truncation)]
pub fn two_sum_optimized(numbers: Vec<i32>, target: i32) -> Vec<i32> {
    let n = numbers.len();

    for i in 0..n {
        let complement = target - numbers[i];

        // RUST INSIGHT: We could use `numbers[i + 1..].binary_search(&complement)`
        // but it returns a relative index to the slice. Manual binary search is easy too.
        if let Ok(rel_idx) = numbers[i + 1..].binary_search(&complement) {
            let j = i + 1 + rel_idx;
            return vec![(i + 1) as i32, (j + 1) as i32];
        }
    }

    vec![]
}

/// Optimal approach: Two Pointers.
///
/// Time: O(n) - We make a single pass through the array.
/// Space: O(1) - We only keep track of two indices.
///
/// By placing one pointer at the start (smallest value) and one at the end (largest value),
/// we can narrow down the window. If the sum is too large, we must decrease it by moving
/// the right pointer left. If the sum is too small, we must increase it by moving
/// the left pointer right.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
#[allow(clippy::cast_possible_wrap)]
#[allow(clippy::cast_possible_truncation)]
pub fn two_sum_optimal(numbers: Vec<i32>, target: i32) -> Vec<i32> {
    let mut left = 0;
    let mut right = numbers.len() - 1;

    // We can use a while loop since we are guaranteed exactly one solution.
    while left < right {
        let sum = numbers[left] + numbers[right];

        // RUST INSIGHT: `cmp` returns an `Ordering` enum (Less, Equal, Greater).
        // Pattern matching on this enum makes the branching extremely clear and safe,
        // rather than using nested if-else blocks.
        match sum.cmp(&target) {
            Ordering::Equal => return vec![(left + 1) as i32, (right + 1) as i32],
            Ordering::Less => left += 1, // Sum too small, need a bigger number
            Ordering::Greater => right -= 1, // Sum too big, need a smaller number
        }
    }

    vec![]
}

/// Main entry point - uses optimal solution
#[must_use]
pub fn two_sum(numbers: Vec<i32>, target: i32) -> Vec<i32> {
    two_sum_optimal(numbers, target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force_example_1() {
        assert_eq!(two_sum_brute_force(vec![2, 7, 11, 15], 9), vec![1, 2]);
    }

    #[test]
    fn test_brute_force_example_2() {
        assert_eq!(two_sum_brute_force(vec![2, 3, 4], 6), vec![1, 3]);
    }

    #[test]
    fn test_brute_force_example_3() {
        assert_eq!(two_sum_brute_force(vec![-1, 0], -1), vec![1, 2]);
    }

    #[test]
    fn test_optimized_example_1() {
        assert_eq!(two_sum_optimized(vec![2, 7, 11, 15], 9), vec![1, 2]);
    }

    #[test]
    fn test_optimized_example_2() {
        assert_eq!(two_sum_optimized(vec![2, 3, 4], 6), vec![1, 3]);
    }

    #[test]
    fn test_optimized_example_3() {
        assert_eq!(two_sum_optimized(vec![-1, 0], -1), vec![1, 2]);
    }

    #[test]
    fn test_optimal_example_1() {
        assert_eq!(two_sum_optimal(vec![2, 7, 11, 15], 9), vec![1, 2]);
    }

    #[test]
    fn test_optimal_example_2() {
        assert_eq!(two_sum_optimal(vec![2, 3, 4], 6), vec![1, 3]);
    }

    #[test]
    fn test_optimal_example_3() {
        assert_eq!(two_sum_optimal(vec![-1, 0], -1), vec![1, 2]);
    }

    #[test]
    fn test_all_approaches_edge_cases() {
        let input = vec![0, 0, 3, 4];
        let target = 0;
        assert_eq!(two_sum_brute_force(input.clone(), target), vec![1, 2]);
        assert_eq!(two_sum_optimized(input.clone(), target), vec![1, 2]);
        assert_eq!(two_sum_optimal(input.clone(), target), vec![1, 2]);
    }

    #[test]
    fn test_large_gap() {
        let input = vec![-1000, -100, 0, 100, 1001];
        let target = 0;
        assert_eq!(two_sum_brute_force(input.clone(), target), vec![2, 4]);
        assert_eq!(two_sum_optimized(input.clone(), target), vec![2, 4]);
        assert_eq!(two_sum_optimal(input, target), vec![2, 4]);
    }
}
