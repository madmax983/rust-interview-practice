//! # 300. Longest Increasing Subsequence
//!
//! Given an integer array `nums`, return the length of the longest strictly increasing subsequence.
//!
//! A subsequence is a sequence that can be derived from an array by deleting some or no elements without changing the order of the remaining elements.
//! For example, `[3,6,2,7]` is a subsequence of the array `[0,3,1,6,2,2,7]`.
//!
//! [LeetCode Problem 300](https://leetcode.com/problems/longest-increasing-subsequence/)
//!
//! ## Why this matters in Rust
//! This problem demonstrates:
//! -   **Binary Search**: Using `binary_search` or `partition_point` from the standard library for O(log n) lookups.
//! -   **Dynamic Programming**: Transitioning from O(2^n) recursion to O(n²) iteration, and finally to O(n log n) with a greedy approach.
//! -   **Vector Operations**: Efficiently managing a `Vec` as a dynamic array or a replacement for a manual stack.
//! -   **Ordering Traits**: Understanding how `Ord` and `PartialOrd` apply to elements.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::dynamic_programming::longest_increasing_subsequence::length_of_lis;
//!
//! assert_eq!(length_of_lis(vec![10, 9, 2, 5, 3, 7, 101, 18]), 4);
//! assert_eq!(length_of_lis(vec![0, 1, 0, 3, 2, 3]), 4);
//! assert_eq!(length_of_lis(vec![7, 7, 7, 7, 7, 7, 7]), 1);
//! ```
//!
//! ## Constraints
//!
//! -   `1 <= nums.length <= 2500`
//! -   `-10^4 <= nums[i] <= 10^4`

/// Brute force approach: Recursive with Memoization.
///
/// We define `helper(prev_index, curr_index)` as the length of the LIS starting at `curr_index`
/// given that the previous element included in the LIS was at `prev_index`.
/// To make this runnable on `LeetCode` constraints (N=2500), we add memoization.
///
/// Time: O(N²) - There are N * N states.
/// Space: O(N²) - For the memoization table.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn length_of_lis_brute_force(nums: Vec<i32>) -> i32 {
    // `prev_index + 1` is always >= 0 and `curr_index` fits an isize for LeetCode sizes.
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_wrap)]
    fn helper(nums: &[i32], prev_index: isize, curr_index: usize, memo: &mut Vec<Vec<i32>>) -> i32 {
        if curr_index == nums.len() {
            return 0;
        }

        // Check memoization table
        // We map prev_index (-1..n-1) to (0..n) for array indexing.
        let memo_prev_idx = (prev_index + 1) as usize;
        if memo[memo_prev_idx][curr_index] != -1 {
            return memo[memo_prev_idx][curr_index];
        }

        // Option 1: Skip the current element
        let skip = helper(nums, prev_index, curr_index + 1, memo);

        // Option 2: Include the current element (if valid)
        let take = if prev_index == -1 || nums[curr_index] > nums[prev_index as usize] {
            1 + helper(nums, curr_index as isize, curr_index + 1, memo)
        } else {
            0
        };

        let result = std::cmp::max(skip, take);
        memo[memo_prev_idx][curr_index] = result;
        result
    }

    let n = nums.len();
    // Memoization table initialized with -1 (indicating uncomputed).
    // `memo[prev_index + 1][curr_index]` stores the result.
    // We offset prev_index by 1 because it can be -1 (initial state).
    let mut memo = vec![vec![-1; n]; n + 1];

    helper(&nums, -1, 0, &mut memo)
}

/// Optimized approach: Iterative Dynamic Programming.
///
/// We define `dp[i]` as the length of the LIS ending at index `i`.
/// For each element `i`, we check all previous elements `j < i`.
/// If `nums[i] > nums[j]`, we can extend the subsequence ending at `j`.
///
/// Time: O(N²) - Nested loops.
/// Space: O(N) - For the `dp` array.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn length_of_lis_optimized(nums: Vec<i32>) -> i32 {
    if nums.is_empty() {
        return 0;
    }

    let n = nums.len();
    // Initialize dp array with 1s, as every element is an LIS of length 1 by itself.
    let mut dp = vec![1; n];
    let mut max_len = 1;

    for i in 1..n {
        for j in 0..i {
            if nums[i] > nums[j] {
                // RUST INSIGHT: `std::cmp::max`
                // Update LIS length ending at i
                dp[i] = std::cmp::max(dp[i], dp[j] + 1);
            }
        }
        max_len = std::cmp::max(max_len, dp[i]);
    }

    max_len
}

/// Optimal approach: Patience Sorting (Greedy + Binary Search).
///
/// We maintain a list `tails`, where `tails[i]` stores the *smallest tail of all increasing subsequences of length i+1*.
/// The key insight is that to maximize the chance of extending a subsequence, we want the ending element to be as small as possible.
///
/// Since `tails` is always sorted, we can use binary search to find the insertion point for each number.
///
/// Time: O(N log N) - Binary search takes O(log N) and we do it N times.
/// Space: O(N) - For the `tails` vector.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
// LeetCode constraints (nums.len() <= 2500) guarantee this cast fits.
#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
pub fn length_of_lis_optimal(nums: Vec<i32>) -> i32 {
    // ⚡ BOLT OPTIMIZATION: Pre-allocate capacity to eliminate dynamic heap reallocations.
    let mut tails = Vec::with_capacity(nums.len());

    for x in nums {
        // RUST INSIGHT: `binary_search` return value
        // `binary_search` returns `Ok(idx)` if the element is found,
        // or `Err(idx)` where `idx` is the index where the element could be inserted while maintaining order.
        // This effectively finds the first element >= x.
        match tails.binary_search(&x) {
            Ok(_) => {
                // Element `x` already exists. In LIS (strict), it doesn't extend the length.
                // We don't need to do anything because replacing `x` with `x` is a no-op.
            }
            Err(idx) => {
                // GOTCHA: The `tails` array does NOT necessarily contain the LIS itself.
                // It stores the smallest tail of all increasing subsequences of length i+1.
                // For example, in `[1, 4, 5, 2, 6]`, `tails` ends up as `[1, 2, 5, 6]`.
                // This has the correct length (4), but `[1, 2, 5, 6]` is NOT a valid subsequence
                // because `5` appears before `2` in the input.
                if idx < tails.len() {
                    // Replace the existing element with `x`.
                    // This lowers the "bar" for future elements to extend a subsequence of this length.
                    tails[idx] = x;
                } else {
                    // `x` is larger than all current tails, so we can extend the longest LIS found so far.
                    tails.push(x);
                }
            }
        }
    }

    tails.len() as i32
}

/// Main entry point - uses optimal solution
#[must_use]
pub fn length_of_lis(nums: Vec<i32>) -> i32 {
    length_of_lis_optimal(nums)
}

/// Alternative Approach: Segment Tree
/// We can use a Segment Tree or Fenwick Tree (Binary Indexed Tree) to query the maximum LIS length
/// for values smaller than current `nums[i]` in O(log M) time, where M is the range of values.
/// This is useful if we need to count the number of LIS or handle updates.
///
/// Alternative Approach: Printing the Subsequence
/// To reconstruct the actual LIS, we need to store the `predecessor` index for each element
/// in the DP or Patience Sort approach, then backtrack from the end.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force_basic() {
        let nums = vec![10, 9, 2, 5, 3, 7, 101, 18];
        assert_eq!(length_of_lis_brute_force(nums), 4);
    }

    #[test]
    fn test_optimized_basic() {
        let nums = vec![10, 9, 2, 5, 3, 7, 101, 18];
        assert_eq!(length_of_lis_optimized(nums), 4);
    }

    #[test]
    fn test_optimal_basic() {
        let nums = vec![10, 9, 2, 5, 3, 7, 101, 18];
        assert_eq!(length_of_lis_optimal(nums), 4);
    }

    #[test]
    fn test_all_approaches_edge_cases() {
        // Case 1: Empty
        let empty: Vec<i32> = vec![];
        assert_eq!(length_of_lis_brute_force(empty.clone()), 0);
        assert_eq!(length_of_lis_optimized(empty.clone()), 0);
        assert_eq!(length_of_lis_optimal(empty.clone()), 0);

        // Case 2: Sorted
        let sorted = vec![1, 2, 3, 4, 5];
        assert_eq!(length_of_lis_brute_force(sorted.clone()), 5);
        assert_eq!(length_of_lis_optimized(sorted.clone()), 5);
        assert_eq!(length_of_lis_optimal(sorted.clone()), 5);

        // Case 3: Reverse Sorted
        let reverse = vec![5, 4, 3, 2, 1];
        assert_eq!(length_of_lis_brute_force(reverse.clone()), 1);
        assert_eq!(length_of_lis_optimized(reverse.clone()), 1);
        assert_eq!(length_of_lis_optimal(reverse.clone()), 1);

        // Case 4: Duplicates
        let dups = vec![7, 7, 7, 7];
        assert_eq!(length_of_lis_brute_force(dups.clone()), 1);
        assert_eq!(length_of_lis_optimized(dups.clone()), 1);
        assert_eq!(length_of_lis_optimal(dups.clone()), 1);

        // Case 5: Wiggle
        let wiggle = vec![1, 3, 6, 7, 9, 4, 10, 5, 6];
        // LIS: [1, 3, 6, 7, 9, 10] -> 6
        assert_eq!(length_of_lis_brute_force(wiggle.clone()), 6);
        assert_eq!(length_of_lis_optimized(wiggle.clone()), 6);
        assert_eq!(length_of_lis_optimal(wiggle.clone()), 6);
    }
}
