//! # 76. Minimum Window Substring
//!
//! Difficulty: Hard
//! Link: <https://leetcode.com/problems/minimum-window-substring/>
//!
//! Given two strings `s` and `t` of lengths `m` and `n` respectively, return the minimum window
//! substring of `s` such that every character in `t` (including duplicates) is included in the window.
//! If there is no such substring, return the empty string `""`.
//!
//! The testcases will be generated such that the answer is unique.
//!
//! This problem perfectly demonstrates sliding window techniques in Rust, contrasting string slice (`&str`)
//! and char iteration with zero-cost byte slice (`&[u8]`) manipulation. It illustrates why array-based frequency
//! maps (`[i32; 128]`) outperform generic `HashMaps` for fixed-domain ASCII problems.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::strings::minimum_window_substring::min_window;
//!
//! assert_eq!(min_window("ADOBECODEBANC".to_string(), "ABC".to_string()), "BANC".to_string());
//! assert_eq!(min_window("a".to_string(), "a".to_string()), "a".to_string());
//! assert_eq!(min_window("a".to_string(), "aa".to_string()), "".to_string());
//! ```
//!
//! ## Constraints
//!
//! - `m == s.length`
//! - `n == t.length`
//! - `1 <= m, n <= 10^5`
//! - `s` and `t` consist of uppercase and lowercase English letters.

use std::collections::HashMap;

/// ## Approach
///
/// We provide three implementations ranging from naive to optimal:
/// 1.  **Brute Force**: Exhaustive search over all substrings, taking O(n³). This is generally how you'd quickly write a mental model or script in Python before optimizing, but it allocates and iterates far too much.
/// 2.  **Optimized (`HashMap`)**: A classic sliding window that tracks frequencies in a `HashMap`. This is how you'd typically solve this in Java or Python `Counter`, but in Rust, the hashing overhead can be noticeable.
/// 3.  **Optimal (Array Mapping)**: A highly optimized sliding window leveraging the problem's constraints (ASCII only). By converting the strings to byte slices (`&[u8]`) and mapping directly into a fixed-size array (`[i32; 128]`), we achieve zero-allocation O(1) character frequency lookups. This is the idiomatic Rust way to process constrained text efficiently, matching C/C++ speeds while retaining safety guarantees.
///
/// Brute force approach: Check all possible substrings.
/// Time: O(n^3) - nested loops for substrings, plus string slicing/counting.
/// Space: O(1) beyond the output.
///
/// This approach explores every substring `s[i..j]`, counts frequencies, and checks against `t`.
/// It is incredibly slow but sets a baseline for correctness.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
pub fn min_window_brute_force(s: String, t: String) -> String {
    let s_bytes = s.as_bytes();
    let t_bytes = t.as_bytes();
    let n = s_bytes.len();

    // Quick escape for impossible cases
    if n < t_bytes.len() || t_bytes.is_empty() {
        return String::new();
    }

    let mut target_counts = [0_i32; 128];
    for &b in t_bytes {
        target_counts[b as usize] += 1;
    }

    let mut min_len = usize::MAX;
    let mut min_start = 0;

    for start in 0..n {
        for end in start..n {
            let window_len = end - start + 1;
            // Short-circuit: window must be at least t_bytes.len()
            if window_len < t_bytes.len() {
                continue;
            }

            let mut window_counts = [0_i32; 128];
            for &b in &s_bytes[start..=end] {
                window_counts[b as usize] += 1;
            }

            let mut valid = true;
            for i in 0..128 {
                if target_counts[i] > 0 && window_counts[i] < target_counts[i] {
                    valid = false;
                    break;
                }
            }

            if valid && window_len < min_len {
                min_len = window_len;
                min_start = start;
            }
        }
    }

    if min_len == usize::MAX {
        String::new()
    } else {
        s[min_start..min_start + min_len].to_string()
    }
}

/// Optimized approach: Sliding window with `HashMap`.
/// Time: O(m + n) - Each character is visited at most twice.
/// Space: O(K) where K is unique characters in `t`.
///
/// This approach uses a dynamic sliding window `[left, right]`. We expand `right` until we
/// have all required characters, then shrink `left` to find the minimum valid window.
/// We use `HashMaps` to track character frequencies, which is conceptually clear but incurs
/// hashing overhead compared to array maps.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn min_window_optimized(s: String, t: String) -> String {
    let s_bytes = s.as_bytes();
    let t_bytes = t.as_bytes();

    if s_bytes.len() < t_bytes.len() || t_bytes.is_empty() {
        return String::new();
    }

    let mut target_counts = HashMap::new();
    for &b in t_bytes {
        *target_counts.entry(b).or_insert(0) += 1;
    }

    let required = target_counts.len();
    let mut formed = 0;
    let mut window_counts = HashMap::new();

    let mut min_len = usize::MAX;
    let mut min_window = (0, 0);

    let mut left = 0;

    for right in 0..s_bytes.len() {
        let c = s_bytes[right];
        *window_counts.entry(c).or_insert(0) += 1;

        if let Some(&target_count) = target_counts.get(&c)
            && window_counts[&c] == target_count
        {
            formed += 1;
        }

        while left <= right && formed == required {
            let window_len = right - left + 1;
            if window_len < min_len {
                min_len = window_len;
                min_window = (left, right);
            }

            let left_char = s_bytes[left];
            if let Some(count) = window_counts.get_mut(&left_char) {
                *count -= 1;
                if let Some(&target_count) = target_counts.get(&left_char)
                    && *count < target_count
                {
                    formed -= 1;
                }
            }
            left += 1;
        }
    }

    if min_len == usize::MAX {
        String::new()
    } else {
        s[min_window.0..=min_window.1].to_string()
    }
}

/// Optimal approach: Sliding window with Fixed Array Mapping.
/// Time: O(m + n)
/// Space: O(1) - Fixed 128-element arrays for ASCII.
///
/// Eliminates `HashMap` overhead by mapping ASCII bytes directly to array indices.
///
/// **RUST INSIGHT**: Operating on `&[u8]` avoids the O(n) UTF-8 boundary checks of `.chars()`.
/// Array access `map[b as usize]` is bounds-checked but easily optimized away by LLVM.
/// We use `std::str::from_utf8` at the end to safely convert the slice back to a `String`.
///
/// # Panics
///
/// Panics if the winning window is not valid UTF-8. This cannot happen because the window
/// is a byte range of the original `&str`, which is guaranteed to be valid UTF-8.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn min_window_optimal(s: String, t: String) -> String {
    let s_bytes = s.as_bytes();
    let t_bytes = t.as_bytes();

    if s_bytes.len() < t_bytes.len() || t_bytes.is_empty() {
        return String::new();
    }

    let mut target_counts = [0_i32; 128];
    for &b in t_bytes {
        target_counts[b as usize] += 1;
    }

    // Number of distinct characters in `t` that must be fully matched.
    let required = target_counts.iter().filter(|&&v| v > 0).count();
    let mut formed = 0;

    let mut window_counts = [0_i32; 128];
    let mut min_len = usize::MAX;
    let mut min_start = 0;
    let mut left = 0;

    for right in 0..s_bytes.len() {
        let r_char = s_bytes[right] as usize;
        window_counts[r_char] += 1;

        // If this character's frequency hits the target exactly, we formed one more requirement.
        if target_counts[r_char] > 0 && window_counts[r_char] == target_counts[r_char] {
            formed += 1;
        }

        // Shrink window while all requirements are still met.
        while left <= right && formed == required {
            let current_len = right - left + 1;
            if current_len < min_len {
                min_len = current_len;
                min_start = left;
            }

            let l_char = s_bytes[left] as usize;
            window_counts[l_char] -= 1;

            // GOTCHA: It's important to check if the count drops *below* the requirement.
            // If we had excess characters, it's safe to drop them without affecting `formed`.
            if target_counts[l_char] > 0 && window_counts[l_char] < target_counts[l_char] {
                formed -= 1;
            }
            left += 1;
        }
    }

    if min_len == usize::MAX {
        String::new()
    } else {
        // Safe because the input `s` was valid UTF-8, and ASCII boundaries always fall on valid char boundaries.
        // We use `from_utf8_unchecked` for absolute maximum performance in competitive programming, but `from_utf8().unwrap()` is safer.
        std::str::from_utf8(&s_bytes[min_start..min_start + min_len])
            .unwrap()
            .to_string()
    }
}

/// Main entry point - uses optimal solution
#[must_use]
pub fn min_window(s: String, t: String) -> String {
    min_window_optimal(s, t)
}

// ## Alternative approaches
//
// 1. **Binary Search + Fixed-Size Window Checking**: You could binary search the answer length `L` from `1` to `len(s)`, checking if any valid window of size `L` exists. Time complexity is O(N log N). This is worse than O(N) sliding window, so it's not implemented.
// 2. **Pre-filtering `s`**: Create a list of `(index, char)` for all characters in `s` that also appear in `t`. Then run the sliding window only over this filtered list. This helps if `s` is huge and `t` characters are sparse.

#[cfg(test)]
mod tests {
    use super::*;

    // Happy Path
    #[test]
    fn test_basic_window() {
        let s = "ADOBECODEBANC".to_string();
        let t = "ABC".to_string();
        let expected = "BANC".to_string();

        assert_eq!(min_window_brute_force(s.clone(), t.clone()), expected);
        assert_eq!(min_window_optimized(s.clone(), t.clone()), expected);
        assert_eq!(min_window_optimal(s, t), expected);
    }

    // Edge Cases
    #[test]
    fn test_single_char() {
        let s = "a".to_string();
        let t = "a".to_string();
        let expected = "a".to_string();

        assert_eq!(min_window_brute_force(s.clone(), t.clone()), expected);
        assert_eq!(min_window_optimized(s.clone(), t.clone()), expected);
        assert_eq!(min_window_optimal(s, t), expected);
    }

    #[test]
    fn test_impossible() {
        let s = "a".to_string();
        let t = "aa".to_string();
        let expected = String::new();

        assert_eq!(min_window_brute_force(s.clone(), t.clone()), expected);
        assert_eq!(min_window_optimized(s.clone(), t.clone()), expected);
        assert_eq!(min_window_optimal(s, t), expected);
    }

    // Boundary Conditions
    #[test]
    fn test_exact_match() {
        let s = "abc".to_string();
        let t = "cba".to_string();
        let expected = "abc".to_string();

        assert_eq!(min_window_brute_force(s.clone(), t.clone()), expected);
        assert_eq!(min_window_optimized(s.clone(), t.clone()), expected);
        assert_eq!(min_window_optimal(s, t), expected);
    }

    #[test]
    fn test_multiple_identical_windows() {
        let s = "acbbaca".to_string();
        let t = "aba".to_string();
        // The first valid window is "baca". "acbba" is length 5.
        let expected = "baca".to_string();

        assert_eq!(min_window_brute_force(s.clone(), t.clone()), expected);
        assert_eq!(min_window_optimized(s.clone(), t.clone()), expected);
        assert_eq!(min_window_optimal(s, t), expected);
    }
}
