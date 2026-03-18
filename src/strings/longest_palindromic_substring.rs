//! # 5. Longest Palindromic Substring
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/longest-palindromic-substring/>
//!
//! Given a string `s`, return the longest palindromic substring in `s`.
//!
//! This problem is an excellent fit for teaching Rust's string representation. It highlights
//! the critical difference between byte-level indexing (`&[u8]`) and character-level iteration (`char`),
//! proving why `s.chars().nth(i)` is O(n) and why direct string indexing `s[i]` doesn't work out-of-the-box
//! without slicing carefully on UTF-8 boundaries.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::strings::longest_palindromic_substring::longest_palindrome;
//!
//! assert_eq!(longest_palindrome("babad".to_string()), "bab".to_string()); // "aba" is also valid
//! assert_eq!(longest_palindrome("cbbd".to_string()), "bb".to_string());
//! ```
//!
//! ## Constraints
//!
//! - `1 <= s.length <= 1000`
//! - `s` consist of only digits and English letters.

/// Brute force approach: Check all possible substrings.
///
/// We iterate over all possible starting and ending indices, slice the string,
/// and check if it's a palindrome.
///
/// Time: O(n^3) - O(n^2) pairs of indices, and O(n) to check each substring.
/// Space: O(1) - checking is done in place without extra allocations.
///
/// # Gotcha
/// `.chars().nth(i)` is an O(n) operation on Rust strings because strings are UTF-8
/// encoded. Using it inside a loop makes the loop O(n^2) by itself.
/// However, the problem specifies `s` consists of only digits and English letters,
/// which means all characters are exactly 1 byte (ASCII). Therefore, we can safely
/// convert the string to a byte slice `&[u8]` for O(1) indexing!
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn longest_palindrome_brute_force(s: String) -> String {
    if s.is_empty() {
        return String::new();
    }

    let bytes = s.as_bytes();
    let n = bytes.len();
    let mut max_start = 0;
    let mut max_len = 1;

    for i in 0..n {
        for j in i..n {
            let len = j - i + 1;
            if len > max_len && is_palindrome(bytes, i, j) {
                max_start = i;
                max_len = len;
            }
        }
    }

    // RUST INSIGHT: Since we only sliced at ASCII character boundaries,
    // this string slice conversion is guaranteed to be valid UTF-8.
    s[max_start..max_start + max_len].to_string()
}

fn is_palindrome(bytes: &[u8], mut left: usize, mut right: usize) -> bool {
    while left < right {
        if bytes[left] != bytes[right] {
            return false;
        }
        left += 1;
        right -= 1;
    }
    true
}

/// Optimized approach: Expand Around Center.
///
/// Instead of checking every substring, we treat every character (and the spaces
/// between characters) as a potential center of a palindrome, expanding outwards.
///
/// Time: O(n^2) - There are `2n - 1` centers, and expanding takes at most O(n).
/// Space: O(1) - Only a few variables are used to track indices.
///
/// # Rust Insight
/// We return indices `(start, length)` from the helper function rather than slicing
/// repeatedly or creating new Strings, ensuring zero-allocation tracking.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn longest_palindrome_optimized(s: String) -> String {
    if s.len() <= 1 {
        return s;
    }

    let bytes = s.as_bytes();
    let mut max_start = 0;
    let mut max_len = 0;

    for i in 0..bytes.len() {
        // Odd length palindromes (centered at character i)
        let (start1, len1) = expand_around_center(bytes, i as isize, i as isize);
        if len1 > max_len {
            max_start = start1;
            max_len = len1;
        }

        // Even length palindromes (centered between i and i+1)
        let (start2, len2) = expand_around_center(bytes, i as isize, (i + 1) as isize);
        if len2 > max_len {
            max_start = start2;
            max_len = len2;
        }
    }

    s[max_start..max_start + max_len].to_string()
}

/// Helper function to expand around a center and return `(start_index, length)`.
/// Using `isize` here safely handles going out of bounds to the left (`-1`).
#[allow(clippy::cast_possible_wrap)]
#[allow(clippy::cast_sign_loss)]
fn expand_around_center(bytes: &[u8], mut left: isize, mut right: isize) -> (usize, usize) {
    while left >= 0
        && (right as usize) < bytes.len()
        && bytes[left as usize] == bytes[right as usize]
    {
        left -= 1;
        right += 1;
    }

    // After the loop, `left` and `right` point to the first non-matching characters.
    // The valid palindrome was from `left + 1` to `right - 1`.
    // Length = (right - 1) - (left + 1) + 1 = right - left - 1.
    let start = (left + 1) as usize;
    let len = (right - left - 1) as usize;
    (start, len)
}

/// Optimal approach: Manacher's Algorithm.
///
/// This algorithm finds the longest palindromic substring in linear time by exploiting
/// the symmetric properties of palindromes to skip redundant checks.
///
/// Time: O(n) - The right boundary `r` only ever moves forward, making it linear.
/// Space: O(n) - We allocate a transformed string and an array of palindrome radii.
///
/// # Alternative approaches
/// Manacher's is overkill for most interviews. The `O(n^2)` Expand Around Center
/// is generally expected unless explicitly asked for O(n). Dynamic Programming (DP)
/// is another O(n^2) time approach but uses O(n^2) space, making it inferior to expansion.
#[must_use]
#[allow(clippy::many_single_char_names)]
#[allow(clippy::needless_pass_by_value)]
pub fn longest_palindrome_optimal(s: String) -> String {
    if s.len() <= 1 {
        return s;
    }

    // Transform string: "aba" -> "^#a#b#a#$"
    // This avoids bounds checking and standardizes even/odd length palindromes.
    let mut t = Vec::with_capacity(s.len() * 2 + 3);
    t.push(b'^');
    for &b in s.as_bytes() {
        t.push(b'#');
        t.push(b);
    }
    t.push(b'#');
    t.push(b'$');

    let n = t.len();
    let mut p = vec![0; n];
    let mut c: usize = 0; // Center of the rightmost palindrome
    let mut r: usize = 0; // Right boundary of the rightmost palindrome

    let mut max_len = 0;
    let mut center_index = 0;

    for i in 1..n - 1 {
        // Mirror of i across center c
        // i >= c because we iterate forward. Thus 2 * c might be smaller than i,
        // causing underflow if we use usize subtraction directly.
        // Actually, since i > c could happen, we must carefully handle it.
        // If i is outside the current right boundary, we just start fresh,
        // so i_mirror is only used if r > i (which implies i < r, but not necessarily i < c).
        // Let's use `usize::saturating_sub` or ensure we only calculate mirror when needed.
        let i_mirror = c.saturating_sub(i.saturating_sub(c)); // c - (i - c)

        // If i is within the right boundary, we can reuse the mirror's value.
        // But we must not expand beyond `r`.
        if r > i {
            p[i] = std::cmp::min(r - i, p[i_mirror]);
        } else {
            p[i] = 0;
        }

        // Expand palindrome centered at i
        while t[i + 1 + p[i]] == t[i - 1 - p[i]] {
            p[i] += 1;
        }

        // If palindrome centered at i expands past r, adjust c and r
        if i + p[i] > r {
            c = i;
            r = i + p[i];
        }

        // Track max palindrome
        if p[i] > max_len {
            max_len = p[i];
            center_index = i;
        }
    }

    // Extract the original substring
    // The start index in the original string is `(center_index - 1 - max_len) / 2`
    let start = (center_index - 1 - max_len) / 2;
    s[start..start + max_len].to_string()
}

/// Main entry point - uses the optimized `O(n^2)` approach as it's the most idiomatic
/// balance of readability, space usage, and speed for standard string sizes.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn longest_palindrome(s: String) -> String {
    longest_palindrome_optimized(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force_happy_path() {
        let result = longest_palindrome_brute_force("babad".to_string());
        assert!(result == "bab" || result == "aba");

        let result2 = longest_palindrome_brute_force("cbbd".to_string());
        assert_eq!(result2, "bb");
    }

    #[test]
    fn test_optimized_happy_path() {
        let result = longest_palindrome_optimized("babad".to_string());
        assert!(result == "bab" || result == "aba");

        let result2 = longest_palindrome_optimized("cbbd".to_string());
        assert_eq!(result2, "bb");
    }

    #[test]
    fn test_optimal_happy_path() {
        let result = longest_palindrome_optimal("babad".to_string());
        assert!(result == "bab" || result == "aba");

        let result2 = longest_palindrome_optimal("cbbd".to_string());
        assert_eq!(result2, "bb");
    }

    #[test]
    fn test_edge_cases() {
        let s1 = "".to_string();
        assert_eq!(longest_palindrome(s1), "");

        let s2 = "a".to_string();
        assert_eq!(longest_palindrome(s2), "a");

        let s3 = "ac".to_string();
        let r3 = longest_palindrome(s3);
        assert!(r3 == "a" || r3 == "c");
    }

    #[test]
    fn test_stress_all_same_chars() {
        // String of 100 'a's
        let s = "a".repeat(100);
        assert_eq!(longest_palindrome_brute_force(s.clone()), s);
        assert_eq!(longest_palindrome_optimized(s.clone()), s);
        assert_eq!(longest_palindrome_optimal(s.clone()), s);
    }

    #[test]
    fn test_all_approaches_consistency() {
        let s = "forgeeksskeegfor".to_string();
        let expected = "geeksskeeg";

        assert_eq!(longest_palindrome_brute_force(s.clone()), expected);
        assert_eq!(longest_palindrome_optimized(s.clone()), expected);
        assert_eq!(longest_palindrome_optimal(s), expected);
    }
}
