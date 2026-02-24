//! # 79. Word Search
//!
//! **Difficulty: Medium**
//!
//! [LeetCode Problem 79](https://leetcode.com/problems/word-search/)
//!
//! Given an `m x n` grid of characters `board` and a string `word`, return `true` if `word` exists in the grid.
//!
//! The word can be constructed from letters of sequentially adjacent cells, where adjacent cells are
//! horizontally or vertically neighboring. The same letter cell may not be used more than once.
//!
//! ## Why this matters in Rust
//! This problem is a classic example of **Backtracking** and **Depth-First Search (DFS)** on a grid.
//! It demonstrates three distinct memory management strategies for tracking visited nodes:
//! 1.  **HashSet**: Using a standard collection (easiest to reason about, worst performance).
//! 2.  **Boolean Matrix**: Using a dedicated grid (faster access, O(MN) space).
//! 3.  **In-Place Mutation**: Modifying the input temporarily (fastest, O(1) space, requires exclusive ownership).
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::backtracking::word_search::exist;
//!
//! let board = vec![
//!     vec!['A', 'B', 'C', 'E'],
//!     vec!['S', 'F', 'C', 'S'],
//!     vec!['A', 'D', 'E', 'E'],
//! ];
//!
//! assert!(exist(board.clone(), "ABCCED".to_string()));
//! assert!(exist(board.clone(), "SEE".to_string()));
//! assert!(!exist(board, "ABCB".to_string()));
//! ```
//!
//! ## Constraints
//!
//! - `m == board.length`
//! - `n = board[i].length`
//! - `1 <= m, n <= 6`
//! - `1 <= word.length <= 15`
//! - `board` and `word` consists of only lowercase and uppercase English letters.

use std::collections::HashSet;

/// Brute Force Approach: DFS with HashSet
///
/// We use a `HashSet` to keep track of visited coordinates `(r, c)`.
/// - **Time**: O(M * N * 3^L) - Standard DFS complexity.
/// - **Space**: O(L) for recursion stack + O(L) for HashSet entries (where L is word length).
///
/// **Why it's "Brute Force"**: Hashing coordinates is computationally expensive compared to direct array indexing.
/// The overhead of `HashSet` operations (hashing, bucket lookup) makes this significantly slower in practice.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn exist_brute_force(board: Vec<Vec<char>>, word: String) -> bool {
    let rows = board.len();
    if rows == 0 {
        return false;
    }
    let cols = board[0].len();
    if cols == 0 {
        return false;
    }

    let word_chars: Vec<char> = word.chars().collect();
    let mut visited = HashSet::new();

    for i in 0..rows {
        for j in 0..cols {
            if dfs_hashset(&board, &word_chars, i, j, 0, &mut visited) {
                return true;
            }
        }
    }
    false
}

fn dfs_hashset(
    board: &[Vec<char>],
    word: &[char],
    i: usize,
    j: usize,
    k: usize,
    visited: &mut HashSet<(usize, usize)>,
) -> bool {
    if k == word.len() {
        return true;
    }

    // Bounds check, char match, and visited check
    if i >= board.len()
        || j >= board[0].len()
        || board[i][j] != word[k]
        || visited.contains(&(i, j))
    {
        return false;
    }

    visited.insert((i, j));

    let found = dfs_hashset(board, word, i + 1, j, k + 1, visited)
        || dfs_hashset(board, word, i, j + 1, k + 1, visited)
        || (i > 0 && dfs_hashset(board, word, i - 1, j, k + 1, visited))
        || (j > 0 && dfs_hashset(board, word, i, j - 1, k + 1, visited));

    visited.remove(&(i, j)); // Backtrack

    found
}

/// Optimized Approach: DFS with Visited Matrix
///
/// We use a boolean matrix `Vec<Vec<bool>>` of the same size as `board` to track visited cells.
/// - **Time**: O(M * N * 3^L).
/// - **Space**: O(M * N) for the visited matrix + O(L) for recursion stack.
///
/// **Trade-off**: Much faster access than HashSet (O(1) index vs hashing), but requires O(M*N) allocation
/// regardless of path length. This is safer than in-place mutation if we only have read access to `board`.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn exist_optimized(board: Vec<Vec<char>>, word: String) -> bool {
    let rows = board.len();
    if rows == 0 {
        return false;
    }
    let cols = board[0].len();
    if cols == 0 {
        return false;
    }

    let word_chars: Vec<char> = word.chars().collect();
    // Allocate visited matrix once
    let mut visited = vec![vec![false; cols]; rows];

    for i in 0..rows {
        for j in 0..cols {
            if dfs_matrix(&board, &word_chars, i, j, 0, &mut visited) {
                return true;
            }
        }
    }
    false
}

fn dfs_matrix(
    board: &[Vec<char>],
    word: &[char],
    i: usize,
    j: usize,
    k: usize,
    visited: &mut Vec<Vec<bool>>,
) -> bool {
    if k == word.len() {
        return true;
    }

    if i >= board.len() || j >= board[0].len() || board[i][j] != word[k] || visited[i][j] {
        return false;
    }

    visited[i][j] = true;

    let found = dfs_matrix(board, word, i + 1, j, k + 1, visited)
        || dfs_matrix(board, word, i, j + 1, k + 1, visited)
        || (i > 0 && dfs_matrix(board, word, i - 1, j, k + 1, visited))
        || (j > 0 && dfs_matrix(board, word, i, j - 1, k + 1, visited));

    visited[i][j] = false; // Backtrack

    found
}

/// Optimal Approach: DFS with In-Place Mutation
///
/// We modify the `board` itself to mark visited cells (e.g., replacing char with '#').
/// - **Time**: O(M * N * 3^L).
/// - **Space**: O(L) for recursion stack. No extra heap allocation for visited state.
///
/// **Why it's Optimal**: Zero extra allocation for state tracking. The mutation is temporary and reversible
/// (backtracking), so the board is restored to its original state after the search.
/// Requires mutable ownership of the board.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn exist_optimal(mut board: Vec<Vec<char>>, word: String) -> bool {
    let rows = board.len();
    if rows == 0 {
        return false;
    }
    let cols = board[0].len();
    if cols == 0 {
        return false;
    }

    let word_chars: Vec<char> = word.chars().collect();

    for i in 0..rows {
        for j in 0..cols {
            if dfs_inplace(&mut board, &word_chars, i, j, 0) {
                return true;
            }
        }
    }
    false
}

fn dfs_inplace(board: &mut Vec<Vec<char>>, word: &[char], i: usize, j: usize, k: usize) -> bool {
    if k == word.len() {
        return true;
    }

    // Check bounds and character match
    // Note: We check `i >= board.len()` to handle `i + 1` overflow safely
    if i >= board.len() || j >= board[0].len() || board[i][j] != word[k] {
        return false;
    }

    // Mark visited
    let temp = board[i][j];
    board[i][j] = '#'; // Use a non-alpha char as sentinel

    let found = dfs_inplace(board, word, i + 1, j, k + 1)
        || dfs_inplace(board, word, i, j + 1, k + 1)
        || (i > 0 && dfs_inplace(board, word, i - 1, j, k + 1))
        || (j > 0 && dfs_inplace(board, word, i, j - 1, k + 1));

    // Backtrack
    board[i][j] = temp;

    found
}

/// Main entry point - uses optimal solution
#[must_use]
pub fn exist(board: Vec<Vec<char>>, word: String) -> bool {
    exist_optimal(board, word)
}

/// ## Alternative approaches
///
/// 1. **Trie (Prefix Tree)**: If searching for *multiple* words (Word Search II), building a Trie of the words allows checking for all words simultaneously during a single DFS traversal.
///    - *Pros*: O(M * N * 3^L) total for W words, instead of O(W * M * N * 3^L).
///    - *Cons*: Implementation complexity; overkill for a single word.
///
/// 2. **Bitmasking**: If the board size is small (e.g., < 64 cells), a `u64` bitmask can track visited status.
///    - *Pros*: Faster than `Vec<bool>` or `HashSet`, O(1) space.
///    - *Cons*: Strictly limited by board size (cannot handle arbitrary N).

#[cfg(test)]
mod tests {
    use super::*;

    // Helper for cross-verification
    fn run_all_approaches(board: Vec<Vec<char>>, word: String, expected: bool) {
        assert_eq!(
            exist_brute_force(board.clone(), word.clone()),
            expected,
            "Brute force failed"
        );
        assert_eq!(
            exist_optimized(board.clone(), word.clone()),
            expected,
            "Optimized failed"
        );
        assert_eq!(
            exist_optimal(board.clone(), word.clone()),
            expected,
            "Optimal failed"
        );
        assert_eq!(exist(board, word), expected, "Main wrapper failed");
    }

    #[test]
    fn test_basic_success() {
        let board = vec![
            vec!['A', 'B', 'C', 'E'],
            vec!['S', 'F', 'C', 'S'],
            vec!['A', 'D', 'E', 'E'],
        ];
        run_all_approaches(board.clone(), "ABCCED".to_string(), true);
        run_all_approaches(board, "SEE".to_string(), true);
    }

    #[test]
    fn test_basic_failure() {
        let board = vec![
            vec!['A', 'B', 'C', 'E'],
            vec!['S', 'F', 'C', 'S'],
            vec!['A', 'D', 'E', 'E'],
        ];
        run_all_approaches(board, "ABCB".to_string(), false);
    }

    #[test]
    fn test_empty_board() {
        let board: Vec<Vec<char>> = vec![];
        run_all_approaches(board, "A".to_string(), false);
    }

    #[test]
    fn test_single_cell() {
        let board = vec![vec!['a']];
        run_all_approaches(board.clone(), "a".to_string(), true);
        run_all_approaches(board, "b".to_string(), false);
    }

    #[test]
    fn test_snake_path() {
        let board = vec![
            vec!['A', 'B', 'C'],
            vec!['F', 'E', 'D'],
            vec!['G', 'H', 'I'],
        ];
        // "ABCDEFGHI"
        run_all_approaches(board, "ABCDEFGHI".to_string(), true);
    }

    #[test]
    fn test_visited_reuse_failure() {
        // "ABA" needs to reuse 'A' if allowed, but it's not.
        let board = vec![vec!['A', 'B'], vec!['C', 'D']];
        run_all_approaches(board, "ABA".to_string(), false);
    }
}
