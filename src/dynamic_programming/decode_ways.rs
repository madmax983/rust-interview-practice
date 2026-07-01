//! # Decode Ways (LeetCode 91)
//! Medium
//! <https://leetcode.com/problems/decode-ways/>
//!
//! **Why this matters in Rust:** This problem showcases how to efficiently process ASCII strings.
//! Instead of using expensive `.chars()` which handles full UTF-8 decoding, we can use
//! `.as_bytes()` to work directly on the underlying `u8` slice safely. It also demonstrates
//! space optimization techniques in dynamic programming.
//!
//! ## Approach
//!
//! The problem can be solved using Dynamic Programming. Let `dp[i]` be the number of ways
//! to decode the string up to index `i`.
//!
//! - If the current digit is not '0', it can be decoded as a single digit (A-I). Thus, `dp[i] += dp[i-1]`.
//! - If the previous digit and the current digit form a valid number between 10 and 26, it can be decoded as a two-digit number (J-Z). Thus, `dp[i] += dp[i-2]`.
//!
//! **Straightforward Approach:** Uses an array of size `N + 1` to store intermediate results, leading to O(N) space.
//!
//! **Optimized Approach:** Since `dp[i]` only depends on `dp[i-1]` and `dp[i-2]`, we only need two variables to keep track of previous states, reducing space complexity to O(1).
//!
//! **Complexity:**
//! - **Time:** O(N) where N is the length of the string `s`, as we iterate through it once.
//! - **Space:** O(N) for the straightforward approach, O(1) for the optimized approach.
//!
//! **Idiomatic Rust:**
//! In Python/Java/C++, string manipulation often involves creating substrings or using inefficient character access.
//! In Rust, `s.as_bytes()` provides a zero-cost, safe, contiguous slice of `u8`. Since the input string is guaranteed
//! to only contain ASCII digits ('0'-'9'), bytes are perfectly equivalent to characters here. We utilize
//! byte slice patterns and avoid allocations entirely.

/// Straightforward Approach: O(N) Space
/// Uses a vector to store the number of ways to decode prefixes of the string.
#[must_use]
pub fn num_decodings_straightforward(s: String) -> i32 {
    if s.is_empty() {
        return 0;
    }

    // GOTCHA: `s.chars().nth(i)` is O(N), leading to O(N^2) total time if used in a loop.
    // Instead, because we know `s` contains only ASCII digits, we can safely and
    // efficiently work with the raw bytes.
    let bytes = s.as_bytes();

    if bytes[0] == b'0' {
        return 0;
    }

    let n = bytes.len();
    let mut dp = vec![0; n + 1];

    // Base cases
    dp[0] = 1; // Empty string has 1 valid decoding (doing nothing)
    dp[1] = 1; // First character (already checked it's not '0')

    for i in 2..=n {
        // Single digit decode
        if bytes[i - 1] != b'0' {
            dp[i] += dp[i - 1];
        }

        // Two digit decode
        // RUST INSIGHT: We don't need to parse strings to integers. We can just check the byte values.
        let two_digit = (bytes[i - 2] - b'0') * 10 + (bytes[i - 1] - b'0');
        if (10..=26).contains(&two_digit) {
            dp[i] += dp[i - 2];
        }
    }

    dp[n]
}

/// Optimized Approach: O(1) Space
/// Uses only two variables to maintain the last two states, reducing memory overhead.
#[must_use]
pub fn num_decodings_optimized(s: String) -> i32 {
    let bytes = s.as_bytes();

    // RUST INSIGHT: The compiler knows `bytes` length is >= 1 if it's not empty,
    // but pattern matching or explicit checks prevent panics on index 0.
    if bytes.is_empty() || bytes[0] == b'0' {
        return 0;
    }

    let mut prev2 = 1; // dp[i-2]
    let mut prev1 = 1; // dp[i-1]

    for i in 1..bytes.len() {
        let mut current = 0;

        // Check if single digit is valid
        if bytes[i] != b'0' {
            current += prev1;
        }

        // Check if two digits form a valid number between 10 and 26
        // RUST INSIGHT: `bytes[i-1]` and `bytes[i]` are just `u8`. We do simple arithmetic.
        let two_digit = (bytes[i - 1] - b'0') * 10 + (bytes[i] - b'0');
        if (10..=26).contains(&two_digit) {
            current += prev2;
        }

        // Advance the state for the next iteration
        prev2 = prev1;
        prev1 = current;
    }

    prev1
}

/// Main entry point - uses optimal solution.
#[must_use]
pub fn num_decodings(s: String) -> i32 {
    num_decodings_optimized(s)
}

// =========================================================================================
// Alternative Approaches
// =========================================================================================
// 1. Recursive with Memoization: Top-down approach. Excellent for intuition, but involves
//    function call overhead and uses O(N) space for the recursion stack and memoization map.
// 2. Iterative with `fold`: One could write the state transition as an iterator `fold` over
//    `bytes.windows(2)`. While very functional and idiomatic, it can sometimes be less
//    readable for standard dynamic programming transitions than a simple loop.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_happy_path() {
        // "12" -> "AB" (1 2), "L" (12)
        assert_eq!(num_decodings("12".to_string()), 2);
        assert_eq!(num_decodings_straightforward("12".to_string()), 2);

        // "226" -> "BZ" (2 26), "VF" (22 6), "BBF" (2 2 6)
        assert_eq!(num_decodings("226".to_string()), 3);
        assert_eq!(num_decodings_straightforward("226".to_string()), 3);
    }

    #[test]
    fn test_edge_cases() {
        // Leading zero
        assert_eq!(num_decodings("06".to_string()), 0);
        assert_eq!(num_decodings_straightforward("06".to_string()), 0);

        // Zero in the middle
        assert_eq!(num_decodings("10".to_string()), 1); // "J"
        assert_eq!(num_decodings("2101".to_string()), 1); // "U A"
        assert_eq!(num_decodings("30".to_string()), 0); // Invalid

        // Empty string
        assert_eq!(num_decodings("".to_string()), 0);
        assert_eq!(num_decodings_straightforward("".to_string()), 0);

        // Single digit
        assert_eq!(num_decodings("5".to_string()), 1);
        assert_eq!(num_decodings("0".to_string()), 0);
    }

    #[test]
    fn test_stress_boundary() {
        // Long valid sequence of 1s (Fibonacci sequence behavior)
        let s = "111111111111111111111111111111111111111111111".to_string();
        let res1 = num_decodings(s.clone());
        let res2 = num_decodings_straightforward(s);
        assert_eq!(res1, res2);
        assert!(res1 > 1000); // Should be very large
    }
}
