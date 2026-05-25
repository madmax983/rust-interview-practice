//! # 91. Decode Ways
//!
//! A message containing letters from `A-Z` can be **encoded** into numbers using the following mapping:
//! `'A' -> "1"`, `'B' -> "2"`, ..., `'Z' -> "26"`
//!
//! To **decode** an encoded message, all the digits must be grouped then mapped back into letters using the reverse of the mapping above (there may be multiple ways).
//! For example, `"11106"` can be mapped into:
//! - `"AAJF"` with the grouping `(1 1 10 6)`
//! - `"KJF"` with the grouping `(11 10 6)`
//!
//! Given a string `s` containing only digits, return the **number of ways** to decode it.
//!
//! - Difficulty: Medium
//! - LeetCode: <https://leetcode.com/problems/decode-ways/>
//!
//! ## Why this matters in Rust
//! This problem perfectly illustrates the performance benefits of zero-cost abstractions in Rust.
//! By using `.as_bytes()`, we can treat an ASCII string as an array of bytes (`&[u8]`) for O(1) indexing
//! and matching, avoiding expensive substring allocations. It also highlights Rust's exhaustive pattern matching
//! and range bounds (like `b'1'..=b'9'`), which make the core logic robust against off-by-one errors and invalid data.
//!
//! ## Approach
//!
//! This is a classic 1D Dynamic Programming problem where the number of ways to decode up to index `i`
//! depends on the number of ways up to `i-1` (single digit match) and `i-2` (double digit match).
//!
//! We explore three implementations:
//! 1.  **Brute Force**: Recursive DFS. Explores every possible decoding path. O(2^n) time.
//! 2.  **Tabulation**: Bottom-Up DP. Uses an array `dp` of size `n+1` to store intermediate results. O(n) time and O(n) space.
//! 3.  **Optimal**: Space-optimized Bottom-Up DP. We only need the last two states, so we use two variables instead of an array. O(n) time and O(1) space.

/// Brute Force Approach: Recursive DFS
///
/// We try to decode 1 or 2 digits at a time. If it's valid, we recursively decode the rest of the string.
///
/// - **Time Complexity**: O(2^n), where n is the length of the string. In the worst case (e.g., "1111..."),
///   every character can branch into two recursive calls.
/// - **Space Complexity**: O(n) for the recursion stack.
///
/// # GOTCHA
/// String slicing in Rust expects byte indices. If `s` had multi-byte Unicode characters,
/// this could panic. LeetCode guarantees `s` only contains ASCII digits, so byte slicing is safe.
#[allow(clippy::needless_pass_by_value)]
pub fn num_decodings_brute_force(s: String) -> i32 {
    fn solve(bytes: &[u8], index: usize) -> i32 {
        // Base case: successfully decoded the whole string
        if index == bytes.len() {
            return 1;
        }

        // If the current digit is '0', it cannot be decoded alone or start a valid pair
        if bytes[index] == b'0' {
            return 0;
        }

        // Option 1: Decode a single digit
        let mut ways = solve(bytes, index + 1);

        // Option 2: Decode two digits if they form a valid number between 10 and 26
        if index + 1 < bytes.len() {
            // RUST INSIGHT: We can easily parse a two-byte slice into an integer.
            // A more manual approach is `(bytes[index] - b'0') * 10 + (bytes[index + 1] - b'0')`.
            let value = (bytes[index] - b'0') * 10 + (bytes[index + 1] - b'0');
            if (10..=26).contains(&value) {
                ways += solve(bytes, index + 2);
            }
        }

        ways
    }

    solve(s.as_bytes(), 0)
}

/// Tabulation Approach: Bottom-Up DP
///
/// We build a `dp` array where `dp[i]` is the number of ways to decode the substring `s[0..i]`.
///
/// - **Time Complexity**: O(n), where n is the length of the string.
/// - **Space Complexity**: O(n) to store the DP array.
#[allow(clippy::needless_pass_by_value)]
pub fn num_decodings_tabulation(s: String) -> i32 {
    let bytes = s.as_bytes();
    let n = bytes.len();

    if n == 0 || bytes[0] == b'0' {
        return 0;
    }

    let mut dp = vec![0; n + 1];

    // dp[0] is the base case (empty string can be decoded 1 way)
    dp[0] = 1;
    // dp[1] depends on the first character
    dp[1] = 1;

    for i in 2..=n {
        // Single digit match (1 to 9)
        if bytes[i - 1] != b'0' {
            dp[i] += dp[i - 1];
        }

        // Double digit match (10 to 26)
        let two_digit = (bytes[i - 2] - b'0') * 10 + (bytes[i - 1] - b'0');
        if (10..=26).contains(&two_digit) {
            dp[i] += dp[i - 2];
        }
    }

    dp[n]
}

/// Optimal Approach: Space-Optimized Bottom-Up DP
///
/// Since `dp[i]` only depends on `dp[i-1]` and `dp[i-2]`, we don't need a full array.
/// We can track just the last two values to achieve O(1) space complexity.
///
/// - **Time Complexity**: O(n), single pass over the string.
/// - **Space Complexity**: O(1), only using a few variables.
///
/// # RUST INSIGHT
/// Using `.as_bytes()` allows us to avoid repeated allocations or UTF-8 boundary checks.
/// The `match` expression provides an exhaustive, expressive way to handle valid character ranges.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn num_decodings_optimal(s: String) -> i32 {
    let bytes = s.as_bytes();

    if bytes.is_empty() || bytes[0] == b'0' {
        return 0;
    }

    // `prev2` represents dp[i-2], `prev1` represents dp[i-1]
    let mut prev2 = 1;
    let mut prev1 = 1;

    for i in 1..bytes.len() {
        let mut curr = 0;

        // Check if single digit is valid
        // RUST INSIGHT: Range patterns like `b'1'..=b'9'` make this check zero-cost and highly readable.
        match bytes[i] {
            b'1'..=b'9' => curr += prev1,
            _ => {}
        }

        // Check if double digit is valid
        let two_digit = (bytes[i - 1] - b'0') * 10 + (bytes[i] - b'0');
        if (10..=26).contains(&two_digit) {
            curr += prev2;
        }

        // Shift state forward
        prev2 = prev1;
        prev1 = curr;
    }

    prev1
}

/// Main entry point
#[must_use]
pub fn num_decodings(s: String) -> i32 {
    num_decodings_optimal(s)
}

// Alternative Approaches:
// 1. **Memoization (Top-Down DP)**: Similar to Brute Force but uses a HashMap or Vec to cache results.
//    Easier to write recursively but uses O(n) space and has slight overhead from function calls.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force_basic() {
        assert_eq!(num_decodings_brute_force("12".to_string()), 2);
        assert_eq!(num_decodings_brute_force("226".to_string()), 3);
        assert_eq!(num_decodings_brute_force("06".to_string()), 0);
    }

    #[test]
    fn test_tabulation_basic() {
        assert_eq!(num_decodings_tabulation("12".to_string()), 2);
        assert_eq!(num_decodings_tabulation("226".to_string()), 3);
        assert_eq!(num_decodings_tabulation("06".to_string()), 0);
    }

    #[test]
    fn test_optimal_basic() {
        assert_eq!(num_decodings_optimal("12".to_string()), 2); // "AB" or "L"
        assert_eq!(num_decodings_optimal("226".to_string()), 3); // "BZ" (2, 26), "VF" (22, 6), or "BBF" (2, 2, 6)
        assert_eq!(num_decodings_optimal("06".to_string()), 0); // Invalid starting zero
    }

    #[test]
    fn test_edge_cases() {
        assert_eq!(num_decodings_optimal("10".to_string()), 1); // "J"
        assert_eq!(num_decodings_optimal("2101".to_string()), 1); // "U" (21), "J" (01 is invalid, so 10?), Wait: 2, 10, 1 -> "BJA"
        assert_eq!(num_decodings_optimal("27".to_string()), 1); // "BG" (2, 7). 27 is invalid.
        assert_eq!(num_decodings_optimal("".to_string()), 0);
    }

    #[test]
    fn test_stress_consecutive_zeros() {
        assert_eq!(num_decodings_optimal("1001".to_string()), 0); // "10" is 'J', but next is '01' invalid.
        assert_eq!(num_decodings_optimal("1111111111".to_string()), 89); // Fibonacci sequence
    }
}
