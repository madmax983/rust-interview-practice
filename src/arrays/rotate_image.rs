//! # 48. Rotate Image
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/rotate-image/>
//!
//! You are given an `n x n` 2D `matrix` representing an image, rotate the image by 90 degrees (clockwise).
//!
//! You have to rotate the image in-place, which means you have to modify the input 2D matrix directly.
//! DO NOT allocate another 2D matrix and do the rotation.
//!
//! ## Why This Matters in Rust
//!
//! This problem is an excellent showcase of in-place matrix manipulation in Rust. It highlights how Rust's
//! borrowing rules interact with 2D array indexing and demonstrates the elegance of standard library
//! functions like `[T]::reverse()`. It forces the programmer to think about `Copy` semantics when swapping
//! elements across different mutable slices.
//!
//! ## Approach
//!
//! - **Brute Force**: Allocate a new `n x n` matrix, copy the elements in their rotated positions, and then
//!   overwrite the original matrix. This requires O(N^2) extra space, which violates the strict in-place constraint
//!   of the problem but serves as a good conceptual starting point.
//! - **Optimal**: We can achieve a 90-degree clockwise rotation by combining two simple transformations:
//!   1. **Transpose the matrix**: Swap elements across the main diagonal (`matrix[i][j]` with `matrix[j][i]`).
//!   2. **Reverse each row**: Horizontally flip the matrix.
//!
//!   Since both operations can be done in-place, this approach requires O(1) extra space.
//!
//! ## Time and Space Complexity
//!
//! - **Time Complexity**: O(N^2), where N is the number of rows/columns. We visit each element a constant number of times.
//! - **Space Complexity**: O(1) for the optimal approach, as we only use a few variables for indices.

/// Brute Force Approach
///
/// Allocates a new matrix to hold the rotated values, then copies them back.
/// Time: O(N^2)
/// Space: O(N^2)
#[allow(clippy::needless_range_loop)]
pub fn rotate_brute_force(matrix: &mut [Vec<i32>]) {
    let n = matrix.len();
    if n == 0 {
        return;
    }

    // GOTCHA: Creating a new matrix violates the "in-place" requirement of the problem,
    // but it's a useful way to understand the index mapping: (i, j) -> (j, n - 1 - i).
    let mut temp = vec![vec![0; n]; n];

    for i in 0..n {
        for j in 0..n {
            temp[j][n - 1 - i] = matrix[i][j];
        }
    }

    // Copy back to original matrix
    for i in 0..n {
        for j in 0..n {
            matrix[i][j] = temp[i][j];
        }
    }
}

/// Optimal Approach
///
/// Transposes the matrix, then reverses each row.
/// Time: O(N^2)
/// Space: O(1)
#[allow(clippy::needless_range_loop)]
pub fn rotate_optimal(matrix: &mut [Vec<i32>]) {
    let n = matrix.len();

    // Step 1: Transpose the matrix (swap matrix[i][j] with matrix[j][i])
    for i in 0..n {
        for j in (i + 1)..n {
            // RUST INSIGHT: Since `i32` implements the `Copy` trait, we can safely read from
            // these indices and reassign them without fighting the borrow checker. If these were
            // non-Copy types (like `String`), we'd use `std::mem::swap` and need `split_at_mut`
            // to get simultaneous mutable references to different rows.
            let temp = matrix[i][j];
            matrix[i][j] = matrix[j][i];
            matrix[j][i] = temp;
        }
    }

    // Step 2: Reverse each row
    // RUST INSIGHT: We use `iter_mut()` to get mutable references to each row.
    // The standard library provides an highly optimized `.reverse()` method for slices.
    for row in matrix.iter_mut() {
        row.reverse();
    }
}

/// Main entry point for the problem.
pub fn rotate(matrix: &mut [Vec<i32>]) {
    rotate_optimal(matrix);
}

/// ## Alternative Approaches
///
/// - **Cycle Replacement (Four-way swap)**: Instead of transposing and reversing, you can rotate the matrix
///   ring by ring, swapping four elements at a time: `top-left -> top-right -> bottom-right -> bottom-left -> top-left`.
///   This is also O(N^2) time and O(1) space, but the indexing math is more complex and error-prone compared
///   to the Transpose + Reverse approach.

#[cfg(test)]
mod tests {
    use super::*;

    // Happy Path
    #[test]
    fn test_rotate_happy_path() {
        let mut matrix1 = vec![vec![1, 2, 3], vec![4, 5, 6], vec![7, 8, 9]];
        rotate_brute_force(&mut matrix1);
        assert_eq!(matrix1, vec![vec![7, 4, 1], vec![8, 5, 2], vec![9, 6, 3],]);

        let mut matrix2 = vec![vec![1, 2, 3], vec![4, 5, 6], vec![7, 8, 9]];
        rotate_optimal(&mut matrix2);
        assert_eq!(matrix2, vec![vec![7, 4, 1], vec![8, 5, 2], vec![9, 6, 3],]);
    }

    // Edge Case
    #[test]
    fn test_rotate_edge_case_single_element() {
        let mut matrix = vec![vec![1]];
        rotate_optimal(&mut matrix);
        assert_eq!(matrix, vec![vec![1]]);
    }

    // Stress/Boundary Test
    #[test]
    fn test_rotate_stress_boundaries_even_size() {
        let mut matrix = vec![
            vec![5, 1, 9, 11],
            vec![2, 4, 8, 10],
            vec![13, 3, 6, 7],
            vec![15, 14, 12, 16],
        ];
        rotate_optimal(&mut matrix);
        assert_eq!(
            matrix,
            vec![
                vec![15, 13, 2, 5],
                vec![14, 3, 4, 1],
                vec![12, 6, 8, 9],
                vec![16, 7, 10, 11],
            ]
        );
    }
}
