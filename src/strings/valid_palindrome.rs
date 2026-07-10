//! # 125. Valid Palindrome
//!
//! Difficulty: Easy
//! Link: <https://leetcode.com/problems/valid-palindrome/>
//!
//! A phrase is a palindrome if, after converting all uppercase letters into lowercase letters
//! and removing all non-alphanumeric characters, it reads the same forward and backward.
//! Alphanumeric characters include letters and numbers.
//!
//! Given a string `s`, return `true` if it is a palindrome, or `false` otherwise.
//!
//! This problem demonstrates string manipulation, iterator adapters, and two-pointer
//! techniques. It perfectly showcases how Rust's zero-cost abstractions (`Iterator`) can match
//! or beat the performance of manual index manipulation while remaining far more readable.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::strings::valid_palindrome::is_palindrome;
//!
//! assert_eq!(is_palindrome("A man, a plan, a canal: Panama".to_string()), true);
//! assert_eq!(is_palindrome("race a car".to_string()), false);
//! assert_eq!(is_palindrome(" ".to_string()), true);
//! ```
//!
//! ## Constraints
//!
//! - `1 <= s.length <= 2 * 10^5`
//! - `s` consists only of printable ASCII characters.

/// Brute force approach: Filter, collect, and reverse.
///
/// Time: O(n) - Iterates through the string twice (once to collect, once to reverse/compare)
/// Space: O(n) - Allocates a new `String` for the cleaned characters
///
/// This approach is idiomatic for quick scripting but allocates memory unnecessarily.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature requires taking ownership of String
pub fn is_palindrome_brute_force(s: String) -> bool {
    // RUST INSIGHT: .chars() returns an iterator over Unicode scalar values.
    // Even though the constraints say ASCII only, idiomatic string iteration in Rust uses chars().
    let cleaned: String = s
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect(); // Allocates a new String

    // GOTCHA: You cannot just do `cleaned == cleaned.chars().rev().collect::<String>()`
    // without another allocation. Here, we create an iterator over the cleaned string,
    // and compare it to its reversed self.
    let forward = cleaned.chars();
    let backward = cleaned.chars().rev();

    forward.eq(backward)
}

/// Optimized approach: Iterator adapters without allocation.
/// Time: O(n) - Single pass from both ends towards the middle
/// Space: O(1) - No allocations, purely evaluated lazily
///
/// This approach shines in Rust. We build a processing pipeline (filter + map)
/// and then rely on the `DoubleEndedIterator` trait to compare elements from both
/// ends simultaneously. This is a zero-cost abstraction!
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_palindrome_optimized(s: String) -> bool {
    // Create an iterator that lazily yields cleaned lowercase characters.
    let iter = s
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase());

    // RUST INSIGHT: Because our iterator pipeline implements `Clone` and `DoubleEndedIterator`,
    // we can clone the iterator state and reverse one copy. `eq` will pull from both
    // iterators lazily and short-circuit on the first mismatch.
    iter.clone().eq(iter.rev())
}

/// Optimal approach: Two pointers operating directly on bytes.
/// Time: O(n) - Single pass
/// Space: O(1) - No allocations
///
/// Since `LeetCode` guarantees the string contains only printable ASCII characters,
/// we can bypass UTF-8 decoding overhead entirely and work directly on bytes (`u8`).
/// This is the absolute fastest approach, common in C/C++, but written safely in Rust.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_palindrome_optimal(s: String) -> bool {
    // RUST INSIGHT: `as_bytes()` is an O(1) operation. We are viewing the String's
    // internal buffer directly.
    let bytes = s.as_bytes();

    // Handle empty string or single character cases
    if bytes.is_empty() {
        return true;
    }

    let mut left = 0;
    // Using saturating_sub prevents underflow if length was somehow 0 (already handled above)
    let mut right = bytes.len().saturating_sub(1);

    while left < right {
        // Move left pointer to next alphanumeric byte
        if !bytes[left].is_ascii_alphanumeric() {
            left += 1;
        }
        // Move right pointer to next alphanumeric byte
        else if !bytes[right].is_ascii_alphanumeric() {
            right -= 1;
        }
        // Compare the lowercase versions of both bytes
        else if !bytes[left].eq_ignore_ascii_case(&bytes[right]) {
            return false;
        }
        // Characters matched, move both pointers inward
        else {
            left += 1;
            right -= 1;
        }
    }

    true
}

/// Main entry point - uses optimal solution
#[must_use]
pub fn is_palindrome(s: String) -> bool {
    is_palindrome_optimal(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Happy Path tests
    #[test]
    fn test_valid_palindrome() {
        let input = "A man, a plan, a canal: Panama".to_string();
        assert!(is_palindrome_brute_force(input.clone()));
        assert!(is_palindrome_optimized(input.clone()));
        assert!(is_palindrome_optimal(input));
    }

    #[test]
    fn test_invalid_palindrome() {
        let input = "race a car".to_string();
        assert!(!is_palindrome_brute_force(input.clone()));
        assert!(!is_palindrome_optimized(input.clone()));
        assert!(!is_palindrome_optimal(input));
    }

    // Edge Case tests
    #[test]
    fn test_empty_string() {
        let input = String::new();
        assert!(is_palindrome_brute_force(input.clone()));
        assert!(is_palindrome_optimized(input.clone()));
        assert!(is_palindrome_optimal(input));
    }

    #[test]
    fn test_whitespace_only() {
        let input = "   ".to_string();
        assert!(is_palindrome_brute_force(input.clone()));
        assert!(is_palindrome_optimized(input.clone()));
        assert!(is_palindrome_optimal(input));
    }

    #[test]
    fn test_single_character() {
        let input = "a".to_string();
        assert!(is_palindrome_brute_force(input.clone()));
        assert!(is_palindrome_optimized(input.clone()));
        assert!(is_palindrome_optimal(input));
    }

    #[test]
    fn test_single_non_alphanumeric() {
        let input = ".".to_string();
        assert!(is_palindrome_brute_force(input.clone()));
        assert!(is_palindrome_optimized(input.clone()));
        assert!(is_palindrome_optimal(input));
    }

    // Stress/Boundary tests
    #[test]
    fn test_numeric_palindrome() {
        let input = "12321".to_string();
        assert!(is_palindrome_brute_force(input.clone()));
        assert!(is_palindrome_optimized(input.clone()));
        assert!(is_palindrome_optimal(input));
    }

    #[test]
    fn test_numeric_non_palindrome() {
        let input = "12345".to_string();
        assert!(!is_palindrome_brute_force(input.clone()));
        assert!(!is_palindrome_optimized(input.clone()));
        assert!(!is_palindrome_optimal(input));
    }
}
