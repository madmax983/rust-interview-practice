//! # 3. Longest Substring Without Repeating Characters
//!
//! Given a string `s`, find the length of the longest substring without repeating characters.
//!
//! ## Examples
//!
//! ```
//! use leetcode::strings::longest_substring_without_repeating::length_of_longest_substring;
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

    let chars: Vec<char> = s.chars().collect();
    let n = chars.len();
    let mut max_len = 0;

    // Check every possible substring
    for i in 0..n {
        for j in (i + 1)..=n {
            let substring = &chars[i..j];
            let mut seen = HashSet::new();
            let mut is_unique = true;

            for &ch in substring {
                if !seen.insert(ch) {
                    is_unique = false;
                    break;
                }
            }

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

    let chars: Vec<char> = s.chars().collect();
    let mut seen = HashSet::new();
    let mut left = 0;
    let mut max_len = 0;

    for right in 0..chars.len() {
        // Shrink window from left while we have a duplicate
        while seen.contains(&chars[right]) {
            seen.remove(&chars[left]);
            left += 1;
        }

        // Add current character to window
        seen.insert(chars[right]);

        // Update max length
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
    use std::collections::HashMap;

    let chars: Vec<char> = s.chars().collect();
    let mut char_index = HashMap::new();
    let mut left = 0;
    let mut max_len = 0;

    #[allow(clippy::needless_range_loop)] // Index needed for multiple operations
    for right in 0..chars.len() {
        let ch = chars[right];

        // If we've seen this character in current window, jump left pointer
        if let Some(&prev_index) = char_index.get(&ch) {
            left = left.max(prev_index + 1);
        }

        // Update the character's latest position
        char_index.insert(ch, right);

        // Update max length
        max_len = max_len.max(right - left + 1);
    }

    max_len as i32
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
