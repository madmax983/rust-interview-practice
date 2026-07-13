//! # 91. Decode Ways
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/decode-ways/>
//!
//! A message containing letters from A-Z can be encoded into numbers using the following mapping:
//! 'A' -> "1", 'B' -> "2", ..., 'Z' -> "26".
//! Given a string `s` containing only digits, return the number of ways to decode it.
//!
//! This problem demonstrates dynamic programming with space optimization (from O(N) to O(1) space).
//! It also highlights the performance benefits of zero-cost, safe byte-level string access using
//! `.as_bytes()` instead of UTF-8 iterator adapters (`.chars()`) when dealing with guaranteed
//! ASCII digit strings.
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
//! - The answer is guaranteed to fit in a 32-bit integer.

/// Brute-force/Standard DP Approach (O(N) Space)
///
/// Time: O(N) where N is the length of the string. We iterate through the string once.
/// Space: O(N) to store the DP table.
///
/// This approach uses a `Vec` to store the number of ways to decode up to each index.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn num_decodings_brute_force(s: String) -> i32 {
    if s.is_empty() {
        return 0;
    }

    // RUST INSIGHT: Since the problem guarantees the input string consists only of ASCII digits,
    // we can safely and efficiently work with it as a byte slice `&[u8]` using `.as_bytes()`.
    // This provides O(1) random access by index, avoiding the O(N) overhead of `.chars().nth(i)`.
    let bytes = s.as_bytes();
    let n = bytes.len();

    if bytes[0] == b'0' {
        return 0;
    }

    // dp[i] represents the number of ways to decode the prefix of length i.
    let mut dp = vec![0; n + 1];
    dp[0] = 1; // Base case: 1 way to decode an empty string
    dp[1] = 1; // Base case: 1 way to decode the first char (since it's not '0')

    for i in 2..=n {
        // Single digit decode (if valid)
        if bytes[i - 1] != b'0' {
            dp[i] += dp[i - 1];
        }

        // Two digit decode (if valid, i.e., between "10" and "26")
        let two_digit = (bytes[i - 2] - b'0') * 10 + (bytes[i - 1] - b'0');
        if (10..=26).contains(&two_digit) {
            dp[i] += dp[i - 2];
        }
    }

    dp[n]
}

/// Optimized DP Approach (O(1) Space)
///
/// Time: O(N) where N is the length of the string.
/// Space: O(1) as we only keep track of the last two DP states.
///
/// Since `dp[i]` only depends on `dp[i-1]` and `dp[i-2]`, we don't need a full array.
/// We can optimize the space complexity by only keeping track of these two previous values.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn num_decodings_optimized(s: String) -> i32 {
    let bytes = s.as_bytes();

    // Quick exit for empty string or strings starting with '0'
    if bytes.is_empty() || bytes[0] == b'0' {
        return 0;
    }

    // Instead of dp[i-2] and dp[i-1], we use prev2 and prev1.
    let mut prev2 = 1; // Represents dp[i-2] (initially dp[0] = 1)
    let mut prev1 = 1; // Represents dp[i-1] (initially dp[1] = 1, since bytes[0] != '0')

    for i in 1..bytes.len() {
        let mut curr = 0;

        // Check if the single digit is valid
        if bytes[i] != b'0' {
            curr += prev1;
        }

        // Check if the two-digit number is valid
        // GOTCHA: Don't parse strings to integers in a loop if you can just do math on bytes.
        let two_digit = (bytes[i - 1] - b'0') * 10 + (bytes[i] - b'0');
        if (10..=26).contains(&two_digit) {
            curr += prev2;
        }

        // Shift the window forward
        prev2 = prev1;
        prev1 = curr;
    }

    prev1
}

/// Main entry point
#[must_use]
pub fn num_decodings(s: String) -> i32 {
    num_decodings_optimized(s)
}

// Alternative Approaches:
// 1. **Memoized Recursion**: A top-down DFS approach with memoization is also valid and has
//    the same O(N) time and O(N) space complexity as the brute-force DP. However, iterative
//    bottom-up DP is generally preferred in Rust to avoid stack overflow risks on deep recursions.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_happy_path() {
        // "12" -> "AB" or "L" -> 2 ways
        assert_eq!(num_decodings_brute_force("12".to_string()), 2);
        assert_eq!(num_decodings_optimized("12".to_string()), 2);

        // "226" -> "BZ", "VF", "BBF" -> 3 ways
        assert_eq!(num_decodings_brute_force("226".to_string()), 3);
        assert_eq!(num_decodings_optimized("226".to_string()), 3);
    }

    #[test]
    fn test_edge_cases() {
        // Leading zero
        assert_eq!(num_decodings_brute_force("06".to_string()), 0);
        assert_eq!(num_decodings_optimized("06".to_string()), 0);

        // Zeros in the middle that make it invalid
        assert_eq!(num_decodings_brute_force("100".to_string()), 0);
        assert_eq!(num_decodings_optimized("100".to_string()), 0);

        // Zeros in the middle that are valid
        assert_eq!(num_decodings_brute_force("2101".to_string()), 1);
        assert_eq!(num_decodings_optimized("2101".to_string()), 1);

        // Just one character
        assert_eq!(num_decodings_brute_force("1".to_string()), 1);
        assert_eq!(num_decodings_optimized("1".to_string()), 1);

        // Empty string
        assert_eq!(num_decodings_brute_force(String::new()), 0);
        assert_eq!(num_decodings_optimized(String::new()), 0);
    }

    #[test]
    fn test_stress() {
        // Long string with many combinations
        // "1111111111" (10 ones)
        assert_eq!(num_decodings_brute_force("1111111111".to_string()), 89); // Fibonacci(11)
        assert_eq!(num_decodings_optimized("1111111111".to_string()), 89);
    }
}
