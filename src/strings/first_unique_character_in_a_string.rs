//! # 387. First Unique Character in a String
//!
//! Difficulty: Easy
//! Link: <https://leetcode.com/problems/first-unique-character-in-a-string/>
//!
//! This problem is a natural fit for demonstrating Rust's efficient string processing
//! and array-based frequency counting. It highlights the difference between iterating
//! over UTF-8 characters and using fixed-size byte arrays when ASCII boundaries are guaranteed.
//!
//! ## Approach
//!
//! - **Brute Force:** Check every character against every other character to find duplicates.
//!   Time complexity is O(n²), which is inefficient for large strings.
//! - **Optimized:** Use a `HashMap` to store character frequencies in a first pass,
//!   then find the first character with a frequency of 1 in a second pass.
//!   Time: O(n), Space: O(n) or O(26) ≈ O(1) for distinct characters.
//! - **Optimal:** Since constraints guarantee only lowercase English letters, we can use
//!   a fixed-size array `[usize; 26]` for counting. By treating the string as bytes
//!   (`s.as_bytes()`), we avoid UTF-8 decoding overhead. Time: O(n), Space: O(1).
//!
//! In Python or Java, you might use `collections.Counter` or an array of `int`.
//! In Rust, `s.as_bytes()` paired with a fixed array gives zero-allocation counting.
//!
//! ## Alternative Approaches
//!
//! - **Using `HashMap` or `BTreeMap`:** Good when the character set is unknown or full Unicode.
//! - **Single Pass with Doubly Linked List:** Maintain order of appearance and counts to
//!   find the answer in one pass, though it introduces more overhead than a simple array for just 26 chars.

use std::collections::HashMap;

/// Brute force approach: Check every character against every other character.
/// Time: O(n²) - nested loops check for duplicates.
/// Space: O(1) - no extra space needed.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
pub fn first_unique_char_brute_force(s: String) -> i32 {
    let chars: Vec<char> = s.chars().collect();

    // RUST INSIGHT: `.chars()` gives an iterator over Unicode scalar values.
    // Collecting to a Vec allows random access, but costs O(n) space and allocation.

    for i in 0..chars.len() {
        let mut is_unique = true;
        for j in 0..chars.len() {
            if i != j && chars[i] == chars[j] {
                is_unique = false;
                break;
            }
        }
        if is_unique {
            return i32::try_from(i).unwrap_or(-1);
        }
    }

    -1
}

/// Optimized approach: Hash map (two passes).
/// Time: O(n) - two passes through the string.
/// Space: O(n) - hash map stores character counts (bounded by alphabet size, so arguably O(1)).
#[must_use]
#[allow(clippy::needless_pass_by_value)]
#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
pub fn first_unique_char_optimized(s: String) -> i32 {
    let mut map = HashMap::new();

    // First pass: Build the hash map of frequencies
    for c in s.chars() {
        *map.entry(c).or_insert(0) += 1;
    }

    // Second pass: Find the first character with frequency 1
    for (i, c) in s.chars().enumerate() {
        // GOTCHA: `s.chars().enumerate()` yields character indices (0, 1, 2...),
        // which correspond to the character's logical position, not its byte offset.
        // For ASCII, these are the same.
        if map.get(&c) == Some(&1) {
            return i32::try_from(i).unwrap_or(-1);
        }
    }

    -1
}

/// Optimal approach: Array-based counting for ASCII lowercase letters.
/// Time: O(n) - two passes through the byte array.
/// Space: O(1) - fixed size array of 26 elements.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
pub fn first_unique_char_optimal(s: String) -> i32 {
    // We use a fixed-size array since we know the input is only lowercase English letters.
    let mut counts = [0_usize; 26];
    let bytes = s.as_bytes();

    // RUST INSIGHT: By using `s.as_bytes()`, we iterate over `u8` bytes directly.
    // This bypasses UTF-8 decoding validation, which is perfectly safe and faster here
    // because LeetCode guarantees ASCII lowercase letters only.

    for &b in bytes {
        counts[usize::from(b - b'a')] += 1;
    }

    for (i, &b) in bytes.iter().enumerate() {
        if counts[usize::from(b - b'a')] == 1 {
            return i32::try_from(i).unwrap_or(-1);
        }
    }

    -1
}

/// Main entry point - uses optimal solution
#[must_use]
pub fn first_unique_char(s: String) -> i32 {
    first_unique_char_optimal(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force() {
        assert_eq!(first_unique_char_brute_force(String::from("leetcode")), 0);
        assert_eq!(
            first_unique_char_brute_force(String::from("loveleetcode")),
            2
        );
        assert_eq!(first_unique_char_brute_force(String::from("aabb")), -1);
    }

    #[test]
    fn test_optimized() {
        assert_eq!(first_unique_char_optimized(String::from("leetcode")), 0);
        assert_eq!(first_unique_char_optimized(String::from("loveleetcode")), 2);
        assert_eq!(first_unique_char_optimized(String::from("aabb")), -1);
    }

    #[test]
    fn test_optimal() {
        assert_eq!(first_unique_char_optimal(String::from("leetcode")), 0);
        assert_eq!(first_unique_char_optimal(String::from("loveleetcode")), 2);
        assert_eq!(first_unique_char_optimal(String::from("aabb")), -1);
    }

    #[test]
    fn test_edge_cases() {
        // Single character
        assert_eq!(first_unique_char(String::from("z")), 0);
        // All identical
        assert_eq!(first_unique_char(String::from("zzzzz")), -1);
        // Unique character at the very end
        assert_eq!(first_unique_char(String::from("aabbc")), 4);
    }

    #[test]
    fn test_stress() {
        // Create a large string where only the last character is unique
        let mut s = String::with_capacity(100_001);
        for _ in 0..50_000 {
            s.push('a');
            s.push('b');
        }
        s.push('c');

        assert_eq!(first_unique_char(s), 100_000);
    }
}
