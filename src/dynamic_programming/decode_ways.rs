//! # 91. Decode Ways
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/decode-ways/>
//!
//! A message containing letters from A-Z can be encoded into numbers using the following mapping:
//! 'A' -> "1", 'B' -> "2", ..., 'Z' -> "26".
//!
//! Given a string `s` containing only digits, return the number of ways to decode it.
//!
//! This problem perfectly demonstrates 1D dynamic programming. It highlights using zero-cost abstractions
//! like `.as_bytes()` for ASCII string slicing and range bounds matching (`(1..=9).contains()`) instead of
//! expensive substring parsing and allocations.
//!
//! ## Approach

Unlike C++ or Java where substring parsing often creates new string allocations, Rust allows us to operate on a zero-cost byte slice `&[u8]`. This guarantees we do not allocate memory while traversing the string.
//!
//! We present two dynamic programming approaches:
//! 1. **Straightforward 1D DP**: Uses an array `dp` of size `N+1` where `dp[i]` represents the number
//!    of ways to decode the prefix of length `i`. Time: O(N), Space: O(N).
//! 2. **Optimal O(1) Space DP**: Notice that `dp[i]` only depends on `dp[i-1]` and `dp[i-2]`. We can
//!    optimize the space complexity to O(1) by only keeping track of the last two values. Time: O(N), Space: O(1).
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::dynamic_programming::decode_ways::{num_decodings, num_decodings_optimal};
//!
//! assert_eq!(num_decodings("12".to_string()), 2); // "AB" or "L"
//! assert_eq!(num_decodings_optimal("226".to_string()), 3); // "BZ", "VF", or "BBF"
//! ```

/// Straightforward 1D dynamic programming approach.
///
/// Time: O(N)
/// Space: O(N)
#[must_use]
pub fn num_decodings_straightforward(s: String) -> i32 {
    if s.is_empty() {
        return 0;
    }

    // RUST INSIGHT: `.as_bytes()` is a zero-cost abstraction for ASCII strings.
    // It gives us a `&[u8]` slice, allowing O(1) indexing instead of O(N) `.chars().nth()`.
    let bytes = s.as_bytes();
    let n = bytes.len();

    if bytes[0] == b'0' {
        return 0;
    }

    let mut dp = vec![0; n + 1];
    dp[0] = 1;
    dp[1] = 1;

    for i in 2..=n {
        // Single digit decode
        if bytes[i - 1] != b'0' {
            dp[i] += dp[i - 1];
        }

        // Two digit decode
        // GOTCHA: We must calculate the value manually or parse.
        // Doing `(bytes[i-2] - b'0') * 10 + (bytes[i-1] - b'0')` is blazingly fast.
        let two_digit = (bytes[i - 2] - b'0') * 10 + (bytes[i - 1] - b'0');
        if (10..=26).contains(&two_digit) {
            dp[i] += dp[i - 2];
        }
    }

    dp[n]
}

/// Optimal approach using O(1) space.
///
/// Time: O(N)
/// Space: O(1)
#[must_use]
pub fn num_decodings_optimal(s: String) -> i32 {
    if s.is_empty() {
        return 0;
    }

    let bytes = s.as_bytes();
    let n = bytes.len();

    if bytes[0] == b'0' {
        return 0;
    }

    let mut prev_prev = 1;
    let mut prev = 1;

    for i in 1..n {
        let mut curr = 0;

        if bytes[i] != b'0' {
            curr += prev;
        }

        let two_digit = (bytes[i - 1] - b'0') * 10 + (bytes[i] - b'0');
        if (10..=26).contains(&two_digit) {
            curr += prev_prev;
        }

        prev_prev = prev;
        prev = curr;
    }

    prev
}

/// Main entry point - uses optimal solution
#[must_use]
pub fn num_decodings(s: String) -> i32 {
    num_decodings_optimal(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_happy_path() {
        assert_eq!(num_decodings_straightforward("12".to_string()), 2);
        assert_eq!(num_decodings_straightforward("226".to_string()), 3);
        assert_eq!(num_decodings_optimal("12".to_string()), 2);
        assert_eq!(num_decodings_optimal("226".to_string()), 3);
    }

    #[test]
    fn test_edge_cases() {
        assert_eq!(num_decodings("06".to_string()), 0);
        assert_eq!(num_decodings("0".to_string()), 0);
        assert_eq!(num_decodings("10".to_string()), 1);
        assert_eq!(num_decodings("2101".to_string()), 1);
    }

    #[test]
    fn test_stress_boundary_case() {
        let long_string = "1".repeat(40);
        // The number of decodings for "1" repeated N times follows the Fibonacci sequence.
        // fib(40) = 165580141 (using standard fib where fib(1)=1, fib(2)=2)
        assert_eq!(num_decodings(long_string.clone()), 165580141);
        assert_eq!(num_decodings_straightforward(long_string), 165580141);
    }
}

// Alternative Approaches:
// 1. Recursive with Memoization: This is the natural way to translate the problem definition
//    into code. It's conceptually simpler but requires O(N) space for the recursion stack
//    and a HashMap or Vec for memoization. The iterative DP approach we used avoids recursion overhead.
