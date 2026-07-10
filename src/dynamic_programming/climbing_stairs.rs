//! # 70. Climbing Stairs
//!
//! Difficulty: Easy
//! Link: <https://leetcode.com/problems/climbing-stairs/>
//!
//! You are climbing a staircase. It takes `n` steps to reach the top.
//! Each time you can either climb 1 or 2 steps. In how many distinct ways can you climb to the top?
//!
//! This problem matters in Rust as an introduction to Dynamic Programming. It highlights
//! the journey from naive recursion to memoization, and finally to space-optimized DP using
//! Rust's standard loop constructs and variable shadowing/mutability semantics.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::dynamic_programming::climbing_stairs::climb_stairs;
//!
//! assert_eq!(climb_stairs(2), 2); // 1+1, 2
//! assert_eq!(climb_stairs(3), 3); // 1+1+1, 1+2, 2+1
//! ```
//!
//! ## Constraints
//!
//! - `1 <= n <= 45`

/// Brute force approach: Naive Recursion.
/// Time: O(2^N) - branching factor of 2 at each step.
/// Space: O(N) - recursion tree depth.
///
/// This approach explores every possible combination recursively. It calculates the same
/// subproblems repeatedly (e.g., `climb_stairs(n-2)` is calculated in both branches).
/// This will Time Limit Exceed (TLE) on `LeetCode` for large N.
#[must_use]
pub fn climb_stairs_brute_force(n: i32) -> i32 {
    if n <= 2 {
        return n;
    }
    climb_stairs_brute_force(n - 1) + climb_stairs_brute_force(n - 2)
}

/// Optimized approach: Dynamic Programming with Memoization (Array/Vector).
/// Time: O(N) - each step is calculated exactly once.
/// Space: O(N) - we store the result for each step in a `Vec`.
///
/// We build a 1D DP table from the bottom up. `dp[i]` stores the number of ways
/// to reach step `i`. This eliminates the redundant calculations of the brute force method.
#[must_use]
pub fn climb_stairs_optimized(n: i32) -> i32 {
    if n <= 2 {
        return n;
    }

    // RUST INSIGHT: We use `n as usize + 1` for 1-based indexing matching the problem.
    // The `vec!` macro initializes the entire vector with zeros, which is fast and safe.
    let mut dp = vec![0; n as usize + 1];

    // Base cases
    dp[1] = 1;
    dp[2] = 2;

    for i in 3..=n as usize {
        dp[i] = dp[i - 1] + dp[i - 2];
    }

    dp[n as usize]
}

/// Optimal approach: Space-Optimized Dynamic Programming.
/// Time: O(N) - exactly N iterations.
/// Space: O(1) - we only keep track of the last two calculated values.
///
/// Since `dp[i]` only depends on `dp[i-1]` and `dp[i-2]`, we don't need to store
/// the entire history. We just need two variables to represent the state, shifting
/// them forward at each step (this is effectively the Fibonacci sequence logic).
#[must_use]
pub fn climb_stairs_optimal(n: i32) -> i32 {
    if n <= 2 {
        return n;
    }

    // `two_steps_behind` corresponds to dp[i-2]
    // `one_step_behind` corresponds to dp[i-1]
    let mut two_steps_behind = 1;
    let mut one_step_behind = 2;

    for _ in 3..=n {
        let current = one_step_behind + two_steps_behind;
        two_steps_behind = one_step_behind;
        one_step_behind = current;
    }

    one_step_behind
}

/// Main entry point - uses the optimal space O(1) DP approach.
#[must_use]
pub fn climb_stairs(n: i32) -> i32 {
    climb_stairs_optimal(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Happy Path tests
    #[test]
    fn test_climb_stairs_small() {
        assert_eq!(climb_stairs_brute_force(2), 2);
        assert_eq!(climb_stairs_optimized(2), 2);
        assert_eq!(climb_stairs_optimal(2), 2);

        assert_eq!(climb_stairs_brute_force(3), 3);
        assert_eq!(climb_stairs_optimized(3), 3);
        assert_eq!(climb_stairs_optimal(3), 3);
    }

    // Edge Case tests
    #[test]
    fn test_climb_stairs_base_case() {
        assert_eq!(climb_stairs_brute_force(1), 1);
        assert_eq!(climb_stairs_optimized(1), 1);
        assert_eq!(climb_stairs_optimal(1), 1);
    }

    // Stress/Boundary tests
    #[test]
    fn test_climb_stairs_large() {
        // The brute force approach takes too long for n=45, so we skip it in the stress test
        // or just test it with a smaller "large" number if we strictly need to test it.
        // We'll test up to 30 for brute force to keep test times reasonable.
        assert_eq!(climb_stairs_brute_force(30), 1346269);

        // For optimized and optimal, we can easily test the max constraint (n=45)
        // 45th Fibonacci-like number for stairs is 1836311903
        assert_eq!(climb_stairs_optimized(45), 1836311903);
        assert_eq!(climb_stairs_optimal(45), 1836311903);
    }
}
