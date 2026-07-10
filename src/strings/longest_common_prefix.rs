//! # 14. Longest Common Prefix
//!
//! Difficulty: Easy
//! Link: <https://leetcode.com/problems/longest-common-prefix/>
//!
//! Write a function to find the longest common prefix string amongst an array of strings.
//! If there is no common prefix, return an empty string `""`.
//!
//! This problem matters in Rust because it teaches you how to work safely with string slices (`&str`),
//! byte representations (`&[u8]`), and iterator combinations. It demonstrates why O(1) byte-level
//! indexing using `.as_bytes()` is powerful when we can guarantee ASCII bounds, and how `zip` can be
//! used to safely compare iterators.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::strings::longest_common_prefix::longest_common_prefix;
//!
//! assert_eq!(
//!     longest_common_prefix(vec!["flower".to_string(), "flow".to_string(), "flight".to_string()]),
//!     "fl".to_string()
//! );
//! assert_eq!(
//!     longest_common_prefix(vec!["dog".to_string(), "racecar".to_string(), "car".to_string()]),
//!     "".to_string()
//! );
//! ```
//!
//! ## Constraints
//!
//! - `1 <= strs.length <= 200`
//! - `0 <= strs[i].length <= 200`
//! - `strs[i]` consists of only lowercase English letters.

/// Optimized approach: Vertical Scanning.
/// Time: O(S) where S is the sum of all characters in all strings.
/// Space: O(1) since we are just borrowing bytes.
///
/// We iterate through the characters of the first string, and for each character,
/// we check if every other string has the same character at the same position.
/// This is linear in the total input size, faster than the sorting brute force.
///
/// # Panics
///
/// Does not panic: the non-empty case is guarded, so moving the first element out of
/// the `Vec` always succeeds.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn longest_common_prefix_optimized(strs: Vec<String>) -> String {
    if strs.is_empty() {
        return String::new();
    }

    let first = strs[0].as_bytes();

    // RUST INSIGHT: Working with `as_bytes()` allows O(1) indexing.
    // This is safe because the problem guarantees lowercase English letters (ASCII).
    for i in 0..first.len() {
        let ch = first[i];

        // Check this character against all other strings
        for s in strs.iter().skip(1) {
            let s_bytes = s.as_bytes();
            // GOTCHA: We must check if `i` is out of bounds for the current string!
            // If it is, or if the character doesn't match, we found the max prefix length.
            if i >= s_bytes.len() || s_bytes[i] != ch {
                // Return a slice of the first string up to `i`
                return strs[0][0..i].to_string();
            }
        }
    }

    // ⚡ BOLT OPTIMIZATION:
    // If we reach here, the first string is the common prefix.
    // Instead of cloning it (`strs[0].clone()`), we consume the `Vec` and move the first element
    // out of it. This is a zero-cost abstraction that eliminates a heap allocation.
    strs.into_iter().next().unwrap()
}

/// Brute force approach: Sorting and comparing extremes.
/// Time: O(S * log N) where N is the number of strings and S is max string length.
/// Space: O(1) auxiliary space beyond the sorted input array.
///
/// By sorting the array lexicographically, the strings that are most different
/// will end up at the first and last positions. We then only need to compare
/// the first and last strings to find the common prefix. The sort dominates,
/// making this the slowest of the three approaches (the extra log N factor).
///
/// # Panics
///
/// Does not panic: the empty case is guarded, so `first()` and `last()` always return
/// `Some` after the early return.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn longest_common_prefix_brute_force(mut strs: Vec<String>) -> String {
    if strs.is_empty() {
        return String::new();
    }

    // Sorting strings in Rust is lexicographical by default.
    strs.sort_unstable();

    let first = strs.first().unwrap().as_bytes();
    let last = strs.last().unwrap().as_bytes();

    let mut i = 0;
    while i < first.len() && i < last.len() && first[i] == last[i] {
        i += 1;
    }

    // We can safely create a String from the valid slice
    strs[0][0..i].to_string()
}

/// Optimal approach: Iterator folding with `zip` in-place.
/// Time: O(S) where S is the sum of all characters in all strings.
/// Space: O(1) auxiliary space beyond the input (which is consumed).
///
/// ⚡ BOLT OPTIMIZATION:
/// We consume the input `Vec<String>` using `.into_iter()`. We take ownership
/// of the first `String` and modify it in-place using `.truncate()`. This
/// eliminates the need to allocate a brand new `String` on the heap at the end
/// and avoids any intermediate heap allocations, representing a zero-cost abstraction.
///
/// This approach uses idiomatic Rust iterators. We start with the first string
/// as our initial "prefix". Then we iterate over the rest of the strings, updating
/// the prefix by safely comparing byte by byte using `zip`.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn longest_common_prefix_optimal(strs: Vec<String>) -> String {
    let mut iter = strs.into_iter();
    let Some(mut prefix) = iter.next() else {
        return String::new();
    };

    for s in iter {
        // Find how many bytes match between the accumulator and the current string
        let match_len = prefix
            .bytes()
            .zip(s.bytes())
            .take_while(|(a, b)| a == b)
            .count();

        // RUST INSIGHT: `truncate` is an O(1) operation on a `String` since it
        // just modifies the internal length property without freeing capacity.
        // We ensure we only truncate at valid ASCII boundaries because `match_len`
        // is determined by matching byte values in ASCII strings.
        prefix.truncate(match_len);

        if prefix.is_empty() {
            break;
        }
    }

    prefix
}

/// Main entry point - uses the optimal iterator folding approach.
#[must_use]
pub fn longest_common_prefix(strs: Vec<String>) -> String {
    longest_common_prefix_optimal(strs)
}

// ============================================================================
// Alternative Approaches
// ============================================================================
// 1. Binary Search: We could binary search the length of the prefix. This might
//    be useful if the strings are very long and checking the prefix is fast, but
//    usually over-complicates the solution here. Time: O(S * log M) where M is min length.
// 2. Trie: We could insert all strings into a Trie, then walk down until a node
//    has > 1 child or is the end of a word. Space intensive but good if queries are frequent.

#[cfg(test)]
mod tests {
    use super::*;

    // Happy Path
    #[test]
    fn test_happy_path() {
        let input = vec![
            "flower".to_string(),
            "flow".to_string(),
            "flight".to_string(),
        ];
        let expected = "fl".to_string();

        assert_eq!(longest_common_prefix_brute_force(input.clone()), expected);
        assert_eq!(longest_common_prefix_optimized(input.clone()), expected);
        assert_eq!(longest_common_prefix_optimal(input.clone()), expected);
        assert_eq!(longest_common_prefix(input), expected);
    }

    // No common prefix
    #[test]
    fn test_no_common_prefix() {
        let input = vec!["dog".to_string(), "racecar".to_string(), "car".to_string()];
        let expected = String::new();

        assert_eq!(longest_common_prefix_brute_force(input.clone()), expected);
        assert_eq!(longest_common_prefix_optimized(input.clone()), expected);
        assert_eq!(longest_common_prefix_optimal(input.clone()), expected);
        assert_eq!(longest_common_prefix(input), expected);
    }

    // Edge Cases
    #[test]
    fn test_empty_array() {
        let input: Vec<String> = vec![];
        let expected = String::new();

        assert_eq!(longest_common_prefix_brute_force(input.clone()), expected);
        assert_eq!(longest_common_prefix_optimized(input.clone()), expected);
        assert_eq!(longest_common_prefix_optimal(input.clone()), expected);
        assert_eq!(longest_common_prefix(input), expected);
    }

    #[test]
    fn test_single_string() {
        let input = vec!["hello".to_string()];
        let expected = "hello".to_string();

        assert_eq!(longest_common_prefix_brute_force(input.clone()), expected);
        assert_eq!(longest_common_prefix_optimized(input.clone()), expected);
        assert_eq!(longest_common_prefix_optimal(input.clone()), expected);
        assert_eq!(longest_common_prefix(input), expected);
    }

    #[test]
    fn test_with_empty_string() {
        let input = vec!["flower".to_string(), String::new(), "flight".to_string()];
        let expected = String::new();

        assert_eq!(longest_common_prefix_brute_force(input.clone()), expected);
        assert_eq!(longest_common_prefix_optimized(input.clone()), expected);
        assert_eq!(longest_common_prefix_optimal(input.clone()), expected);
        assert_eq!(longest_common_prefix(input), expected);
    }

    // Stress / Boundary Cases
    #[test]
    fn test_all_same_characters() {
        let input = vec!["a".repeat(200), "a".repeat(150), "a".repeat(200)];
        let expected = "a".repeat(150);

        assert_eq!(longest_common_prefix_brute_force(input.clone()), expected);
        assert_eq!(longest_common_prefix_optimized(input.clone()), expected);
        assert_eq!(longest_common_prefix_optimal(input.clone()), expected);
        assert_eq!(longest_common_prefix(input), expected);
    }
}
