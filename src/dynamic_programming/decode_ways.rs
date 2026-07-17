//! # 91. Decode Ways
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/decode-ways/>
//!
//! A message containing letters from `A-Z` can be encoded into numbers using the following mapping:
//! `A -> 1`, `B -> 2`, ..., `Z -> 26`. Given a string `s` containing only digits, return the number
//! of ways to decode it. If the entire string cannot be decoded in any valid way, return `0`.
//!
//! This problem naturally lends itself to dynamic programming. In Rust, it is a great opportunity
//! to demonstrate the performance benefits of zero-cost, safe byte-level string access using
//! `.as_bytes()` instead of UTF-8 iterator adapters (`.chars()`), since we are dealing with guaranteed
//! ASCII digit strings. It also demonstrates dynamic programming space optimization (from `O(N)` to `O(1)` space).
//!
//! ## Approach
//!
//! Let `dp[i]` be the number of ways to decode the prefix of string `s` of length `i`.
//! - If `s[i-1]` is not `'0'`, it can be decoded as a single digit. We can add `dp[i-1]` to `dp[i]`.
//! - If the substring `s[i-2..i]` represents a number between 10 and 26, it can be decoded as a two-digit number.
//!   We can add `dp[i-2]` to `dp[i]`.
//!
//! Since `dp[i]` only depends on `dp[i-1]` and `dp[i-2]`, we don't need an array of size `N`.
//! We can optimize the space complexity to `O(1)` by keeping track of only the two previous states.
//!
//! ## Idiomatic Rust
//!
//! - **`.as_bytes()` vs `.chars()`**: Since `LeetCode` strings for this problem are strictly digits (`'0'`-`'9'`),
//!   using `.as_bytes()` is safe, idiomatic, and significantly faster because it avoids the overhead of UTF-8
//!   validation and multi-byte character boundary checking on every iteration.
//! - **Space optimization**: Maintaining just `prev1` and `prev2` rather than a full `Vec` makes it clean and highly efficient.
//!
//! ## Alternative Approaches
//!
//! - **Top-down Memoization (Recursion + Cache)**: You could use a recursive function and memoize the results
//!   in a `HashMap` or a simple vector. This has the same `O(N)` time complexity but worse space complexity
//!   (`O(N)` due to recursion depth and the cache).
//! - **Standard O(N) Bottom-up DP**: Maintaining a `Vec<i32>` of size `s.len() + 1`. This is easier to conceptualize
//!   initially but has `O(N)` space.

#[allow(clippy::needless_pass_by_value)]
#[must_use]
pub fn num_decodings(s: String) -> i32 {
    let bytes = s.as_bytes();

    // RUST INSIGHT: A string with length 0 or starting with '0' cannot be decoded.
    // Rust's slice access is boundary-checked, so checking `is_empty` first avoids panics.
    if bytes.is_empty() || bytes[0] == b'0' {
        return 0;
    }

    let n = bytes.len();

    // RUST INSIGHT: Here we optimize space from O(N) to O(1).
    // `prev2` represents dp[i-2] (number of ways to decode string up to index i-2)
    // `prev1` represents dp[i-1] (number of ways to decode string up to index i-1)
    let mut prev2 = 1;
    let mut prev1 = 1;

    for i in 1..n {
        let mut curr = 0;

        // Single digit decode: valid if it's not '0'
        if bytes[i] != b'0' {
            curr += prev1;
        }

        // Two digit decode: valid if it forms a number between 10 and 26
        // GOTCHA: byte arithmetic is safe here because we know we have ASCII digits,
        // but we must subtract b'0' to get the numeric value.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_happy_path() {
        // "12" could be decoded as "AB" (1 2) or "L" (12)
        assert_eq!(num_decodings("12".to_string()), 2);

        // "226" could be decoded as "BZ" (2 26), "VF" (22 6), or "BBF" (2 2 6)
        assert_eq!(num_decodings("226".to_string()), 3);
    }

    #[test]
    fn test_edge_cases() {
        // Cannot start with 0
        assert_eq!(num_decodings("06".to_string()), 0);
        // Single valid digit
        assert_eq!(num_decodings("1".to_string()), 1);
        // Double zeros are invalid
        assert_eq!(num_decodings("00".to_string()), 0);
        // Valid 10 and 20
        assert_eq!(num_decodings("10".to_string()), 1);
        assert_eq!(num_decodings("20".to_string()), 1);
        // Invalid ending zero
        assert_eq!(num_decodings("30".to_string()), 0);
    }

    #[test]
    fn test_stress_and_boundaries() {
        // Long string of 1s (Fibonacci sequence behavior)
        // Length 5: "11111" -> dp is [1, 1, 2, 3, 5, 8] -> 8
        assert_eq!(num_decodings("11111".to_string()), 8);

        // Number where no two consecutive digits form a number <= 26
        assert_eq!(num_decodings("33333".to_string()), 1);

        // Invalid sequence mixed in
        assert_eq!(num_decodings("2101".to_string()), 1);
        assert_eq!(num_decodings("1001".to_string()), 0);
    }
}
