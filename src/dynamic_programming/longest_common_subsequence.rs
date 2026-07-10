//! # 1143. Longest Common Subsequence
//!
//! Difficulty: Medium
//! Link: https://leetcode.com/problems/longest-common-subsequence/
//!
//! Given two strings `text1` and `text2`, return the length of their longest common subsequence.
//! If there is no common subsequence, return 0.
//!
//! A subsequence of a string is a new string generated from the original string with some characters
//! (can be none) deleted without changing the relative order of the remaining characters.
//!
//! For example, "ace" is a subsequence of "abcde".
//! A common subsequence of two strings is a subsequence that is common to both strings.
//!
//! ## Why this matters in Rust
//!
//! This problem provides a perfect opportunity to discuss Rust's handling of strings versus raw bytes.
//! By default, Rust's `String` and `&str` are guaranteed to be valid UTF-8. Iterating over them character by
//! character (`.chars()`) is `O(N)` because UTF-8 characters can have variable widths (1 to 4 bytes).
//! However, if the problem constraint guarantees that the strings consist only of lowercase English letters (ASCII),
//! we can cast the strings to byte slices (`.as_bytes()`). This acts as a **zero-cost abstraction**,
//! allowing us to index into the string in `O(1)` time without UTF-8 decoding overhead.
//!
//! It also demonstrates array initialization patterns in Rust (`vec![0; n]`), ensuring safe,
//! uninitialized-memory-free dynamic programming tables.
//!
//! ## Approach
//!
//! **Dynamic Programming**
//!
//! 1.  **Brute Force (naive recursion)**: We compare the last characters of the two prefixes. If they match, the
//!     answer is `1 + lcs(i-1, j-1)`; otherwise it is `max(lcs(i-1, j), lcs(i, j-1))`. This recomputes the same
//!     overlapping subproblems exponentially.
//!
//! 2.  **Optimized 2D DP Table**: We create a 2D matrix `dp` of size `(M+1) x (N+1)`, where `M` and `N` are
//!     the lengths of `text1` and `text2`. `dp[i][j]` represents the length of the longest common subsequence
//!     of `text1[0..i]` and `text2[0..j]`.
//!     If `text1[i-1] == text2[j-1]`, then `dp[i][j] = dp[i-1][j-1] + 1`.
//!     Otherwise, `dp[i][j] = max(dp[i-1][j], dp[i][j-1])`.
//!
//! 3.  **Optimal 1D DP Array**: Notice that `dp[i][j]` only depends on the current row `dp[i][...]` and
//!     the previous row `dp[i-1][...]`. We can reduce the space complexity from `O(M * N)` to `O(min(M, N))`
//!     by only keeping track of two rows. We can further optimize it to a single row plus a `prev_diagonal` variable.
//!
//! ## Time and Space Complexity
//!
//! - **Brute Force**: Time `O(2^(M + N))` (exponential branching), Space `O(M + N)` for the recursion stack.
//! - **Optimized (2D DP)**: Time `O(M * N)`, Space `O(M * N)` for the 2D DP table.
//! - **Optimal (1D DP)**: Time `O(M * N)`, Space `O(min(M, N))` by storing only a single row of DP state,
//!   caching the smaller string.

use std::cmp;

/// Brute force approach: Naive recursion.
///
/// Compares prefixes character by character, branching on every mismatch. This recomputes the same
/// subproblems exponentially and will Time Limit Exceed on large inputs; it exists to show the base recurrence.
///
/// Time: O(2^(M + N)) - Exponential branching on mismatches.
/// Space: O(M + N) - Recursion stack depth.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn longest_common_subsequence_brute_force(text1: String, text2: String) -> i32 {
    let t1 = text1.as_bytes();
    let t2 = text2.as_bytes();

    fn solve(t1: &[u8], t2: &[u8], i: usize, j: usize) -> i32 {
        // Base case: one prefix is empty, so no common subsequence remains.
        if i == 0 || j == 0 {
            return 0;
        }

        if t1[i - 1] == t2[j - 1] {
            // Characters match: extend the LCS from the diagonal.
            1 + solve(t1, t2, i - 1, j - 1)
        } else {
            // Mismatch: drop the last char of either prefix and take the better branch.
            cmp::max(solve(t1, t2, i - 1, j), solve(t1, t2, i, j - 1))
        }
    }

    solve(t1, t2, t1.len(), t2.len())
}

/// Optimized approach: Straightforward 2D DP Table (tabulation).
///
/// Builds a full `(M+1) x (N+1)` matrix to store overlapping subproblems.
///
/// Time: O(M * N) - We fill every cell of the table once.
/// Space: O(M * N) - The full 2D DP table.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn longest_common_subsequence_optimized(text1: String, text2: String) -> i32 {
    // RUST INSIGHT: Since the problem constrains characters to lowercase English letters,
    // they are guaranteed to be valid ASCII. Operating on `&[u8]` avoids the O(N) overhead
    // of `.chars()` iterator decoding UTF-8. It's a zero-cost cast.
    let t1 = text1.as_bytes();
    let t2 = text2.as_bytes();

    let m = t1.len();
    let n = t2.len();

    // Initialize 2D DP table with zeros.
    // dp[i][j] represents LCS length of text1[..i] and text2[..j].
    // Note: Rust enforces bounds checking, but accessing `dp[i][j]` directly in nested
    // loops with vectors of vectors is idiomatic and safe.
    let mut dp = vec![vec![0; n + 1]; m + 1];

    for i in 1..=m {
        for j in 1..=n {
            if t1[i - 1] == t2[j - 1] {
                // Characters match, extend the LCS from the diagonal
                dp[i][j] = dp[i - 1][j - 1] + 1;
            } else {
                // Characters don't match, take max from skipping either char
                dp[i][j] = cmp::max(dp[i - 1][j], dp[i][j - 1]);
            }
        }
    }

    dp[m][n]
}

/// Optimal approach: 1D DP Array (space-optimized).
///
/// Reduces space complexity to O(min(M, N)) by maintaining a single row and a `prev` variable.
///
/// Time: O(M * N) - Same work as the 2D table.
/// Space: O(min(M, N)) - Only one row of the shorter string is stored.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn longest_common_subsequence_optimal(text1: String, text2: String) -> i32 {
    let mut t1 = text1.as_bytes();
    let mut t2 = text2.as_bytes();

    // RUST INSIGHT: To maximize space savings, we ensure we only allocate an array
    // for the shorter string. We swap the references if necessary.
    if t1.len() < t2.len() {
        std::mem::swap(&mut t1, &mut t2);
    }

    let m = t1.len();
    let n = t2.len();

    // dp array keeps track of the previous row's state.
    // Size is n + 1 (the length of the shorter string).
    let mut dp = vec![0; n + 1];

    for i in 1..=m {
        // prev represents dp[i-1][j-1] before it gets overwritten
        let mut prev = 0;

        for j in 1..=n {
            // Store the current dp[j] before we overwrite it; it becomes
            // the `prev` (diagonal) for the *next* inner iteration (j+1)
            let temp = dp[j];

            if t1[i - 1] == t2[j - 1] {
                // GOTCHA: We must use `prev`, not dp[j-1]. dp[j-1] corresponds to
                // dp[i][j-1] (computed in this row iteration), whereas `prev`
                // corresponds to dp[i-1][j-1] (from the previous row).
                dp[j] = prev + 1;
            } else {
                dp[j] = cmp::max(dp[j], dp[j - 1]);
            }

            prev = temp;
        }
    }

    dp[n]
}

/// Main entry point - uses the optimal solution
#[must_use]
pub fn longest_common_subsequence(text1: String, text2: String) -> i32 {
    longest_common_subsequence_optimal(text1, text2)
}

// Alternative Approaches:
//
// 1. Recursive with Memoization (Top-Down DP):
//    A recursive function `lcs(i, j)` that returns the LCS of text1[i..] and text2[j..],
//    caching results in a HashMap or a 2D array. While easier to write initially,
//    it incurs call stack overhead and is generally slower than bottom-up DP in Rust.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force_happy_path() {
        assert_eq!(
            longest_common_subsequence_brute_force("abcde".to_string(), "ace".to_string()),
            3
        );
        assert_eq!(
            longest_common_subsequence_brute_force("abc".to_string(), "abc".to_string()),
            3
        );
    }

    #[test]
    fn test_optimized_happy_path() {
        assert_eq!(
            longest_common_subsequence_optimized("abcde".to_string(), "ace".to_string()),
            3
        );
        assert_eq!(
            longest_common_subsequence_optimized("abc".to_string(), "abc".to_string()),
            3
        );
    }

    #[test]
    fn test_optimal_happy_path() {
        assert_eq!(
            longest_common_subsequence_optimal("abcde".to_string(), "ace".to_string()),
            3
        );
        assert_eq!(
            longest_common_subsequence_optimal("abc".to_string(), "abc".to_string()),
            3
        );
    }

    #[test]
    fn test_all_approaches_agreement() {
        // Keep inputs small so the exponential brute force stays fast.
        let cases = vec![
            ("abcde", "ace"),
            ("abc", "abc"),
            ("abc", "def"),
            ("", "abc"),
            ("bl", "yby"),
            ("ezupkr", "ubmrapg"),
        ];

        for (a, b) in cases {
            let expected = longest_common_subsequence_optimal(a.to_string(), b.to_string());
            assert_eq!(
                longest_common_subsequence_brute_force(a.to_string(), b.to_string()),
                expected,
                "brute force mismatch for ({a:?}, {b:?})"
            );
            assert_eq!(
                longest_common_subsequence_optimized(a.to_string(), b.to_string()),
                expected,
                "optimized mismatch for ({a:?}, {b:?})"
            );
        }
    }

    #[test]
    fn test_no_common_subsequence() {
        assert_eq!(
            longest_common_subsequence("abc".to_string(), "def".to_string()),
            0
        );
        assert_eq!(
            longest_common_subsequence("x".to_string(), "y".to_string()),
            0
        );
    }

    #[test]
    fn test_edge_cases() {
        // One string is empty
        assert_eq!(
            longest_common_subsequence("".to_string(), "abc".to_string()),
            0
        );
        assert_eq!(
            longest_common_subsequence("abc".to_string(), "".to_string()),
            0
        );
        // Single characters
        assert_eq!(
            longest_common_subsequence("a".to_string(), "a".to_string()),
            1
        );
    }

    #[test]
    fn test_stress_longer_strings() {
        let t1 = "pmjghexybyrgzczy".to_string();
        let t2 = "hafcdqbgncrcbihkd".to_string();
        assert_eq!(longest_common_subsequence(t1, t2), 4);
    }

    #[test]
    fn test_shorter_first() {
        // text1 is shorter than text2, forcing the swap logic in optimized
        let t1 = "ace".to_string();
        let t2 = "abcde".to_string();
        assert_eq!(longest_common_subsequence(t1, t2), 3);
    }
}
