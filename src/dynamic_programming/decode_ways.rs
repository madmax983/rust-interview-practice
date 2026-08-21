//! # 91. Decode Ways
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/decode-ways/>
//!
//! A message containing letters from A-Z can be encoded into numbers using the following mapping:
//! 'A' -> "1", 'B' -> "2", ..., 'Z' -> "26".
//! Given a string `s` containing only digits, return the number of ways to decode it.
//!
//! This problem perfectly demonstrates dynamic programming space optimization (from O(N) to O(1) space).
//! It also highlights the performance benefits of zero-cost, safe byte-level string access in Rust
//! using `.as_bytes()` instead of UTF-8 iterator adapters (`.chars()`) when dealing with guaranteed ASCII digit strings.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::dynamic_programming::decode_ways::num_decodings;
//!
//! assert_eq!(num_decodings("12".to_string()), 2); // "AB" (1 2) or "L" (12)
//! assert_eq!(num_decodings("226".to_string()), 3); // "BZ" (2 26), "VF" (22 6), or "BBF" (2 2 6)
//! assert_eq!(num_decodings("06".to_string()), 0); // "06" cannot be mapped
//! ```
//!
//! ## Constraints
//!
//! - `1 <= s.length <= 100`
//! - `s` contains only digits and may contain leading zero(s).
//! - The answer is guaranteed to fit in a 32-bit integer.

/// Brute Force approach: Memoization with DP Array
///
/// Time: O(N) - We visit each character of the string a constant number of times.
/// Space: O(N) - We allocate a `Vec` to store the intermediate DP states.
///
/// We define `dp[i]` as the number of ways to decode the substring of length `i` (or up to index `i`).
/// At each step, we can either take a single digit (if valid) or two digits (if valid).
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn num_decodings_dp_array(s: String) -> i32 {
    if s.is_empty() {
        return 0;
    }

    // RUST INSIGHT: Since the problem guarantees `s` contains only ASCII digits,
    // we can use `.as_bytes()` for O(1) indexing instead of `.chars().nth(i)` which is O(N).
    // GOTCHA: UTF-8 Strings cannot be indexed with `s[i]` directly in Rust to prevent panics
    // on multi-byte characters. `.as_bytes()` explicitly returns a `&[u8]`.
    let bytes = s.as_bytes();

    if bytes[0] == b'0' {
        return 0; // A string starting with '0' cannot be decoded.
    }

    let n = bytes.len();
    let mut dp = vec![0; n + 1];

    // Base cases
    dp[0] = 1; // Empty string has 1 valid decoding (the base of the DP)
    dp[1] = 1; // 1 char string has 1 valid decoding if it's not '0' (handled above)

    for i in 2..=n {
        // Single digit decode
        let single_digit = bytes[i - 1] - b'0';
        if single_digit >= 1 {
            dp[i] += dp[i - 1];
        }

        // Two digits decode
        let two_digits = (bytes[i - 2] - b'0') * 10 + single_digit;
        if (10..=26).contains(&two_digits) {
            dp[i] += dp[i - 2];
        }
    }

    dp[n]
}

/// Optimal approach: Space-Optimized Dynamic Programming
///
/// Time: O(N) - Exactly N iterations.
/// Space: O(1) - We only keep track of the last two calculated values.
///
/// Because `dp[i]` only depends on `dp[i-1]` and `dp[i-2]`, we don't need to store
/// the entire array. We can just keep two variables and shift them forward, drastically
/// improving memory efficiency and cache locality.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn num_decodings_optimal(s: String) -> i32 {
    let bytes = s.as_bytes();

    if bytes.is_empty() || bytes[0] == b'0' {
        return 0;
    }

    // `two_back` corresponds to dp[i-2]
    // `one_back` corresponds to dp[i-1]
    let mut two_back = 1;
    let mut one_back = 1;

    for i in 1..bytes.len() {
        let mut current = 0;

        // Check single digit
        if bytes[i] != b'0' {
            current += one_back;
        }

        // Check two digits
        let two_digits = (bytes[i - 1] - b'0') * 10 + (bytes[i] - b'0');
        if (10..=26).contains(&two_digits) {
            current += two_back;
        }

        // Shift pointers for next iteration
        two_back = one_back;
        one_back = current;
    }

    one_back
}

/// Main entry point
#[must_use]
pub fn num_decodings(s: String) -> i32 {
    num_decodings_optimal(s)
}

// Alternative Approaches:
// 1. Recursive with Memoization: Top-down approach using a HashMap or Vec to cache results.
//    While logically identical to the DP array approach, it incurs function call overhead
//    and risks stack overflows for extremely large strings (though within these constraints it is fine).

#[cfg(test)]
mod tests {
    use super::*;

    // Happy Path
    #[test]
    fn test_valid_strings() {
        assert_eq!(num_decodings("12".to_string()), 2);
        assert_eq!(num_decodings("226".to_string()), 3);

        assert_eq!(num_decodings_dp_array("12".to_string()), 2);
        assert_eq!(num_decodings_dp_array("226".to_string()), 3);
    }

    // Edge Cases
    #[test]
    fn test_leading_zeros() {
        assert_eq!(num_decodings("06".to_string()), 0);
        assert_eq!(num_decodings("0".to_string()), 0);
        assert_eq!(num_decodings("00".to_string()), 0);

        assert_eq!(num_decodings_dp_array("06".to_string()), 0);
        assert_eq!(num_decodings_dp_array("0".to_string()), 0);
    }

    #[test]
    fn test_zeros_in_middle() {
        assert_eq!(num_decodings("10".to_string()), 1); // Only "10"
        assert_eq!(num_decodings("2101".to_string()), 1); // "2", "10", "1"
        assert_eq!(num_decodings("30".to_string()), 0); // Invalid, no "30" mapped

        assert_eq!(num_decodings_dp_array("10".to_string()), 1);
        assert_eq!(num_decodings_dp_array("30".to_string()), 0);
    }

    // Boundary Cases
    #[test]
    fn test_single_characters() {
        assert_eq!(num_decodings("1".to_string()), 1);
        assert_eq!(num_decodings("9".to_string()), 1);

        assert_eq!(num_decodings_dp_array("1".to_string()), 1);
    }
}
