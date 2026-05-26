//! # 91. Decode Ways
//!
//! Difficulty: Medium
//! Link: https://leetcode.com/problems/decode-ways/
//!
//! A message containing letters from A-Z can be encoded into numbers using the following mapping:
//! 'A' -> "1", 'B' -> "2", ..., 'Z' -> "26".
//! Given a string `s` containing only digits, return the number of ways to decode it.
//!
//! This problem perfectly demonstrates zero-cost abstractions in Rust. We can use `.as_bytes()`
//! for safe, zero-allocation ASCII slicing, and range bounds matching `(b'1'..=b'9').contains(&val)`
//! instead of expensive substring parsing or regex.
//!
//! ## Approach
//!
//! This is a classic 1D Dynamic Programming problem. Let `dp[i]` be the number of ways to decode
//! the prefix of length `i`. The transition is:
//! - If the current single digit is valid (1-9), add `dp[i-1]`.
//! - If the current and previous digits form a valid two-digit number (10-26), add `dp[i-2]`.
//!
//! In Python or Java, this might involve string slicing like `int(s[i-1:i+1])`. In Rust, we work
//! directly with the byte array slice. Since the state only depends on the previous two values
//! (`dp[i-1]` and `dp[i-2]`), we can optimize the space from O(N) to O(1) by keeping track of
//! just the last two counts.
//!
//! ## Time/Space Complexity
//!
//! - **Time:** O(N) where N is the length of the string. We iterate through the string once.
//! - **Space:** O(1) for the optimized approach, as we only store two integers regardless of string length.
//!
//! ## Alternative Approaches
//!
//! - **Top-Down DFS with Memoization:** Good for understanding the recursive tree, but uses O(N)
//!   call stack space and requires a `HashMap` or a `Vec` for memoization.
//! - **O(N) DP Array:** Stores the result for every index. Easier to reason about and trace
//!   during an interview, but uses extra memory unnecessarily.

/// Straightforward approach: O(N) Space
///
/// Uses an explicit DP array to track the number of ways to decode up to each index.
#[must_use]
pub fn num_decodings_straightforward(s: &str) -> i32 {
    let bytes = s.as_bytes();
    let n = bytes.len();

    // GOTCHA: An empty string or a string starting with '0' cannot be decoded.
    if n == 0 || bytes[0] == b'0' {
        return 0;
    }

    let mut dp = vec![0; n + 1];
    dp[0] = 1;
    dp[1] = 1;

    // RUST INSIGHT: Iterating directly with indices allows us to look back at `i-1` and `i-2`.
    // The compiler checks bounds, but our logic guarantees we won't panic.
    for i in 2..=n {
        let single_digit = bytes[i - 1];
        let double_digit = (bytes[i - 2] - b'0') * 10 + (bytes[i - 1] - b'0');

        // Single digit decoding
        if (b'1'..=b'9').contains(&single_digit) {
            dp[i] += dp[i - 1];
        }

        // Double digit decoding
        if (10..=26).contains(&double_digit) {
            dp[i] += dp[i - 2];
        }
    }

    dp[n]
}

/// Optimized approach: O(1) Space
///
/// Instead of an entire array, we only keep track of the two previous states.
#[must_use]
pub fn num_decodings_optimized(s: &str) -> i32 {
    let bytes = s.as_bytes();

    if bytes.is_empty() || bytes[0] == b'0' {
        return 0;
    }

    // dp_two_back corresponds to dp[i-2]
    // dp_one_back corresponds to dp[i-1]
    let mut dp_two_back = 1;
    let mut dp_one_back = 1;

    for i in 1..bytes.len() {
        let mut current_dp = 0;

        let single_digit = bytes[i];
        // RUST INSIGHT: No string parsing needed. Simple arithmetic on ASCII bytes is fast.
        let double_digit = (bytes[i - 1] - b'0') * 10 + (bytes[i] - b'0');

        if (b'1'..=b'9').contains(&single_digit) {
            current_dp += dp_one_back;
        }

        if (10..=26).contains(&double_digit) {
            current_dp += dp_two_back;
        }

        // Shift states
        dp_two_back = dp_one_back;
        dp_one_back = current_dp;
    }

    dp_one_back
}

/// Main entry point
#[must_use]
pub fn num_decodings(s: &str) -> i32 {
    num_decodings_optimized(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_happy_path() {
        assert_eq!(num_decodings("12"), 2); // "AB", "L"
        assert_eq!(num_decodings("226"), 3); // "BZ", "VF", "BBF"
        assert_eq!(num_decodings("11106"), 2); // "AAJF", "KJF"
    }

    #[test]
    fn test_edge_cases() {
        // Starts with zero
        assert_eq!(num_decodings("0"), 0);
        assert_eq!(num_decodings("06"), 0);

        // Invalid double digits
        assert_eq!(num_decodings("30"), 0);

        // Empty string
        assert_eq!(num_decodings(""), 0);

        // Single valid digit
        assert_eq!(num_decodings("5"), 1);
    }

    #[test]
    fn test_boundary_stress() {
        // A long string of ones: each index is the next Fibonacci number
        let s = "1".repeat(20);
        // Fib(20+1) for dp logic:
        // dp[0]=1, dp[1]=1, dp[2]=2, dp[3]=3, dp[4]=5, dp[5]=8, dp[6]=13, dp[7]=21, dp[8]=34,
        // dp[9]=55, dp[10]=89, dp[11]=144, dp[12]=233, dp[13]=377, dp[14]=610, dp[15]=987,
        // dp[16]=1597, dp[17]=2584, dp[18]=4181, dp[19]=6765, dp[20]=10946
        assert_eq!(num_decodings(&s), 10946);

        // Ensure straightforward approach works identically
        assert_eq!(num_decodings_straightforward("226"), 3);
        assert_eq!(num_decodings_straightforward("06"), 0);
        assert_eq!(num_decodings_straightforward(&s), 10946);
    }
}
