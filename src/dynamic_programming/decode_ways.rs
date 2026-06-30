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
//! This problem highlights dynamic programming space optimization (from O(N) to O(1) space) and
//! demonstrates the performance benefits of zero-cost, safe byte-level string access using `.as_bytes()`
//! instead of UTF-8 iterator adapters (`.chars()`) when dealing with guaranteed ASCII digit strings.
//!
//! ## Approaches
//!
//! 1.  **Dynamic Programming - O(N) Space**:
//!     -   Use an array `dp` where `dp[i]` represents the number of ways to decode the substring `s[0..i]`.
//!     -   Time: O(N) where N is the length of the string.
//!     -   Space: O(N) for the DP array.
//! 2.  **Dynamic Programming - O(1) Space (Optimized)**:
//!     -   Notice that `dp[i]` only depends on `dp[i-1]` and `dp[i-2]`. We can optimize the space to O(1) by only keeping track of the last two DP states.
//!     -   Time: O(N).
//!     -   Space: O(1).
//!
//! ## Alternative Approaches
//!
//! - **Recursive with Memoization**: A top-down approach using a memoization table. Time O(N), Space O(N) due to recursion stack and memo table.

// =========================================================================================
// Approach 1: Dynamic Programming - O(N) Space
// =========================================================================================

/// Time: O(N)
/// Space: O(N)
#[must_use]
pub fn num_decodings_dp_on_space(s: String) -> i32 {
    if s.is_empty() {
        return 0;
    }

    // RUST INSIGHT: Since the problem guarantees the input string only contains digits (ASCII),
    // we can use `.as_bytes()` for zero-cost, O(1) indexable byte-level access.
    // GOTCHA: If the string could contain multi-byte UTF-8 characters, indexing bytes would be unsafe
    // and we would need to use `.chars()` which is O(N) for random access.
    let bytes = s.as_bytes();
    let n = bytes.len();

    // DP array where dp[i] is the number of ways to decode s[0..i]
    let mut dp = vec![0; n + 1];
    dp[0] = 1; // Base case: 1 way to decode an empty string

    // Check first character
    dp[1] = if bytes[0] == b'0' { 0 } else { 1 };

    for i in 2..=n {
        // Single digit decode (if it's not '0')
        if bytes[i - 1] != b'0' {
            dp[i] += dp[i - 1];
        }

        // Two digit decode (if it's between "10" and "26")
        let two_digit = (bytes[i - 2] - b'0') * 10 + (bytes[i - 1] - b'0');
        if (10..=26).contains(&two_digit) {
            dp[i] += dp[i - 2];
        }
    }

    dp[n]
}

// =========================================================================================
// Approach 2: Dynamic Programming - O(1) Space (Optimal)
// =========================================================================================

/// Time: O(N)
/// Space: O(1)
#[must_use]
pub fn num_decodings_optimal(s: String) -> i32 {
    if s.is_empty() {
        return 0;
    }

    let bytes = s.as_bytes();

    // RUST INSIGHT: We handle the edge case of starting with '0' immediately.
    if bytes[0] == b'0' {
        return 0;
    }

    // We only need the last two states for the DP transition.
    // prev2 represents dp[i-2], prev1 represents dp[i-1]
    let mut prev2 = 1;
    let mut prev1 = 1;

    for i in 1..bytes.len() {
        let mut curr = 0;

        // Single digit decode
        if bytes[i] != b'0' {
            curr += prev1;
        }

        // Two digit decode
        let two_digit = (bytes[i - 1] - b'0') * 10 + (bytes[i] - b'0');
        if (10..=26).contains(&two_digit) {
            curr += prev2;
        }

        // Update states for next iteration
        prev2 = prev1;
        prev1 = curr;
    }

    prev1
}

/// Main entry point - uses optimal solution
#[must_use]
pub fn num_decodings(s: String) -> i32 {
    num_decodings_optimal(s)
}

// =========================================================================================
// Tests
// =========================================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_happy_path() {
        // "12" -> "AB" (1 2) or "L" (12)
        assert_eq!(num_decodings_dp_on_space("12".to_string()), 2);
        assert_eq!(num_decodings_optimal("12".to_string()), 2);
        assert_eq!(num_decodings("12".to_string()), 2);

        // "226" -> "BZ" (2 26), "VF" (22 6), or "BBF" (2 2 6)
        assert_eq!(num_decodings_dp_on_space("226".to_string()), 3);
        assert_eq!(num_decodings_optimal("226".to_string()), 3);
        assert_eq!(num_decodings("226".to_string()), 3);
    }

    #[test]
    fn test_edge_cases_with_zeros() {
        // "06" cannot be mapped to "F" because "06" is not a valid encoding
        assert_eq!(num_decodings_dp_on_space("06".to_string()), 0);
        assert_eq!(num_decodings_optimal("06".to_string()), 0);

        // "10" -> "J" (10)
        assert_eq!(num_decodings_dp_on_space("10".to_string()), 1);
        assert_eq!(num_decodings_optimal("10".to_string()), 1);

        // "2101" -> "U" (21), "A" (1) ... Wait, "10" must be decoded together.
        // "2 10 1" -> B, J, A -> 1 way.
        assert_eq!(num_decodings_dp_on_space("2101".to_string()), 1);
        assert_eq!(num_decodings_optimal("2101".to_string()), 1);

        // Invalid sequence with '0'
        assert_eq!(num_decodings_optimal("30".to_string()), 0);
    }

    #[test]
    fn test_boundary_long_string() {
        // "111111111111111111111111111111111111111111111"
        // This is a Fibonacci sequence of decodings.
        assert_eq!(num_decodings_optimal("1111".to_string()), 5);
        assert_eq!(num_decodings_optimal("11111".to_string()), 8);
        assert_eq!(num_decodings_optimal("111111".to_string()), 13);
    }
}
