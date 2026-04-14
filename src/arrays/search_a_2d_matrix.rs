//! # 74. Search a 2D Matrix
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/search-a-2d-matrix/>
//!
//! You are given an `m x n` integer matrix `matrix` with the following two properties:
//! - Each row is sorted in non-decreasing order.
//! - The first integer of each row is greater than the last integer of the previous row.
//!
//! Given an integer `target`, return `true` if `target` is in `matrix` or `false` otherwise.
//!
//! You must write a solution in `O(log(m * n))` time complexity.
//!
//! ## Why This Matters in Rust
//!
//! This problem is an excellent exercise in translating 1D binary search over to a 2D array, demonstrating
//! index math and safe indexing. It highlights Rust's `as` casting for handling calculations securely across
//! array dimensions (like mapping a 1D index `mid` to a 2D matrix element `matrix[row][col]`), while
//! ensuring no overflow or out-of-bounds panics happen in optimal approaches.
//!
//! ## Approach
//!
//! We provide two implementations:
//! 1. **Brute Force**: A naive linear scan nested loop to check every element for `target`. Time `O(m * n)`.
//! 2. **Optimal**: Binary search. Since the matrix is essentially a sorted 1D array cut into rows, we can treat it as a flattened sorted array of length `m * n`. We perform binary search by mapping a 1D index to a 2D `(row, col)` pair via `/` and `%` operators. Time `O(log(m * n))`.
//!
//! ## Alternative Approaches
//!
//! - Binary searching the first column to find the potential row, and then binary searching within that specific row. This still yields `O(log m + log n)` which mathematically equals `O(log(m * n))`. This avoids the `(row, col)` integer math mapping and can prevent potential overflow issues if `m * n` is excessively large (though not possible within constraints here).

/// Brute Force Approach
///
/// Time: O(M * N) - worst case scans the entire matrix
/// Space: O(1) - constant extra space used
///
/// This approach iterates through every cell in the matrix.
/// Idiomatic Rust replaces manual looping with iterator chaining `.iter().flatten().any()`.
#[must_use]
pub fn search_matrix_brute_force(matrix: Vec<Vec<i32>>, target: i32) -> bool {
    // RUST INSIGHT: `.flatten()` seamlessly transforms our `Vec<Vec<i32>>`
    // (an iterator of iterators) into a single flat iterator of `&i32`.
    // `.any()` provides short-circuiting logic just like a manual `return true`.
    matrix.iter().flatten().any(|&val| val == target)
}

/// Optimal Approach: 1D Binary Search
///
/// Time: O(log(M * N)) - binary search bounds halved each iteration
/// Space: O(1) - just integer pointers for `left`, `right`, and `mid`
///
/// Conceptually flatten the matrix and use binary search. We find the elements by
/// converting the mid index of our imaginary 1D array back to `row` and `col` of the 2D matrix.
#[must_use]
pub fn search_matrix_optimal(matrix: Vec<Vec<i32>>, target: i32) -> bool {
    if matrix.is_empty() || matrix[0].is_empty() {
        return false;
    }

    let rows = matrix.len();
    let cols = matrix[0].len();

    // RUST INSIGHT: We use `usize` for indexing, but mathematical operations (like calculating right bound)
    // must not underflow. Since `rows >= 1` and `cols >= 1`, `rows * cols` is at least 1.
    let mut left = 0;
    // GOTCHA: Ensure the subtraction doesn't underflow.
    // With our early return checks, `rows * cols` is > 0.
    let mut right = rows * cols - 1;

    while left <= right {
        // Prevent potential overflow instead of `(left + right) / 2`
        let mid = left + (right - left) / 2;

        // Map 1D `mid` index to 2D matrix `(row, col)`
        let row = mid / cols;
        let col = mid % cols;

        let mid_val = matrix[row][col];

        if mid_val == target {
            return true;
        } else if mid_val < target {
            // Target is greater, search the right half
            left = mid + 1;
        } else {
            // Target is smaller, search the left half.
            // RUST INSIGHT: Since `mid` is `usize`, `mid - 1` could underflow if `mid == 0`.
            // We use `checked_sub` or break manually if `mid == 0`.
            if mid == 0 {
                break;
            }
            right = mid - 1;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force_happy_path() {
        let matrix = vec![vec![1, 3, 5, 7], vec![10, 11, 16, 20], vec![23, 30, 34, 60]];
        assert!(search_matrix_brute_force(matrix.clone(), 3));
        assert!(!search_matrix_brute_force(matrix.clone(), 13));
    }

    #[test]
    fn test_optimal_happy_path() {
        let matrix = vec![vec![1, 3, 5, 7], vec![10, 11, 16, 20], vec![23, 30, 34, 60]];
        assert!(search_matrix_optimal(matrix.clone(), 3));
        assert!(!search_matrix_optimal(matrix.clone(), 13));
    }

    #[test]
    fn test_edge_cases() {
        // Empty matrix
        assert!(!search_matrix_optimal(vec![], 0));

        // Matrix with empty row
        assert!(!search_matrix_optimal(vec![vec![]], 0));

        // Single element matrix (found)
        assert!(search_matrix_optimal(vec![vec![1]], 1));

        // Single element matrix (not found)
        assert!(!search_matrix_optimal(vec![vec![1]], 0));
        assert!(!search_matrix_optimal(vec![vec![1]], 2));
    }

    #[test]
    fn test_stress_single_row_col() {
        // Single row
        let row_matrix = vec![vec![1, 3, 5, 7, 9]];
        assert!(search_matrix_optimal(row_matrix.clone(), 5));
        assert!(!search_matrix_optimal(row_matrix.clone(), 4));

        // Single column
        let col_matrix = vec![vec![1], vec![3], vec![5], vec![7], vec![9]];
        assert!(search_matrix_optimal(col_matrix.clone(), 5));
        assert!(!search_matrix_optimal(col_matrix.clone(), 4));
    }
}
