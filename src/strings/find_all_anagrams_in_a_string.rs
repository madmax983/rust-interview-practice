//! # 438. Find All Anagrams in a String
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/find-all-anagrams-in-a-string/>
//!
//! Given two strings `s` and `p`, return an array of all the start indices of `p`'s anagrams in `s`.
//! You may return the answer in any order.
//!
//! This problem perfectly demonstrates Rust's zero-cost abstractions for string manipulation.
//! It highlights the trade-offs between highly idiomatic iterator combinators (like `.windows()`)
//! and manual state management for algorithmic optimality. It also reinforces the performance
//! benefits of operating on raw bytes (`&[u8]`) instead of Unicode characters (`char`) when
//! the domain is constrained to ASCII, demonstrating how the compiler optimizes fixed-size array comparisons.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::strings::find_all_anagrams_in_a_string::find_anagrams;
//!
//! assert_eq!(find_anagrams("cbaebabacd".to_string(), "abc".to_string()), vec![0, 6]);
//! assert_eq!(find_anagrams("abab".to_string(), "ab".to_string()), vec![0, 1, 2]);
//! ```
//!
//! ## Constraints
//!
//! - `1 <= s.length, p.length <= 3 * 10^4`
//! - `s` and `p` consist of lowercase English letters.

/// Brute force approach: Collect to Vectors and Sort.
/// Time: O(N * P log P) - where N is `s.len()` and P is `p.len()`.
/// Space: O(P) - allocating vectors for each window.
///
/// For every possible starting window of size `p.len()`, we extract the substring,
/// collect its characters into a `Vec`, sort it, and compare it to the sorted characters of `p`.
/// This is highly inefficient because it allocates memory for every single character window
/// and performs a sort operation `N` times.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::cast_possible_wrap)]
pub fn find_anagrams_brute_force(s: String, p: String) -> Vec<i32> {
    let s_len = s.len();
    let p_len = p.len();

    if s_len < p_len {
        return vec![];
    }

    let mut result = Vec::new();

    // BOLT OPTIMIZATION: Avoid `.chars().collect::<Vec<char>>()` O(N) allocation overhead.
    // Sort p's bytes once
    let mut p_bytes: Vec<u8> = p.into_bytes();
    p_bytes.sort_unstable();

    // RUST INSIGHT: Collecting `.chars()` into a Vec allocates heap memory.
    // Doing this in a loop creates significant GC/allocator pressure.
    let s_bytes = s.as_bytes();
    for i in 0..=s_len - p_len {
        let window = &s_bytes[i..i + p_len];
        let mut window_bytes = window.to_vec();
        window_bytes.sort_unstable();

        // If sorted window matches sorted p, we found an anagram
        if window_bytes == p_bytes {
            // Safe to cast to i32 per LeetCode constraints
            result.push(i as i32);
        }
    }

    result
}

/// Optimized approach: Idiomatic Iterator with `.windows()`.
/// Time: O(N * P) - We build a frequency array for every window from scratch.
/// Space: O(1) - Fixed-size arrays `[i32; 26]` allocated purely on the stack.
///
/// This approach utilizes Rust's powerful slice iterators. We convert the string to bytes
/// and use `.windows(p_len)` to iterate through all substrings of the required length.
/// For each window, we build a fresh character frequency map and compare it to `p`'s map.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::cast_possible_wrap)]
pub fn find_anagrams_optimized(s: String, p: String) -> Vec<i32> {
    if s.len() < p.len() {
        return vec![];
    }

    let mut p_count = [0i32; 26];

    // GOTCHA: `.as_bytes()` is O(1) and prevents UTF-8 decoding overhead.
    // However, never blindly replace `chars()` with `as_bytes()` unless the problem constraints
    // strictly guarantee ASCII (like this problem does: "lowercase English letters").
    // Doing this on arbitrary Unicode strings will cause critical functional regressions.
    for &b in p.as_bytes() {
        p_count[(b - b'a') as usize] += 1;
    }

    // RUST INSIGHT: `.enumerate()` gives us the starting index of each window natively.
    // `.filter_map()` concisely combines filtering conditions and mapping the return value.
    s.as_bytes()
        .windows(p.len())
        .enumerate()
        .filter_map(|(i, window)| {
            let mut window_count = [0i32; 26];
            for &b in window {
                window_count[(b - b'a') as usize] += 1;
            }

            // Fixed-size array comparison is highly optimized by LLVM.
            if window_count == p_count {
                Some(i as i32)
            } else {
                None
            }
        })
        .collect()
}

/// Optimal approach: Imperative Sliding Window with Incremental Frequency Updates.
/// Time: O(N) - Single pass through `s`. Array comparisons `[i32; 26] == [i32; 26]` are O(1)
/// Space: O(1) - Fixed-size arrays on the stack.
///
/// Instead of recalculating the entire frequency map for every window, we maintain a running
/// frequency map. When the window slides right, we increment the frequency of the new character
/// entering the window and decrement the frequency of the old character leaving the window.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::cast_possible_wrap)]
pub fn find_anagrams_optimal(s: String, p: String) -> Vec<i32> {
    let s_len = s.len();
    let p_len = p.len();

    if s_len < p_len {
        return vec![];
    }

    let mut result = Vec::new();

    // Fixed-size arrays for tracking counts (initialized to zeros)
    let mut p_count = [0i32; 26];
    let mut s_count = [0i32; 26];

    let s_bytes = s.as_bytes();
    let p_bytes = p.as_bytes();

    // RUST INSIGHT: Initialize the first window and the target `p` frequencies
    // in the same loop to minimize passes.
    for i in 0..p_len {
        p_count[(p_bytes[i] - b'a') as usize] += 1;
        s_count[(s_bytes[i] - b'a') as usize] += 1;
    }

    // Check the very first window
    if s_count == p_count {
        result.push(0);
    }

    // Slide the window across the rest of the string
    for i in p_len..s_len {
        // Add the new character entering from the right
        s_count[(s_bytes[i] - b'a') as usize] += 1;

        // Remove the old character that left from the left
        s_count[(s_bytes[i - p_len] - b'a') as usize] -= 1;

        // Compare the running state to the target state
        if s_count == p_count {
            result.push((i - p_len + 1) as i32);
        }
    }

    result
}

/// Main entry point - uses the optimal sliding window approach.
#[must_use]
pub fn find_anagrams(s: String, p: String) -> Vec<i32> {
    find_anagrams_optimal(s, p)
}

// ============================================================================
// Alternative Approaches
// ============================================================================
// 1. Array element tracking variable: Instead of comparing the full `[i32; 26]` array
//    each iteration, maintain a `matches` integer. Increment when a character's frequency
//    matches `p`, decrement when it un-matches. If `matches == 26`, it's an anagram.
//    This brings the comparison down from 26 ops to 1 op per iteration, but array
//    comparison is usually already vectorized by LLVM, making the practical difference negligible.

#[cfg(test)]
mod tests {
    use super::*;

    // Happy Path tests
    #[test]
    fn test_find_anagrams_basic() {
        let s = "cbaebabacd".to_string();
        let p = "abc".to_string();
        let expected = vec![0, 6];

        assert_eq!(find_anagrams_brute_force(s.clone(), p.clone()), expected);
        assert_eq!(find_anagrams_optimized(s.clone(), p.clone()), expected);
        assert_eq!(find_anagrams_optimal(s.clone(), p.clone()), expected);
    }

    #[test]
    fn test_find_anagrams_overlapping() {
        let s = "abab".to_string();
        let p = "ab".to_string();
        let expected = vec![0, 1, 2];

        assert_eq!(find_anagrams_brute_force(s.clone(), p.clone()), expected);
        assert_eq!(find_anagrams_optimized(s.clone(), p.clone()), expected);
        assert_eq!(find_anagrams_optimal(s.clone(), p.clone()), expected);
    }

    // Edge Case tests
    #[test]
    fn test_s_shorter_than_p() {
        let s = "a".to_string();
        let p = "ab".to_string();
        let expected: Vec<i32> = vec![];

        assert_eq!(find_anagrams_brute_force(s.clone(), p.clone()), expected);
        assert_eq!(find_anagrams_optimized(s.clone(), p.clone()), expected);
        assert_eq!(find_anagrams_optimal(s.clone(), p.clone()), expected);
    }

    #[test]
    fn test_no_anagrams() {
        let s = "abcdefg".to_string();
        let p = "z".to_string();
        let expected: Vec<i32> = vec![];

        assert_eq!(find_anagrams_brute_force(s.clone(), p.clone()), expected);
        assert_eq!(find_anagrams_optimized(s.clone(), p.clone()), expected);
        assert_eq!(find_anagrams_optimal(s.clone(), p.clone()), expected);
    }

    #[test]
    fn test_identical_strings() {
        let s = "abc".to_string();
        let p = "abc".to_string();
        let expected = vec![0];

        assert_eq!(find_anagrams_brute_force(s.clone(), p.clone()), expected);
        assert_eq!(find_anagrams_optimized(s.clone(), p.clone()), expected);
        assert_eq!(find_anagrams_optimal(s.clone(), p.clone()), expected);
    }

    // Stress/Boundary tests
    #[test]
    fn test_long_string_all_anagrams() {
        // String of 10,000 'a's, p is "a"
        let s = "a".repeat(10_000);
        let p = "a".to_string();

        // Expected indices: 0 to 9999
        let expected: Vec<i32> = (0..10_000).collect();

        // Testing the optimal and optimized approaches for large inputs
        assert_eq!(find_anagrams_optimized(s.clone(), p.clone()), expected);
        assert_eq!(find_anagrams_optimal(s.clone(), p.clone()), expected);
    }
}
