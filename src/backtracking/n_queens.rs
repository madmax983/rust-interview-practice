//! # 51. N-Queens
//!
//! **Difficulty: Hard**
//!
//! [LeetCode Problem 51](https://leetcode.com/problems/n-queens/)
//!
//! The **n-queens** puzzle is the problem of placing `n` queens on an `n x n` chessboard such that no two queens attack each other.
//! Given an integer `n`, return all distinct solutions to the **n-queens puzzle**. You may return the answer in any order.
//!
//! Each solution contains a distinct board configuration of the n-queens' placement, where `'Q'` and `'.'` both indicate a queen and an empty space, respectively.
//!
//! ## Why this matters in Rust
//! This problem is a classic example of **Backtracking** and **Bitwise Optimization**.
//! It demonstrates how changing the underlying data structure for state tracking (from `HashSet` to `Vec<bool>` to `Bitmask`)
//! can dramatically impact performance and memory usage without changing the algorithmic complexity.
//!
//! It also highlights:
//! - **Recursion**: Managing state across recursive calls.
//! - **Ownership**: Efficiently constructing the complex `Vec<Vec<String>>` output from a compact internal representation.
//! - **Iterators**: Using iterators to generate the board strings cleanly.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::backtracking::n_queens::solve_n_queens;
//!
//! let n = 4;
//! let solutions = solve_n_queens(n);
//! assert_eq!(solutions.len(), 2);
//! // Solution 1: [[".Q..","...Q","Q...","..Q."], ["..Q.","Q...","...Q",".Q.."]]
//! ```
//!
//! ## Constraints
//!
//! - `1 <= n <= 9`

#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::needless_pass_by_value)]

/// Brute Force Approach: Generate Permutations and Validate
///
/// **Strategy**:
/// 1. Generate all permutations of column indices `[0, 1, ..., n-1]`.
///    Each permutation represents a board where row `i` has a queen at column `cols[i]`.
///    This automatically satisfies the row and column constraints (one queen per row, one per column).
/// 2. For each permutation, check if any two queens are on the same diagonal.
///
/// **Time**: O(N!) - We generate N! permutations. For each, we check diagonals in O(N). Total O(N * N!).
/// **Space**: O(N) - Recursion stack and storage for the current permutation.
#[must_use]
pub fn solve_n_queens_brute_force(n: i32) -> Vec<Vec<String>> {
    let mut results = Vec::new();
    let mut cols: Vec<i32> = (0..n).collect();
    generate_permutations(n as usize, 0, &mut cols, &mut results);
    results
}

fn generate_permutations(
    n: usize,
    start: usize,
    cols: &mut Vec<i32>,
    results: &mut Vec<Vec<String>>,
) {
    if start == n {
        if is_valid_diagonal(cols) {
            results.push(format_board(cols, n));
        }
        return;
    }

    for i in start..n {
        cols.swap(start, i);
        generate_permutations(n, start + 1, cols, results);
        cols.swap(start, i); // Backtrack
    }
}

fn is_valid_diagonal(cols: &[i32]) -> bool {
    let n = cols.len();
    for i in 0..n {
        for j in (i + 1)..n {
            let row_diff = (j - i) as i32;
            let col_diff = (cols[j] - cols[i]).abs();
            if row_diff == col_diff {
                return false; // Same diagonal
            }
        }
    }
    true
}

/// Optimized Approach: Backtracking with Boolean Arrays
///
/// **Strategy**:
/// Use standard backtracking, but instead of checking all diagonals after placing all queens,
/// we check validity *incrementally*. We maintain three boolean arrays to track occupied constraints:
/// 1. `cols`: Is column `c` occupied?
/// 2. `diag1`: Is main diagonal `row - col` occupied? (Mapped to `row - col + n` to be non-negative).
/// 3. `diag2`: Is anti-diagonal `row + col` occupied?
///
/// **Time**: O(N!) - In the worst case, we explore many paths, but pruning cuts down the search space significantly.
/// **Space**: O(N) - Arrays for constraints and recursion stack.
#[must_use]
pub fn solve_n_queens_optimized(n: i32) -> Vec<Vec<String>> {
    let n_usize = n as usize;
    let mut results = Vec::new();
    let mut board_cols = vec![0; n_usize];

    // Constraint trackers
    // RUST INSIGHT: Using fixed-size vectors is much faster than HashSet for dense integer keys.
    let mut cols = vec![false; n_usize];
    let mut diag1 = vec![false; 2 * n_usize]; // row - col + n
    let mut diag2 = vec![false; 2 * n_usize]; // row + col

    backtrack_arrays(
        0,
        n_usize,
        &mut board_cols,
        &mut cols,
        &mut diag1,
        &mut diag2,
        &mut results,
    );
    results
}

#[allow(clippy::too_many_arguments)]
fn backtrack_arrays(
    row: usize,
    n: usize,
    board_cols: &mut Vec<i32>,
    cols: &mut [bool],
    diag1: &mut [bool],
    diag2: &mut [bool],
    results: &mut Vec<Vec<String>>,
) {
    if row == n {
        results.push(format_board(board_cols, n));
        return;
    }

    for col in 0..n {
        let d1 = row as i32 - col as i32 + n as i32;
        let d2 = row + col;

        // Check if valid
        // GOTCHA: Note the `d1` calculation. `row - col` can be negative, so we add `n`.
        if cols[col] || diag1[d1 as usize] || diag2[d2] {
            continue;
        }

        // Place queen
        board_cols[row] = col as i32;
        cols[col] = true;
        diag1[d1 as usize] = true;
        diag2[d2] = true;

        // Recurse
        backtrack_arrays(row + 1, n, board_cols, cols, diag1, diag2, results);

        // Backtrack (Remove queen)
        cols[col] = false;
        diag1[d1 as usize] = false;
        diag2[d2] = false;
    }
}

/// Optimal Approach: Backtracking with Bitmasks
///
/// **Strategy**:
/// Instead of boolean arrays, we use bits in an integer to track constraints.
/// - `cols`: Bit `i` is 1 if column `i` is occupied.
/// - `diag1`: Bit `i` is 1 if diagonal `i` is occupied. We shift these bits as we move rows.
/// - `diag2`: Bit `i` is 1 if anti-diagonal `i` is occupied.
///
/// **Time**: O(N!) - Same complexity, but much faster constant factor due to bitwise operations.
/// **Space**: O(N) - Recursion stack. Zero heap allocation for constraint tracking.
#[must_use]
pub fn solve_n_queens_optimal(n: i32) -> Vec<Vec<String>> {
    let mut results = Vec::new();
    let mut board_cols = vec![0; n as usize];

    // RUST INSIGHT: Recursion with value passing (Copy types) avoids mutable borrow hell.
    // By passing the updated state (bitmasks) directly to the next call, we avoid
    // the need to manually "undo" changes (backtrack step) on the state variables.
    // The state is implicit in the call stack.
    backtrack_bitmask(0, n as usize, 0, 0, 0, &mut board_cols, &mut results);
    results
}

fn backtrack_bitmask(
    row: usize,
    n: usize,
    cols: i32,
    diag1: i32,
    diag2: i32,
    board_cols: &mut Vec<i32>,
    results: &mut Vec<Vec<String>>,
) {
    if row == n {
        results.push(format_board(board_cols, n));
        return;
    }

    // Available positions for this row
    // `((1 << n) - 1)` creates a mask of N ones.
    // `!(cols | diag1 | diag2)` gives us bits that are 0 (available) as 1s (because of NOT).
    // We mask with `available_mask` to ensure we only consider bits within N bounds.
    let available_mask = ((1 << n) - 1) & !(cols | diag1 | diag2);

    let mut remaining = available_mask;
    while remaining != 0 {
        // Extract the lowest set bit
        // RUST INSIGHT: `x & -x` is a classic bit hack to get the lowest set bit.
        // It works because -x is Two's Complement (~x + 1).
        let p = remaining & -remaining;

        // Remove the bit from remaining
        remaining ^= p; // or remaining -= p

        // Find the column index from the bit (log2)
        // `trailing_zeros` is a hardware-accelerated intrinsic (TZCNT).
        let col = p.trailing_zeros() as usize;

        board_cols[row] = col as i32;

        // Recurse
        // Shift diag1 left (<< 1) because moving to next row shifts diagonals left relative to columns.
        // Shift diag2 right (>> 1) because moving to next row shifts anti-diagonals right relative to columns.
        backtrack_bitmask(
            row + 1,
            n,
            cols | p,
            (diag1 | p) << 1,
            (diag2 | p) >> 1,
            board_cols,
            results,
        );
    }
}

/// Helper function to format the board from column indices
fn format_board(cols: &[i32], n: usize) -> Vec<String> {
    cols.iter()
        .map(|&c| {
            let mut row = vec!['.'; n];
            row[c as usize] = 'Q';
            row.into_iter().collect()
        })
        .collect()
}

/// Main entry point - uses optimal solution
#[must_use]
pub fn solve_n_queens(n: i32) -> Vec<Vec<String>> {
    solve_n_queens_optimal(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to sort results for consistent comparison
    fn normalize_results(mut results: Vec<Vec<String>>) -> Vec<Vec<String>> {
        results.sort();
        results
    }

    #[test]
    fn test_brute_force_n4() {
        let results = solve_n_queens_brute_force(4);
        assert_eq!(results.len(), 2);

        let normalized = normalize_results(results);
        let expected = normalize_results(vec![
            vec![
                ".Q..".to_string(),
                "...Q".to_string(),
                "Q...".to_string(),
                "..Q.".to_string(),
            ],
            vec![
                "..Q.".to_string(),
                "Q...".to_string(),
                "...Q".to_string(),
                ".Q..".to_string(),
            ],
        ]);
        assert_eq!(normalized, expected);
    }

    #[test]
    fn test_optimized_n4() {
        let results = solve_n_queens_optimized(4);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_optimal_n4() {
        let results = solve_n_queens_optimal(4);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_all_approaches_n1() {
        let expected = vec![vec!["Q".to_string()]];

        let res_bf = solve_n_queens_brute_force(1);
        assert_eq!(res_bf, expected);

        let res_opt = solve_n_queens_optimized(1);
        assert_eq!(res_opt, expected);

        let res_optimal = solve_n_queens_optimal(1);
        assert_eq!(res_optimal, expected);
    }

    #[test]
    fn test_stress_n8() {
        // N=8 should have 92 distinct solutions
        let res_bf = solve_n_queens_brute_force(8);
        assert_eq!(res_bf.len(), 92);

        let res_opt = solve_n_queens_optimized(8);
        assert_eq!(res_opt.len(), 92);

        let res_optimal = solve_n_queens_optimal(8);
        assert_eq!(res_optimal.len(), 92);
    }

    #[test]
    fn test_no_solution_n2_n3() {
        assert_eq!(solve_n_queens_brute_force(2).len(), 0);
        assert_eq!(solve_n_queens_brute_force(3).len(), 0);

        assert_eq!(solve_n_queens_optimal(2).len(), 0);
        assert_eq!(solve_n_queens_optimal(3).len(), 0);
    }

    #[test]
    fn test_consistency() {
        // Check consistency across methods for N=5 (10 solutions)
        let n = 5;
        let bf = normalize_results(solve_n_queens_brute_force(n));
        let opt = normalize_results(solve_n_queens_optimized(n));
        let optimal = normalize_results(solve_n_queens_optimal(n));

        assert_eq!(bf.len(), 10);
        assert_eq!(bf, opt);
        assert_eq!(opt, optimal);
    }
}
