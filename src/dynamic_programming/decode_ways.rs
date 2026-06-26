//! # 91. Decode Ways
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/decode-ways/>
//!
//! A message containing letters from A-Z can be encoded into numbers using the mapping `A -> 1`, `B -> 2`, ..., `Z -> 26`.
//! Given a string `s` containing only digits, return the number of ways to decode it.
//!
//! This problem matters in Rust because it perfectly demonstrates the performance difference
//! between UTF-8 character iteration (`.chars()`) and raw byte manipulation (`.as_bytes()`).
//! Since we know the input consists solely of ASCII digits, dropping down to byte slices allows
//! for zero-cost, O(1) random access while remaining completely safe.
//!
//! ## Approach
//!
//! We provide three solutions to illustrate the progression of dynamic programming:
//! 1. **Brute Force (Recursive):** Explores all valid 1-digit and 2-digit combinations. Time: O(2^N), Space: O(N) for the call stack.
//! 2. **Optimized (O(N) DP Array):** Memoizes the recursive approach into an array. Time: O(N), Space: O(N).
//! 3. **Optimal (O(1) Space DP):** Since `dp[i]` only depends on `dp[i-1]` and `dp[i-2]`, we only need two variables. Time: O(N), Space: O(1).
//!
//! Idiomatic Rust favors the `.as_bytes()` approach over `.chars().nth(i)` because the latter
//! is O(N) for string indexing in Rust due to UTF-8 variable width characters. Using a `&[u8]` gives us
//! guaranteed O(1) indexing and avoids bounds-checking panics when carefully traversed.
//!
//! ## Alternative Approaches
//! - **Recursive with Memoization (`HashMap` or `Vec`):** Top-down DP. Functionally equivalent to the O(N) DP array but with function call overhead.
//! - **Iterator-based Fold:** While possible to implement with `fold`, it often becomes less readable than a simple `for` loop because of the overlapping two-element lookback.

/// Brute force approach: Naive Recursion.
/// Time: O(2^N) - In the worst case, we branch twice for every digit.
/// Space: O(N) - Maximum depth of the recursion tree.
///
/// This approach explores all possible valid decodings recursively.
/// It will Time Limit Exceed (TLE) on LeetCode for larger strings.
#[must_use]
pub fn num_decodings_brute_force(s: String) -> i32 {
    // GOTCHA: We use `s.as_bytes()` here. If we used `s.chars().nth(index)`,
    // each `nth` call would be O(N), making the overall time complexity even worse!
    let bytes = s.as_bytes();

    fn decode(bytes: &[u8], index: usize) -> i32 {
        if index == bytes.len() {
            return 1;
        }

        if bytes[index] == b'0' {
            return 0;
        }

        let mut count = decode(bytes, index + 1);

        if index + 1 < bytes.len() {
            let two_digit = (bytes[index] - b'0') * 10 + (bytes[index + 1] - b'0');
            if (10..=26).contains(&two_digit) {
                count += decode(bytes, index + 2);
            }
        }

        count
    }

    decode(bytes, 0)
}

/// Optimized approach: Dynamic Programming with Array.
/// Time: O(N) - We iterate through the string exactly once.
/// Space: O(N) - We allocate a DP array of size N + 1.
#[must_use]
pub fn num_decodings_optimized(s: String) -> i32 {
    if s.is_empty() {
        return 0;
    }

    let bytes = s.as_bytes();
    if bytes[0] == b'0' {
        return 0;
    }

    let n = bytes.len();
    // RUST INSIGHT: `vec![0; n + 1]` safely and efficiently zero-initializes the vector.
    let mut dp = vec![0; n + 1];

    dp[0] = 1;
    dp[1] = 1;

    for i in 2..=n {
        let single_digit = bytes[i - 1] - b'0';
        let double_digit = (bytes[i - 2] - b'0') * 10 + single_digit;

        if single_digit != 0 {
            dp[i] += dp[i - 1];
        }

        if (10..=26).contains(&double_digit) {
            dp[i] += dp[i - 2];
        }
    }

    dp[n]
}

/// Optimal approach: Space-Optimized Dynamic Programming.
/// Time: O(N) - We iterate through the string exactly once.
/// Space: O(1) - We only maintain the last two DP states.
#[must_use]
pub fn num_decodings_optimal(s: String) -> i32 {
    if s.is_empty() {
        return 0;
    }

    let bytes = s.as_bytes();
    if bytes[0] == b'0' {
        return 0;
    }

    // `two_back` corresponds to dp[i-2]
    // `one_back` corresponds to dp[i-1]
    let mut two_back = 1;
    let mut one_back = 1;

    for i in 1..bytes.len() {
        let mut current = 0;
        let single_digit = bytes[i] - b'0';
        let double_digit = (bytes[i - 1] - b'0') * 10 + single_digit;

        // RUST INSIGHT: Notice how pattern matching isn't strictly necessary here.
        // Simple `if` conditions are idiomatic when evaluating numeric ranges.
        if single_digit != 0 {
            current += one_back;
        }

        if (10..=26).contains(&double_digit) {
            current += two_back;
        }

        two_back = one_back;
        one_back = current;
    }

    one_back
}

/// Main entry point - uses the optimal O(1) space approach.
#[must_use]
pub fn num_decodings(s: String) -> i32 {
    num_decodings_optimal(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Happy Path tests
    #[test]
    fn test_num_decodings_happy() {
        assert_eq!(num_decodings_brute_force("12".to_string()), 2);
        assert_eq!(num_decodings_optimized("12".to_string()), 2);
        assert_eq!(num_decodings_optimal("12".to_string()), 2);

        assert_eq!(num_decodings_brute_force("226".to_string()), 3);
        assert_eq!(num_decodings_optimized("226".to_string()), 3);
        assert_eq!(num_decodings_optimal("226".to_string()), 3);
    }

    // Edge Case tests
    #[test]
    fn test_num_decodings_edge() {
        // Starts with 0
        assert_eq!(num_decodings_brute_force("06".to_string()), 0);
        assert_eq!(num_decodings_optimized("06".to_string()), 0);
        assert_eq!(num_decodings_optimal("06".to_string()), 0);

        // Contains invalid double digit
        assert_eq!(num_decodings_brute_force("27".to_string()), 1);
        assert_eq!(num_decodings_optimized("27".to_string()), 1);
        assert_eq!(num_decodings_optimal("27".to_string()), 1);

        // Zero in the middle
        assert_eq!(num_decodings_brute_force("2101".to_string()), 1);
        assert_eq!(num_decodings_optimized("2101".to_string()), 1);
        assert_eq!(num_decodings_optimal("2101".to_string()), 1);

        // Invalid zeroes
        assert_eq!(num_decodings_brute_force("100".to_string()), 0);
        assert_eq!(num_decodings_optimized("100".to_string()), 0);
        assert_eq!(num_decodings_optimal("100".to_string()), 0);
    }

    // Stress/Boundary tests
    #[test]
    fn test_num_decodings_stress() {
        let stress_str = "111111111111111111111111111111111111111111111".to_string(); // 45 ones
        // Brute force would TLE on 45 ones, so we test it up to a smaller size
        let small_stress = "111111111111111".to_string(); // 15 ones
        assert_eq!(num_decodings_brute_force(small_stress), 987);

        // Optimal solutions can easily handle the large string (which evaluates to the 46th Fibonacci number)
        assert_eq!(num_decodings_optimized(stress_str.clone()), 1836311903);
        assert_eq!(num_decodings_optimal(stress_str), 1836311903);
    }
}
