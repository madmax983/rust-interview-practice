//! # 91. Decode Ways
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/decode-ways/>
//!
//! A message containing letters from `A-Z` can be encoded into numbers using the following mapping:
//! `A -> 1, B -> 2 ... Z -> 26`. Given a string `s` containing only digits, return the number of ways to decode it.
//!
//! This problem perfectly demonstrates Rust's zero-cost safe abstractions. By using `.as_bytes()`, we get immediate
//! O(1) random access to string characters without the overhead of UTF-8 decoding iterators (like `.chars()`), because
//! the problem guarantees the input string is purely ASCII digits.
//!
//! ## Approach
//!
//! We can solve this using dynamic programming.
//! 1. Brute Force recursion: Branch on taking 1 or 2 characters. (O(2^n) time)
//! 2. DP Array (O(n) space): Maintain a `dp` array where `dp[i]` is the number of ways to decode `s[..i]`.
//! 3. DP Optimized (O(1) space): Since `dp[i]` only depends on `dp[i-1]` and `dp[i-2]`, we can maintain just two variables (`two_back`, `one_back`).
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::dynamic_programming::decode_ways::num_decodings;
//!
//! assert_eq!(num_decodings("12".to_string()), 2); // "AB" (1 2) or "L" (12)
//! assert_eq!(num_decodings("226".to_string()), 3); // "BZ" (2 26), "VF" (22 6), or "BBF" (2 2 6)
//! assert_eq!(num_decodings("06".to_string()), 0); // "0" is invalid
//! ```

/// Decodes the number of ways a string of digits can be parsed into letters.
///
/// Time: O(n) - Single pass through the string
/// Space: O(1) - Maintaining only two variables for the state
#[must_use]
pub fn num_decodings(s: String) -> i32 {
    if s.is_empty() {
        return 0;
    }

    // GOTCHA: `.chars()` is an O(N) iterator that must scan for UTF-8 variable-length boundaries.
    // Since LeetCode guarantees `s` contains only digits '0'-'9', all characters are ASCII (1 byte).
    // Using `.as_bytes()` gives us a safe, zero-cost `&[u8]` slice with O(1) indexing!
    let bytes = s.as_bytes();

    if bytes[0] == b'0' {
        return 0;
    }

    // dp[i-2] represents ways to decode if we skipped the last 2 chars
    let mut two_back = 1;
    // dp[i-1] represents ways to decode up to the previous char
    let mut one_back = 1;

    // RUST INSIGHT: Notice how byte literals (`b'0'`, `b'1'`, `b'2'`, `b'6'`) integrate seamlessly with pattern matching and comparison operators.
    for i in 1..bytes.len() {
        let mut current = 0;

        // Single digit decode (if it's not '0')
        if bytes[i] != b'0' {
            current += one_back;
        }

        // Two digit decode (if it forms 10-26)
        if bytes[i - 1] == b'1' || (bytes[i - 1] == b'2' && bytes[i] <= b'6') {
            current += two_back;
        }

        two_back = one_back;
        one_back = current;
    }

    one_back
}

// Alternative Approaches:
// 1. **Memoized Recursion**: Top-down DP using a `HashMap` or `Vec` for memoization. Conceptually simpler but incurs recursion stack overhead and allocations.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_happy_path() {
        assert_eq!(num_decodings("12".to_string()), 2);
        assert_eq!(num_decodings("226".to_string()), 3);
    }

    #[test]
    fn test_edge_cases_with_zeros() {
        assert_eq!(num_decodings("0".to_string()), 0);
        assert_eq!(num_decodings("06".to_string()), 0);
        assert_eq!(num_decodings("10".to_string()), 1); // Only "10" (J)
        assert_eq!(num_decodings("2101".to_string()), 1); // Only "2" "10" "1" (B J A)
    }

    #[test]
    fn test_stress_consecutive_invalid() {
        assert_eq!(num_decodings("100".to_string()), 0); // "10" followed by "0" (invalid)
        assert_eq!(num_decodings("30".to_string()), 0); // "30" is not valid
        assert_eq!(num_decodings("27".to_string()), 1); // Only "2" "7"
    }
}
