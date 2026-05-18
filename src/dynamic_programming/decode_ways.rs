//! # 91. Decode Ways
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/decode-ways/>
//!
//! A message containing letters from A-Z can be encoded into numbers using the following mapping:
//! - 'A' -> "1"
//! - 'B' -> "2"
//! - ...
//! - 'Z' -> "26"
//!
//! To decode an encoded message, all the digits must be grouped then mapped back into letters using the reverse of the mapping above (there may be multiple ways). For example, "11106" can be mapped into:
//! - "AAJF" with the grouping (1 1 10 6)
//! - "KJF" with the grouping (11 10 6)
//!
//! Note that the grouping (1 11 06) is invalid because "06" cannot be mapped into 'F' since "6" is different from "06".
//!
//! Given a string `s` containing only digits, return the number of ways to decode it.
//!
//! ## Why this matters in Rust
//! This problem emphasizes the difference between strings and byte slices in Rust. Since the input only contains ASCII digits ('0'-'9'), using `.as_bytes()` allows for zero-cost abstraction, giving us direct, `O(1)` indexing into the underlying buffer without the overhead of `.chars()` iterator processing.
//! It also highlights matching over conditions directly and carefully handling bounds with Rust's explicit error checking to avoid panics.
//!
//! ## Approach
//!
//! This is a classic 1D Dynamic Programming problem that boils down to variations of the Fibonacci sequence, heavily constrained by validity checks.
//! In languages like Python or Java, one might parse substrings using expensive slicing or casting. In Rust, we work directly with byte slices, explicitly matching bounds.
//!
//! 1.  **1D DP Array (Straightforward)**:
//!     - Let `dp[i]` be the number of ways to decode the string up to index `i`.
//!     - If `s[i-1]` forms a valid single digit ('1'-'9'), `dp[i] += dp[i-1]`.
//!     - If `s[i-2..i]` forms a valid two-digit number ('10'-'26'), `dp[i] += dp[i-2]`.
//!     - Time: O(N), Space: O(N).
//!
//! 2.  **Optimized Space O(1)**:
//!     - Notice that to compute `dp[i]`, we only ever need `dp[i-1]` and `dp[i-2]`.
//!     - We can optimize the space to O(1) by keeping track of only the last two states (`prev1` and `prev2`).
//!     - Time: O(N), Space: O(1).

/// 1D DP Array Approach
///
/// Uses a `Vec<i32>` of size `n + 1` to store the intermediate results.
///
/// Time Complexity: O(N) where N is the length of the string.
/// Space Complexity: O(N) for the DP array.
pub fn num_decodings_dp(s: String) -> i32 {
    let bytes = s.as_bytes();
    let n = bytes.len();

    // RUST INSIGHT: Returning early when the input is trivially invalid.
    if n == 0 || bytes[0] == b'0' {
        return 0;
    }

    // dp[i] represents the number of decodings for the prefix of length i.
    let mut dp = vec![0; n + 1];
    dp[0] = 1; // Base case: an empty string has 1 way to be "decoded".
    dp[1] = 1; // Base case: the first character is already verified to not be '0'.

    // GOTCHA: We iterate from 2 up to n (inclusive) because dp is size n + 1.
    // So dp[i] corresponds to the state after processing bytes[i-1].
    for i in 2..=n {
        let one_digit = bytes[i - 1];
        let two_digits = (bytes[i - 2] - b'0') * 10 + (bytes[i - 1] - b'0');

        // Can we form a single digit? Must be between '1' and '9'.
        if one_digit != b'0' {
            dp[i] += dp[i - 1];
        }

        // Can we form a double digit? Must be between "10" and "26".
        if (10..=26).contains(&two_digits) {
            dp[i] += dp[i - 2];
        }
    }

    dp[n]
}

/// Space-Optimized O(1) DP Approach
///
/// Uses two variables (`prev1`, `prev2`) to store the previous two states, avoiding the O(N) allocation.
///
/// Time Complexity: O(N)
/// Space Complexity: O(1)
pub fn num_decodings_optimal(s: String) -> i32 {
    let bytes = s.as_bytes();
    let n = bytes.len();

    if n == 0 || bytes[0] == b'0' {
        return 0;
    }

    // prev2 represents dp[i-2]
    // prev1 represents dp[i-1]
    let mut prev2 = 1;
    let mut prev1 = 1;

    for i in 1..n {
        let mut curr = 0;
        let one_digit = bytes[i];
        let two_digits = (bytes[i - 1] - b'0') * 10 + (bytes[i] - b'0');

        if one_digit != b'0' {
            curr += prev1;
        }

        if (10..=26).contains(&two_digits) {
            curr += prev2;
        }

        // Update states for the next iteration
        prev2 = prev1;
        prev1 = curr;
    }

    prev1
}

/// Main entry point (Defaults to the optimal solution)
pub fn num_decodings(s: String) -> i32 {
    num_decodings_optimal(s)
}

// Alternative Approaches:
// 1. **Recursive with Memoization**: A top-down DFS approach. Less idiomatic because it involves recursion overhead and usually a HashMap or array for caching, which uses O(N) space and is slower in practice than bottom-up.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_happy_path() {
        assert_eq!(num_decodings("12".to_string()), 2); // "AB" (1 2) or "L" (12)
        assert_eq!(num_decodings("226".to_string()), 3); // "BZ" (2 26), "VF" (22 6), or "BBF" (2 2 6)
    }

    #[test]
    fn test_edge_case_zeros() {
        assert_eq!(num_decodings("06".to_string()), 0); // Leading zero is invalid
        assert_eq!(num_decodings("10".to_string()), 1); // Only "10" (J)
        assert_eq!(num_decodings("2101".to_string()), 1); // "2" "10" "1"
        assert_eq!(num_decodings("27".to_string()), 1); // "2" "7", "27" is out of bounds
        assert_eq!(num_decodings("30".to_string()), 0); // "30" is invalid
    }

    #[test]
    fn test_stress_boundary() {
        // "111111111111111111111111111111111111111111111" (lots of 1s -> Fibonacci number)
        // Check dp and optimal match
        let s = "111111111111111111111".to_string();
        assert_eq!(num_decodings_dp(s.clone()), num_decodings_optimal(s));
    }
}
