//! # 91. Decode Ways
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/decode-ways/>
//!
//! A message containing letters from `A-Z` can be encoded into numbers using the following mapping:
//! `'A' -> "1"`, `'B' -> "2"`, ..., `'Z' -> "26"`.
//!
//! Given a string `s` containing only digits, return the number of ways to decode it.
//!
//! This problem demonstrates how to use zero-cost abstractions like `.as_bytes()` in Rust
//! for safe, ASCII-only string slicing without the overhead of UTF-8 validation, and how
//! pattern matching and range bounds (`contains()`) make validation concise and readable.
//!
//! ## Approach
//!
//! The problem asks for the number of ways to decode a string. Since the decoding of a suffix
//! only depends on the remaining characters, this is a perfect fit for Dynamic Programming.
//!
//! We start with a 1D DP array where `dp[i]` represents the number of ways to decode the prefix of length `i`.
//! A character `s[i-1]` can be a valid single-digit decoding if it's between '1' and '9'.
//! Two characters `s[i-2..i]` can be a valid two-digit decoding if they form a number between 10 and 26.
//!
//! Then we optimize it: since `dp[i]` only depends on `dp[i-1]` and `dp[i-2]`, we can reduce
//! the space complexity to O(1) by only keeping track of the last two states, similar to the Fibonacci sequence.
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

/// Brute force approach: Recursive Depth-First Search.
/// Time: O(2^N) - We branch up to 2 times for each index.
/// Space: O(N) - Recursion stack depth.
///
/// This approach tries every possible valid partition of the string.
/// It recalculates the same suffixes many times, leading to Time Limit Exceeded
/// for larger strings.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn num_decodings_brute_force(s: String) -> i32 {
    fn dfs(bytes: &[u8], index: usize) -> i32 {
        if index == bytes.len() {
            return 1;
        }

        // RUST INSIGHT: Byte string literals like `b'0'` allow fast, direct comparisons
        // against UTF-8 string bytes without needing `unwrap()` or `chars()`.
        if bytes[index] == b'0' {
            return 0;
        }

        let mut ways = dfs(bytes, index + 1);

        if index + 1 < bytes.len() {
            let two_digit = (bytes[index] - b'0') * 10 + (bytes[index + 1] - b'0');
            // RUST INSIGHT: `.contains()` on a RangeInclusive `..=` is idiomatic
            // and compiles down to efficient bounds checking.
            if (10..=26).contains(&two_digit) {
                ways += dfs(bytes, index + 2);
            }
        }

        ways
    }

    // GOTCHA: Calling `s.chars()` and indexing into a `Vec<char>` is O(N) overhead.
    // Since the string is guaranteed to contain only digits (ASCII), we use `.as_bytes()`.
    dfs(s.as_bytes(), 0)
}

/// Optimized approach: 1D Dynamic Programming (Array).
/// Time: O(N) - We iterate through the string once.
/// Space: O(N) - We store the number of ways for each prefix length in a `Vec`.
///
/// We build a `dp` table where `dp[i]` is the number of ways to decode the first `i` characters.
/// This trades memory for speed, eliminating redundant recursive calls.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn num_decodings_optimized(s: String) -> i32 {
    let bytes = s.as_bytes();
    let n = bytes.len();

    if n == 0 || bytes[0] == b'0' {
        return 0;
    }

    let mut dp = vec![0; n + 1];
    dp[0] = 1;
    dp[1] = 1;

    for i in 2..=n {
        let single_digit = bytes[i - 1] - b'0';
        let double_digit = (bytes[i - 2] - b'0') * 10 + single_digit;

        if single_digit != 0 {
            dp[i] += dp[i - 1];
        }

        if (10..=26).contains(&double_digit) {
            dp[i] += dp[i - 2];
        }
    }

    dp[n]
}

/// Optimal approach: Space-Optimized Dynamic Programming.
/// Time: O(N) - Single pass through the string.
/// Space: O(1) - Only two variables are needed for state.
///
/// Since `dp[i]` only depends on `dp[i-1]` and `dp[i-2]`, we don't need an array
/// of size N. We just maintain the last two states, massively reducing memory usage.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn num_decodings_optimal(s: String) -> i32 {
    let bytes = s.as_bytes();

    if bytes.is_empty() || bytes[0] == b'0' {
        return 0;
    }

    let mut two_back = 1; // dp[i-2]
    let mut one_back = 1; // dp[i-1]

    // RUST INSIGHT: `.iter().skip(1).enumerate()` is zero-cost and avoids manual indexing
    // where possible, though we still index `bytes` for the `two_back` logic.
    for i in 1..bytes.len() {
        let mut current = 0;
        let single_digit = bytes[i] - b'0';
        let double_digit = (bytes[i - 1] - b'0') * 10 + single_digit;

        if single_digit != 0 {
            current += one_back;
        }

        if (10..=26).contains(&double_digit) {
            current += two_back;
        }

        two_back = one_back;
        one_back = current;
    }

    one_back
}

/// Main entry point - uses optimal space O(1) DP approach.
#[must_use]
pub fn num_decodings(s: String) -> i32 {
    num_decodings_optimal(s)
}

// ## Alternative Approaches
//
// 1. **Memoized Recursion**: `dfs` with a `HashMap<usize, i32>` or `Vec<Option<i32>>` to cache results.
//    This achieves O(N) time but still uses O(N) space and has function call overhead compared to iterative DP.
// 2. **Iterative DP with mutable array**: Sometimes `dp` tables can be more readable for 2D or complex problems,
//    but for Fibonacci-like sequences, constant space is preferred.

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
    }

    // Stress/Boundary tests
    #[test]
    fn test_num_decodings_stress() {
        let long_string = "11111111111111111111111111111111111111111111".to_string(); // 44 characters
        // We skip brute force here because O(2^44) is way too slow
        let expected = 1134903170; // 45th Fibonacci number
        assert_eq!(num_decodings_optimized(long_string.clone()), expected);
        assert_eq!(num_decodings_optimal(long_string), expected);
    }
}
