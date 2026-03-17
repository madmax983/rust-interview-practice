//! # 3. Longest Substring Without Repeating Characters
//!
//! Given a string `s`, find the length of the longest substring without repeating characters.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::strings::longest_substring_without_repeating::length_of_longest_substring;
//!
//! assert_eq!(length_of_longest_substring("abcabcbb".to_string()), 3);
//! assert_eq!(length_of_longest_substring("bbbbb".to_string()), 1);
//! assert_eq!(length_of_longest_substring("pwwkew".to_string()), 3);
//! ```
//!
//! ## Constraints
//!
//! - 0 <= s.length <= 5 * 10^4
//! - s consists of English letters, digits, symbols and spaces.

/// Brute force approach: Check all substrings.
/// Time: O(n³) - O(n²) substrings × O(n) uniqueness check
/// Space: O(min(n, m)) where m is charset size
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
#[allow(clippy::cast_possible_truncation)] // LeetCode constraints guarantee it fits
#[allow(clippy::cast_possible_wrap)]
pub fn length_of_longest_substring_brute_force(s: String) -> i32 {
    use std::collections::HashSet;

    // Convert string to vec for O(1) indexing
    let chars: Vec<char> = s.chars().collect();
    let n = chars.len();
    let mut max_len = 0; // Track the longest unique substring found

    // Strategy: Check every possible substring
    for i in 0..n {
        // Try all substrings starting at position i
        for j in (i + 1)..=n {
            let substring = &chars[i..j]; // Get substring from i to j
            let mut seen = HashSet::new(); // Track characters in this substring
            let mut is_unique = true;

            // Check if all characters in substring are unique
            for &ch in substring {
                if !seen.insert(ch) {
                    // insert() returns false if char was already present
                    is_unique = false;
                    break;
                }
            }

            // If substring has all unique chars, update max length
            if is_unique {
                max_len = max_len.max(substring.len());
            }
        }
    }

    max_len as i32
}

/// Optimized approach: Sliding window with `HashSet`.
/// Time: O(n) - each character visited at most twice
/// Space: O(min(n, m)) where m is charset size
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
#[allow(clippy::cast_possible_truncation)] // LeetCode constraints guarantee it fits
#[allow(clippy::cast_possible_wrap)]
pub fn length_of_longest_substring_optimized(s: String) -> i32 {
    use std::collections::HashSet;

    // Convert to vec for indexing
    let chars: Vec<char> = s.chars().collect();
    let mut seen = HashSet::new(); // Track characters in current window
    let mut left = 0; // Left boundary of sliding window
    let mut max_len = 0; // Best result so far

    // Strategy: Sliding window - expand right, shrink left when needed
    for right in 0..chars.len() {
        // Right pointer always moves forward

        // If current char creates a duplicate, shrink window from left
        while seen.contains(&chars[right]) {
            seen.remove(&chars[left]); // Remove leftmost character
            left += 1; // Move left boundary right
        }

        // Now window [left..=right] has all unique characters
        seen.insert(chars[right]); // Add current char to window

        // Update max length seen (window size is right - left + 1)
        max_len = max_len.max(right - left + 1);
    }

    max_len as i32
}

/// Optimal approach: Sliding window with `HashMap` to store character indices.
///
/// Allows skipping directly to the position after the duplicate.
/// Time: O(n) - each character visited exactly once
/// Space: O(min(n, m)) where m is charset size
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
#[allow(clippy::cast_possible_truncation)] // LeetCode constraints guarantee it fits
#[allow(clippy::cast_possible_wrap)]
pub fn length_of_longest_substring_optimal(s: String) -> i32 {
    // RUST INSIGHT: Since the problem constraints specify English letters, digits,
    // symbols and spaces, we are dealing with ASCII characters. We can use a
    // flat array of size 128 instead of a HashMap, and operate on bytes
    // directly avoiding a Vec<char> allocation and UTF-8 decoding overhead.
    // We use a size of 256 to ensure we don't panic on non-ASCII characters.
    let mut char_index = [-1i32; 256]; // Track: character -> last seen index
    let mut left = 0i32; // Left boundary of sliding window
    let mut max_len = 0i32; // Best result so far

    // Strategy: Sliding window with smart jumping
    // Instead of incrementing left by 1, we jump directly past duplicates
    for (right, &b) in s.as_bytes().iter().enumerate() {
        let ch = b as usize;
        let prev_index = char_index[ch];

        // Jump left boundary past the duplicate (but never move left backward)
        left = left.max(prev_index + 1);

        // Always update this character's latest position
        char_index[ch] = right as i32;

        // Calculate current window size and update max
        max_len = max_len.max(right as i32 - left + 1);
    }

    max_len
}

/// Main entry point - uses optimal solution
#[must_use]
pub fn length_of_longest_substring(s: String) -> i32 {
    length_of_longest_substring_optimal(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test cases for brute force approach
    #[test]
    fn test_brute_force_example_1() {
        assert_eq!(
            length_of_longest_substring_brute_force("abcabcbb".to_string()),
            3
        );
    }

    #[test]
    fn test_brute_force_example_2() {
        assert_eq!(
            length_of_longest_substring_brute_force("bbbbb".to_string()),
            1
        );
    }

    #[test]
    fn test_brute_force_example_3() {
        assert_eq!(
            length_of_longest_substring_brute_force("pwwkew".to_string()),
            3
        );
    }

    #[test]
    fn test_brute_force_empty() {
        assert_eq!(length_of_longest_substring_brute_force("".to_string()), 0);
    }

    // Test cases for optimized approach
    #[test]
    fn test_optimized_example_1() {
        assert_eq!(
            length_of_longest_substring_optimized("abcabcbb".to_string()),
            3
        );
    }

    #[test]
    fn test_optimized_example_2() {
        assert_eq!(
            length_of_longest_substring_optimized("bbbbb".to_string()),
            1
        );
    }

    #[test]
    fn test_optimized_example_3() {
        assert_eq!(
            length_of_longest_substring_optimized("pwwkew".to_string()),
            3
        );
    }

    #[test]
    fn test_optimized_empty() {
        assert_eq!(length_of_longest_substring_optimized("".to_string()), 0);
    }

    // Test cases for optimal approach
    #[test]
    fn test_optimal_example_1() {
        assert_eq!(
            length_of_longest_substring_optimal("abcabcbb".to_string()),
            3
        );
    }

    #[test]
    fn test_optimal_example_2() {
        assert_eq!(length_of_longest_substring_optimal("bbbbb".to_string()), 1);
    }

    #[test]
    fn test_optimal_example_3() {
        assert_eq!(length_of_longest_substring_optimal("pwwkew".to_string()), 3);
    }

    #[test]
    fn test_optimal_empty() {
        assert_eq!(length_of_longest_substring_optimal("".to_string()), 0);
    }

    // Test all approaches with edge cases
    #[test]
    fn test_all_approaches_single_char() {
        let input = "a".to_string();
        assert_eq!(length_of_longest_substring_brute_force(input.clone()), 1);
        assert_eq!(length_of_longest_substring_optimized(input.clone()), 1);
        assert_eq!(length_of_longest_substring_optimal(input.clone()), 1);
    }

    #[test]
    fn test_all_approaches_all_unique() {
        let input = "abcdef".to_string();
        assert_eq!(length_of_longest_substring_brute_force(input.clone()), 6);
        assert_eq!(length_of_longest_substring_optimized(input.clone()), 6);
        assert_eq!(length_of_longest_substring_optimal(input.clone()), 6);
    }

    #[test]
    fn test_all_approaches_with_spaces() {
        let input = "a b".to_string();
        assert_eq!(length_of_longest_substring_brute_force(input.clone()), 3);
        assert_eq!(length_of_longest_substring_optimized(input.clone()), 3);
        assert_eq!(length_of_longest_substring_optimal(input.clone()), 3);
    }

    // Main function tests (uses optimal)
    #[test]
    fn test_example_1() {
        assert_eq!(length_of_longest_substring("abcabcbb".to_string()), 3);
    }

    #[test]
    fn test_example_2() {
        assert_eq!(length_of_longest_substring("bbbbb".to_string()), 1);
    }

    #[test]
    fn test_example_3() {
        assert_eq!(length_of_longest_substring("pwwkew".to_string()), 3);
    }
}
