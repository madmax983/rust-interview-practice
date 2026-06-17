//! # 91. Decode Ways
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/decode-ways/>
//!
//! A message containing letters from A-Z can be encoded into numbers using the following mapping:
//! 'A' -> "1", 'B' -> "2", ..., 'Z' -> "26"
//!
//! To decode an encoded message, all the digits must be grouped then mapped back into letters
//! using the reverse of the mapping above (there may be multiple ways).
//!
//! Given a string `s` containing only digits, return the number of ways to decode it.
//!
//! This problem is a classic 1D dynamic programming challenge (similar to climbing stairs but with conditions).
//! It demonstrates 1D dynamic programming with an O(1) space optimization and highlights using Rust's
//! zero-cost abstractions like `.as_bytes()` for ASCII string slicing and range bounds matching
//! (`(1..=9).contains()`) instead of expensive substring parsing.
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

/// Brute Force / Standard DP approach
///
/// Time: O(N) - We iterate through the string of length N once.
/// Space: O(N) - We use an array `dp` of length N+1.
///
/// `dp[i]` represents the number of ways to decode the prefix of length `i`.
/// We can transition from `dp[i-1]` (if the current character is '1'-'9')
/// and from `dp[i-2]` (if the last two characters form '10'-'26').
#[must_use]
pub fn num_decodings_brute_force(s: String) -> i32 {
    let bytes = s.as_bytes(); // RUST INSIGHT: O(1) zero-cost abstraction for ASCII.
    let n = bytes.len();
    if n == 0 || bytes[0] == b'0' {
        return 0;
    }

    let mut dp = vec![0; n + 1];
    dp[0] = 1; // Base case for empty string
    dp[1] = 1; // Base case for first character (we already checked it's not '0')

    for i in 2..=n {
        let single_digit = bytes[i - 1] - b'0';
        let double_digit = (bytes[i - 2] - b'0') * 10 + single_digit;

        // If the single digit is valid (1-9), we can append it to all decodings of length i-1
        if (1..=9).contains(&single_digit) {
            dp[i] += dp[i - 1];
        }

        // If the double digit is valid (10-26), we can append it to all decodings of length i-2
        if (10..=26).contains(&double_digit) {
            dp[i] += dp[i - 2];
        }
    }

    dp[n]
}

/// Optimized approach: O(1) Space DP
///
/// Time: O(N) - We iterate through the string once.
/// Space: O(1) - We only store the last two DP states.
///
/// Since `dp[i]` only depends on `dp[i-1]` and `dp[i-2]`, we don't need the full array.
/// We can just maintain two variables `two_back` (dp[i-2]) and `one_back` (dp[i-1]).
#[must_use]
pub fn num_decodings_optimized(s: String) -> i32 {
    let bytes = s.as_bytes();
    let n = bytes.len();
    if n == 0 || bytes[0] == b'0' {
        return 0;
    }

    // `two_back` corresponds to dp[i-2], `one_back` to dp[i-1]
    let mut two_back = 1;
    let mut one_back = 1;

    for i in 2..=n {
        let mut current = 0;
        let single_digit = bytes[i - 1] - b'0';
        let double_digit = (bytes[i - 2] - b'0') * 10 + single_digit;

        if (1..=9).contains(&single_digit) {
            current += one_back;
        }

        if (10..=26).contains(&double_digit) {
            current += two_back;
        }

        // Advance the sliding window
        two_back = one_back;
        one_back = current;
    }

    one_back
}

/// Main entry point
#[must_use]
pub fn num_decodings(s: String) -> i32 {
    num_decodings_optimized(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_happy_path() {
        assert_eq!(num_decodings_brute_force("12".to_string()), 2);
        assert_eq!(num_decodings_optimized("12".to_string()), 2);

        assert_eq!(num_decodings_brute_force("226".to_string()), 3);
        assert_eq!(num_decodings_optimized("226".to_string()), 3);
    }

    #[test]
    fn test_edge_case_leading_zero() {
        assert_eq!(num_decodings_brute_force("06".to_string()), 0);
        assert_eq!(num_decodings_optimized("06".to_string()), 0);
    }

    #[test]
    fn test_edge_case_embedded_zero() {
        // "10" is valid -> 1 (J)
        assert_eq!(num_decodings_brute_force("10".to_string()), 1);
        assert_eq!(num_decodings_optimized("10".to_string()), 1);

        // "2101" -> 1 (B, J, A) or (U, A) -> wait, "2101":
        // 2, 10, 1 -> B, J, A (1 way)
        // 21, 01 (invalid) -> 0
        // So 1 way.
        assert_eq!(num_decodings_brute_force("2101".to_string()), 1);
        assert_eq!(num_decodings_optimized("2101".to_string()), 1);

        // "30" is invalid
        assert_eq!(num_decodings_brute_force("30".to_string()), 0);
        assert_eq!(num_decodings_optimized("30".to_string()), 0);
    }

    #[test]
    fn test_stress_all_valid() {
        // "11111" -> dp is Fibonacci sequence
        // dp[0]=1, dp[1]=1, dp[2]=2, dp[3]=3, dp[4]=5, dp[5]=8
        assert_eq!(num_decodings_brute_force("11111".to_string()), 8);
        assert_eq!(num_decodings_optimized("11111".to_string()), 8);
    }
}
