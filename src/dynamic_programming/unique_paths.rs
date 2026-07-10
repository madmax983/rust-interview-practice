//! # 62. Unique Paths
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/unique-paths>/
//!
//! A robot is located at the top-left corner of a `m x n` grid.
//! The robot can only move either down or right at any point in time.
//! The robot is trying to reach the bottom-right corner of the grid.
//! How many possible unique paths are there?
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::dynamic_programming::unique_paths::unique_paths;
//!
//! assert_eq!(unique_paths(3, 7), 28);
//! assert_eq!(unique_paths(3, 2), 3);
//! ```
//!
//! ## Constraints
//!
//! - `1 <= m, n <= 100`
//! - It's guaranteed that the answer will be less than or equal to `2 * 10^9`.
//!
//! ## Why this matters in Rust
//!
//! This problem perfectly illustrates the progression from basic recursion to 2D Dynamic
//! Programming, and ultimately to 1D space optimization. In Rust, minimizing heap allocations
//! (like creating `Vec<Vec<i32>>` in loops) is a key performance tenet. By optimizing state down
//! to a single 1D `Vec<i32>` array updated in place, we leverage Rust's strict mutability rules
//! to safely build a high-performance, zero-overhead solution.

/// Brute force approach: Pure Recursion
///
/// The robot can move down or right. We can recursively branch into both choices.
///
/// Time: O(2^(m+n)) - At each cell, we have 2 choices, leading to an exponential tree.
/// Space: O(m+n) - Max depth of the recursion stack.
///
/// # GOTCHA:
/// This solution will Time Out on `LeetCode` for large grids because we repeatedly compute
/// the same subproblems. It is solely here to demonstrate the mathematical base structure.
#[must_use]
pub fn unique_paths_brute_force(m: i32, n: i32) -> i32 {
    // Helper function to perform recursion
    fn dfs(r: i32, c: i32, m: i32, n: i32) -> i32 {
        // Base case: out of bounds
        if r >= m || c >= n {
            return 0;
        }
        // Base case: reached destination
        if r == m - 1 && c == n - 1 {
            return 1;
        }

        // RUST INSIGHT: We rely on standard tail-recursion-like calls, but Rust doesn't
        // guarantee TCO (Tail Call Optimization). Regardless, depth is bounded by m+n.
        dfs(r + 1, c, m, n) + dfs(r, c + 1, m, n)
    }

    dfs(0, 0, m, n)
}

/// Optimized approach: 2D Dynamic Programming
///
/// We can memorize the number of paths to each cell.
/// `dp[i][j] = dp[i-1][j] + dp[i][j-1]`
///
/// Time: O(m * n) - We iterate through each cell once.
/// Space: O(m * n) - We allocate a 2D grid.
#[must_use]
#[allow(clippy::cast_sign_loss)] // m and n are >= 1
pub fn unique_paths_optimized(m: i32, n: i32) -> i32 {
    let rows = m as usize;
    let cols = n as usize;

    // GOTCHA: Allocating a `Vec<Vec<T>>` involves multiple heap allocations
    // (one for the outer Vec, and one for each inner Vec).
    // This is fine for O(m*n) space but can be inefficient if m is very large.
    let mut dp = vec![vec![1; cols]; rows];

    // RUST INSIGHT: Notice we skip row 0 and col 0 entirely, since they are already
    // initialized to 1 (there is only 1 path to anywhere in the first row or first col).
    for r in 1..rows {
        for c in 1..cols {
            // Update the current cell with paths from top and paths from left
            dp[r][c] = dp[r - 1][c] + dp[r][c - 1];
        }
    }

    dp[rows - 1][cols - 1]
}

/// Optimal approach: 1D Dynamic Programming (Space Optimization)
///
/// Notice that `dp[i][j]` only depends on `dp[i-1][j]` (the cell directly above)
/// and `dp[i][j-1]` (the cell directly to the left).
/// We can compress our 2D array into a 1D array representing the "current row".
///
/// Time: O(m * n) - We still visit every conceptual cell.
/// Space: O(n) - We only store one row of size `n`.
#[must_use]
#[allow(clippy::cast_sign_loss)] // constraints guarantee positive m, n
pub fn unique_paths_optimal(m: i32, n: i32) -> i32 {
    // If we want to be fully optimal, we can iterate over the smaller dimension,
    // but here we keep it simple: row length is `n`.
    let cols = n as usize;

    // RUST INSIGHT: We allocate a single contiguous `Vec<i32>` on the heap.
    // This is significantly faster and more cache-friendly than `Vec<Vec<i32>>`.
    let mut row = vec![1; cols];

    // We start at row 1 because row 0 is already correctly filled with 1s.
    for _ in 1..m {
        // RUST INSIGHT: We iterate mutably from left to right.
        // `row[c]` conceptually acts as both:
        // 1. the value from the previous row `dp[r-1][c]` (before we update it)
        // 2. the newly updated value `dp[r][c]` (after we update it)
        for c in 1..cols {
            row[c] += row[c - 1];
        }
    }

    row[cols - 1]
}

/// Main entry point - uses optimal solution
#[must_use]
pub fn unique_paths(m: i32, n: i32) -> i32 {
    unique_paths_optimal(m, n)
}

// =========================================================================================
// Alternative Approaches
// =========================================================================================
// 1. Math (Combinatorics): The robot needs to make exactly (m-1) down moves and (n-1) right moves.
//    Total moves = (m-1) + (n-1) = m+n-2.
//    The answer is simply (m+n-2) Choose (m-1) or (m+n-2) Choose (n-1).
//    Time: O(min(m, n)), Space: O(1).
//    This is mathematically optimal but the DP approach teaches array state management.

#[cfg(test)]
mod tests {
    use super::*;

    // Happy Path
    #[test]
    fn test_brute_force_example_1() {
        assert_eq!(unique_paths_brute_force(3, 7), 28);
    }

    #[test]
    fn test_optimized_example_1() {
        assert_eq!(unique_paths_optimized(3, 7), 28);
    }

    #[test]
    fn test_optimal_example_1() {
        assert_eq!(unique_paths_optimal(3, 7), 28);
    }

    // Secondary Example
    #[test]
    fn test_optimal_example_2() {
        assert_eq!(unique_paths_optimal(3, 2), 3);
    }

    // Edge Cases
    #[test]
    fn test_edge_case_1x1() {
        assert_eq!(unique_paths_brute_force(1, 1), 1);
        assert_eq!(unique_paths_optimized(1, 1), 1);
        assert_eq!(unique_paths_optimal(1, 1), 1);
    }

    #[test]
    fn test_edge_case_1_row() {
        assert_eq!(unique_paths_optimal(1, 100), 1);
    }

    #[test]
    fn test_edge_case_1_col() {
        assert_eq!(unique_paths_optimal(100, 1), 1);
    }

    // Cross-implementation validation
    #[test]
    fn test_cross_implementation() {
        // Cannot use large numbers for brute force since it's O(2^(m+n))
        let pairs = vec![(2, 2), (3, 3), (4, 4), (5, 3), (3, 5)];

        for (m, n) in pairs {
            let bf = unique_paths_brute_force(m, n);
            let opt = unique_paths_optimized(m, n);
            let optimal = unique_paths_optimal(m, n);

            assert_eq!(
                bf, opt,
                "Mismatch between brute force and optimized for ({}, {})",
                m, n
            );
            assert_eq!(
                opt, optimal,
                "Mismatch between optimized and optimal for ({}, {})",
                m, n
            );
        }
    }
}
