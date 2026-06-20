//! # 91. Decode Ways
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/decode-ways/>
//!
//! A message containing letters from `A-Z` can be encoded into numbers using the following mapping:
//! `'A' -> "1"`, `'B' -> "2"`, ..., `'Z' -> "26"`.
//!
//! To decode an encoded message, all the digits must be grouped then mapped back into letters
//! using the reverse of the mapping above (there may be multiple ways). For example, `"11106"`
//! can be mapped into:
//! - `"AAJF"` with the grouping `(1 1 10 6)`
//! - `"KJF"` with the grouping `(11 10 6)`
//!
//! Note that the grouping `(1 11 06)` is invalid because `"06"` cannot be mapped into `'F'` since `"6"` is different from `"06"`.
//! Given a string `s` containing only digits, return the number of ways to decode it.
//!
//! This problem matters in Rust as it demonstrates 1D dynamic programming with an O(1) space optimization and highlights using zero-cost abstractions like `.as_bytes()` for ASCII string slicing and range bounds matching (`(1..=9).contains()`) instead of expensive substring parsing.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::dynamic_programming::decode_ways::num_decodings;
//!
//! assert_eq!(num_decodings("12".to_string()), 2); // "AB" (1 2) or "L" (12)
//! assert_eq!(num_decodings("226".to_string()), 3); // "BZ" (2 26), "VF" (22 6), or "BBF" (2 2 6)
//! assert_eq!(num_decodings("06".to_string()), 0);
//! ```
//!
//! ## Constraints
//!
//! - `1 <= s.length <= 100`
//! - `s` contains only digits and may contain leading zero(s).

/// Brute force approach: Naive Recursion.
/// Time: O(2^N) - branching factor of up to 2 at each step.
/// Space: O(N) - recursion tree depth.
///
/// This approach explores every possible decoding recursively.
/// It will Time Limit Exceed (TLE) on LeetCode for large strings.
#[must_use]
pub fn num_decodings_brute_force(s: String) -> i32 {
    fn dfs(i: usize, bytes: &[u8]) -> i32 {
        if i == bytes.len() {
            return 1;
        }
        if bytes[i] == b'0' {
            return 0;
        }

        // Single digit decode
        let mut ways = dfs(i + 1, bytes);

        // Double digit decode
        if i + 1 < bytes.len() {
            let two_digit = (bytes[i] - b'0') * 10 + (bytes[i + 1] - b'0');
            if (10..=26).contains(&two_digit) {
                ways += dfs(i + 2, bytes);
            }
        }

        ways
    }

    // RUST INSIGHT: .as_bytes() provides O(1) zero-cost abstraction to treat the string as a byte slice.
    // Since the problem guarantees digits only (ASCII), we don't need expensive UTF-8 parsing.
    dfs(0, s.as_bytes())
}

/// Optimized approach: Dynamic Programming with Memoization (Array/Vector).
/// Time: O(N) - each step is calculated exactly once.
/// Space: O(N) - we store the result for each step in a `Vec`.
///
/// We build a 1D DP table from the bottom up. `dp[i]` stores the number of ways
/// to decode `s[i..]`. This eliminates redundant calculations.
#[must_use]
pub fn num_decodings_optimized(s: String) -> i32 {
    let bytes = s.as_bytes();
    let n = bytes.len();

    if n == 0 || bytes[0] == b'0' {
        return 0;
    }

    let mut dp = vec![0; n + 1];
    dp[0] = 1; // Base case: 1 way to decode empty string
    dp[1] = 1; // Base case: 1 way to decode length 1 string if not '0'

    for i in 2..=n {
        // Single digit match
        if bytes[i - 1] != b'0' {
            dp[i] += dp[i - 1];
        }

        // Two digit match
        let two_digit = (bytes[i - 2] - b'0') * 10 + (bytes[i - 1] - b'0');
        if (10..=26).contains(&two_digit) {
            dp[i] += dp[i - 2];
        }
    }

    dp[n]
}

/// Optimal approach: Space-Optimized Dynamic Programming.
/// Time: O(N) - single pass over the string.
/// Space: O(1) - we only keep track of the last two calculated values.
///
/// Since `dp[i]` only depends on `dp[i-1]` and `dp[i-2]`, we only need two variables to represent the state.
#[must_use]
pub fn num_decodings_optimal(s: String) -> i32 {
    let bytes = s.as_bytes();
    let n = bytes.len();

    // GOTCHA: Leading zero immediately invalidates the entire string.
    if n == 0 || bytes[0] == b'0' {
        return 0;
    }

    // `two_steps_behind` corresponds to dp[i-2]
    // `one_step_behind` corresponds to dp[i-1]
    let mut two_steps_behind = 1;
    let mut one_step_behind = 1;

    for i in 1..n {
        let mut current = 0;

        // Single digit match
        if bytes[i] != b'0' {
            current += one_step_behind;
        }

        // Two digit match
        let two_digit = (bytes[i - 1] - b'0') * 10 + (bytes[i] - b'0');
        if (10..=26).contains(&two_digit) {
            current += two_steps_behind;
        }

        two_steps_behind = one_step_behind;
        one_step_behind = current;
    }

    one_step_behind
}

/// Main entry point - uses optimal space O(1) DP approach.
#[must_use]
pub fn num_decodings(s: String) -> i32 {
    num_decodings_optimal(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Happy Path tests
    #[test]
    fn test_num_decodings_happy_path() {
        assert_eq!(num_decodings_brute_force("12".to_string()), 2);
        assert_eq!(num_decodings_optimized("12".to_string()), 2);
        assert_eq!(num_decodings_optimal("12".to_string()), 2);

        assert_eq!(num_decodings_brute_force("226".to_string()), 3);
        assert_eq!(num_decodings_optimized("226".to_string()), 3);
        assert_eq!(num_decodings_optimal("226".to_string()), 3);
    }

    // Edge Case tests
    #[test]
    fn test_num_decodings_edge_cases() {
        assert_eq!(num_decodings_brute_force("06".to_string()), 0);
        assert_eq!(num_decodings_optimized("06".to_string()), 0);
        assert_eq!(num_decodings_optimal("06".to_string()), 0);

        assert_eq!(num_decodings_brute_force("10".to_string()), 1);
        assert_eq!(num_decodings_optimized("10".to_string()), 1);
        assert_eq!(num_decodings_optimal("10".to_string()), 1);

        assert_eq!(num_decodings_brute_force("27".to_string()), 1);
        assert_eq!(num_decodings_optimized("27".to_string()), 1);
        assert_eq!(num_decodings_optimal("27".to_string()), 1);

        assert_eq!(num_decodings_brute_force("2101".to_string()), 1);
        assert_eq!(num_decodings_optimized("2101".to_string()), 1);
        assert_eq!(num_decodings_optimal("2101".to_string()), 1);
    }

    // Stress/Boundary tests
    #[test]
    fn test_num_decodings_stress() {
        let long_string = "111111111111111111111111111111111111111111111"; // 45 ones
        assert_eq!(num_decodings_optimized(long_string.to_string()), 1836311903);
        assert_eq!(num_decodings_optimal(long_string.to_string()), 1836311903);
        // Note: brute force will TLE here, so we skip testing it on the long string
    }
}

/// ## Alternative Approaches
///
/// 1. **Backtracking (DFS) with Memoization**: Instead of building a DP table from the bottom up,
///    we could use recursion with a `HashMap` or a simple array to cache results from the top down.
///    This is often more intuitive to write but carries function call overhead compared to iterative DP.
///    In Python or Java, you might use an `@lru_cache` or a simple array, but in Rust, iterative DP
///    is generally preferred to avoid recursion depth issues and borrow checker complexities around
///    passing a mutable cache through recursive calls.
///
/// 2. **Iterative DP with String Slicing**: We could iterate over the string and use `&s[i-1..=i]`
///    to check two-digit codes. However, string slicing in Rust returns `&str` and requires UTF-8 validation
///    checks, which adds unnecessary overhead when we know the input is ASCII digits. The `.as_bytes()`
///    approach used here is much more idiomatic and performant for this specific problem.
