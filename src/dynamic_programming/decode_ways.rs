//! # 91. Decode Ways
//!
//! Difficulty: Medium
//! Link: https://leetcode.com/problems/decode-ways/
//!
//! Why this matters in Rust: This problem demonstrates dynamic programming space
//! optimization (from O(N) to O(1) space) and highlights the performance benefits
//! of zero-cost, safe byte-level string access using `.as_bytes()` instead of
//! UTF-8 iterator adapters (`.chars()`) when dealing with guaranteed ASCII string
//! parsing.
//!
//! ## Approach
//!
//! A message containing letters from A-Z is being encoded to numbers from 1-26.
//! We need to find the total number of ways to decode a given string of digits.
//!
//! This is a classic dynamic programming problem. At each step `i`, we can either:
//! 1. Decode the single digit at `i` (if it's not '0'). The number of ways is `dp[i-1]`.
//! 2. Decode the two digits at `i-1` and `i` (if they form a valid number between 10 and 26).
//!    The number of ways is `dp[i-2]`.
//!
//! The total number of ways at `i` is the sum of these two possibilities.
//!
//! Two solutions are provided:
//! 1. `num_decodings_dp`: Uses an O(N) auxiliary array to store the number of ways at each step.
//! 2. `num_decodings_optimal`: Optimizes space to O(1) by only keeping track of the last two states (`dp[i-1]` and `dp[i-2]`), similar to the Fibonacci sequence calculation.
//!
//! Time Complexity: O(N) for both, where N is the length of the string.
//! Space Complexity: O(N) for `num_decodings_dp`, O(1) for `num_decodings_optimal`.
//!
//! ## Alternative Approaches
//!
//! * Recursion with memoization is another valid approach, but it incurs function call overhead
//!   and can potentially hit stack limits, making iterative DP preferred in Rust.

/// Straightforward O(N) space dynamic programming approach
pub fn num_decodings_dp(s: String) -> i32 {
    // GOTCHA: `.chars().nth(i)` is O(N), making a loop O(N^2).
    // Since the string is guaranteed to be ASCII digits, `.as_bytes()` provides O(1) indexing.
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

        if single_digit >= 1 && single_digit <= 9 {
            dp[i] += dp[i - 1];
        }

        if double_digit >= 10 && double_digit <= 26 {
            dp[i] += dp[i - 2];
        }
    }

    dp[n]
}

/// Optimized O(1) space dynamic programming approach
pub fn num_decodings_optimal(s: String) -> i32 {
    let bytes = s.as_bytes();
    if bytes.is_empty() || bytes[0] == b'0' {
        return 0;
    }

    // RUST INSIGHT: We only need the last two values, avoiding heap allocation entirely.
    let mut prev2 = 1; // dp[i-2]
    let mut prev1 = 1; // dp[i-1]

    for i in 1..bytes.len() {
        let mut current = 0;
        let single_digit = bytes[i] - b'0';
        let double_digit = (bytes[i - 1] - b'0') * 10 + single_digit;

        if single_digit >= 1 && single_digit <= 9 {
            current += prev1;
        }

        if double_digit >= 10 && double_digit <= 26 {
            current += prev2;
        }

        prev2 = prev1;
        prev1 = current;
    }

    prev1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_num_decodings_happy_path() {
        assert_eq!(num_decodings_dp("12".to_string()), 2); // "AB" or "L"
        assert_eq!(num_decodings_optimal("12".to_string()), 2);

        assert_eq!(num_decodings_dp("226".to_string()), 3); // "BZ", "VF", "BBF"
        assert_eq!(num_decodings_optimal("226".to_string()), 3);
    }

    #[test]
    fn test_num_decodings_edge_cases() {
        // Leading zero
        assert_eq!(num_decodings_dp("06".to_string()), 0);
        assert_eq!(num_decodings_optimal("06".to_string()), 0);

        // Zero in the middle
        assert_eq!(num_decodings_dp("2101".to_string()), 1);
        assert_eq!(num_decodings_optimal("2101".to_string()), 1);

        // Invalid zero in the middle
        assert_eq!(num_decodings_dp("230".to_string()), 0);
        assert_eq!(num_decodings_optimal("230".to_string()), 0);
    }

    #[test]
    fn test_num_decodings_stress() {
        // Long string of 1s and 2s (Fibonacci-like growth)
        let s = "11111111111111111111".to_string(); // 20 chars
        assert_eq!(num_decodings_dp(s.clone()), 10946);
        assert_eq!(num_decodings_optimal(s), 10946);
    }
}
