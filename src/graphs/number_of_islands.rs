//! # Number of Islands
//!
//! Given an `m x n` 2D binary grid `grid` which represents a map of '1's (land) and '0's (water),
//! return the number of islands.
//!
//! An island is surrounded by water and is formed by connecting adjacent lands horizontally or vertically.
//! You may assume all four edges of the grid are all surrounded by water.
//!
//! Difficulty: Medium
//! `LeetCode`: 200. Number of Islands
//! Link: <https://leetcode.com/problems/number-of-islands>/
//!
//! ## Why this matters in Rust
//! This problem is a classic example of **Graph Traversal** (DFS/BFS) on a grid. In Rust, it highlights:
//! 1.  **Ownership & Mutability**: The standard space-optimized solution modifies the grid in-place to mark visited cells.
//!     This requires `&mut Vec<Vec<char>>`, which means the caller loses ownership or must pass a mutable reference.
//! 2.  **Recursion vs Iteration**: DFS can hit stack limits on large grids, making iterative BFS/DFS safer in production.
//! 3.  **Boundary Checks**: Safe indexing in Rust (`get` vs `[]`) prevents panic on out-of-bounds access, critical in grid traversal.

use std::collections::VecDeque;

/// Optimal approach: Depth-First Search (Recursive), in-place marking
///
/// Approaches the problem by iterating through each cell. When a '1' is found, it increments the island count
/// and recursively sinks the entire island (turning '1's to '0's) so it's not counted again.
///
/// NOTE: DFS and BFS are equivalent-complexity alternatives here (both O(M * N) time). This DFS
/// version is labeled `_optimal` and the BFS version `_optimized` only to fit the standard suffix
/// scheme; neither is asymptotically superior. DFS is picked as the default for its lower constant
/// factors and minimal code, at the cost of O(M * N) worst-case recursion depth.
///
/// Time: O(M * N) - we visit each cell once.
/// Space: O(M * N) - worst case recursion stack (e.g., all land).
///
/// # Arguments
/// * `grid` - A mutable reference to the grid. Modified in-place to mark visited cells.
pub fn num_islands_optimal(grid: &mut Vec<Vec<char>>) -> i32 {
    if grid.is_empty() {
        return 0;
    }

    let rows = grid.len();
    let cols = grid[0].len();
    let mut count = 0;

    for r in 0..rows {
        for c in 0..cols {
            // RUST INSIGHT: We can access grid[r][c] directly here because we're iterating within bounds.
            // However, inside the helper, we'll need boundary checks.
            if grid[r][c] == '1' {
                count += 1;
                // Sink the island
                dfs(grid, r, c);
            }
        }
    }

    count
}

/// Helper function for DFS traversal.
///
/// Marks the current cell as visited ('0') and recursively visits neighbors.
fn dfs(grid: &mut Vec<Vec<char>>, r: usize, c: usize) {
    let rows = grid.len();
    let cols = grid[0].len();

    // Base case: check bounds and if cell is water
    // GOTCHA: Always check bounds first to avoid panic or use `get()`.
    // Since we pass r, c as usize, they are always >= 0. We only check upper bounds.
    if r >= rows || c >= cols || grid[r][c] == '0' {
        return;
    }

    // Mark as visited (sink the island)
    grid[r][c] = '0';

    // Visit neighbors (up, down, left, right)
    // RUST INSIGHT: usize cannot be negative.
    // To check "left" (c - 1) or "up" (r - 1), we must ensure c > 0 / r > 0 first.
    // Alternatively, we can use checking adds/subs or cast to isize, but careful logic is idiomatic.

    if r > 0 {
        dfs(grid, r - 1, c);
    } // Up
    dfs(grid, r + 1, c); // Down
    if c > 0 {
        dfs(grid, r, c - 1);
    } // Left
    dfs(grid, r, c + 1); // Right
}

/// Optimized approach: Breadth-First Search (Iterative), in-place marking
///
/// Uses a `VecDeque` queue to traverse the island level-by-level. This avoids recursion depth issues
/// on very large grids (stack overflow risk).
///
/// NOTE: This is an equivalent-complexity alternative to the DFS (`_optimal`) approach above
/// (both O(M * N) time). Its advantage is a much smaller O(min(M, N)) worst-case auxiliary space
/// and no recursion-depth limit, which is why it is a legitimate co-equal implementation rather
/// than a strictly worse one.
///
/// Time: O(M * N)
/// Space: O(min(M, N)) - worst case queue size is proportional to the smaller dimension (diagonal traversal).
pub fn num_islands_optimized(grid: &mut [Vec<char>]) -> i32 {
    if grid.is_empty() {
        return 0;
    }

    let rows = grid.len();
    let cols = grid[0].len();
    let mut count = 0;

    for r in 0..rows {
        for c in 0..cols {
            if grid[r][c] == '1' {
                count += 1;
                bfs(grid, r, c);
            }
        }
    }

    count
}

// Grid indices are bounded by rows/cols and stay non-negative after the guard, so isize<->usize
// casts are in range; `nr_isize`/`nc_isize` are the standard new-row/new-col names.
#[allow(clippy::cast_possible_wrap)]
#[allow(clippy::cast_sign_loss)]
#[allow(clippy::similar_names)]
fn bfs(grid: &mut [Vec<char>], start_r: usize, start_c: usize) {
    let rows = grid.len();
    let cols = grid[0].len();

    // Queue stores (row, col)
    let mut queue = VecDeque::new();
    queue.push_back((start_r, start_c));

    // Mark start as visited immediately to avoid re-queuing
    grid[start_r][start_c] = '0';

    // Directions: Down, Up, Right, Left
    // Using (isize, isize) allows simple addition, but requires casting back to usize
    let directions = [(1, 0), (-1, 0), (0, 1), (0, -1)];

    while let Some((r, c)) = queue.pop_front() {
        for (dr, dc) in directions {
            // Calculate new coordinates carefully with casting
            // RUST INSIGHT: `r as isize + dr` prevents underflow panic of `usize - 1`
            let nr_isize = r as isize + dr;
            let nc_isize = c as isize + dc;

            if nr_isize >= 0
                && nr_isize < rows as isize
                && nc_isize >= 0
                && nc_isize < cols as isize
            {
                let nr = nr_isize as usize;
                let nc = nc_isize as usize;

                if grid[nr][nc] == '1' {
                    grid[nr][nc] = '0'; // Mark visited
                    queue.push_back((nr, nc));
                }
            }
        }
    }
}

/// Brute force approach: Immutable Input (clone-then-sink)
///
/// Instead of modifying the caller's grid, we defensively clone the entire grid up front and run
/// the destructive DFS on the copy. This preserves the original data but pays a full O(M * N) extra
/// allocation on top of the traversal — the least space-efficient of the three approaches, which is
/// why it is labeled `_brute_force` despite matching the others on time complexity.
///
/// Time: O(M * N)
/// Space: O(M * N) - explicit copy of grid plus recursion stack.
#[allow(clippy::ptr_arg)] // Taking ownership or &Vec is a design choice here
#[must_use] 
pub fn num_islands_brute_force(grid: &Vec<Vec<char>>) -> i32 {
    // Clone the grid so we can mutate the copy
    let mut working_grid = grid.clone();
    num_islands_optimal(&mut working_grid)
}

/// Main entry point - uses the optimal (DFS) solution.
///
/// Takes ownership of the grid so callers get the canonical `LeetCode` signature; the grid is
/// consumed and sunk in place internally.
#[must_use]
pub fn number_of_islands(mut grid: Vec<Vec<char>>) -> i32 {
    num_islands_optimal(&mut grid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_optimal_dfs_basic() {
        let mut grid = vec![
            vec!['1', '1', '1', '1', '0'],
            vec!['1', '1', '0', '1', '0'],
            vec!['1', '1', '0', '0', '0'],
            vec!['0', '0', '0', '0', '0'],
        ];
        assert_eq!(num_islands_optimal(&mut grid), 1);
    }

    #[test]
    fn test_optimized_bfs_basic() {
        let mut grid = vec![
            vec!['1', '1', '0', '0', '0'],
            vec!['1', '1', '0', '0', '0'],
            vec!['0', '0', '1', '0', '0'],
            vec!['0', '0', '0', '1', '1'],
        ];
        // Islands: Top-left block, middle-single, bottom-right pair = 3
        assert_eq!(num_islands_optimized(&mut grid), 3);
    }

    #[test]
    fn test_empty_grid() {
        let mut grid: Vec<Vec<char>> = vec![];
        assert_eq!(num_islands_optimal(&mut grid), 0);
        assert_eq!(num_islands_optimized(&mut grid), 0);
        assert_eq!(number_of_islands(vec![]), 0);
    }

    #[test]
    fn test_single_cell() {
        let mut grid_land = vec![vec!['1']];
        assert_eq!(num_islands_optimal(&mut grid_land), 1);

        let mut grid_water = vec![vec!['0']];
        assert_eq!(num_islands_optimized(&mut grid_water), 0);
    }

    #[test]
    fn test_brute_force_immutable() {
        let grid = vec![
            vec!['1', '0', '1'],
            vec!['0', '1', '0'],
            vec!['1', '0', '1'],
        ];
        // Checkerboard pattern = 5 islands
        assert_eq!(num_islands_brute_force(&grid), 5);

        // Verify original grid is unchanged (non-destructive)
        assert_eq!(grid[0][0], '1');
    }

    #[test]
    fn test_main_wrapper() {
        let grid = vec![
            vec!['1', '1', '1', '1', '0'],
            vec!['1', '1', '0', '1', '0'],
            vec!['1', '1', '0', '0', '0'],
            vec!['0', '0', '0', '0', '0'],
        ];
        assert_eq!(number_of_islands(grid), 1);
    }

    #[test]
    fn test_all_approaches_agree() {
        // Cross-implementation agreement across DFS, BFS, and the immutable brute force.
        let grids = vec![
            vec![
                vec!['1', '1', '0', '0', '0'],
                vec!['1', '1', '0', '0', '0'],
                vec!['0', '0', '1', '0', '0'],
                vec!['0', '0', '0', '1', '1'],
            ],
            vec![
                vec!['1', '0', '1'],
                vec!['0', '1', '0'],
                vec!['1', '0', '1'],
            ],
            vec![vec!['1']],
            vec![vec!['0']],
        ];

        for grid in grids {
            let brute = num_islands_brute_force(&grid);

            let mut g_dfs = grid.clone();
            let optimal = num_islands_optimal(&mut g_dfs);

            let mut g_bfs = grid.clone();
            let optimized = num_islands_optimized(&mut g_bfs);

            let wrapped = number_of_islands(grid);

            assert_eq!(brute, optimal);
            assert_eq!(optimal, optimized);
            assert_eq!(optimized, wrapped);
        }
    }

    #[test]
    fn test_large_island() {
        // A generic large U shape
        let mut grid = vec![
            vec!['1', '0', '0', '1'],
            vec!['1', '0', '0', '1'],
            vec!['1', '1', '1', '1'],
        ];
        assert_eq!(num_islands_optimal(&mut grid), 1);
    }
}
