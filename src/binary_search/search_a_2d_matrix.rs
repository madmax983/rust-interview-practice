//! # 74. Search a 2D Matrix
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/search-a-2d-matrix>/
//!
//! This problem extends 1D binary search over to a 2D matrix, demonstrating
//! index math and safe indexing. It highlights Rust's `as` casting for handling calculations
//! securely across array dimensions while ensuring no overflow or out-of-bounds panics happen.
//!
//! Note: single canonical implementation; the brute/optimized/optimal progression does not apply here.

use std::cmp::Ordering;

/// Approach: 1D Binary Search
///
/// Time Complexity: O(log(M * N))
/// Space Complexity: O(1)
///
/// Why this is idiomatic Rust:
/// Conceptually flatten the matrix and use binary search. We find the elements by
/// converting the mid index of our imaginary 1D array back to `row` and `col` of the 2D matrix.
/// We utilize `usize` indexing with strict bounds checking.
pub struct Solution;

impl Solution {
    #[must_use]
    #[allow(clippy::needless_pass_by_value)]
    pub fn search_matrix(matrix: Vec<Vec<i32>>, target: i32) -> bool {
        if matrix.is_empty() || matrix[0].is_empty() {
            return false;
        }

        let rows = matrix.len();
        let cols = matrix[0].len();

        let mut left = 0;
        let mut right = rows * cols - 1;

        while left <= right {
            let mid = left + (right - left) / 2;

            // Map 1D `mid` index to 2D matrix `(row, col)`
            // RUST INSIGHT: / and % efficiently handle the dimensional mapping.
            let row = mid / cols;
            let col = mid % cols;

            let mid_val = matrix[row][col];

            match mid_val.cmp(&target) {
                Ordering::Equal => return true,
                Ordering::Less => left = mid + 1,
                Ordering::Greater => {
                    if mid == 0 {
                        break;
                    }
                    right = mid - 1;
                }
            }
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_happy_path() {
        let matrix = vec![vec![1, 3, 5, 7], vec![10, 11, 16, 20], vec![23, 30, 34, 60]];
        assert!(Solution::search_matrix(matrix.clone(), 3));
        assert!(!Solution::search_matrix(matrix.clone(), 13));
    }

    #[test]
    fn test_edge_cases() {
        assert!(!Solution::search_matrix(vec![], 0));
        assert!(!Solution::search_matrix(vec![vec![]], 0));
        assert!(Solution::search_matrix(vec![vec![1]], 1));
        assert!(!Solution::search_matrix(vec![vec![1]], 0));
    }

    #[test]
    fn test_stress_boundaries() {
        let row_matrix = vec![vec![1, 3, 5, 7, 9]];
        assert!(Solution::search_matrix(row_matrix.clone(), 5));
        assert!(!Solution::search_matrix(row_matrix.clone(), 4));

        let col_matrix = vec![vec![1], vec![3], vec![5], vec![7], vec![9]];
        assert!(Solution::search_matrix(col_matrix.clone(), 5));
        assert!(!Solution::search_matrix(col_matrix.clone(), 4));
    }
}
