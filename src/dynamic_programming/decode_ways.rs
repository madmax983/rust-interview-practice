//! # 91. Decode Ways
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/decode-ways/>
//!
//! A message containing letters from A-Z can be encoded into numbers using the following mapping:
//! 'A' -> "1", 'B' -> "2", ... 'Z' -> "26".
//!
//! To decode an encoded message, all the digits must be grouped then mapped back into letters using the reverse of the mapping above (there may be multiple ways).
//! For example, "11106" can be mapped into:
//! - "AAJF" with the grouping (1 1 10 6)
//! - "KJF" with the grouping (11 10 6)
//!
//! Given a string `s` containing only digits, return the number of ways to decode it.
//!
//! This problem demonstrates dynamic programming space optimization (from O(N) to O(1) space) and highlights the performance benefits of zero-cost, safe byte-level string access using `.as_bytes()` instead of UTF-8 iterator adapters (`.chars()`) when dealing with guaranteed ASCII digit strings.
//!
//! ## Approach
//!
//! The problem can be broken down into subproblems: the number of ways to decode a string of length `i` depends on the number of ways to decode strings of length `i-1` and `i-2`.
//! If the last digit is valid (1-9), we can decode it by itself, adding `dp[i-1]` ways.
//! If the last two digits form a valid number (10-26), we can decode them together, adding `dp[i-2]` ways.
//!
//! We start with an optimized O(N) space DP array, and then refine it to an O(1) space optimal solution since we only need to look back two steps, similar to the Fibonacci sequence.
//!
//! RUST INSIGHT: Idiomatic Rust uses `.as_bytes()` when we are guaranteed ASCII input. This avoids the O(N) character lookup time that `.chars().nth(i)` would take, allowing safe O(1) byte-level access.

/// Brute Force approach: Recursive (Top-Down) without memoization
///
/// Time: O(2^N) - We branch up to two times for every character.
/// Space: O(N) - Recursion stack depth.
#[must_use]
pub fn num_decodings_brute_force(s: String) -> i32 {
    fn dfs(i: usize, bytes: &[u8]) -> i32 {
        if i == bytes.len() {
            return 1;
        }
        if bytes[i] == b'0' {
            return 0;
        }

        let mut res = dfs(i + 1, bytes);

        if i + 1 < bytes.len() && (bytes[i] == b'1' || (bytes[i] == b'2' && bytes[i + 1] <= b'6')) {
            res += dfs(i + 2, bytes);
        }

        res
    }

    dfs(0, s.as_bytes())
}

/// Optimized approach: Bottom-Up DP with O(N) Space
///
/// Time: O(N) - We iterate through the string once.
/// Space: O(N) - We allocate a DP array of size N + 1.
#[must_use]
pub fn num_decodings_optimized(s: String) -> i32 {
    if s.is_empty() {
        return 0;
    }

    // GOTCHA: `.chars()` is O(n) on UTF-8 strings. Don't index with `s.chars().nth(i)`.
    // Use `.as_bytes()` for guaranteed ASCII strings for zero-cost O(1) indexing.
    let bytes = s.as_bytes();
    let n = bytes.len();

    if bytes[0] == b'0' {
        return 0;
    }

    let mut dp = vec![0; n + 1];
    dp[0] = 1;
    dp[1] = 1;

    for i in 2..=n {
        // One step jump
        if bytes[i - 1] != b'0' {
            dp[i] += dp[i - 1];
        }

        // Two step jump
        let two_digit = (bytes[i - 2] - b'0') * 10 + (bytes[i - 1] - b'0');
        if (10..=26).contains(&two_digit) {
            dp[i] += dp[i - 2];
        }
    }

    dp[n]
}

/// Optimal approach: Bottom-Up DP with O(1) Space
///
/// Time: O(N) - We iterate through the string once.
/// Space: O(1) - We only keep track of the last two DP states.
#[must_use]
pub fn num_decodings_optimal(s: String) -> i32 {
    if s.is_empty() {
        return 0;
    }

    let bytes = s.as_bytes();
    if bytes[0] == b'0' {
        return 0;
    }

    let mut prev2 = 1;
    let mut prev1 = 1;

    for i in 1..bytes.len() {
        let mut current = 0;

        if bytes[i] != b'0' {
            current += prev1;
        }

        let two_digit = (bytes[i - 1] - b'0') * 10 + (bytes[i] - b'0');
        if (10..=26).contains(&two_digit) {
            current += prev2;
        }

        prev2 = prev1;
        prev1 = current;
    }

    prev1
}

/// Main entry point
#[must_use]
pub fn num_decodings(s: String) -> i32 {
    num_decodings_optimal(s)
}

// Alternative Approaches:
// 1. **Memoized DFS**: Same logic as the brute force recursive approach, but uses an array or HashMap to cache results of `dfs(i)`, bringing the time down to O(N) while keeping the top-down readability.
// 2. **Iterator-based fold**: It's possible to formulate the O(1) space DP transition as a fold over the string's byte windows, but it often ends up less readable than the straightforward mutable state loop shown above.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_happy_path() {
        // "12" -> "AB" or "L"
        assert_eq!(num_decodings_brute_force("12".to_string()), 2);
        assert_eq!(num_decodings_optimized("12".to_string()), 2);
        assert_eq!(num_decodings_optimal("12".to_string()), 2);

        // "226" -> "BZ" or "VF" or "BBF"
        assert_eq!(num_decodings_brute_force("226".to_string()), 3);
        assert_eq!(num_decodings_optimized("226".to_string()), 3);
        assert_eq!(num_decodings_optimal("226".to_string()), 3);
    }

    #[test]
    fn test_edge_cases() {
        // Starts with 0
        assert_eq!(num_decodings_brute_force("06".to_string()), 0);
        assert_eq!(num_decodings_optimized("06".to_string()), 0);
        assert_eq!(num_decodings_optimal("06".to_string()), 0);

        // Contains invalid 0 combination
        assert_eq!(num_decodings_brute_force("206".to_string()), 1); // "20" -> 'T', "6" -> 'F'
        assert_eq!(num_decodings_optimized("206".to_string()), 1);
        assert_eq!(num_decodings_optimal("206".to_string()), 1);

        assert_eq!(num_decodings_brute_force("30".to_string()), 0);
        assert_eq!(num_decodings_optimized("30".to_string()), 0);
        assert_eq!(num_decodings_optimal("30".to_string()), 0);
    }

    #[test]
    fn test_stress() {
        // A long valid sequence
        let s = "1111111111".to_string(); // 10 ones, fibonacci sequence -> 89 ways
        // Brute force is too slow for very large N, but for 10 it's fine.
        assert_eq!(num_decodings_brute_force(s.clone()), 89);
        assert_eq!(num_decodings_optimized(s.clone()), 89);
        assert_eq!(num_decodings_optimal(s), 89);
    }
}
