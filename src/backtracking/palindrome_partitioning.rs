//! # 131. Palindrome Partitioning
//!
//! Difficulty: Medium
//! Link: https://leetcode.com/problems/palindrome-partitioning/
//!
//! Given a string `s`, partition `s` such that every substring of the partition is a
//! palindrome. Return all possible palindrome partitioning of `s`.
//!
//! A palindrome string is a string that reads the same backward as forward.
//!
//! This problem perfectly illustrates the power of Rust's string slices (`&str`).
//! While other languages often allocate new substrings during recursive exploration
//! (like Java's `s.substring()`), Rust allows us to pass zero-cost string views
//! (`&'a str`) that are guaranteed to outlive the recursive calls, thanks to the
//! borrow checker.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::backtracking::palindrome_partitioning::partition;
//!
//! let res = partition("aab".to_string());
//! assert_eq!(res.len(), 2);
//! assert!(res.contains(&vec!["a".to_string(), "a".to_string(), "b".to_string()]));
//! assert!(res.contains(&vec!["aa".to_string(), "b".to_string()]));
//! ```
//!
//! ## Constraints
//!
//! - `1 <= s.length <= 16`
//! - `s` contains only lowercase English letters.

/// Brute force approach: Exploring all partitions with heavy allocations
/// Time: O(N * 2^N) - generating all possible partitions
/// Space: O(N * 2^N) - storing all intermediate strings heavily
///
/// In this approach, we slice the `String` into new allocated `String`s for
/// every recursive branch. This is an anti-pattern in Rust, reflecting a Java
/// or Python mindset where string slicing might naturally produce new objects
/// or where memory usage is less strictly controlled.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // Required by LeetCode signature
pub fn partition_brute_force(s: String) -> Vec<Vec<String>> {
    fn is_palindrome(s: &str) -> bool {
        // GOTCHA: `s.chars().rev().eq(s.chars())` is O(N) but can be slow due to Unicode
        // decoding on every check. Since we only have lowercase ASCII per constraints,
        // comparing bytes is much faster.
        let bytes = s.as_bytes();
        let mut left = 0;
        let mut right = bytes.len().saturating_sub(1);
        while left < right {
            if bytes[left] != bytes[right] {
                return false;
            }
            left += 1;
            right -= 1;
        }
        true
    }

    fn backtrack(remaining: String, path: &mut Vec<String>, result: &mut Vec<Vec<String>>) {
        if remaining.is_empty() {
            // Reached the end, we found a valid partition sequence
            result.push(path.clone());
            return;
        }

        for i in 1..=remaining.len() {
            // Anti-pattern: Allocating new Strings for both the prefix and the suffix
            // in every step of the recursion tree.
            let prefix = remaining[..i].to_string();
            let suffix = remaining[i..].to_string();

            if is_palindrome(&prefix) {
                path.push(prefix);
                backtrack(suffix, path, result);
                path.pop(); // Backtrack
            }
        }
    }

    let mut result = Vec::new();
    let mut path = Vec::new();
    backtrack(s, &mut path, &mut result);
    result
}

/// Optimized approach: Backtracking with zero-cost string slices (`&str`)
/// Time: O(N * 2^N) - generating partitions
/// Space: O(N) - recursion depth and path slice tracker, plus O(N * 2^N) for the final result
///
/// This is the idiomatic Rust solution. We pass `&str` through the recursion
/// tree instead of `String`. The compiler statically guarantees that these
/// references are valid, eliminating unnecessary heap allocations during exploration.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn partition_optimized(s: String) -> Vec<Vec<String>> {
    fn is_palindrome(bytes: &[u8]) -> bool {
        let mut left = 0;
        let mut right = bytes.len().saturating_sub(1);
        while left < right {
            if bytes[left] != bytes[right] {
                return false;
            }
            left += 1;
            right -= 1;
        }
        true
    }

    // RUST INSIGHT: We use lifetimes explicitly to show that `path` stores
    // references that live as long as the original string `s`.
    fn backtrack<'a>(
        s_bytes: &'a [u8],
        start: usize,
        path: &mut Vec<&'a str>,
        result: &mut Vec<Vec<String>>,
    ) {
        if start == s_bytes.len() {
            // We only allocate when we successfully find a full valid partition.
            // RUST INSIGHT: `.map(|s| s.to_string()).collect()` transforms our
            // zero-cost views into the required owned Strings for the final answer.
            result.push(path.iter().map(|&s| s.to_string()).collect());
            return;
        }

        for end in start + 1..=s_bytes.len() {
            let slice = &s_bytes[start..end];
            if is_palindrome(slice) {
                // Safety: We know the input is valid ASCII/UTF-8 lowercase letters
                // per the problem constraints. Unwrapping `from_utf8` is safe here.
                let str_slice = std::str::from_utf8(slice).unwrap();
                path.push(str_slice);

                backtrack(s_bytes, end, path, result);

                path.pop(); // Backtrack state
            }
        }
    }

    let mut result = Vec::new();
    let mut path = Vec::new();
    // Operating on bytes avoids UTF-8 character boundary checks during slicing
    backtrack(s.as_bytes(), 0, &mut path, &mut result);
    result
}

/// Optimal approach: Backtracking with pre-computed DP for palindromes
/// Time: O(N * 2^N) worst case, but significantly faster on average by avoiding
///       repeated palindrome checks.
/// Space: O(N^2) for DP table, plus recursion depth O(N).
///
/// We first build a DP table `is_pal[i][j]` which tells us if `s[i..=j]` is a
/// palindrome in O(1) time. We then run the standard slice-based backtracking.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn partition_optimal(s: String) -> Vec<Vec<String>> {
    let n = s.len();
    if n == 0 {
        return vec![];
    }

    let bytes = s.as_bytes();

    // DP table: dp[i][j] is true if s[i..=j] is a palindrome
    // RUST INSIGHT: Flat 1D vector acting as a 2D array is often more cache-friendly
    // than Vec<Vec<bool>>, avoiding double pointer indirection.
    let mut dp = vec![false; n * n];

    // Fill the DP table
    for i in (0..n).rev() {
        for j in i..n {
            if bytes[i] == bytes[j] && (j - i <= 2 || dp[(i + 1) * n + (j - 1)]) {
                dp[i * n + j] = true;
            }
        }
    }

    fn backtrack<'a>(
        s_bytes: &'a [u8],
        start: usize,
        n: usize,
        dp: &[bool],
        path: &mut Vec<&'a str>,
        result: &mut Vec<Vec<String>>,
    ) {
        if start == n {
            result.push(path.iter().map(|&slice| slice.to_string()).collect());
            return;
        }

        for end in start..n {
            if dp[start * n + end] {
                let str_slice = std::str::from_utf8(&s_bytes[start..=end]).unwrap();
                path.push(str_slice);
                backtrack(s_bytes, end + 1, n, dp, path, result);
                path.pop();
            }
        }
    }

    let mut result = Vec::new();
    let mut path = Vec::new();
    backtrack(bytes, 0, n, &dp, &mut path, &mut result);
    result
}

/// Main entry point - uses optimal solution
#[must_use]
pub fn partition(s: String) -> Vec<Vec<String>> {
    partition_optimal(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sort_partitions(mut parts: Vec<Vec<String>>) -> Vec<Vec<String>> {
        parts.sort();
        parts
    }

    #[test]
    fn test_brute_force_example_1() {
        let expected = vec![
            vec!["a".to_string(), "a".to_string(), "b".to_string()],
            vec!["aa".to_string(), "b".to_string()],
        ];
        let res = partition_brute_force("aab".to_string());
        assert_eq!(sort_partitions(res), sort_partitions(expected));
    }

    #[test]
    fn test_optimized_example_1() {
        let expected = vec![
            vec!["a".to_string(), "a".to_string(), "b".to_string()],
            vec!["aa".to_string(), "b".to_string()],
        ];
        let res = partition_optimized("aab".to_string());
        assert_eq!(sort_partitions(res), sort_partitions(expected));
    }

    #[test]
    fn test_optimal_example_1() {
        let expected = vec![
            vec!["a".to_string(), "a".to_string(), "b".to_string()],
            vec!["aa".to_string(), "b".to_string()],
        ];
        let res = partition_optimal("aab".to_string());
        assert_eq!(sort_partitions(res), sort_partitions(expected));
    }

    #[test]
    fn test_edge_case_single_char() {
        let expected = vec![vec!["a".to_string()]];
        assert_eq!(partition_brute_force("a".to_string()), expected);
        assert_eq!(partition_optimized("a".to_string()), expected);
        assert_eq!(partition_optimal("a".to_string()), expected);
    }

    #[test]
    fn test_stress_all_same_chars() {
        let expected = vec![
            vec!["a".to_string(), "a".to_string(), "a".to_string()],
            vec!["a".to_string(), "aa".to_string()],
            vec!["aa".to_string(), "a".to_string()],
            vec!["aaa".to_string()],
        ];
        assert_eq!(
            sort_partitions(partition_optimal("aaa".to_string())),
            sort_partitions(expected)
        );
    }
}

// Alternative approaches
// 1. Iterative with BFS: Instead of recursive DFS backtracking, you could use a queue
//    to explore partitions level by level. This is generally more memory-intensive
//    than DFS (due to queue size growing exponentially) and is less idiomatic for
//    finding all paths.
// 2. Iterative DP: You can build the partitions bottom-up. However, since the problem
//    requires *all* possible partitions rather than just the minimum cuts, generating
//    all paths still takes O(N * 2^N) space and time, so the recursive backtracking
//    approach with `&str` remains the most elegant and memory-efficient in Rust.
