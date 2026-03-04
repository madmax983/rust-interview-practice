//! # 139. Word Break
//!
//! Given a string `s` and a dictionary of strings `wordDict`, return `true` if `s` can be segmented into a space-separated sequence of one or more dictionary words.
//!
//! **Note** that the same word in the dictionary may be reused multiple times in the segmentation.
//!
//! - Difficulty: Medium
//! - LeetCode: <https://leetcode.com/problems/word-break/>
//!
//! ## Why this matters in Rust
//! This problem perfectly illustrates Rust's zero-cost string slicing (`&str`) vs heap allocation (`String`).
//! Instead of copying substrings (like in Java or Python without care), Rust allows us to create O(1) string slices that point to the original string.
//! It also demonstrates managing lifetimes effectively: we can store `&str` references in a `HashSet` tied to the lifetime of the input `wordDict`, avoiding any cloning.
//!
//! ## Approach
//!
//! The problem asks if a string can be broken down. This is a classic DP problem where we want to know:
//! "Can we break the string up to index `i`?" which depends on "Can we break the string up to index `j` (where `j < i`) AND is `s[j..i]` in our dictionary?"
//!
//! We explore three implementations:
//! 1.  **Brute Force**: Recursive DFS. Checks every possible prefix. O(2^n) time in the worst case.
//! 2.  **Memoized**: Top-Down DP. Caches the boolean result for each starting index. O(n^3) time.
//! 3.  **Optimal**: Bottom-Up DP with Tabulation and max word length optimization. O(n^2 * m) time where m is max word length.

use std::collections::{HashMap, HashSet};

/// Brute Force Approach: Recursive DFS
///
/// We try every possible prefix. If the prefix is in the dictionary, we recursively
/// check if the remaining suffix can be segmented.
///
/// - **Time Complexity**: O(2^n). Consider `s = "aaaaaaa"` and `wordDict = ["a", "aa", "aaa", ...]`.
/// - **Space Complexity**: O(n) for the recursion stack.
///
/// # GOTCHA
/// String slicing in Rust `&s[..i]` expects byte indices. If the string contains multi-byte UTF-8 characters,
/// this could panic. LeetCode guarantees the string contains only lowercase English letters (ASCII),
/// making byte indexing perfectly safe and fast.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn word_break_brute_force(s: String, word_dict: Vec<String>) -> bool {
    // RUST INSIGHT: We convert the Vec<String> into a HashSet<&str> to get O(1) lookups.
    // We only borrow the strings inside the dict, avoiding any allocations.
    let dict: HashSet<&str> = word_dict.iter().map(String::as_str).collect();

    fn solve(s: &str, dict: &HashSet<&str>) -> bool {
        if s.is_empty() {
            return true;
        }

        // Try every prefix length
        for i in 1..=s.len() {
            let prefix = &s[..i];
            // If prefix is in dictionary and the suffix can be broken down...
            if dict.contains(prefix) && solve(&s[i..], dict) {
                return true;
            }
        }
        false
    }

    solve(&s, &dict)
}

/// Memoized Approach: Top-Down DP
///
/// We use a `HashMap<usize, bool>` to cache whether the substring starting at index `start`
/// can be segmented. This prunes the overlapping subproblems.
///
/// - **Time Complexity**: O(n^3). There are `n` states. For each state, we iterate up to `n` times,
///   and string slicing/hashing takes up to O(n) time.
/// - **Space Complexity**: O(n) for the recursion stack and memoization map.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn word_break_optimized(s: String, word_dict: Vec<String>) -> bool {
    let dict: HashSet<&str> = word_dict.iter().map(String::as_str).collect();
    let mut memo: HashMap<usize, bool> = HashMap::new();

    fn solve(start: usize, s: &str, dict: &HashSet<&str>, memo: &mut HashMap<usize, bool>) -> bool {
        if start == s.len() {
            return true;
        }

        if let Some(&res) = memo.get(&start) {
            return res;
        }

        for end in start + 1..=s.len() {
            let prefix = &s[start..end];
            if dict.contains(prefix) && solve(end, s, dict, memo) {
                memo.insert(start, true);
                return true;
            }
        }

        memo.insert(start, false);
        false
    }

    solve(0, &s, &dict, &mut memo)
}

/// Optimal Approach: Bottom-Up DP (Tabulation) with Optimization
///
/// We build a `dp` array where `dp[i]` is true if `s[0..i]` can be segmented.
///
/// **Optimization**: Instead of checking all `j < i`, we only need to check `j` such that
/// the length `i - j` does not exceed the maximum word length in our dictionary.
///
/// - **Time Complexity**: O(n * m * k) where `n` is string length, `m` is max word length,
///   and `k` is the average word length (for hashing the slice). If `m` is small, this is closer to O(n).
/// - **Space Complexity**: O(n + d) where `n` is the DP array and `d` is the dictionary size.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn word_break_optimal(s: String, word_dict: Vec<String>) -> bool {
    let dict: HashSet<&str> = word_dict.iter().map(String::as_str).collect();

    // Find the maximum length of a word in the dictionary
    // RUST INSIGHT: `map` + `max` is an idiomatic way to find the max property.
    // `unwrap_or(0)` handles the edge case of an empty dictionary safely.
    let max_word_len = word_dict.iter().map(String::len).max().unwrap_or(0);

    let n = s.len();
    // dp[i] represents if s[0..i] can be segmented into dictionary words
    let mut dp = vec![false; n + 1];

    // Base case: empty string is trivially valid
    dp[0] = true;

    for i in 1..=n {
        // Optimization: We only look back up to max_word_len characters.
        // RUST INSIGHT: `saturating_sub` prevents underflow when `i < max_word_len`.
        let start_j = i.saturating_sub(max_word_len);

        for j in start_j..i {
            // If the substring up to j is valid, AND the remaining part s[j..i] is a dictionary word...
            if dp[j] && dict.contains(&s[j..i]) {
                dp[i] = true;
                break; // No need to check other j's for this i
            }
        }
    }

    dp[n]
}

/// Main entry point
#[must_use]
pub fn word_break(s: String, word_dict: Vec<String>) -> bool {
    word_break_optimal(s, word_dict)
}

// Alternative Approaches:
// 1. **Trie + DP**: You can build a Trie from the dictionary. For each index `i`,
//    you traverse the Trie starting from `s[i]`. This avoids hashing substrings and
//    can be faster in practice when many words share prefixes.

#[cfg(test)]
mod tests {
    use super::*;

    fn vec_str(words: &[&str]) -> Vec<String> {
        words.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn test_brute_force_basic() {
        assert!(word_break_brute_force(
            "leetcode".to_string(),
            vec_str(&["leet", "code"])
        ));
        assert!(word_break_brute_force(
            "applepenapple".to_string(),
            vec_str(&["apple", "pen"])
        ));
        assert!(!word_break_brute_force(
            "catsandog".to_string(),
            vec_str(&["cats", "dog", "sand", "and", "cat"])
        ));
    }

    #[test]
    fn test_optimized_basic() {
        assert!(word_break_optimized(
            "leetcode".to_string(),
            vec_str(&["leet", "code"])
        ));
        assert!(word_break_optimized(
            "applepenapple".to_string(),
            vec_str(&["apple", "pen"])
        ));
        assert!(!word_break_optimized(
            "catsandog".to_string(),
            vec_str(&["cats", "dog", "sand", "and", "cat"])
        ));
    }

    #[test]
    fn test_optimal_basic() {
        assert!(word_break_optimal(
            "leetcode".to_string(),
            vec_str(&["leet", "code"])
        ));
        assert!(word_break_optimal(
            "applepenapple".to_string(),
            vec_str(&["apple", "pen"])
        ));
        assert!(!word_break_optimal(
            "catsandog".to_string(),
            vec_str(&["cats", "dog", "sand", "and", "cat"])
        ));
    }

    #[test]
    fn test_stress_reusing_words() {
        // All approaches should pass, but brute force would be slow if n is large.
        // This is a small enough stress test.
        let s = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaab".to_string();
        let dict = vec_str(&["a", "aa", "aaa", "aaaa", "aaaaa"]);

        assert!(!word_break_optimized(s.clone(), dict.clone()));
        assert!(!word_break_optimal(s, dict));
    }

    #[test]
    fn test_single_word() {
        assert!(word_break_optimal("a".to_string(), vec_str(&["a"])));
        assert!(!word_break_optimal("b".to_string(), vec_str(&["a"])));
    }
}
