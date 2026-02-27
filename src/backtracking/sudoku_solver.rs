//! # 37. Sudoku Solver
//!
//! **Difficulty: Hard**
//!
//! [LeetCode Problem 37](https://leetcode.com/problems/sudoku-solver/)
//!
//! Write a program to solve a Sudoku puzzle by filling the empty cells.
//! A sudoku solution must satisfy all of the following rules:
//! 1. Each of the digits `1-9` must occur exactly once in each row.
//! 2. Each of the digits `1-9` must occur exactly once in each column.
//! 3. Each of the digits `1-9` must occur exactly once in each of the 9 `3x3` sub-boxes of the grid.
//!
//! The `'.'` character indicates empty cells.
//!
//! ## Why this matters in Rust
//! This problem is an excellent case study for **Backtracking** and **State Management**.
//! It demonstrates how representing constraints (row, column, box availability) affects performance:
//! 1.  **Scanning**: Checking validity by iterating (slow, O(9) per check).
//! 2.  **Lookup Tables**: Using boolean arrays for O(1) validity checks (trade memory for speed).
//! 3.  **Bitmasks**: Using bits to track constraints (fastest, cache-friendly, zero-allocation).
//!
//! It also highlights:
//! - **Interior Mutability**: While not strictly necessary here, this pattern often leads to `RefCell` in more complex graph problems.
//! - **Array Indexing**: Efficiently mapping 2D coordinates `(row, col)` to 1D flat arrays or 3x3 box indices.
//! - **in-place mutation**: Modifying the board directly is required by the problem statement.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::backtracking::sudoku_solver::solve_sudoku;
//!
//! let mut board = vec![
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
//!
//! solve_sudoku(&mut board);
//! assert_eq!(board[0][0], '5');
//! ```

#![allow(clippy::needless_range_loop)]

/// Approach 1: Naive Backtracking
///
/// **Strategy**:
/// Find the first empty cell. Try placing digits 1-9.
/// For each digit, scan the current row, column, and 3x3 sub-box to check if it's valid.
/// If valid, place it and recurse. If recursion returns true, we're done.
/// If not, backtrack (reset cell to '.') and try next digit.
///
/// **Time**: O(9^(M)), where M is the number of empty cells. In practice, much faster due to pruning.
/// **Space**: O(M) for recursion stack.
///
/// **Pros**: Simple to implement, no extra memory for state.
/// **Cons**: `is_valid` is O(9) = O(1), but repeated constantly. Slower than tracking state.
pub fn solve_sudoku_naive(board: &mut Vec<Vec<char>>) {
    solve_naive(board);
}

fn solve_naive(board: &mut Vec<Vec<char>>) -> bool {
    for i in 0..9 {
        for j in 0..9 {
            if board[i][j] == '.' {
                for c in '1'..='9' {
                    if is_valid_naive(board, i, j, c) {
                        board[i][j] = c;
                        if solve_naive(board) {
                            return true;
                        }
                        board[i][j] = '.'; // Backtrack
                    }
                }
                return false; // No valid number found for this empty cell
            }
        }
    }
    true // No empty cells left
}

fn is_valid_naive(board: &[Vec<char>], row: usize, col: usize, c: char) -> bool {
    for i in 0..9 {
        // Check Row
        if board[row][i] == c {
            return false;
        }
        // Check Column
        if board[i][col] == c {
            return false;
        }
        // Check 3x3 Sub-box
        // RUST INSIGHT: Integer division truncates, so `3 * (row / 3)` snaps to the top-left of the 3x3 box.
        let box_row = 3 * (row / 3) + i / 3;
        let box_col = 3 * (col / 3) + i % 3;
        if board[box_row][box_col] == c {
            return false;
        }
    }
    true
}

/// Approach 2: Optimized Backtracking with Lookup Tables
///
/// **Strategy**:
/// Maintain 3 sets of boolean arrays:
/// 1. `rows[9][10]`: `rows[r][d]` is true if digit `d` exists in row `r`.
/// 2. `cols[9][10]`: `cols[c][d]` is true if digit `d` exists in column `c`.
/// 3. `boxes[9][10]`: `boxes[b][d]` is true if digit `d` exists in box `b`.
///
/// Checking validity becomes O(1) table lookup.
///
/// **Time**: O(9^M). Faster constant factor than naive.
/// **Space**: O(1) extra space (fixed size arrays 9x10).
pub fn solve_sudoku_optimized(board: &mut Vec<Vec<char>>) {
    let mut rows = [[false; 10]; 9];
    let mut cols = [[false; 10]; 9];
    let mut boxes = [[false; 10]; 9];

    // Initialize state from existing board
    for i in 0..9 {
        for j in 0..9 {
            if let Some(digit) = board[i][j].to_digit(10) {
                let d = digit as usize;
                let box_idx = (i / 3) * 3 + j / 3;
                rows[i][d] = true;
                cols[j][d] = true;
                boxes[box_idx][d] = true;
            }
        }
    }

    solve_optimized(board, &mut rows, &mut cols, &mut boxes);
}

fn solve_optimized(
    board: &mut Vec<Vec<char>>,
    rows: &mut [[bool; 10]; 9],
    cols: &mut [[bool; 10]; 9],
    boxes: &mut [[bool; 10]; 9],
) -> bool {
    for i in 0..9 {
        for j in 0..9 {
            if board[i][j] == '.' {
                let box_idx = (i / 3) * 3 + j / 3;

                for d in 1..=9 {
                    if !rows[i][d] && !cols[j][d] && !boxes[box_idx][d] {
                        // Place digit
                        let c = char::from_digit(d as u32, 10).unwrap();
                        board[i][j] = c;
                        rows[i][d] = true;
                        cols[j][d] = true;
                        boxes[box_idx][d] = true;

                        if solve_optimized(board, rows, cols, boxes) {
                            return true;
                        }

                        // Backtrack
                        board[i][j] = '.';
                        rows[i][d] = false;
                        cols[j][d] = false;
                        boxes[box_idx][d] = false;
                    }
                }
                return false;
            }
        }
    }
    true
}

/// Approach 3: Optimal Backtracking with Bitmasks
///
/// **Strategy**:
/// Similar to Approach 2, but use `u16` bitmasks instead of boolean arrays.
/// - `rows[i]`: Bit `d` is set if digit `d` is used in row `i`.
/// - `cols[j]`: Bit `d` is set if digit `d` is used in column `j`.
/// - `boxes[k]`: Bit `d` is set if digit `d` is used in box `k`.
///
/// **Bitwise Logic**:
/// - Check if digit `d` is available: `!rows[i] & !cols[j] & !boxes[k] & (1 << d)`.
/// - Set digit: `rows[i] |= (1 << d)`.
/// - Clear digit: `rows[i] &= !(1 << d)` (or `^=` if known to be set).
///
/// **Why Optimal**:
/// Bitwise operations are extremely fast (single CPU cycle) and cache-friendly.
///
/// **Time**: O(9^M). Lowest constant factor.
/// **Space**: O(1).
pub fn solve_sudoku_optimal(board: &mut Vec<Vec<char>>) {
    let mut rows = [0u16; 9];
    let mut cols = [0u16; 9];
    let mut boxes = [0u16; 9];

    // Initialize state
    for i in 0..9 {
        for j in 0..9 {
            if let Some(digit) = board[i][j].to_digit(10) {
                let bit = 1 << digit;
                let box_idx = (i / 3) * 3 + j / 3;
                rows[i] |= bit;
                cols[j] |= bit;
                boxes[box_idx] |= bit;
            }
        }
    }

    solve_bitmask(board, &mut rows, &mut cols, &mut boxes);
}

fn solve_bitmask(
    board: &mut Vec<Vec<char>>,
    rows: &mut [u16; 9],
    cols: &mut [u16; 9],
    boxes: &mut [u16; 9],
) -> bool {
    // Optimization: Find the empty cell with the fewest possibilities (Constraint Propagation)
    // This is a heuristic often used in Sudoku solvers, but for standard backtracking,
    // we'll stick to simple linear scan for clarity, similar to previous approaches.

    // RUST INSIGHT: We iterate manually to find the first empty cell.
    // A more advanced solver might pick the "best" cell to fill next.
    for i in 0..9 {
        for j in 0..9 {
            if board[i][j] == '.' {
                let box_idx = (i / 3) * 3 + j / 3;

                // Get all used numbers for this cell
                let used = rows[i] | cols[j] | boxes[box_idx];

                // Iterate through digits 1-9
                for d in 1..=9 {
                    let bit = 1 << d;
                    // Check if bit is NOT set in `used`
                    if used & bit == 0 {
                        // Place digit
                        board[i][j] = char::from_digit(d as u32, 10).unwrap();
                        rows[i] |= bit;
                        cols[j] |= bit;
                        boxes[box_idx] |= bit;

                        if solve_bitmask(board, rows, cols, boxes) {
                            return true;
                        }

                        // Backtrack
                        // XORing with the bit removes it (toggles 1 -> 0)
                        board[i][j] = '.';
                        rows[i] ^= bit;
                        cols[j] ^= bit;
                        boxes[box_idx] ^= bit;
                    }
                }
                return false;
            }
        }
    }
    true
}

/// Main entry point - uses optimal solution
pub fn solve_sudoku(board: &mut Vec<Vec<char>>) {
    solve_sudoku_optimal(board);
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to get a standard solvable board
    fn get_board() -> Vec<Vec<char>> {
        vec![
            vec!['5', '3', '.', '.', '7', '.', '.', '.', '.'],
            vec!['6', '.', '.', '1', '9', '5', '.', '.', '.'],
            vec!['.', '9', '8', '.', '.', '.', '.', '6', '.'],
            vec!['8', '.', '.', '.', '6', '.', '.', '.', '3'],
            vec!['4', '.', '.', '8', '.', '3', '.', '.', '1'],
            vec!['7', '.', '.', '.', '2', '.', '.', '.', '6'],
            vec!['.', '6', '.', '.', '.', '.', '2', '8', '.'],
            vec!['.', '.', '.', '4', '1', '9', '.', '.', '5'],
            vec!['.', '.', '.', '.', '8', '.', '.', '7', '9'],
        ]
    }

    // Helper to check if a board is valid and full
    fn is_solved(board: &[Vec<char>]) -> bool {
        for i in 0..9 {
            let mut row_set = 0u16;
            let mut col_set = 0u16;
            let mut box_set = 0u16;
            for j in 0..9 {
                // Check rows
                if let Some(d) = board[i][j].to_digit(10) {
                    row_set |= 1 << d;
                } else {
                    return false; // Empty cell
                }

                // Check cols
                if let Some(d) = board[j][i].to_digit(10) {
                    col_set |= 1 << d;
                }

                // Check boxes
                let r = 3 * (i / 3) + j / 3;
                let c = 3 * (i % 3) + j % 3;
                if let Some(d) = board[r][c].to_digit(10) {
                    box_set |= 1 << d;
                }
            }
            // Check if all digits 1-9 are present (bits 1-9 set)
            // 1<<1 | ... | 1<<9 = 1022 (binary 1111111110)
            let target = (1 << 10) - 2;
            if row_set != target || col_set != target || box_set != target {
                return false;
            }
        }
        true
    }

    #[test]
    fn test_naive_solver() {
        let mut board = get_board();
        solve_sudoku_naive(&mut board);
        assert!(
            is_solved(&board),
            "Naive solver failed to solve valid board"
        );
    }

    #[test]
    fn test_optimized_solver() {
        let mut board = get_board();
        solve_sudoku_optimized(&mut board);
        assert!(
            is_solved(&board),
            "Optimized solver failed to solve valid board"
        );
    }

    #[test]
    fn test_optimal_solver() {
        let mut board = get_board();
        solve_sudoku_optimal(&mut board);
        assert!(
            is_solved(&board),
            "Optimal solver failed to solve valid board"
        );
    }

    #[test]
    fn test_main_wrapper() {
        let mut board = get_board();
        solve_sudoku(&mut board);
        assert!(is_solved(&board));
    }

    #[test]
    fn test_already_solved() {
        let mut board = get_board();
        solve_sudoku_optimal(&mut board);
        let solved_clone = board.clone();

        // Run again on solved board
        solve_sudoku_optimal(&mut board);
        assert_eq!(board, solved_clone);
    }
}
