//! # 416. Partition Equal Subset Sum
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/partition-equal-subset-sum/>
//!
//! Given an integer array `nums`, return `true` if you can partition the array into two subsets such that the sum of the elements in both subsets is equal or `false` otherwise.
//!
//! ## Why this matters in Rust
//! This problem perfectly demonstrates Rust's zero-cost iterator abstractions (`iter().sum()`), its powerful match ergonomics to eliminate edge cases early, and how safe mutable slices (`Vec<bool>`) can act as a sliding window state for 1D Dynamic Programming. It shows how Rust protects you from concurrent access bugs while letting you write tight, allocation-free inner loops.
//!
//! ## Approach
//!
//! The problem requires finding a subset that sums up to exactly half of the total sum of the array. If the total sum is odd, it's impossible to split into two equal integer halves.
//!
//! We provide two approaches:
//! 1. **2D Dynamic Programming (`can_partition_optimized`)**: A straightforward approach where `dp[i][j]` means "can we form sum `j` using a subset of the first `i` items". This makes the state transitions obvious but takes O(N * Target) space.
//! 2. **1D Space Optimized DP (`can_partition_optimal`)**: We can reduce the space to O(Target) by using a 1D array. Since `dp[j]` only depends on `dp[j]` and `dp[j - num]` from the *previous* row, we must iterate backwards through the target sums to avoid using a number more than once in a single step.
//!
//! Both approaches run in O(N * Target) time, where Target is the sum of all elements divided by 2.
//!
//! ## Alternative Approaches
//! - **Bitset**: Using an integer bitmask or a crate like `bit-vec` can further optimize the 1D approach by doing parallel OR operations to shift and add sums (`dp |= dp << num`). This is extremely fast for small sums but requires managing bits explicitly.
//! - **Memoized DFS**: A top-down recursive approach with a cache (`HashSet` or `HashMap`) is valid but has overhead from recursion and hashing.

/// Optimized approach: straightforward 2D Dynamic Programming (tabulation).
///
/// Time: O(N * Target), where N is the number of elements and Target is sum / 2.
/// Space: O(N * Target)
#[must_use]
#[allow(clippy::needless_pass_by_value)]
// LeetCode signature
// LeetCode constraints (values and sums fit in i32) guarantee these casts are non-negative.
#[allow(clippy::cast_sign_loss)]
pub fn can_partition_optimized(nums: Vec<i32>) -> bool {
    let total_sum: i32 = nums.iter().sum();

    // RUST INSIGHT: We can quickly reject odd sums using modulo arithmetic.
    if total_sum % 2 != 0 {
        return false;
    }

    let target = (total_sum / 2) as usize;
    let n = nums.len();

    // dp[i][j] will be true if we can form sum `j` using a subset of the first `i` items.
    // We initialize a 2D vector using the vec! macro.
    let mut dp = vec![vec![false; target + 1]; n + 1];

    // Base case: A sum of 0 is always achievable with an empty subset.
    for row in &mut dp {
        row[0] = true;
    }

    for i in 1..=n {
        let num = nums[i - 1] as usize;
        for j in 1..=target {
            if j >= num {
                // We can either include the current number or exclude it.
                dp[i][j] = dp[i - 1][j] || dp[i - 1][j - num];
            } else {
                // We can't include the current number because it's larger than the target sum.
                dp[i][j] = dp[i - 1][j];
            }
        }
    }

    dp[n][target]
}

/// Optimal approach: 1D space-optimized Dynamic Programming (rolling row).
///
/// Time: O(N * Target)
/// Space: O(Target)
#[must_use]
#[allow(clippy::needless_pass_by_value)]
// LeetCode signature
// LeetCode constraints (values and sums fit in i32) guarantee these casts are non-negative.
#[allow(clippy::cast_sign_loss)]
pub fn can_partition_optimal(nums: Vec<i32>) -> bool {
    // RUST INSIGHT: Using iterators and closures is idiomatic and often faster than manual loops.
    // The compiler can unroll and vectorize this sum.
    let total_sum: i32 = nums.iter().sum();

    if total_sum % 2 != 0 {
        return false;
    }

    let target = (total_sum / 2) as usize;

    // RUST INSIGHT: We only allocate one 1D vector. This is cache-friendly and minimizes heap allocations.
    let mut dp = vec![false; target + 1];
    dp[0] = true;

    for &num in &nums {
        let num = num as usize;
        // GOTCHA: We must iterate backwards!
        // If we iterate forwards, we might use the same `num` multiple times
        // because `dp[j]` would read from `dp[j - num]` that was already updated in the current loop.
        for j in (num..=target).rev() {
            dp[j] = dp[j] || dp[j - num];
        }
    }

    dp[target]
}

/// Main entry point - uses the optimal 1D space-optimized solution.
#[must_use]
pub fn can_partition(nums: Vec<i32>) -> bool {
    can_partition_optimal(nums)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_can_partition_happy_path() {
        // [1, 5, 11, 5] -> subsets [1, 5, 5] and [11], both sum to 11.
        let nums = vec![1, 5, 11, 5];
        assert!(can_partition_optimized(nums.clone()));
        assert!(can_partition_optimal(nums.clone()));
        assert!(can_partition(nums));
    }

    #[test]
    fn test_can_partition_edge_case_odd_sum() {
        // [1, 2, 3, 5] -> sum is 11, cannot be halved.
        let nums = vec![1, 2, 3, 5];
        assert!(!can_partition_optimized(nums.clone()));
        assert!(!can_partition_optimal(nums.clone()));
        assert!(!can_partition(nums));
    }

    #[test]
    fn test_can_partition_boundary_case_large_numbers() {
        // [100, 100, 100, 100, 100, 100, 100, 100] -> subsets of 400.
        let nums = vec![100, 100, 100, 100, 100, 100, 100, 100];
        assert!(can_partition_optimized(nums.clone()));
        assert!(can_partition_optimal(nums));
    }

    #[test]
    fn test_can_partition_edge_case_two_elements() {
        // Only two elements, must be equal.
        let nums1 = vec![1, 1];
        assert!(can_partition_optimized(nums1.clone()));
        assert!(can_partition_optimal(nums1));

        let nums2 = vec![1, 2];
        assert!(!can_partition_optimized(nums2.clone()));
        assert!(!can_partition_optimal(nums2));
    }

    #[test]
    fn test_all_approaches_agreement() {
        // Both approaches (and the main entry) must agree on the same inputs.
        let cases = vec![
            vec![1, 5, 11, 5],
            vec![1, 2, 3, 5],
            vec![2, 2, 3, 5],
            vec![1, 1],
            vec![1, 2],
            vec![3, 3, 3, 4, 5],
            vec![14, 9, 8, 4, 3, 2],
        ];

        for nums in cases {
            let expected = can_partition_optimal(nums.clone());
            assert_eq!(
                can_partition_optimized(nums.clone()),
                expected,
                "2D vs 1D mismatch for {:?}",
                nums
            );
            assert_eq!(
                can_partition(nums.clone()),
                expected,
                "wrapper mismatch for {:?}",
                nums
            );
        }
    }
}
