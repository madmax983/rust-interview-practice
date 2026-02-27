//! # 198. House Robber
//!
//! You are a professional robber planning to rob houses along a street. Each house has a certain amount of money stashed, the only constraint stopping you from robbing each of them is that adjacent houses have security systems connected and **it will automatically contact the police if two adjacent houses were broken into on the same night**.
//!
//! Given an integer array `nums` representing the amount of money of each house, return *the maximum amount of money you can rob tonight without alerting the police*.
//!
//! - Difficulty: Medium
//! - LeetCode: <https://leetcode.com/problems/house-robber/>
//!
//! ## Why this matters in Rust
//! This problem is an excellent showcase for **iterator combinators** and **state transformation**.
//! While the classic Dynamic Programming solution uses an array, Rust's `Iterator::fold` allows us to express the state transition as a functional pipeline, turning an imperative loop into a concise, expression-oriented solution. This demonstrates how Rust's zero-cost abstractions can make code both cleaner and just as fast as C++.
//!
//! ## Approach
//!
//! The core recurrence relation is:
//! `dp[i] = max(dp[i-1], dp[i-2] + nums[i])`
//!
//! We explore four implementations:
//! 1.  **Brute Force**: Naive recursion. Tries all valid subsequences. Exponential time.
//! 2.  **Memoization**: Top-Down DP. Caches results to avoid re-computation. O(n) time/space.
//! 3.  **Tabulation**: Bottom-Up DP. Builds a `dp` table iteratively. O(n) time/space.
//! 4.  **Optimal**: Space-optimized iterative solution using `fold`. O(n) time, O(1) space.

use std::cmp::max;

/// Brute Force Approach: Recursive DFS
///
/// We recursively decide for each house: rob it (and skip the next one) or skip it (and consider the next one).
///
/// - **Time Complexity**: O(2^n). Each step branches into two recursive calls.
/// - **Space Complexity**: O(n) for the recursion stack.
///
/// # RUST INSIGHT
/// Recursion in Rust is straightforward but lacks tail-call optimization (TCO) generally.
/// Deep recursion can overflow the stack, though for `n=100` (problem constraint), it is safe.
#[allow(clippy::needless_pass_by_value)]
pub fn rob_brute_force(nums: Vec<i32>) -> i32 {
    fn solve(nums: &[i32], i: usize) -> i32 {
        if i >= nums.len() {
            return 0;
        }
        // Option 1: Rob current house `i`, skip `i+1`, solve for `i+2`
        let rob = nums[i] + solve(nums, i + 2);
        // Option 2: Skip current house `i`, solve for `i+1`
        let skip = solve(nums, i + 1);

        max(rob, skip)
    }

    solve(&nums, 0)
}

/// Memoized Approach: Top-Down DP
///
/// We use a `Vec<Option<i32>>` to store results of subproblems `solve(i)`.
/// - `None` indicates the subproblem hasn't been solved.
/// - `Some(val)` is the cached result.
///
/// - **Time Complexity**: O(n). Each state `i` is computed once.
/// - **Space Complexity**: O(n) for the memoization table and recursion stack.
///
/// # RUST INSIGHT
/// `Vec<Option<T>>` is a common pattern for memoization where `0` or `-1` are valid results.
/// Unlike `HashMap`, looking up an index in a `Vec` is O(1) and cache-friendly.
///
/// # GOTCHA
/// Be careful with `usize` indices. `nums.len()` returns `usize`.
#[allow(clippy::needless_pass_by_value)]
pub fn rob_memoized(nums: Vec<i32>) -> i32 {
    let n = nums.len();
    let mut memo = vec![None; n];

    fn solve(nums: &[i32], i: usize, memo: &mut [Option<i32>]) -> i32 {
        if i >= nums.len() {
            return 0;
        }
        // Check cache
        if let Some(val) = memo[i] {
            return val;
        }

        let rob = nums[i] + solve(nums, i + 2, memo);
        let skip = solve(nums, i + 1, memo);
        let res = max(rob, skip);

        // Cache result
        memo[i] = Some(res);
        res
    }

    solve(&nums, 0, &mut memo)
}

/// Tabulation Approach: Bottom-Up DP
///
/// We build a `dp` array where `dp[i]` represents the max money robbable from the first `i` houses.
/// - `dp[i] = max(dp[i-1], dp[i-2] + nums[i])`
///
/// - **Time Complexity**: O(n).
/// - **Space Complexity**: O(n) for the `dp` vector.
///
/// # GOTCHA
/// Handling indices `i-1` and `i-2` requires care.
/// We pad the `dp` array or handle base cases explicitly to avoid underflow/panic.
#[allow(clippy::needless_pass_by_value)]
pub fn rob_tabulation(nums: Vec<i32>) -> i32 {
    if nums.is_empty() {
        return 0;
    }
    if nums.len() == 1 {
        return nums[0];
    }

    let n = nums.len();
    let mut dp = vec![0; n];

    // Base cases
    dp[0] = nums[0];
    dp[1] = max(nums[0], nums[1]);

    for i in 2..n {
        dp[i] = max(dp[i - 1], dp[i - 2] + nums[i]);
    }

    dp[n - 1]
}

/// Optimal Approach: Iterator `fold`
///
/// We only need the previous two states (`prev1` and `prev2`) to calculate the current state.
/// This reduces space complexity to O(1).
///
/// We use `iter().fold()` to process the array in a single pass.
/// The accumulator state is `(prev2, prev1)`, representing `dp[i-2]` and `dp[i-1]`.
///
/// - **Time Complexity**: O(n).
/// - **Space Complexity**: O(1).
///
/// # RUST INSIGHT
/// `fold` is powerful for state machines. The closure `|(prev2, prev1), &num|`
/// cleanly destructures the old state and the current input, returning the new state `(prev1, new_max)`.
/// This is idiomatic Rust: immutable, expression-oriented, and efficient.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn rob_optimal(nums: Vec<i32>) -> i32 {
    let (_final_prev2, final_prev1) = nums.iter().fold((0, 0), |(prev2, prev1), &num| {
        // prev2 is dp[i-2], prev1 is dp[i-1], num is nums[i]
        // new_max = max(dp[i-1], dp[i-2] + num)
        let new_max = max(prev1, prev2 + num);
        // Shift state: prev1 becomes the new prev2, new_max becomes the new prev1
        (prev1, new_max)
    });

    // The last computed max is in the second position of the tuple
    final_prev1
}

/// Main entry point
#[must_use]
pub fn rob(nums: Vec<i32>) -> i32 {
    rob_optimal(nums)
}

// Alternative Approaches:
// 1. **Imperative Loop with Swap**:
//    Instead of `fold`, we can use a `for` loop with two mutable variables (`prev1`, `prev2`)
//    and `std::mem::swap` or temporary variables. This is more "C-like" but equally efficient (O(1) space).
//    It might be preferred if the loop body is very complex or requires early exit (`break`).

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force_basic() {
        assert_eq!(rob_brute_force(vec![1, 2, 3, 1]), 4);
        assert_eq!(rob_brute_force(vec![2, 7, 9, 3, 1]), 12);
    }

    #[test]
    fn test_memoized_basic() {
        assert_eq!(rob_memoized(vec![1, 2, 3, 1]), 4);
        assert_eq!(rob_memoized(vec![2, 7, 9, 3, 1]), 12);
    }

    #[test]
    fn test_tabulation_basic() {
        assert_eq!(rob_tabulation(vec![1, 2, 3, 1]), 4);
        assert_eq!(rob_tabulation(vec![2, 7, 9, 3, 1]), 12);
    }

    #[test]
    fn test_optimal_basic() {
        assert_eq!(rob_optimal(vec![1, 2, 3, 1]), 4);
        assert_eq!(rob_optimal(vec![2, 7, 9, 3, 1]), 12);
    }

    #[test]
    fn test_empty() {
        let nums = vec![];
        assert_eq!(rob_brute_force(nums.clone()), 0);
        assert_eq!(rob_memoized(nums.clone()), 0);
        assert_eq!(rob_tabulation(nums.clone()), 0);
        assert_eq!(rob_optimal(nums), 0);
    }

    #[test]
    fn test_single_element() {
        let nums = vec![10];
        assert_eq!(rob_brute_force(nums.clone()), 10);
        assert_eq!(rob_memoized(nums.clone()), 10);
        assert_eq!(rob_tabulation(nums.clone()), 10);
        assert_eq!(rob_optimal(nums), 10);
    }

    #[test]
    fn test_two_elements() {
        let nums = vec![10, 20];
        assert_eq!(rob_brute_force(nums.clone()), 20);
        assert_eq!(rob_memoized(nums.clone()), 20);
        assert_eq!(rob_tabulation(nums.clone()), 20);
        assert_eq!(rob_optimal(nums), 20);
    }

    #[test]
    fn test_large_gap() {
        // [2, 1, 1, 2] -> should pick 2 + 2 = 4
        let nums = vec![2, 1, 1, 2];
        assert_eq!(rob_optimal(nums), 4);
    }

    #[test]
    fn test_all_approaches_consistency() {
        let nums = vec![1, 2, 3, 1, 1, 10, 2, 5, 8];
        let expected = rob_optimal(nums.clone());
        assert_eq!(rob_brute_force(nums.clone()), expected);
        assert_eq!(rob_memoized(nums.clone()), expected);
        assert_eq!(rob_tabulation(nums.clone()), expected);
    }
}
