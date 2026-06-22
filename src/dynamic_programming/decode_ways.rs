//! # 91. Decode Ways
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/decode-ways/>
//!
//! A message containing letters from A-Z can be encoded into numbers using the following mapping:
//! 'A' -> "1", 'B' -> "2", ..., 'Z' -> "26"
//!
//! Given a string `s` containing only digits, return the number of ways to decode it.
//!
//! This problem naturally fits a 1D dynamic programming approach. In Rust, it provides an excellent
//! opportunity to practice zero-cost abstractions like `.as_bytes()` for ASCII string traversal,
//! avoiding expensive substring parsing, and utilizing `O(1)` space optimizations.
//!
//! ## Examples
//!
//! ```
//! // Omitted for brevity, see tests.
//! ```
//!
//! ## Constraints
//!
//! - `1 <= s.length <= 100`
//! - `s` contains only digits and may contain leading zero(s).

/// Brute force DP with O(N) array.
///
/// Time: O(N) where N is the length of the string.
/// Space: O(N) for the DP array.
///
/// RUST INSIGHT: We use `s.as_bytes()` because LeetCode guarantees the string only
/// contains ASCII digits. This allows us to index into the string in O(1) time safely,
/// without worrying about multibyte UTF-8 characters.
#[must_use]
pub fn num_decodings_dp(s: &str) -> i32 {
    if s.is_empty() {
        return 0;
    }

    let bytes = s.as_bytes();
    if bytes[0] == b'0' {
        return 0;
    }

    let n = bytes.len();
    let mut dp = vec![0; n + 1];

    // dp[i] represents the number of ways to decode a string of length i.
    dp[0] = 1;
    dp[1] = 1; // Since bytes[0] != '0'

    for i in 2..=n {
        // Single digit decode
        if bytes[i - 1] != b'0' {
            dp[i] += dp[i - 1];
        }

        // Two digit decode
        let two_digit = (bytes[i - 2] - b'0') * 10 + (bytes[i - 1] - b'0');
        if (10..=26).contains(&two_digit) {
            dp[i] += dp[i - 2];
        }
    }

    dp[n]
}

/// Optimized DP with O(1) space.
///
/// Time: O(N) where N is the length of the string.
/// Space: O(1) since we only keep track of the last two DP states.
///
/// RUST INSIGHT: Notice how we map `Option` and match on ranges. `(10..=26).contains()`
/// is a highly idiomatic way to check bounds without writing `val >= 10 && val <= 26`.
#[must_use]
pub fn num_decodings_optimized(s: &str) -> i32 {
    if s.is_empty() {
        return 0;
    }

    let bytes = s.as_bytes();
    if bytes[0] == b'0' {
        return 0;
    }

    // `two_back` represents dp[i-2]
    // `one_back` represents dp[i-1]
    let mut two_back = 1;
    let mut one_back = 1;

    for i in 1..bytes.len() {
        let mut current = 0;

        // Single digit decode
        if bytes[i] != b'0' {
            current += one_back;
        }

        // Two digit decode
        let two_digit = (bytes[i - 1] - b'0') * 10 + (bytes[i] - b'0');
        if (10..=26).contains(&two_digit) {
            current += two_back;
        }

        two_back = one_back;
        one_back = current;
    }

    one_back
}

/// Main entry point
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature sometimes passes String
pub fn num_decodings(s: String) -> i32 {
    num_decodings_optimized(&s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_happy_path() {
        assert_eq!(num_decodings_optimized("12"), 2); // "AB" or "L"
        assert_eq!(num_decodings_optimized("226"), 3); // "BZ", "VF", "BBF"
    }

    #[test]
    fn test_edge_cases() {
        assert_eq!(num_decodings_optimized("0"), 0);
        assert_eq!(num_decodings_optimized("06"), 0);
        assert_eq!(num_decodings_optimized("10"), 1); // "J"
        assert_eq!(num_decodings_optimized("27"), 1); // "BG"
    }

    #[test]
    fn test_stress() {
        assert_eq!(
            num_decodings_optimized("111111111111111111111111111111111111111111111"),
            1836311903
        );
    }
}
