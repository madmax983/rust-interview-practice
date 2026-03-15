//! # 54. Spiral Matrix
//!
//! Given an `m x n` `matrix`, return all elements of the `matrix` in spiral order.
//!
//! Difficulty: Medium
//! Link: https://leetcode.com/problems/spiral-matrix/
//!
//! Why this matters in Rust:
//! This problem perfectly illustrates the tension between Rust's strict `usize` indexing
//! and algorithms that naturally want to use negative numbers or decrement past zero.
//! It also provides an excellent opportunity to demonstrate how we can encapsulate
//! complex, stateful traversal logic into a clean, reusable `Iterator` using Rust's
//! trait system, turning an imperative while-loop into a functional chain.

/// Straightforward approach: Imperative Boundary Tracking
///
/// We track four boundaries (`top`, `bottom`, `left`, `right`) and simulate
/// the spiral traversal layer by layer.
///
/// Time: O(M * N) where M and N are the dimensions of the matrix.
/// Space: O(1) auxiliary space (excluding the output vector).
///
/// # Gotcha
/// In languages like Java or C++, it's common to use signed integers (`int`) for boundaries.
/// In Rust, indices are strictly `usize`. If we use `usize` for `right` or `bottom`, decrementing
/// them when they are `0` will cause a panic (integer underflow). We must carefully manage
/// bounds or intentionally cast to `isize` for the boundary logic. Here, we use `isize`
/// to safely represent boundaries that might cross over each other and go below zero.
#[must_use]
#[allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)]
pub fn spiral_order_straightforward(matrix: Vec<Vec<i32>>) -> Vec<i32> {
    if matrix.is_empty() || matrix[0].is_empty() {
        return vec![];
    }

    let rows = matrix.len() as isize;
    let cols = matrix[0].len() as isize;
    let mut result = Vec::with_capacity((rows * cols) as usize);

    let mut top = 0isize;
    let mut bottom = rows - 1;
    let mut left = 0isize;
    let mut right = cols - 1;

    // RUST INSIGHT:
    // While loops with mutable external state are standard for this algorithm,
    // but they lack the composability of iterators.
    while top <= bottom && left <= right {
        // Traverse Right
        for c in left..=right {
            result.push(matrix[top as usize][c as usize]);
        }
        top += 1;

        // Traverse Down
        for r in top..=bottom {
            result.push(matrix[r as usize][right as usize]);
        }
        right -= 1;

        if top <= bottom {
            // Traverse Left
            for c in (left..=right).rev() {
                result.push(matrix[bottom as usize][c as usize]);
            }
            bottom -= 1;
        }

        if left <= right {
            // Traverse Up
            for r in (top..=bottom).rev() {
                result.push(matrix[r as usize][left as usize]);
            }
            left += 1;
        }
    }

    result
}

// -----------------------------------------------------------------------------
// Optimal Approach: Custom Iterator State Machine
// -----------------------------------------------------------------------------

/// The direction of our spiral traversal.
#[derive(Clone, Copy, Debug)]
enum Direction {
    Right,
    Down,
    Left,
    Up,
}

/// An iterator that yields elements of a matrix in spiral order.
///
/// # Rust Insight
/// By implementing `Iterator`, we encapsulate the messy boundary logic into
/// a discrete state machine. The consumer of this struct just calls `.next()`
/// or uses a `for` loop, entirely decoupled from the matrix dimension complexities.
pub struct SpiralIterator<'a, T> {
    matrix: &'a [Vec<T>],
    top: isize,
    bottom: isize,
    left: isize,
    right: isize,
    r: isize,
    c: isize,
    dir: Direction,
}

impl<'a, T> SpiralIterator<'a, T> {
    #[must_use]
    #[allow(clippy::cast_possible_wrap)]
    pub fn new(matrix: &'a [Vec<T>]) -> Self {
        if matrix.is_empty() || matrix[0].is_empty() {
            return Self {
                matrix,
                top: 0,
                bottom: -1, // immediately forces termination in `next`
                left: 0,
                right: -1,
                r: 0,
                c: 0,
                dir: Direction::Right,
            };
        }

        Self {
            matrix,
            top: 0,
            bottom: matrix.len() as isize - 1,
            left: 0,
            right: matrix[0].len() as isize - 1,
            r: 0,
            c: 0,
            dir: Direction::Right,
        }
    }
}

impl<'a, T> Iterator for SpiralIterator<'a, T> {
    type Item = &'a T;

    #[allow(clippy::cast_sign_loss)]
    fn next(&mut self) -> Option<Self::Item> {
        // If our boundaries have crossed, iteration is complete.
        if self.top > self.bottom || self.left > self.right {
            return None;
        }

        // Fetch the current item safely using usize casting.
        // We know r and c are within bounds because of the boundary checks above.
        let val = &self.matrix[self.r as usize][self.c as usize];

        // RUST INSIGHT:
        // Exhaustive pattern matching guarantees we handle every direction.
        // If we added a new direction to the enum, the compiler would force us to handle it here.
        match self.dir {
            Direction::Right => {
                if self.c == self.right {
                    self.top += 1;
                    self.dir = Direction::Down;
                    self.r += 1;
                } else {
                    self.c += 1;
                }
            }
            Direction::Down => {
                if self.r == self.bottom {
                    self.right -= 1;
                    self.dir = Direction::Left;
                    self.c -= 1;
                } else {
                    self.r += 1;
                }
            }
            Direction::Left => {
                if self.c == self.left {
                    self.bottom -= 1;
                    self.dir = Direction::Up;
                    self.r -= 1;
                } else {
                    self.c -= 1;
                }
            }
            Direction::Up => {
                if self.r == self.top {
                    self.left += 1;
                    self.dir = Direction::Right;
                    self.c += 1;
                } else {
                    self.r -= 1;
                }
            }
        }

        Some(val)
    }
}

/// Optimal approach: Utilizing a Custom Iterator
///
/// This approach shows how we can use our `SpiralIterator` to elegantly
/// process the matrix.
///
/// Time: O(M * N) - visits each element exactly once.
/// Space: O(1) auxiliary space (iterator state is constant size).
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn spiral_order_optimal(matrix: Vec<Vec<i32>>) -> Vec<i32> {
    // We can just collect our iterator!
    // We use `.copied()` because our iterator yields `&i32` but we want `i32`.
    SpiralIterator::new(&matrix).copied().collect()
}

/// Main entry point - uses optimal solution
#[must_use]
pub fn spiral_order(matrix: Vec<Vec<i32>>) -> Vec<i32> {
    spiral_order_optimal(matrix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_happy_path_square() {
        let matrix = vec![
            vec![1, 2, 3],
            vec![4, 5, 6],
            vec![7, 8, 9]
        ];
        let expected = vec![1, 2, 3, 6, 9, 8, 7, 4, 5];
        assert_eq!(spiral_order_straightforward(matrix.clone()), expected);
        assert_eq!(spiral_order_optimal(matrix.clone()), expected);
        assert_eq!(spiral_order(matrix), expected);
    }

    #[test]
    fn test_happy_path_rectangle() {
        let matrix = vec![
            vec![1, 2, 3, 4],
            vec![5, 6, 7, 8],
            vec![9, 10, 11, 12]
        ];
        let expected = vec![1, 2, 3, 4, 8, 12, 11, 10, 9, 5, 6, 7];
        assert_eq!(spiral_order_straightforward(matrix.clone()), expected);
        assert_eq!(spiral_order_optimal(matrix), expected);
    }

    #[test]
    fn test_edge_case_single_row_col() {
        // Single row
        let matrix = vec![vec![1, 2, 3]];
        let expected = vec![1, 2, 3];
        assert_eq!(spiral_order_straightforward(matrix.clone()), expected);
        assert_eq!(spiral_order_optimal(matrix.clone()), expected);

        // Single column
        let matrix_col = vec![vec![1], vec![2], vec![3]];
        let expected_col = vec![1, 2, 3];
        assert_eq!(spiral_order_straightforward(matrix_col.clone()), expected_col);
        assert_eq!(spiral_order_optimal(matrix_col), expected_col);
    }

    #[test]
    fn test_stress_boundary_empty_or_single_element() {
        // Empty matrix
        let empty: Vec<Vec<i32>> = vec![];
        let expected_empty: Vec<i32> = vec![];
        assert_eq!(spiral_order_straightforward(empty.clone()), expected_empty);
        assert_eq!(spiral_order_optimal(empty), expected_empty);

        // Empty inner
        let empty_inner: Vec<Vec<i32>> = vec![vec![]];
        assert_eq!(spiral_order_straightforward(empty_inner.clone()), expected_empty);
        assert_eq!(spiral_order_optimal(empty_inner), expected_empty);

        // Single element
        let single = vec![vec![42]];
        assert_eq!(spiral_order_straightforward(single.clone()), vec![42]);
        assert_eq!(spiral_order_optimal(single), vec![42]);
    }
}

// -----------------------------------------------------------------------------
// Alternative Approaches
// -----------------------------------------------------------------------------
// 1. Matrix Rotation / Peeling:
//    Pop the first row, then rotate the remaining matrix counter-clockwise and recurse.
//    While elegant in python (`return list(matrix.pop(0)) + spiralOrder(list(zip(*matrix))[::-1])`),
//    this is very slow and memory-intensive in Rust due to constant reallocations,
//    making it an anti-pattern for performance-sensitive Rust code.
//
// 2. Visited Boolean Matrix / HashSet:
//    Keep a direction vector and move forward until hitting a boundary or an already visited cell.
//    This works nicely and avoids shrinking boundaries, but requires O(M * N) extra space
//    for the `visited` map, making it strictly worse than the O(1) space pointer approach.
