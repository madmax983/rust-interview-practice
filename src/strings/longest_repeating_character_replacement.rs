//! # 424. Longest Repeating Character Replacement
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/longest-repeating-character-replacement/>
//!
//! You are given a string `s` and an integer `k`. You can choose any character of the string and
//! change it to any other uppercase English character. You can perform this operation at most `k` times.
//!
//! Return the length of the longest substring containing the same letter you can get after performing
//! the above operations.
//!
//! ## Why this matters in Rust
//! This problem provides a perfect opportunity to demonstrate zero-cost abstractions over strings
//! in Rust. It contrasts the naive approach of iterating over characters with the high-performance
//! sliding window technique operating directly on bytes (`u8`). It also highlights how constant-sized
//! arrays (`[usize; 26]`) can replace `HashMap`s for significant performance gains when the domain
//! is constrained (e.g., uppercase English letters).
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::strings::longest_repeating_character_replacement::character_replacement;
//!
//! let s = "ABAB".to_string();
//! let k = 2;
//! assert_eq!(character_replacement(s, k), 4);
//! // Explanation: Replace the two 'A's with two 'B's or vice versa.
//!
//! let s = "AABABBA".to_string();
//! let k = 1;
//! assert_eq!(character_replacement(s, k), 4);
//! // Explanation: Replace the one 'A' in the middle with 'B' and form "AABBBBA".
//! // The substring "BBBB" has the longest repeating letters, which is 4.
//! ```
//!
//! ## Constraints
//!
//! - `1 <= s.length <= 10^5`
//! - `s` consists of only uppercase English letters.
//! - `0 <= k <= s.length`

/// Brute force approach: Check all possible substrings
/// Time: O(N^2) - Iterating all start and end positions, counting frequencies
/// Space: O(1) - Constant size array for frequency map
///
/// This approach explores all possible substrings. For each substring, it finds the most
/// frequent character. If the length of the substring minus the frequency of the most
/// frequent character is less than or equal to `k`, the substring is valid.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
#[allow(clippy::cast_sign_loss)] // LeetCode constraints guarantee k >= 0
#[allow(clippy::cast_possible_truncation)] // LeetCode constraints guarantee it fits
#[allow(clippy::cast_possible_wrap)] // LeetCode constraints guarantee it fits
pub fn character_replacement_brute_force(s: String, k: i32) -> i32 {
    let bytes = s.as_bytes();
    let mut max_len = 0;

    for i in 0..bytes.len() {
        let mut counts = [0; 26];
        let mut max_freq = 0;

        for (offset, &byte) in bytes[i..].iter().enumerate() {
            let idx = (byte - b'A') as usize;
            counts[idx] += 1;
            max_freq = std::cmp::max(max_freq, counts[idx]);

            let window_len = offset + 1;
            if window_len - max_freq <= k as usize {
                max_len = std::cmp::max(max_len, window_len);
            } else {
                // If the substring is invalid, extending it further won't make it valid
                // because we add 1 to both window_len and at most 1 to max_freq,
                // so window_len - max_freq is monotonically non-decreasing.
                break;
            }
        }
    }

    max_len as i32
}

/// Optimal approach: Sliding window
/// Time: O(N) - Both pointers `left` and `right` traverse the string at most once.
/// Space: O(1) - Constant size array for the frequency map.
///
/// This uses a sliding window. We expand the window by moving the `right` pointer and
/// updating the frequency map. We track the `max_freq` of any character within the current
/// window. If the window becomes invalid (`window_len - max_freq > k`), we shrink it
/// from the left.
///
/// RUST INSIGHT: Notice that we maintain `max_freq` as the *historical* maximum frequency
/// of any character in the window. We don't decrement it when `left` pointer moves past
/// the most frequent character. Why? Because we only care about finding a window *longer*
/// than our current max. A longer window would require a *higher* `max_freq`. Thus, an
/// outdated, larger `max_freq` only prevents the window from shrinking, which is fine!
#[must_use]
#[allow(clippy::needless_pass_by_value)]
#[allow(clippy::cast_sign_loss)] // LeetCode constraints guarantee k >= 0
#[allow(clippy::cast_possible_truncation)] // LeetCode constraints guarantee it fits
#[allow(clippy::cast_possible_wrap)] // LeetCode constraints guarantee it fits
pub fn character_replacement_optimal(s: String, k: i32) -> i32 {
    // GOTCHA: Do not use `.chars().collect::<Vec<char>>()` here!
    // Since LeetCode guarantees the string contains only uppercase ASCII letters,
    // we can bypass UTF-8 decoding overhead entirely and work directly on bytes (`u8`).
    // This turns an O(N) allocation into an O(1) view into the string's buffer.
    let bytes = s.as_bytes();

    // RUST INSIGHT: We use a fixed-size array `[usize; 26]` instead of a `HashMap`.
    // This is incredibly fast because it's stack-allocated, cache-friendly, and avoids
    // the hashing overhead. Array indexing is practically zero-cost.
    let mut counts = [0; 26];

    let mut left = 0;
    let mut max_freq = 0;
    let mut max_len = 0;

    for right in 0..bytes.len() {
        let right_idx = (bytes[right] - b'A') as usize;
        counts[right_idx] += 1;

        max_freq = std::cmp::max(max_freq, counts[right_idx]);

        // window_len - max_freq > k means the current window cannot be made valid
        // by replacing k characters.
        let mut window_len = right - left + 1;
        if window_len - max_freq > k as usize {
            let left_idx = (bytes[left] - b'A') as usize;
            counts[left_idx] -= 1;
            left += 1;

            // RUST INSIGHT / ALGORITHM NOTE:
            // We decrement window_len here so the upcoming `max_len = std::cmp::max(...)`
            // doesn't incorrectly record an invalid window. Notice we do NOT decrement
            // `max_freq` even though the character leaving the window might be the most
            // frequent one. This is because we only care about finding a *longer* valid
            // window. A longer window requires a *larger* `max_freq`. An outdated, larger
            // `max_freq` just prevents the window from incorrectly shrinking, which is correct!
            window_len -= 1;
        }

        max_len = std::cmp::max(max_len, window_len);
    }

    max_len as i32
}

/// Main entry point - uses optimal solution
#[must_use]
pub fn character_replacement(s: String, k: i32) -> i32 {
    character_replacement_optimal(s, k)
}

// =========================================================================================
// Alternative Approaches
// =========================================================================================
//
// 1. Binary Search + Sliding Window (O(N log N)):
//    You can binary search for the answer length `L` between 1 and N. For a given `L`,
//    use a sliding window of fixed size `L` to see if there's any valid window. If there is,
//    search larger `L`, otherwise smaller `L`. This is more complex and slower than O(N).
//
// 2. HashMap for frequencies (O(N)):
//    Instead of a `[usize; 26]` array, use a `std::collections::HashMap<char, usize>`.
//    This is idiomatic when the character set is large or unbounded (e.g., full UTF-8),
//    but introduces significant hashing overhead and memory fragmentation. For constrained
//    domains like uppercase English letters, the array is strictly superior.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_happy_path_1() {
        let s = "ABAB".to_string();
        let k = 2;
        assert_eq!(character_replacement_brute_force(s.clone(), k), 4);
        assert_eq!(character_replacement_optimal(s, k), 4);
    }

    #[test]
    fn test_happy_path_2() {
        let s = "AABABBA".to_string();
        let k = 1;
        assert_eq!(character_replacement_brute_force(s.clone(), k), 4);
        assert_eq!(character_replacement_optimal(s, k), 4);
    }

    #[test]
    fn test_edge_case_k_is_zero() {
        let s = "AABA".to_string();
        let k = 0;
        assert_eq!(character_replacement_brute_force(s.clone(), k), 2);
        assert_eq!(character_replacement_optimal(s, k), 2);
    }

    #[test]
    fn test_edge_case_k_larger_than_string() {
        let s = "ABCDE".to_string();
        let k = 10;
        assert_eq!(character_replacement_brute_force(s.clone(), k), 5);
        assert_eq!(character_replacement_optimal(s, k), 5);
    }

    #[test]
    fn test_edge_case_single_character() {
        let s = "A".to_string();
        let k = 0;
        assert_eq!(character_replacement_brute_force(s.clone(), k), 1);
        assert_eq!(character_replacement_optimal(s, k), 1);
    }

    #[test]
    fn test_edge_case_all_same_characters() {
        let s = "AAAAA".to_string();
        let k = 2;
        assert_eq!(character_replacement_brute_force(s.clone(), k), 5);
        assert_eq!(character_replacement_optimal(s, k), 5);
    }

    #[test]
    fn test_stress_long_string() {
        // Build a string "A B C D E F ..." repeating many times.
        let mut s = String::with_capacity(100_000);
        for i in 0..10_000 {
            let c = (b'A' + (i % 26) as u8) as char;
            s.push(c);
        }
        let k = 50;
        // Brute force would be too slow here, so we only test optimal.
        // O(N) is expected to finish almost instantly.
        let res = character_replacement_optimal(s, k);
        // The max valid window will contain 50 replaced characters + all characters of the most frequent letter.
        // For string "ABC...ZABC...Z", each letter appears ~384 times.
        // If we pick 'A', its max freq in a window of size L won't exceed L/26 + 1.
        // This is a complex calculation but we just want to ensure it completes fast and doesn't panic.
        assert!(res > k);
    }
}
