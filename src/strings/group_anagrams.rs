//! # 49. Group Anagrams
//!
//! Given an array of strings `strs`, group the anagrams together. You can return the answer in any order.
//!
//! An Anagram is a word or phrase formed by rearranging the letters of a different word or phrase,
//! typically using all the original letters exactly once.
//!
//! ## Why this matters in Rust
//! This problem perfectly demonstrates ownership and the power of the `Entry` API in `HashMap`.
//! Unlike languages where strings are reference types, Rust's `String` owns its data. Grouping them
//! requires thinking about whether to clone keys, move values, or use references. It also highlights
//! how to use complex types (like `Vec<char>` or `[u8; 26]`) as `HashMap` keys.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::strings::group_anagrams::group_anagrams;
//!
//! let input = vec![
//!     "eat".to_string(), "tea".to_string(), "tan".to_string(),
//!     "ate".to_string(), "nat".to_string(), "bat".to_string()
//! ];
//! let mut result = group_anagrams(input);
//!
//! // Sort inner vectors and outer vector for consistent comparison
//! for group in &mut result {
//!     group.sort();
//! }
//! result.sort_by(|a, b| a[0].cmp(&b[0]));
//!
//! assert_eq!(result, vec![
//!     vec!["ate".to_string(), "eat".to_string(), "tea".to_string()],
//!     vec!["bat".to_string()],
//!     vec!["nat".to_string(), "tan".to_string()]
//! ]);
//! ```
//!
//! ## Constraints
//!
//! - 1 <= strs.length <= 10^4
//! - 0 <= strs[i].length <= 100
//! - strs[i] consists of lowercase English letters.

use std::collections::HashMap;

/// Brute force approach: Sort each string to use as a key.
/// Time: O(N * K * log K) where N is the number of strings and K is the max length of a string.
/// Space: O(N * K) to store the hash map.
///
/// This is the most intuitive approach. Two strings are anagrams if and only if their sorted characters are identical.
/// The per-string sort adds a `log K` factor, making it slower than the frequency-count approach.
/// It does, however, handle arbitrary Unicode input correctly (see `group_anagrams_optimal` for the ASCII-only fast path).
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
pub fn group_anagrams_brute_force(strs: Vec<String>) -> Vec<Vec<String>> {
    // RUST INSIGHT: `HashMap` ownership.
    // We need to store the strings in groups. The key is the sorted version (temporary),
    // and the value is a vector of original strings. The HashMap owns the keys and the values.
    // BOLT OPTIMIZATION: Pre-allocate capacity to avoid reallocations.
    // In the worst case (all unique strings), we need `strs.len()` capacity.
    let mut map: HashMap<Vec<char>, Vec<String>> = HashMap::with_capacity(strs.len());

    for s in strs {
        // Create the key by sorting characters
        // GOTCHA: `.chars()` iterates over Unicode Scalar Values.
        // Sorting `Vec<char>` handles Unicode correctly, unlike sorting bytes of a UTF-8 string directly.
        let mut key: Vec<char> = s.chars().collect();
        key.sort_unstable(); // `sort_unstable` is generally faster than `sort` and sufficient here

        // RUST INSIGHT: The `Entry` API.
        // `entry(key)` handles the lookup. `or_default()` inserts an empty Vec if the key is missing.
        // This avoids the double lookup of `if map.contains_key(...) { map.get_mut(...) } else { map.insert(...) }`.
        map.entry(key).or_default().push(s);
    }

    // Convert the values (groups) into the result vector
    map.into_values().collect()
}

/// Optimal approach: Frequency Count (for lowercase English letters).
/// Time: O(N * K) - we iterate over each character of each string once, no sorting.
/// Space: O(N * K) - map storage.
///
/// Instead of sorting, we count the frequency of each character 'a' through 'z'.
/// The count array `[u8; 26]` serves as the hash map key. This drops the `log K`
/// factor of the sorting brute force, making it the fastest approach.
///
/// NOTE: This assumes ASCII lowercase input (per the problem constraints). It will
/// panic on non-ASCII input; use `group_anagrams_brute_force` for arbitrary Unicode.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn group_anagrams_optimal(strs: Vec<String>) -> Vec<Vec<String>> {
    // Key is an array of 26 counts. `[u8; 26]` implements `Hash` and `Eq` automatically.
    // BOLT OPTIMIZATION: Pre-allocate capacity to avoid reallocations.
    // In the worst case (all unique strings), we need `strs.len()` capacity.
    let mut map: HashMap<[u8; 26], Vec<String>> = HashMap::with_capacity(strs.len());

    for s in strs {
        let mut count = [0u8; 26];

        // BOLT OPTIMIZATION: Iterate over the bytes directly via `.as_bytes()`.
        // This is a zero-cost abstraction that skips the overhead of the `.bytes()` iterator
        // adapter, which can be measurably faster in hot loops.
        for &byte in s.as_bytes() {
            // RUST INSIGHT: Byte-level iteration.
            // Since constraints guarantee lowercase English letters (ASCII), `s.as_bytes()` is efficient.
            // We subtract b'a' to map 'a'..='z' to 0..=25.
            count[(byte - b'a') as usize] += 1;
        }

        map.entry(count).or_default().push(s);
    }

    map.into_values().collect()
}

/// Main entry point - uses the optimal frequency-count approach.
///
/// Note: this assumes ASCII lowercase input per the `LeetCode` constraints. For arbitrary
/// Unicode input, call `group_anagrams_brute_force` (the sorting approach) directly.
#[must_use]
pub fn group_anagrams(strs: Vec<String>) -> Vec<Vec<String>> {
    group_anagrams_optimal(strs)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to sort results for deterministic comparison
    fn normalize(mut groups: Vec<Vec<String>>) -> Vec<Vec<String>> {
        for group in &mut groups {
            group.sort();
        }
        groups.sort_by(|a, b| a[0].cmp(&b[0])); // Sort by first element of each group
        groups
    }

    #[test]
    fn test_group_anagrams_basic() {
        let input = vec![
            "eat".to_string(),
            "tea".to_string(),
            "tan".to_string(),
            "ate".to_string(),
            "nat".to_string(),
            "bat".to_string(),
        ];
        let expected = vec![
            vec!["ate".to_string(), "eat".to_string(), "tea".to_string()],
            vec!["bat".to_string()],
            vec!["nat".to_string(), "tan".to_string()],
        ];

        let result_sort = normalize(group_anagrams_brute_force(input.clone()));
        let result_freq = normalize(group_anagrams_optimal(input.clone()));
        let result_main = normalize(group_anagrams(input)); // Should use the optimal frequency approach internally

        // We compare normalized results because the order of groups and order within groups is not guaranteed
        // by the problem statement, but our `normalize` helper enforces a canonical order for testing.
        // Note: `expected` must be pre-sorted according to the same logic as `normalize`.
        // The expected vector above IS sorted: "ate" < "bat" < "nat".

        assert_eq!(result_sort, expected);
        assert_eq!(result_freq, expected);
        assert_eq!(result_main, expected);
    }

    #[test]
    fn test_group_anagrams_empty() {
        let input: Vec<String> = vec![];
        let result = group_anagrams(input);
        assert!(result.is_empty());
    }

    #[test]
    fn test_group_anagrams_single_empty_string() {
        let input = vec![String::new()];
        let expected = vec![vec![String::new()]];

        let result = group_anagrams(input);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_group_anagrams_no_anagrams() {
        let input = vec!["abc".to_string(), "def".to_string(), "ghi".to_string()];
        let expected = vec![
            vec!["abc".to_string()],
            vec!["def".to_string()],
            vec!["ghi".to_string()],
        ];

        let result = normalize(group_anagrams(input));
        // Expected must be sorted for comparison
        let expected_sorted = normalize(expected);

        assert_eq!(result, expected_sorted);
    }

    #[test]
    fn test_group_anagrams_unicode() {
        // The optimal frequency method would panic here (non-ASCII bytes).
        // We test `group_anagrams_brute_force` (the sorting approach) directly,
        // which is the Unicode-safe implementation.
        let input = vec!["café".to_string(), "féac".to_string()];
        let expected = vec![vec!["café".to_string(), "féac".to_string()]];

        let result = normalize(group_anagrams_brute_force(input));
        let expected_sorted = normalize(expected);

        assert_eq!(result, expected_sorted);
    }
}
