//! # 242. Valid Anagram
//!
//! Difficulty: Easy
//! Link: <https://leetcode.com/problems/valid-anagram/>
//!
//! Given two strings `s` and `t`, return `true` if `t` is an anagram of `s`, and `false` otherwise.
//!
//! An Anagram is a word or phrase formed by rearranging the letters of a different word or phrase,
//! typically using all the original letters exactly once.
//!
//! This problem perfectly demonstrates Rust's approach to string iteration, ownership, and lightweight
//! data structures. It highlights how iterators and pattern matching can replace manual loops
//! and conditionals, yielding robust, zero-cost abstractions.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::strings::valid_anagram::is_anagram;
//!
//! assert_eq!(is_anagram("anagram".to_string(), "nagaram".to_string()), true);
//! assert_eq!(is_anagram("rat".to_string(), "car".to_string()), false);
//! ```
//!
//! ## Constraints
//!
//! - `1 <= s.length, t.length <= 5 * 10^4`
//! - `s` and `t` consist of lowercase English letters.

use std::collections::HashMap;

/// Brute force approach: Sorting.
/// Time: O(N log N) - where N is the length of the strings (sorting dominates)
/// Space: O(N) - to store the character vectors for sorting
///
/// Converts both strings into vectors of characters, sorts them, and compares them for equality.
/// While simple and correct for Unicode, it's inefficient due to allocation and sorting overhead.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_anagram_brute_force(s: String, t: String) -> bool {
    // Quick length check to short-circuit
    if s.len() != t.len() {
        return false;
    }

    // GOTCHA: `chars()` returns an iterator. We must collect it into a collection
    // (like Vec) before we can sort it, because iterators themselves aren't sortable.
    let mut s_chars: Vec<char> = s.chars().collect();
    let mut t_chars: Vec<char> = t.chars().collect();

    s_chars.sort_unstable();
    t_chars.sort_unstable();

    s_chars == t_chars
}

/// Optimized approach: HashMap for character frequency counting.
/// Time: O(N) - single pass over both strings
/// Space: O(K) - where K is the number of unique characters (up to 26 for lowercase English)
///
/// Uses a `HashMap` to track the frequency of characters in `s`, then decrements
/// counts based on `t`. This gracefully handles arbitrary Unicode characters.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_anagram_optimized(s: String, t: String) -> bool {
    if s.len() != t.len() {
        return false;
    }

    // RUST INSIGHT: `HashMap` is the generic way to handle character counting.
    // The `entry` API is extremely idiomatic here.
    let mut char_counts: HashMap<char, i32> = HashMap::new();

    for c in s.chars() {
        *char_counts.entry(c).or_insert(0) += 1;
    }

    for c in t.chars() {
        // Find the entry. If it exists, modify it.
        let count = char_counts.entry(c).or_insert(0);
        *count -= 1;
        // If count goes below zero, `t` has more of this character than `s`.
        if *count < 0 {
            return false;
        }
    }

    // Because lengths are equal, and no count went below zero, all counts must be exactly zero.
    true
}

/// Optimal approach: Fixed-size array for character frequency counting.
/// Time: O(N) - single pass over the strings
/// Space: O(1) - constant space array of size 26
///
/// Since the problem guarantees strings consist of lowercase English letters,
/// we can use a fixed-size array of 26 integers to track character frequencies.
/// We iterate over the raw bytes, completely bypassing UTF-8 decoding overhead.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_anagram_optimal(s: String, t: String) -> bool {
    if s.len() != t.len() {
        return false;
    }

    // Array initialized to 0. Stores the net count of each letter 'a' through 'z'.
    let mut counts = [0i32; 26];

    // RUST INSIGHT: `.bytes()` is O(1) per byte and avoids UTF-8 boundary checks.
    // It's perfectly safe here because we know the input is ASCII lowercase.
    for byte in s.bytes() {
        // Subtract b'a' (97) to map ASCII a-z to indices 0-25
        counts[(byte - b'a') as usize] += 1;
    }

    for byte in t.bytes() {
        let index = (byte - b'a') as usize;
        counts[index] -= 1;

        // Short-circuit if `t` has more of a specific character than `s`
        if counts[index] < 0 {
            return false;
        }
    }

    true
}

/// Main entry point - uses the optimal fixed-array approach due to problem constraints.
#[must_use]
pub fn is_anagram(s: String, t: String) -> bool {
    is_anagram_optimal(s, t)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Happy Path tests
    #[test]
    fn test_valid_anagram() {
        let s = "anagram".to_string();
        let t = "nagaram".to_string();

        assert!(is_anagram_brute_force(s.clone(), t.clone()));
        assert!(is_anagram_optimized(s.clone(), t.clone()));
        assert!(is_anagram_optimal(s, t));
    }

    #[test]
    fn test_invalid_anagram() {
        let s = "rat".to_string();
        let t = "car".to_string();

        assert!(!is_anagram_brute_force(s.clone(), t.clone()));
        assert!(!is_anagram_optimized(s.clone(), t.clone()));
        assert!(!is_anagram_optimal(s, t));
    }

    // Edge Case tests
    #[test]
    fn test_different_lengths() {
        let s = "a".to_string();
        let t = "ab".to_string();

        assert!(!is_anagram_brute_force(s.clone(), t.clone()));
        assert!(!is_anagram_optimized(s.clone(), t.clone()));
        assert!(!is_anagram_optimal(s, t));
    }

    #[test]
    fn test_empty_strings() {
        let s = "".to_string();
        let t = "".to_string();

        assert!(is_anagram_brute_force(s.clone(), t.clone()));
        assert!(is_anagram_optimized(s.clone(), t.clone()));
        assert!(is_anagram_optimal(s, t));
    }

    #[test]
    fn test_single_character() {
        let s = "a".to_string();
        let t = "a".to_string();

        assert!(is_anagram_brute_force(s.clone(), t.clone()));
        assert!(is_anagram_optimized(s.clone(), t.clone()));
        assert!(is_anagram_optimal(s, t));
    }

    // Stress/Boundary tests
    #[test]
    fn test_large_anagram() {
        // Creates two 50,000 character identical strings
        let s = "a".repeat(25000) + &"b".repeat(25000);
        let t = "b".repeat(25000) + &"a".repeat(25000);

        assert!(is_anagram_brute_force(s.clone(), t.clone()));
        assert!(is_anagram_optimized(s.clone(), t.clone()));
        assert!(is_anagram_optimal(s, t));
    }
}
