//! # 91. Decode Ways
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/decode-ways/>
//!
//! A message containing letters from A-Z can be encoded into numbers using the following mapping:
//! 'A' -> "1", 'B' -> "2", ..., 'Z' -> "26".
//! Given a string `s` containing only digits, return the number of ways to decode it.
//!
//! This problem matters in Rust because it perfectly demonstrates how to perform high-performance
//! dynamic programming over strings using raw byte access. The problem guarantees ASCII digits,
//! so instead of `O(N)` UTF-8 decoding overhead using `.chars()`, we can use zero-cost
//! `.as_bytes()` to process the string with maximum efficiency.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::dynamic_programming::decode_ways::num_decodings;
//!
//! assert_eq!(num_decodings("12".to_string()), 2); // "AB" (1 2) or "L" (12)
//! assert_eq!(num_decodings("226".to_string()), 3); // "BZ" (2 26), "VF" (22 6), or "BBF" (2 2 6)
//! assert_eq!(num_decodings("06".to_string()), 0); // "06" cannot be mapped to "F" because of the leading zero
//! ```
//!
//! ## Constraints
//!
//! - `1 <= s.length <= 100`
//! - `s` contains only digits and may contain leading zero(s).
//!
//! ## Approach
//!
//! This is a classic dynamic programming problem. At each step `i`, we can:
//! 1. Decode the single digit at `s[i]` if it's between '1' and '9'.
//! 2. Decode the two digits ending at `s[i]` if they form a valid number between '10' and '26'.
//!
//! The number of ways to decode up to `i` is the sum of ways to decode up to `i-1` (if single digit valid)
//! plus ways to decode up to `i-2` (if two digits valid).
//!
//! We provide two approaches:
//! 1. `num_decodings_straightforward`: Uses an `O(N)` space DP array. Good for understanding the state transitions.
//! 2. `num_decodings_optimized`: Uses `O(1)` space since we only ever need the previous two states.

/// Straightforward approach: O(N) Space Dynamic Programming.
///
/// Time: O(N) where N is the length of the string.
/// Space: O(N) for the DP array.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn num_decodings_straightforward(s: String) -> i32 {
    let bytes = s.as_bytes();
    let n = bytes.len();
    if n == 0 || bytes[0] == b'0' {
        return 0;
    }

    // dp[i] stores the number of ways to decode the prefix of length i
    let mut dp = vec![0; n + 1];
    dp[0] = 1;
    dp[1] = 1;

    for i in 2..=n {
        let single_digit = bytes[i - 1] - b'0';
        let double_digit = (bytes[i - 2] - b'0') * 10 + single_digit;

        if single_digit >= 1 {
            dp[i] += dp[i - 1];
        }
        if (10..=26).contains(&double_digit) {
            dp[i] += dp[i - 2];
        }
    }

    dp[n]
}

/// Optimized approach: O(1) Space Dynamic Programming.
///
/// Time: O(N) where N is the length of the string.
/// Space: O(1) as we only store the previous two states.
///
/// # Idiomatic Rust
/// - `.as_bytes()` allows zero-cost access to individual characters. Since the constraints
///   guarantee the string only contains digits (ASCII), we don't need UTF-8 awareness.
/// - We use byte literals like `b'0'` to safely perform arithmetic on the byte values.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn num_decodings_optimized(s: String) -> i32 {
    let bytes = s.as_bytes();

    // GOTCHA: It's important to handle empty strings or leading zeroes immediately.
    if bytes.is_empty() || bytes[0] == b'0' {
        return 0;
    }

    // RUST INSIGHT: Variables `two_back` and `one_back` track our two previous states,
    // exactly like the Fibonacci sequence. This drops space complexity from O(N) to O(1).
    let mut two_back = 1; // Represents dp[i-2] (starts at dp[0])
    let mut one_back = 1; // Represents dp[i-1] (starts at dp[1])

    for i in 1..bytes.len() {
        let mut current = 0;

        let single_digit = bytes[i] - b'0';
        let double_digit = (bytes[i - 1] - b'0') * 10 + single_digit;

        if single_digit >= 1 {
            current += one_back;
        }
        if (10..=26).contains(&double_digit) {
            current += two_back;
        }

        // Advance the sliding window of state
        two_back = one_back;
        one_back = current;
    }

    one_back
}

/// Main entry point - uses the optimal space O(1) DP approach.
#[must_use]
pub fn num_decodings(s: String) -> i32 {
    num_decodings_optimized(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_happy_path() {
        assert_eq!(num_decodings_straightforward("12".to_string()), 2);
        assert_eq!(num_decodings_optimized("12".to_string()), 2);

        assert_eq!(num_decodings_straightforward("226".to_string()), 3);
        assert_eq!(num_decodings_optimized("226".to_string()), 3);
    }

    #[test]
    fn test_edge_cases() {
        // Leading zero
        assert_eq!(num_decodings_straightforward("06".to_string()), 0);
        assert_eq!(num_decodings_optimized("06".to_string()), 0);

        // Zero in the middle, invalid encoding
        assert_eq!(num_decodings_straightforward("30".to_string()), 0);
        assert_eq!(num_decodings_optimized("30".to_string()), 0);

        // Zero in the middle, valid encoding
        assert_eq!(num_decodings_straightforward("20".to_string()), 1);
        assert_eq!(num_decodings_optimized("20".to_string()), 1);

        // Only zero
        assert_eq!(num_decodings_straightforward("0".to_string()), 0);
        assert_eq!(num_decodings_optimized("0".to_string()), 0);

        // Single valid digit
        assert_eq!(num_decodings_straightforward("1".to_string()), 1);
        assert_eq!(num_decodings_optimized("1".to_string()), 1);
    }

    #[test]
    fn test_stress_boundary() {
        // All 1s, which branches a lot (Fibonacci-like growth)
        // 111111111111111
        // Length 15, expected is Fib(16) = 987
        let ones = "1".repeat(15);
        assert_eq!(num_decodings_straightforward(ones.clone()), 987);
        assert_eq!(num_decodings_optimized(ones), 987);

        // Max possible constraint sequence length is 100
        let twos = "2".repeat(45);
        assert_eq!(num_decodings_straightforward(twos.clone()), 1_836_311_903);
        assert_eq!(num_decodings_optimized(twos), 1_836_311_903);
    }
}
