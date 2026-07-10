//! # 994. Rotting Oranges
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/rotting-oranges/>
//!
//! You are given an `m x n` `grid` where each cell can have one of three values:
//! - `0` representing an empty cell,
//! - `1` representing a fresh orange, or
//! - `2` representing a rotten orange.
//!
//! Every minute, any fresh orange that is 4-directionally adjacent to a rotten orange becomes rotten.
//!
//! Return the minimum number of minutes that must elapse until no cell has a fresh orange. If this is impossible, return `-1`.
//!
//! ## Why this matters in Rust
//! This problem perfectly illustrates **Multi-source Breadth-First Search (BFS)** using Rust's `std::collections::VecDeque`.
//! It highlights:
//! 1. **Zero-cost Abstractions**: Using tuple destructuring `(r, c)` and iterating over a predefined array of directions.
//! 2. **Explicit Boundary Checks**: Safe indexing and `usize` conversions.
//! 3. **Ownership and Mutability**: Modifying the grid in-place while traversing it.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::graphs::rotting_oranges::oranges_rotting;
//!
//! let grid = vec![
//!     vec![2, 1, 1],
//!     vec![1, 1, 0],
//!     vec![0, 1, 1]
//! ];
//! assert_eq!(oranges_rotting(grid), 4);
//! ```

use std::collections::VecDeque;

// =========================================================================================
// Brute Force Approach
// =========================================================================================

/// Brute Force: Iterative Simulation
///
/// Simulates the rotting process minute by minute. In each minute, we scan the entire grid to find
/// freshly rotted oranges from the previous minute, and rot their neighbors.
/// We use a cloned grid to avoid rotting oranges and propagating them in the same minute.
///
/// Time: O((M * N)^2) - worst case, a single line of oranges rots one by one, taking M*N passes.
/// Space: O(M * N) - we need a copy of the grid for each iteration.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
// Grid indices are bounded by rows/cols and stay non-negative, so isize<->usize casts are in range.
#[allow(clippy::cast_possible_wrap)]
#[allow(clippy::cast_sign_loss)]
pub fn oranges_rotting_brute_force(mut grid: Vec<Vec<i32>>) -> i32 {
    let rows = grid.len();
    if rows == 0 {
        return 0;
    }
    let cols = grid[0].len();
    let mut minutes = 0;

    loop {
        let mut changed = false;
        let mut next_grid = grid.clone();

        for r in 0..rows {
            for c in 0..cols {
                if grid[r][c] == 2 {
                    // Check 4 neighbors
                    let directions = [(0, 1), (0, -1), (1, 0), (-1, 0)];
                    for (dr, dc) in directions {
                        let nr = r as isize + dr;
                        let nc = c as isize + dc;
                        if nr >= 0 && nr < rows as isize && nc >= 0 && nc < cols as isize {
                            let nr = nr as usize;
                            let nc = nc as usize;
                            if grid[nr][nc] == 1 {
                                next_grid[nr][nc] = 2;
                                changed = true;
                            }
                        }
                    }
                }
            }
        }

        if !changed {
            break;
        }
        grid = next_grid;
        minutes += 1;
    }

    // Check if any fresh oranges are left
    if grid.iter().flatten().any(|&cell| cell == 1) {
        return -1;
    }

    minutes
}

// =========================================================================================
// Optimal Approach
// =========================================================================================

/// Optimal: Multi-source Breadth-First Search (BFS)
///
/// Instead of scanning the entire grid repeatedly, we start by enqueuing all initially
/// rotten oranges. We also count the fresh oranges. Then, we process the queue level
/// by level. Each level represents one minute.
///
/// Time: O(M * N) - Each cell is visited at most once.
/// Space: O(M * N) - Worst case queue size is all oranges in the grid.
///
/// ## Rust Insight
/// We use `std::collections::VecDeque` for efficient front-popping in our BFS queue.
/// A queue size variable helps us track levels (minutes).
///
/// # Panics
/// Does not panic on valid input: the inner `pop_front().unwrap()` runs exactly `queue.len()`
/// times per level, so the queue is always non-empty when it is called.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
// Grid indices are bounded by rows/cols and stay non-negative, so isize<->usize casts are in range;
// `nr_isize`/`nc_isize` are the standard new-row/new-col names.
#[allow(clippy::cast_possible_wrap)]
#[allow(clippy::cast_sign_loss)]
#[allow(clippy::similar_names)]
pub fn oranges_rotting_optimal(mut grid: Vec<Vec<i32>>) -> i32 {
    let rows = grid.len();
    if rows == 0 {
        return 0;
    }
    let cols = grid[0].len();

    let mut queue = VecDeque::new();
    let mut fresh_count = 0;

    // 1. Initial scan: enqueue all rotten oranges and count fresh ones
    for (r, row) in grid.iter().enumerate() {
        for (c, &cell) in row.iter().enumerate() {
            // RUST INSIGHT: Match is exhaustive. If grid values were an enum, this would be even safer.
            match cell {
                2 => queue.push_back((r, c)),
                1 => fresh_count += 1,
                _ => {} // Empty cell
            }
        }
    }

    if fresh_count == 0 {
        return 0; // No fresh oranges to rot
    }

    let mut minutes = 0;
    let directions = [(0, 1), (0, -1), (1, 0), (-1, 0)];

    // 2. Multi-source BFS
    while !queue.is_empty() && fresh_count > 0 {
        let level_size = queue.len();

        for _ in 0..level_size {
            let (r, c) = queue.pop_front().unwrap(); // Safe because queue is not empty

            for (dr, dc) in directions {
                // GOTCHA: Using `as isize` prevents underflow panics when checking negative directions.
                let nr_isize = r as isize + dr;
                let nc_isize = c as isize + dc;

                if nr_isize >= 0
                    && nr_isize < rows as isize
                    && nc_isize >= 0
                    && nc_isize < cols as isize
                {
                    let nr = nr_isize as usize;
                    let nc = nc_isize as usize;

                    if grid[nr][nc] == 1 {
                        // Rot the fresh orange
                        grid[nr][nc] = 2;
                        fresh_count -= 1;
                        queue.push_back((nr, nc));
                    }
                }
            }
        }
        minutes += 1;
    }

    if fresh_count > 0 { -1 } else { minutes }
}

/// Main entry point - uses optimal solution.
#[must_use]
pub fn oranges_rotting(grid: Vec<Vec<i32>>) -> i32 {
    oranges_rotting_optimal(grid)
}

// =========================================================================================
// Tests
// =========================================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_happy_path() {
        let grid = vec![vec![2, 1, 1], vec![1, 1, 0], vec![0, 1, 1]];
        assert_eq!(oranges_rotting_brute_force(grid.clone()), 4);
        assert_eq!(oranges_rotting_optimal(grid), 4);
    }

    #[test]
    fn test_impossible_to_rot() {
        let grid = vec![vec![2, 1, 1], vec![0, 1, 1], vec![1, 0, 1]];
        assert_eq!(oranges_rotting_brute_force(grid.clone()), -1);
        assert_eq!(oranges_rotting_optimal(grid), -1);
    }

    #[test]
    fn test_already_rotten_or_empty() {
        let grid = vec![vec![0, 2]];
        assert_eq!(oranges_rotting_brute_force(grid.clone()), 0);
        assert_eq!(oranges_rotting_optimal(grid), 0);
    }

    #[test]
    fn test_empty_grid() {
        let grid: Vec<Vec<i32>> = vec![];
        assert_eq!(oranges_rotting_brute_force(grid.clone()), 0);
        assert_eq!(oranges_rotting_optimal(grid), 0);
    }

    #[test]
    fn test_stress_boundary() {
        // A long line of oranges, with one rotten at the start
        // 2, 1, 1, 1, 1...
        let mut grid = vec![vec![1; 100]];
        grid[0][0] = 2;

        assert_eq!(oranges_rotting_brute_force(grid.clone()), 99);
        assert_eq!(oranges_rotting_optimal(grid), 99);
    }
}

// =========================================================================================
// Alternative Approaches
// =========================================================================================
//
// 1. **In-place State Mutation (No Queue)**: You can use special state variables (like using
//    minutes elapsed + 2 as the new rotten value) to simulate BFS entirely in-place without a queue.
//    This saves O(N) space but makes the code less readable and more prone to errors.
//
// 2. **Union-Find**: Not suitable for this problem, as Union-Find tracks connectivity but not the
//    shortest path (minutes elapsed), which BFS handles naturally.
