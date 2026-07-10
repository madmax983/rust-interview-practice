//! # 72. Edit Distance
//!
//! Given two strings `word1` and `word2`, return the minimum number of operations required to convert `word1` to `word2`.
//!
//! You have the following three operations permitted on a word:
//!
//! *   Insert a character
//! *   Delete a character
//! *   Replace a character
//!
//! [LeetCode Problem 72](https://leetcode.com/problems/edit-distance/)
//!
//! ## Why this matters in Rust
//!
//! This problem is a classic Dynamic Programming challenge that highlights Rust's handling of:
//! *   **String Handling**: Properly dealing with UTF-8 characters vs bytes (a common pitfall).
//! *   **Vector Initialization**: Efficiently creating 2D vectors (`vec![vec![...]; ...]`).
//! *   **Iterator Adapters**: Using `.chars()` and `.enumerate()` to traverse strings safely.
//! *   **Borrow Checker**: Managing mutable borrows in recursive memoization functions.
//!
//! ## Approach
//!
//! We use standard Dynamic Programming. Let `dp[i][j]` be the edit distance between the first `i` characters of `word1` and the first `j` characters of `word2`.
//!
//! The transitions are:
//! *   If `word1[i-1] == word2[j-1]`: `dp[i][j] = dp[i-1][j-1]` (no operation needed).
//! *   Else: `dp[i][j] = 1 + min(insert, delete, replace)`
//!     *   Insert: `dp[i][j-1]`
//!     *   Delete: `dp[i-1][j]`
//!     *   Replace: `dp[i-1][j-1]`
//!
//! In Rust, accessing string characters by index is O(N). To avoid O(N³) complexity in the DP loops (where each index access would traverse the string), we first collect characters into `Vec<char>`. This is an O(N) space trade-off for O(1) access.

use std::cmp;

/// Brute force approach: Recursive solution.
///
/// This explores all possible operations at each step.
///
/// Time: O(3^(m+n)) - Exponential, very slow.
/// Space: O(m+n) - Stack depth.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn min_distance_brute_force(word1: String, word2: String) -> i32 {
    // LeetCode constraints (word lengths <= 500) guarantee these casts fit.
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    fn solve(i: usize, j: usize, s1: &[char], s2: &[char]) -> i32 {
        // Base cases: if one string is empty, we must insert/delete all remaining chars of the other
        if i == 0 {
            return j as i32;
        }
        if j == 0 {
            return i as i32;
        }

        // RUST INSIGHT: indexing slice with `i-1` is safe here because we checked `i == 0` above.
        // We work with 1-based indices for the logic, so `i` represents the i-th character (index i-1).
        if s1[i - 1] == s2[j - 1] {
            solve(i - 1, j - 1, s1, s2)
        } else {
            1 + cmp::min(
                solve(i, j - 1, s1, s2), // Insert
                cmp::min(
                    solve(i - 1, j, s1, s2),     // Delete
                    solve(i - 1, j - 1, s1, s2), // Replace
                ),
            )
        }
    }

    let s1: Vec<char> = word1.chars().collect();
    let s2: Vec<char> = word2.chars().collect();
    solve(s1.len(), s2.len(), &s1, &s2)
}

/// Optimized approach: Top-Down DP (Memoization).
///
/// We store results of subproblems in a table to avoid re-computation.
/// `memo[i][j]` stores the result for `solve(i, j)`. `Option<i32>` allows distinguishing uncomputed (`None`) states.
///
/// Time: O(m * n) - Each state computed once.
/// Space: O(m * n) - Memoization table + recursion stack.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn min_distance_optimized(word1: String, word2: String) -> i32 {
    // LeetCode constraints (word lengths <= 500) guarantee these casts fit.
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    fn solve(
        i: usize,
        j: usize,
        s1: &[char],
        s2: &[char],
        memo: &mut Vec<Vec<Option<i32>>>,
    ) -> i32 {
        if let Some(val) = memo[i][j] {
            return val;
        }

        let res = if i == 0 {
            j as i32
        } else if j == 0 {
            i as i32
        } else if s1[i - 1] == s2[j - 1] {
            solve(i - 1, j - 1, s1, s2, memo)
        } else {
            // RUST INSIGHT: We must compute these sequentially rather than in one `min(solve(...), solve(...))` call
            // because `solve` takes `&mut memo`. Rust's borrow checker prevents multiple mutable borrows
            // of `memo` in the same function call arguments.
            let insert_op = solve(i, j - 1, s1, s2, memo);
            let delete_op = solve(i - 1, j, s1, s2, memo);
            let replace_op = solve(i - 1, j - 1, s1, s2, memo);

            1 + insert_op.min(delete_op).min(replace_op)
        };

        memo[i][j] = Some(res);
        res
    }

    // GOTCHA: `word1.len()` gives byte length, not char count. Using it for array sizing with Unicode would be wrong.
    // We collect chars to handle Unicode correctly and get O(1) access.
    let s1: Vec<char> = word1.chars().collect();
    let s2: Vec<char> = word2.chars().collect();
    let m = s1.len();
    let n = s2.len();

    // Initialize memo table with None
    let mut memo = vec![vec![None; n + 1]; m + 1];

    solve(m, n, &s1, &s2, &mut memo)
}

/// Optimal approach: Bottom-Up DP with Space Optimization.
///
/// We build the table iteratively. Notice that to compute `dp[i][...]`, we only need `dp[i-1][...]`.
/// Thus, we can reduce space to O(min(m, n)) by keeping only the previous row.
///
/// Time: O(m * n)
/// Space: O(min(m, n)) - We only store one row.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
// LeetCode constraints (word lengths <= 500) guarantee these casts fit.
#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
pub fn min_distance_optimal(word1: String, word2: String) -> i32 {
    let s1: Vec<char> = word1.chars().collect();
    let s2: Vec<char> = word2.chars().collect();
    let m = s1.len();
    let n = s2.len();

    // Optimization: Ensure s2 is the smaller string to minimize space
    if m < n {
        return min_distance_optimal(word2, word1);
    }

    // `dp` array represents the previous row (initially row 0, where string1 is empty)
    // dp[j] is edit distance between "" (empty) and word2[0..j]
    let mut dp: Vec<i32> = (0..=n as i32).collect();

    for i in 1..=m {
        // `prev` stores dp[i-1][j-1] (the diagonal value) before `dp[j]` is updated
        let mut prev_diagonal = dp[0];

        // Update first column: edit distance between word1[0..i] and "" is i
        dp[0] = i as i32;

        for j in 1..=n {
            let temp = dp[j]; // Store dp[i-1][j] before overwriting

            // At this point:
            // dp[j] corresponds to dp[i-1][j] (Delete op)
            // dp[j-1] corresponds to dp[i][j-1] (Insert op, already updated in this row)
            // prev_diagonal corresponds to dp[i-1][j-1] (Replace op)

            if s1[i - 1] == s2[j - 1] {
                dp[j] = prev_diagonal;
            } else {
                dp[j] = 1 + dp[j].min(dp[j - 1]).min(prev_diagonal);
            }
            prev_diagonal = temp; // Update for next iteration
        }
    }

    dp[n]
}

/// Main entry point
#[must_use]
pub fn min_distance(word1: String, word2: String) -> i32 {
    min_distance_optimal(word1, word2)
}

// Alternative Approaches:
// 1. Two-Row Space Optimization: Instead of a single array + variable, use two vectors `prev` and `curr`.
//    This is slightly more readable but uses 2*N space instead of N.
// 2. Hirschberg's Algorithm: O(N) space and O(MN) time, but allows reconstructing the actual edit path.
// 3. Wagner-Fischer Algorithm: The standard name for this DP approach.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force_example_1() {
        let w1 = "horse".to_string();
        let w2 = "ros".to_string();
        assert_eq!(min_distance_brute_force(w1, w2), 3);
    }

    #[test]
    fn test_optimized_example_1() {
        let w1 = "horse".to_string();
        let w2 = "ros".to_string();
        assert_eq!(min_distance_optimized(w1, w2), 3);
    }

    #[test]
    fn test_optimal_example_1() {
        let w1 = "horse".to_string();
        let w2 = "ros".to_string();
        assert_eq!(min_distance_optimal(w1, w2), 3);
    }

    #[test]
    fn test_example_2() {
        let w1 = "intention".to_string();
        let w2 = "execution".to_string();
        // brute force might be slow here? 9x9 is fine.
        assert_eq!(min_distance_brute_force(w1.clone(), w2.clone()), 5);
        assert_eq!(min_distance_optimized(w1.clone(), w2.clone()), 5);
        assert_eq!(min_distance_optimal(w1, w2), 5);
    }

    #[test]
    fn test_edge_case_empty() {
        assert_eq!(min_distance_optimal(String::new(), String::new()), 0);
        assert_eq!(min_distance_optimal("a".to_string(), String::new()), 1);
        assert_eq!(min_distance_optimal(String::new(), "abc".to_string()), 3);
    }

    #[test]
    fn test_unicode() {
        let w1 = "café".to_string(); // 4 chars
        let w2 = "coffee".to_string(); // 6 chars
        // café -> coffee
        // c-a-f-é
        // c-o-f-f-e-e
        // 1. a -> o (sub) -> cofé
        // 2. é -> f (sub) -> coff
        // 3. insert e -> coffe
        // 4. insert e -> coffee
        // Distance: 4
        assert_eq!(min_distance_optimal(w1, w2), 4);
    }

    #[test]
    fn test_same_strings() {
        let w = "rustacean".to_string();
        assert_eq!(min_distance_optimal(w.clone(), w), 0);
    }
}
