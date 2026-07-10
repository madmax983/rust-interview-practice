//! # 41. First Missing Positive
//!
//! Hard
//! <https://leetcode.com/problems/first-missing-positive>/
//!
//! Given an unsorted integer array `nums`, return the smallest missing positive integer.
//! You must implement an algorithm that runs in `O(n)` time and uses `O(1)` auxiliary space.
//!
//! This problem perfectly demonstrates how to use a mutable slice to perform in-place "cyclic sort"
//! in Rust, replacing array values via safe swaps. It challenges us to reason about indices, values,
//! and ownership simultaneously, emphasizing Rust's strict but powerful array bounds and mutability guarantees.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::arrays::first_missing_positive::first_missing_positive;
//!
//! assert_eq!(first_missing_positive(vec![1, 2, 0]), 3);
//! assert_eq!(first_missing_positive(vec![3, 4, -1, 1]), 2);
//! assert_eq!(first_missing_positive(vec![7, 8, 9, 11, 12]), 1);
//! ```
//!
//! ## Constraints
//!
//! - `1 <= nums.length <= 10^5`
//! - `-2^31 <= nums[i] <= 2^31 - 1`

use std::collections::HashSet;

/// Brute force approach: Sort then scan.
///
/// **Time Complexity:** O(N log N) - dominated by the sort.
/// **Space Complexity:** O(1) auxiliary space (sorting in place with `sort_unstable`).
///
/// Sort the array, then walk it in order tracking the smallest positive integer we still
/// expect to see. Non-positives and duplicates are skipped; the first gap is the answer.
/// This fails the required O(N) time constraint, but it is the most obvious correct approach.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn first_missing_positive_brute_force(mut nums: Vec<i32>) -> i32 {
    nums.sort_unstable();

    let mut expected = 1;
    for &num in &nums {
        if num == expected {
            // Found the next expected positive; advance the target.
            expected += 1;
        } else if num > expected {
            // A gap: `expected` was never seen.
            break;
        }
        // num < expected (non-positive or duplicate): skip.
    }

    expected
}

/// Optimized approach: Hash Set
///
/// **Time Complexity:** O(N)
/// **Space Complexity:** O(N) auxiliary space.
///
/// We insert all elements into a `HashSet`, and then iterate from `1` upwards, checking if
/// the number exists in the set. While this achieves O(N) time complexity, it fails the
/// O(1) space constraint of the problem.
///
/// **Idiomatic Rust:** We can use iterator adapters to filter out non-positive numbers
/// and collect the rest into a `HashSet`. Then, we simply search from 1 upwards.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn first_missing_positive_optimized(nums: Vec<i32>) -> i32 {
    // RUST INSIGHT: `.into_iter()` consumes the vector, moving ownership of its elements.
    // `.filter()` allows us to only keep positive numbers, avoiding unnecessary insertions.
    // `.collect()` is a powerful, zero-cost abstraction that knows how to build a HashSet.
    let set: HashSet<i32> = nums.into_iter().filter(|&x| x > 0).collect();

    // The maximum possible answer is nums.len() + 1
    // We can just count up from 1 until we find a missing number.
    let mut current = 1;
    loop {
        if !set.contains(&current) {
            return current;
        }
        current += 1;
    }
}

/// # Approach: Cyclic Sort (In-Place)
///
/// **Time Complexity:** O(N)
/// **Space Complexity:** O(1) auxiliary space (modifying the input array in place).
///
/// The core insight is that for an array of size `N`, the first missing positive integer
/// *must* be in the range `[1, N + 1]`. Thus, we can use the input array itself as a hash map!
///
/// We place each number `x` at the index `x - 1` (so `1` goes to index `0`, `2` to index `1`, etc.).
/// We iterate through the array, and if we see a number in the valid range `[1, N]` that isn't
/// already at its correct index, we swap it with the number currently at its target index.
///
/// After processing, we do a second pass. The first index `i` where `nums[i] != i + 1`
/// tells us that `i + 1` is the missing positive. If all are in place, the missing positive is `N + 1`.
///
/// **Idiomatic Rust vs. Others:** In languages like Python or C++, you might write a `while` loop
/// with raw array indexing `nums[nums[i] - 1]`. Rust enforces array bounds, and doing so blindly
/// can lead to panics. However, by strictly checking bounds and casting (`x as usize`), we can safely
/// execute the algorithm. Rust's `swap` method makes exchanging elements clean and safe.
#[must_use]
#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap, clippy::cast_sign_loss)] // LeetCode constraints guarantee it fits
pub fn first_missing_positive_optimal(mut nums: Vec<i32>) -> i32 {
    let n = nums.len();

    for i in 0..n {
        // We use a while loop here because after a swap, the new number at `nums[i]`
        // might also need to be placed at its correct index.
        // We keep swapping until the current position holds a number we can't swap
        // (out of bounds, non-positive, or already at the correct position).
        while nums[i] > 0 && nums[i] <= n as i32 {
            // GOTCHA: We must cast to `usize` for array indexing, and subtract 1
            // because a value of `1` should be at index `0`.
            let target_idx = (nums[i] - 1) as usize;

            // RUST INSIGHT: The condition `nums[i] != nums[target_idx]` prevents infinite loops
            // if there are duplicates. If `nums[i]` is already correct, we stop.
            if nums[i] == nums[target_idx] {
                break;
            }
            nums.swap(i, target_idx);
        }
    }

    // Second pass: Find the first index where the value is incorrect.
    for (i, &val) in nums.iter().enumerate() {
        if val != (i + 1) as i32 {
            return (i + 1) as i32;
        }
    }

    // If all numbers from 1 to N are present, the answer is N + 1.
    (n + 1) as i32
}

/// Main entry point - uses optimal cyclic sort solution to satisfy O(1) space constraint.
#[must_use]
pub fn first_missing_positive(nums: Vec<i32>) -> i32 {
    first_missing_positive_optimal(nums)
}

/*
 * Alternative Approaches:
 * 1. Sorting: We could sort the array first (O(N log N)) and then scan for the first missing
 *    positive. This is O(1) space but fails the O(N) time constraint.
 * 2. Marking via negative signs: Similar to cyclic sort, we can use the input array as storage.
 *    First pass: turn non-positives to N+1. Second pass: for every value `x`, make the value at
 *    index `|x| - 1` negative. Third pass: find the first positive value. The index + 1 is the answer.
 *    This avoids swaps but involves more passes and sign manipulation.
 */

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_happy_path() {
        assert_eq!(first_missing_positive(vec![1, 2, 0]), 3);
        assert_eq!(first_missing_positive(vec![3, 4, -1, 1]), 2);
    }

    #[test]
    fn test_edge_cases() {
        // Empty array conceptually (though constraints say length >= 1)
        assert_eq!(first_missing_positive(vec![]), 1);

        // Single element, answer is 1
        assert_eq!(first_missing_positive(vec![5]), 1);
        assert_eq!(first_missing_positive(vec![-5]), 1);

        // Single element, answer is 2
        assert_eq!(first_missing_positive(vec![1]), 2);

        // Array already correctly ordered
        assert_eq!(first_missing_positive(vec![1, 2, 3, 4, 5]), 6);

        // Array containing all negatives
        assert_eq!(first_missing_positive(vec![-1, -2, -3]), 1);

        // Array with duplicates
        assert_eq!(first_missing_positive(vec![1, 1]), 2);
        assert_eq!(first_missing_positive(vec![2, 2]), 1);
    }

    #[test]
    fn test_stress_boundary() {
        // Large array where the missing positive is exactly in the middle
        let mut nums: Vec<i32> = (1..=10000).collect();
        nums.remove(5000); // 5001 is missing
        assert_eq!(first_missing_positive(nums), 5001);

        // Max/Min values boundaries
        assert_eq!(first_missing_positive(vec![i32::MAX, i32::MIN]), 1);
    }

    #[test]
    fn test_brute_force_implementation() {
        assert_eq!(first_missing_positive_brute_force(vec![1, 2, 0]), 3);
        assert_eq!(first_missing_positive_brute_force(vec![3, 4, -1, 1]), 2);
        assert_eq!(first_missing_positive_brute_force(vec![7, 8, 9, 11, 12]), 1);
    }

    #[test]
    fn test_optimized_implementation() {
        assert_eq!(first_missing_positive_optimized(vec![1, 2, 0]), 3);
        assert_eq!(first_missing_positive_optimized(vec![3, 4, -1, 1]), 2);
        assert_eq!(first_missing_positive_optimized(vec![7, 8, 9, 11, 12]), 1);
    }

    #[test]
    fn test_optimal_implementation() {
        assert_eq!(first_missing_positive_optimal(vec![1, 2, 0]), 3);
        assert_eq!(first_missing_positive_optimal(vec![3, 4, -1, 1]), 2);
        assert_eq!(first_missing_positive_optimal(vec![7, 8, 9, 11, 12]), 1);
    }

    #[test]
    fn test_all_approaches_agree() {
        let cases = vec![
            vec![1, 2, 0],
            vec![3, 4, -1, 1],
            vec![7, 8, 9, 11, 12],
            vec![],
            vec![1],
            vec![5],
            vec![-1, -2, -3],
            vec![1, 1],
            vec![2, 2],
            vec![1, 2, 3, 4, 5],
        ];
        for case in cases {
            let expected = first_missing_positive_brute_force(case.clone());
            assert_eq!(first_missing_positive_optimized(case.clone()), expected);
            assert_eq!(first_missing_positive_optimal(case.clone()), expected);
        }
    }
}
