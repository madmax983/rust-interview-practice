//! # 77. Combinations
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/combinations/>
//!
//! Given two integers `n` and `k`, return all possible combinations of `k` numbers chosen from the range `[1, n]`.
//!
//! This problem matters in Rust because it perfectly illustrates the power of recursive backtracking and how to manage
//! memory efficiently using a single mutable buffer (`&mut Vec<i32>`). Instead of allocating a new vector at every step
//! of the recursion, we reuse the same buffer and clone it only when a complete combination is found, demonstrating
//! zero-cost abstractions over memory management.
//!
//! ## Approach
//!
//! We will explore two approaches:
//! 1.  **Brute Force (Straightforward Recursive)**: Uses a standard backtracking template. We loop through valid
//!     candidates, push a candidate onto our path buffer, recurse to find the remaining numbers, and pop to backtrack.
//! 2.  **Optimal (Pruned Backtracking)**: Adds an algorithmic optimization. If the remaining available numbers
//!     are insufficient to reach the required combination size `k`, we can mathematically short-circuit
//!     and stop exploring that branch entirely. Both approaches share the same `O(k * C(n, k))` complexity; the
//!     pruning simply avoids descending into branches that can never complete.
//!
//! Time Complexity: O(k * C(n, k)) where C(n, k) is the binomial coefficient. We generate all combinations,
//! and each takes O(k) time to clone into the result array.
//! Space Complexity: O(k) auxiliary stack space for the recursion depth and path buffer.
//!
//! ## Alternative Approaches
//!
//! - **Iterative Lexicographical Generation**: Uses an iterative algorithm to find the next combination in lexicographical
//!   order instead of using recursion. While more memory efficient (O(1) extra space), it can be more complex to read
//!   and reason about than standard backtracking.
//! - **Bit Manipulation (Gosper's Hack)**: Generates combinations by using bitwise operations to find the next number
//!   with the same number of set bits.

/// Brute force approach: Straightforward Backtracking (no pruning).
///
/// Uses a `&mut Vec<i32>` buffer to store the current combination. When the buffer reaches size `k`,
/// we clone it and push it into the results.
///
/// ⚡ BOLT OPTIMIZATION: We pre-calculate the mathematical combinations `C(n, k)` to pre-allocate
/// the exact capacity for the `results` vector, completely preventing dynamic heap reallocations.
#[must_use]
// LeetCode constraints (1 <= k <= n <= 20) guarantee these casts fit.
#[allow(clippy::cast_sign_loss)]
pub fn combine_brute_force(n: i32, k: i32) -> Vec<Vec<i32>> {
    fn backtrack(start: i32, n: i32, k: i32, path: &mut Vec<i32>, results: &mut Vec<Vec<i32>>) {
        // Base case: we have selected exactly `k` elements.
        // RUST INSIGHT: path.len() returns a `usize`, so we cast `k` to `usize` for a safe comparison.
        if path.len() == k as usize {
            // Clone the current path into the results list. This is our only heap allocation per valid combination.
            results.push(path.clone());
            return;
        }

        // Loop through all possible next elements.
        for i in start..=n {
            path.push(i);
            backtrack(i + 1, n, k, path, results);
            // GOTCHA: Don't forget to pop! Backtracking requires us to restore the state
            // to what it was before we recursed, so we can try the next candidate.
            path.pop();
        }
    }

    let capacity = {
        let k_min = k.min(n - k);
        let mut res = 1;
        for i in 1..=k_min {
            res = res * (n - k_min + i) as usize / i as usize;
        }
        res
    };
    let mut results = Vec::with_capacity(capacity);
    let mut current_path = Vec::with_capacity(k as usize);

    backtrack(1, n, k, &mut current_path, &mut results);
    results
}

/// Optimal approach: Pruned Backtracking.
///
/// This version includes an early exit condition. If we are currently exploring a branch
/// where the number of remaining elements is less than the number of elements we still need to pick,
/// we stop iterating. This is the canonical solution; it shares the brute force's `O(k * C(n, k))`
/// complexity but prunes dead branches for a better constant factor.
///
/// ⚡ BOLT OPTIMIZATION: We pre-calculate the mathematical combinations `C(n, k)` to pre-allocate
/// the exact capacity for the `results` vector, completely preventing dynamic heap reallocations.
#[must_use]
// LeetCode constraints (1 <= k <= n <= 20) guarantee these casts fit.
#[allow(
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap
)]
pub fn combine_optimal(n: i32, k: i32) -> Vec<Vec<i32>> {
    fn backtrack(start: i32, n: i32, k: i32, path: &mut Vec<i32>, results: &mut Vec<Vec<i32>>) {
        if path.len() == k as usize {
            results.push(path.clone());
            return;
        }

        // Optimization: if we don't have enough elements left to make a full combination, we can stop early.
        // `k as usize - path.len()` is the number of elements we still need.
        // If `n - i + 1` (the number of available elements) is less than what we need, the loop condition will fail.
        let needed = k as usize - path.len();
        // RUST INSIGHT: We use `max(0)` implicitly by clamping bounds to avoid underflow if logic gets tweaked,
        // but here the bounds are guaranteed safe. `n - needed as i32 + 1` calculates the maximum starting value.
        let max_start = n - needed as i32 + 1;

        for i in start..=max_start {
            path.push(i);
            backtrack(i + 1, n, k, path, results);
            path.pop();
        }
    }

    let capacity = {
        let k_min = k.min(n - k);
        let mut res = 1;
        for i in 1..=k_min {
            res = res * (n - k_min + i) as usize / i as usize;
        }
        res
    };
    let mut results = Vec::with_capacity(capacity);
    // Mathematically pre-allocate to eliminate dynamic reallocations of our active path
    let mut current_path = Vec::with_capacity(k as usize);

    backtrack(1, n, k, &mut current_path, &mut results);
    results
}

/// Main entry point - aliases to the optimal solution for competitive programming defaults.
#[must_use]
pub fn combine(n: i32, k: i32) -> Vec<Vec<i32>> {
    combine_optimal(n, k)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to check if results match, ignoring the order of combinations
    fn check_results(mut actual: Vec<Vec<i32>>, mut expected: Vec<Vec<i32>>) {
        actual.sort_unstable();
        expected.sort_unstable();
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_brute_force_example1() {
        let n = 4;
        let k = 2;
        let expected = vec![
            vec![1, 2],
            vec![1, 3],
            vec![1, 4],
            vec![2, 3],
            vec![2, 4],
            vec![3, 4],
        ];
        check_results(combine_brute_force(n, k), expected);
    }

    #[test]
    fn test_optimal_example1() {
        let n = 4;
        let k = 2;
        let expected = vec![
            vec![1, 2],
            vec![1, 3],
            vec![1, 4],
            vec![2, 3],
            vec![2, 4],
            vec![3, 4],
        ];
        check_results(combine_optimal(n, k), expected);
    }

    #[test]
    fn test_main_example1() {
        let n = 4;
        let k = 2;
        let expected = vec![
            vec![1, 2],
            vec![1, 3],
            vec![1, 4],
            vec![2, 3],
            vec![2, 4],
            vec![3, 4],
        ];
        check_results(combine(n, k), expected);
    }

    #[test]
    fn test_edge_case_n_equals_k() {
        // When n == k, there is only one valid combination.
        let n = 3;
        let k = 3;
        let expected = vec![vec![1, 2, 3]];
        check_results(combine(n, k), expected.clone());
        check_results(combine_brute_force(n, k), expected);
    }

    #[test]
    fn test_boundary_case_k_equals_1() {
        // When k == 1, each individual number is a combination.
        let n = 3;
        let k = 1;
        let expected = vec![vec![1], vec![2], vec![3]];
        check_results(combine(n, k), expected.clone());
        check_results(combine_brute_force(n, k), expected);
    }

    #[test]
    fn test_all_approaches_agree() {
        // Cross-implementation agreement: both approaches produce the same set of
        // combinations (order-independent) for a non-trivial input.
        let n = 5;
        let k = 3;
        let mut bf = combine_brute_force(n, k);
        let mut opt = combine_optimal(n, k);
        bf.sort_unstable();
        opt.sort_unstable();
        assert_eq!(bf, opt);
    }
}
