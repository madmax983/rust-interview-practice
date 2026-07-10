//! # 36. Valid Sudoku
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/valid-sudoku/>
//!
//! Determine if a `9 x 9` Sudoku board is valid. Only the filled cells need to be validated
//! according to the following rules:
//! 1. Each row must contain the digits `1-9` without repetition.
//! 2. Each column must contain the digits `1-9` without repetition.
//! 3. Each of the nine `3 x 3` sub-boxes of the grid must contain the digits `1-9` without repetition.
//!
//! Note: A Sudoku board (partially filled) could be valid but is not necessarily solvable.
//! Only the filled cells need to be validated according to the mentioned rules.
//!
//! This problem is an excellent showcase of managing state across multi-dimensional arrays in Rust.
//! It highlights the transition from typical `HashSets` (heap-allocated) to simple arrays,
//! and ultimately to primitive bitmasks. It demonstrates how Rust's integer bitwise operations
//! provide zero-cost, type-safe abstractions for sets, avoiding memory allocation overhead entirely.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::arrays::valid_sudoku::is_valid_sudoku;
//!
//! let board = vec![
//!     vec!['5','3','.','.','7','.','.','.','.'],
//!     vec!['6','.','.','1','9','5','.','.','.'],
//!     vec!['.','9','8','.','.','.','.','6','.'],
//!     vec!['8','.','.','.','6','.','.','.','3'],
//!     vec!['4','.','.','8','.','3','.','.','1'],
//!     vec!['7','.','.','.','2','.','.','.','6'],
//!     vec!['.','6','.','.','.','.','2','8','.'],
//!     vec!['.','.','.','4','1','9','.','.','5'],
//!     vec!['.','.','.','.','8','.','.','7','9']
//! ];
//! assert_eq!(is_valid_sudoku(board), true);
//! ```

use std::collections::HashSet;

/// Brute force approach: Independent `HashSets` for rows, columns, and sub-boxes
///
/// Time: O(1) - The board size is always 9x9, so we do 81 iterations * 3 passes.
/// Space: O(1) - The `HashSets` store at most 9 elements each.
///
/// This approach iterates the board multiple times. First checking all rows, then
/// all columns, and finally all 3x3 sub-boxes. Each check allocates a new `HashSet`
/// to track seen digits.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
pub fn is_valid_sudoku_brute_force(board: Vec<Vec<char>>) -> bool {
    // Check rows
    for r in 0..9 {
        let mut seen = HashSet::with_capacity(9);
        for c in 0..9 {
            let val = board[r][c];
            if val != '.' && !seen.insert(val) {
                return false;
            }
        }
    }

    // Check columns
    for c in 0..9 {
        let mut seen = HashSet::with_capacity(9);
        for r in 0..9 {
            let val = board[r][c];
            if val != '.' && !seen.insert(val) {
                return false;
            }
        }
    }

    // Check 3x3 sub-boxes
    for box_r in 0..3 {
        for box_c in 0..3 {
            let mut seen = HashSet::with_capacity(9);
            // Check 3x3 area
            for r in 0..3 {
                for c in 0..3 {
                    // Calculate absolute row/col indices from the box indices
                    let val = board[box_r * 3 + r][box_c * 3 + c];
                    if val != '.' && !seen.insert(val) {
                        return false;
                    }
                }
            }
        }
    }

    true
}

/// Optimized approach: One pass using arrays of `HashSets`
///
/// Time: O(1) - Single pass over the 9x9 board.
/// Space: O(1) - 27 `HashSets` initialized once, holding at most 9 elements each.
///
/// By instantiating arrays of `HashSets` for rows, columns, and boxes simultaneously,
/// we can validate the board in a single pass. The box index is computed using integer
/// division: `(r / 3) * 3 + (c / 3)`.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
pub fn is_valid_sudoku_optimized(board: Vec<Vec<char>>) -> bool {
    // RUST INSIGHT: To initialize an array of complex types (like HashSet) that don't
    // implement `Copy`, we use `std::array::from_fn`. A simple `[HashSet::new(); 9]`
    // won't compile because `HashSet` is not `Copy`.
    let mut rows: [HashSet<char>; 9] = std::array::from_fn(|_| HashSet::with_capacity(9));
    let mut cols: [HashSet<char>; 9] = std::array::from_fn(|_| HashSet::with_capacity(9));
    let mut boxes: [HashSet<char>; 9] = std::array::from_fn(|_| HashSet::with_capacity(9));

    for r in 0..9 {
        for c in 0..9 {
            let val = board[r][c];
            if val == '.' {
                continue;
            }

            // Calculate which 3x3 box this cell belongs to (0-8)
            let box_idx = (r / 3) * 3 + (c / 3);

            // GOTCHA: `HashSet::insert` returns false if the value was already present.
            // Using `if !set.insert(val)` avoids two lookups (one for contains, one for insert).
            if !rows[r].insert(val) || !cols[c].insert(val) || !boxes[box_idx].insert(val) {
                return false;
            }
        }
    }

    true
}

/// Optimal approach: One pass using bitmasks
///
/// Time: O(1) - Single pass over the 9x9 board.
/// Space: O(1) - Uses exactly 3 arrays of 9 `u16` integers (54 bytes total).
///
/// Instead of heavy `HashSet`s, we represent the presence of digits 1-9 using bits
/// in a `u16` (since we only need 9 bits). Bitwise operations provide the exact
/// same duplicate-checking semantics but with zero allocations and perfect cache locality.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
pub fn is_valid_sudoku_optimal(board: Vec<Vec<char>>) -> bool {
    // RUST INSIGHT: Primitives implement `Copy`, so we can concisely initialize them.
    let mut rows = [0u16; 9];
    let mut cols = [0u16; 9];
    let mut boxes = [0u16; 9];

    for r in 0..9 {
        for c in 0..9 {
            let val = board[r][c];
            if val == '.' {
                continue;
            }

            // Map '1'-'9' to 0-8 for bit shifting
            // We know it's a valid digit from constraints, so unwrap is safe, but
            // using `to_digit` safely handles any char.
            let digit = val.to_digit(10).unwrap() as u16 - 1;

            // Create a bitmask for this digit (e.g., '3' -> 0b000000100)
            let mask = 1 << digit;
            let box_idx = (r / 3) * 3 + (c / 3);

            // Bitwise AND checks if the bit is already set. If it is, we have a duplicate.
            if (rows[r] & mask) != 0 || (cols[c] & mask) != 0 || (boxes[box_idx] & mask) != 0 {
                return false;
            }

            // Bitwise OR sets the bit for future checks.
            rows[r] |= mask;
            cols[c] |= mask;
            boxes[box_idx] |= mask;
        }
    }

    true
}

/// Main entry point - uses optimal bitmask solution
#[must_use]
pub fn is_valid_sudoku(board: Vec<Vec<char>>) -> bool {
    is_valid_sudoku_optimal(board)
}

// Alternative Approaches:
// 1. **Functional Combinators**: We can collect the coordinates and digits into a flat iterator
//    and use `fold` or a single HashSet of tuples like `(row, digit)`, `(col, digit)`, `(box, digit)`.
//    This is very elegant functionally but allocates significantly more strings/tuples.

#[cfg(test)]
mod tests {
    use super::*;

    // Helper macro to quickly create board rows
    macro_rules! row {
        ($($x:expr),*) => {
            vec![$($x),*]
        };
    }

    #[test]
    fn test_valid_sudoku_happy_path() {
        let board = vec![
            row!['5', '3', '.', '.', '7', '.', '.', '.', '.'],
            row!['6', '.', '.', '1', '9', '5', '.', '.', '.'],
            row!['.', '9', '8', '.', '.', '.', '.', '6', '.'],
            row!['8', '.', '.', '.', '6', '.', '.', '.', '3'],
            row!['4', '.', '.', '8', '.', '3', '.', '.', '1'],
            row!['7', '.', '.', '.', '2', '.', '.', '.', '6'],
            row!['.', '6', '.', '.', '.', '.', '2', '8', '.'],
            row!['.', '.', '.', '4', '1', '9', '.', '.', '5'],
            row!['.', '.', '.', '.', '8', '.', '.', '7', '9'],
        ];

        assert!(is_valid_sudoku_brute_force(board.clone()));
        assert!(is_valid_sudoku_optimized(board.clone()));
        assert!(is_valid_sudoku_optimal(board.clone()));
    }

    #[test]
    fn test_invalid_sudoku_row_duplicate() {
        // Same as happy path, but top left 5 is replaced with 3
        let board = vec![
            row!['3', '3', '.', '.', '7', '.', '.', '.', '.'],
            row!['6', '.', '.', '1', '9', '5', '.', '.', '.'],
            row!['.', '9', '8', '.', '.', '.', '.', '6', '.'],
            row!['8', '.', '.', '.', '6', '.', '.', '.', '3'],
            row!['4', '.', '.', '8', '.', '3', '.', '.', '1'],
            row!['7', '.', '.', '.', '2', '.', '.', '.', '6'],
            row!['.', '6', '.', '.', '.', '.', '2', '8', '.'],
            row!['.', '.', '.', '4', '1', '9', '.', '.', '5'],
            row!['.', '.', '.', '.', '8', '.', '.', '7', '9'],
        ];

        assert!(!is_valid_sudoku_brute_force(board.clone()));
        assert!(!is_valid_sudoku_optimized(board.clone()));
        assert!(!is_valid_sudoku_optimal(board.clone()));
    }

    #[test]
    fn test_invalid_sudoku_col_duplicate() {
        // First column has two 6s
        let board = vec![
            row!['6', '3', '.', '.', '7', '.', '.', '.', '.'],
            row!['6', '.', '.', '1', '9', '5', '.', '.', '.'],
            row!['.', '9', '8', '.', '.', '.', '.', '6', '.'],
            row!['8', '.', '.', '.', '6', '.', '.', '.', '3'],
            row!['4', '.', '.', '8', '.', '3', '.', '.', '1'],
            row!['7', '.', '.', '.', '2', '.', '.', '.', '6'],
            row!['.', '6', '.', '.', '.', '.', '2', '8', '.'],
            row!['.', '.', '.', '4', '1', '9', '.', '.', '5'],
            row!['.', '.', '.', '.', '8', '.', '.', '7', '9'],
        ];

        assert!(!is_valid_sudoku_brute_force(board.clone()));
        assert!(!is_valid_sudoku_optimized(board.clone()));
        assert!(!is_valid_sudoku_optimal(board.clone()));
    }

    #[test]
    fn test_invalid_sudoku_box_duplicate() {
        // Top-left box has two 5s
        let board = vec![
            row!['5', '3', '.', '.', '7', '.', '.', '.', '.'],
            row!['6', '.', '.', '1', '9', '5', '.', '.', '.'],
            row!['.', '9', '5', '.', '.', '.', '.', '6', '.'],
            row!['8', '.', '.', '.', '6', '.', '.', '.', '3'],
            row!['4', '.', '.', '8', '.', '3', '.', '.', '1'],
            row!['7', '.', '.', '.', '2', '.', '.', '.', '6'],
            row!['.', '6', '.', '.', '.', '.', '2', '8', '.'],
            row!['.', '.', '.', '4', '1', '9', '.', '.', '5'],
            row!['.', '.', '.', '.', '8', '.', '.', '7', '9'],
        ];

        assert!(!is_valid_sudoku_brute_force(board.clone()));
        assert!(!is_valid_sudoku_optimized(board.clone()));
        assert!(!is_valid_sudoku_optimal(board.clone()));
    }

    #[test]
    fn test_empty_board() {
        let board = vec![
            row!['.', '.', '.', '.', '.', '.', '.', '.', '.'],
            row!['.', '.', '.', '.', '.', '.', '.', '.', '.'],
            row!['.', '.', '.', '.', '.', '.', '.', '.', '.'],
            row!['.', '.', '.', '.', '.', '.', '.', '.', '.'],
            row!['.', '.', '.', '.', '.', '.', '.', '.', '.'],
            row!['.', '.', '.', '.', '.', '.', '.', '.', '.'],
            row!['.', '.', '.', '.', '.', '.', '.', '.', '.'],
            row!['.', '.', '.', '.', '.', '.', '.', '.', '.'],
            row!['.', '.', '.', '.', '.', '.', '.', '.', '.'],
        ];

        assert!(is_valid_sudoku_brute_force(board.clone()));
        assert!(is_valid_sudoku_optimized(board.clone()));
        assert!(is_valid_sudoku_optimal(board.clone()));
    }
}
