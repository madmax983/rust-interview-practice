//! # 560. Subarray Sum Equals K
//!
//! Difficulty: Medium
//! Link: https://leetcode.com/problems/subarray-sum-equals-k/
//!
//! Given an array of integers `nums` and an integer `k`, return the total number of subarrays whose sum equals to `k`.
//!
//! A subarray is a contiguous non-empty sequence of elements within an array.
//!
//! ## Why this matters in Rust
//! This problem highlights the power of the `HashMap` Entry API (`entry().or_insert()`) and
//! shows how functional combinators (`Iterator::fold`) can eliminate mutable variables while
//! safely tracking complex state (prefix sum frequencies). It teaches proper dereferencing
//! (`&count`) and avoiding double lookups that plague standard dict access in Python or Java.
//!
//! ## Approach
//! A naive approach checks all possible subarrays, taking `O(N^2)` time.
//! Instead, we use the **Prefix Sum + Hash Map** technique.
//!
//! If the cumulative sum up to index `i` is `prefix_sum`, and we want a subarray ending at `i`
//! with sum `k`, we need to check if there is a past cumulative sum equal to `prefix_sum - k`.
//! By keeping a frequency map of past prefix sums, we can find the number of valid subarrays in `O(1)` time per element.
//!
//! - **Time Complexity**: `O(N)` - We iterate through the array exactly once. Hash map operations take `O(1)` average time.
//! - **Space Complexity**: `O(N)` - In the worst case, all prefix sums are distinct, requiring `O(N)` space in the hash map.
//!
//! **Idiomatic Rust vs Other Languages:**
//! In languages like Python, you might write `counts.get(sum - k, 0)`. In Rust, we use
//! `counts.get(&(sum - k)).copied().unwrap_or(0)` for zero-cost abstraction without allocating.
//! Furthermore, Rust's Entry API (`counts.entry(sum).and_modify(|c| *c += 1).or_insert(1)`)
//! modifies values in-place without rehashing, whereas `dict[sum] = dict.get(sum, 0) + 1` in Python hashes the key twice.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::arrays::subarray_sum_equals_k::subarray_sum_imperative;
//!
//! let nums = vec![1, 1, 1];
//! let k = 2;
//! assert_eq!(subarray_sum_imperative(&nums, k), 2);
//! ```

use std::collections::HashMap;

/// Straightforward imperative approach using a `for` loop and a mutable HashMap.
///
/// This is the most readable and standard approach in system languages, manually tracking
/// a running sum and counter.
#[must_use]
pub fn subarray_sum_imperative(nums: &[i32], k: i32) -> i32 {
    // Stores the frequency of prefix sums encountered so far.
    let mut counts: HashMap<i32, i32> = HashMap::new();

    // Base case: A prefix sum of 0 has occurred exactly once (an empty subarray).
    counts.insert(0, 1);

    let mut current_sum = 0;
    let mut total_subarrays = 0;

    for &num in nums {
        current_sum += num;

        // If (current_sum - k) exists in the map, it means there is a subarray
        // ending at the current element that sums to k.
        let target = current_sum - k;

        // RUST INSIGHT: `.copied()` converts `Option<&i32>` to `Option<i32>`.
        // `.unwrap_or(0)` gracefully falls back to 0 without panicking.
        total_subarrays += counts.get(&target).copied().unwrap_or(0);

        // GOTCHA: Do not use `.insert(current_sum, counts.get(&current_sum).unwrap_or(0) + 1)`
        // as it performs two lookups. The Entry API does it in one pass.
        *counts.entry(current_sum).or_insert(0) += 1;
    }

    total_subarrays
}

/// Functional approach using `Iterator::fold`.
///
/// This approach avoids all external mutable state by threading the state
/// `(HashMap, current_sum, total_subarrays)` through the fold operation.
/// It's a great example of Rust's capability to express complex algorithms functionally.
#[must_use]
pub fn subarray_sum_functional(nums: &[i32], k: i32) -> i32 {
    let mut initial_counts = HashMap::new();
    initial_counts.insert(0, 1);

    // The accumulator state is: (prefix_sum_frequencies, current_sum, total_valid_subarrays)
    let (_, _, total) = nums.iter().fold(
        (initial_counts, 0, 0),
        |(mut counts, current_sum, total_subarrays), &num| {
            let next_sum = current_sum + num;
            let target = next_sum - k;

            // Add the frequency of the required past prefix sum
            let added_subarrays = counts.get(&target).copied().unwrap_or(0);

            // Update the map with the new prefix sum
            *counts.entry(next_sum).or_insert(0) += 1;

            // Return the new state for the next iteration
            (counts, next_sum, total_subarrays + added_subarrays)
        },
    );

    total
}

/// Main entry point - uses the functional approach to demonstrate idiomatic Rust.
#[must_use]
pub fn subarray_sum(nums: &[i32], k: i32) -> i32 {
    subarray_sum_functional(nums, k)
}

// =========================================================================================
// Alternative Approaches
// =========================================================================================
// 1. **Brute Force (O(N^2))**: Using a nested loop to calculate the sum of all `N*(N+1)/2` subarrays.
//    Never preferred due to time limit exceeded (TLE) constraints.
// 2. **Sliding Window (O(N))**: Only works if the array contains strictly non-negative integers.
//    Because `nums` can contain negative numbers in LeetCode 560, the sliding window constraint breaks
//    (expanding the window doesn't guarantee the sum increases), making the Prefix Sum approach mandatory.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_imperative_happy_path() {
        assert_eq!(subarray_sum_imperative(&[1, 1, 1], 2), 2);
        assert_eq!(subarray_sum_imperative(&[1, 2, 3], 3), 2);
    }

    #[test]
    fn test_functional_happy_path() {
        assert_eq!(subarray_sum_functional(&[1, 1, 1], 2), 2);
        assert_eq!(subarray_sum_functional(&[1, 2, 3], 3), 2);
    }

    #[test]
    fn test_edge_case_negative_numbers() {
        // Sliding window fails here, but Prefix Sum handles it perfectly.
        assert_eq!(subarray_sum(&[1, -1, 0], 0), 3); // [1, -1], [1, -1, 0], [0]
    }

    #[test]
    fn test_edge_case_zeros() {
        assert_eq!(subarray_sum(&[0, 0, 0, 0, 0], 0), 15);
    }

    #[test]
    fn test_stress_boundary() {
        // Array with elements summing up exactly to k
        assert_eq!(subarray_sum(&[3, 4, 7, 2, -3, 1, 4, 2], 7), 4);
    }
}
