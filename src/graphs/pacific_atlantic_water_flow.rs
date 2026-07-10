//! # 417. Pacific Atlantic Water Flow
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/pacific-atlantic-water-flow/>
//!
//! There is an `m x n` rectangular island that borders both the Pacific Ocean and Atlantic Ocean.
//! The Pacific Ocean touches the island's left and top edges, and the Atlantic Ocean touches the island's right and bottom edges.
//!
//! The island is partitioned into a grid of square cells. You are given an `m x n` integer matrix `heights`
//! where `heights[r][c]` represents the height above sea level of the cell at coordinate `(r, c)`.
//!
//! The island receives a lot of rain, and the rain water can flow to neighboring cells directly north, south, east, and west
//! if the neighboring cell's height is less than or equal to the current cell's height. Water can flow from any cell adjacent to an ocean into the ocean.
//!
//! Return a 2D list of grid coordinates `result` where `result[i] = [ri, ci]` denotes that rain water can flow from cell `(ri, ci)` to both the Pacific and Atlantic oceans.
//!
//! This problem perfectly demonstrates:
//! 1.  **Reverse Thinking**: Instead of starting from every node and seeing if it reaches the ocean, start from the ocean and see which nodes can reach it by traversing "uphill".
//! 2.  **Grid Traversal in Rust**: Handling boundaries safely without panics.
//! 3.  **State Management**: Keeping track of visited states efficiently.
//!
//!
//! ## Alternative Approaches
//! - A Union-Find (Disjoint Set) approach can be used by connecting cells to a virtual "Pacific" node and "Atlantic" node, but grid problems with explicit boundary connections are often more naturally solved with BFS/DFS.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::graphs::pacific_atlantic_water_flow::pacific_atlantic;
//!
//! let heights = vec![
//!     vec![1, 2, 2, 3, 5],
//!     vec![3, 2, 3, 4, 4],
//!     vec![2, 4, 5, 3, 1],
//!     vec![6, 7, 1, 4, 5],
//!     vec![5, 1, 1, 2, 4]
//! ];
//! let expected = vec![
//!     vec![0, 4], vec![1, 3], vec![1, 4], vec![2, 2], vec![3, 0], vec![3, 1], vec![4, 0]
//! ];
//! assert_eq!(pacific_atlantic(heights), expected);
//! ```

use std::collections::VecDeque;

// =========================================================================================
// Brute Force Approach
// =========================================================================================

/// Brute Force: DFS from every cell
///
/// For every cell in the grid, perform a DFS to check if we can find a path to both
/// the Pacific and Atlantic oceans.
///
/// Time: O((M*N)^2) - In the worst case, every DFS visits every other cell.
/// Space: O(M*N) - Recursion stack for DFS.
///
/// # Gotcha
/// This approach will likely TLE (Time Limit Exceeded) on `LeetCode` for larger grids.
#[must_use]
pub fn pacific_atlantic_brute_force(heights: Vec<Vec<i32>>) -> Vec<Vec<i32>> {
    let rows = heights.len();
    if rows == 0 {
        return vec![];
    }
    let cols = heights[0].len();
    let mut result = Vec::new();

    for r in 0..rows {
        for c in 0..cols {
            let mut visited = vec![vec![false; cols]; rows];
            let mut can_reach_pacific = false;
            let mut can_reach_atlantic = false;

            #[allow(clippy::too_many_arguments)]
            fn dfs(
                r: usize,
                c: usize,
                rows: usize,
                cols: usize,
                heights: &[Vec<i32>],
                visited: &mut [Vec<bool>],
                can_reach_pacific: &mut bool,
                can_reach_atlantic: &mut bool,
            ) {
                if visited[r][c] {
                    return;
                }
                visited[r][c] = true;

                if r == 0 || c == 0 {
                    *can_reach_pacific = true;
                }
                if r == rows - 1 || c == cols - 1 {
                    *can_reach_atlantic = true;
                }

                if *can_reach_pacific && *can_reach_atlantic {
                    return;
                }

                let directions = [(0, 1), (1, 0), (0, -1), (-1, 0)];
                for (dr, dc) in directions {
                    let nr = r as i32 + dr;
                    let nc = c as i32 + dc;

                    if nr >= 0 && nr < rows as i32 && nc >= 0 && nc < cols as i32 {
                        let nr = nr as usize;
                        let nc = nc as usize;
                        if heights[nr][nc] <= heights[r][c] {
                            dfs(
                                nr,
                                nc,
                                rows,
                                cols,
                                heights,
                                visited,
                                can_reach_pacific,
                                can_reach_atlantic,
                            );
                        }
                    }
                }
            }

            dfs(
                r,
                c,
                rows,
                cols,
                &heights,
                &mut visited,
                &mut can_reach_pacific,
                &mut can_reach_atlantic,
            );

            if can_reach_pacific && can_reach_atlantic {
                result.push(vec![r as i32, c as i32]);
            }
        }
    }

    result
}

// =========================================================================================
// Optimal Approach: Reverse DFS
// =========================================================================================

/// Optimal approach: Reverse DFS from the Ocean
///
/// Instead of starting from each cell and checking if it can reach the ocean,
/// we start from the ocean boundaries and traverse backwards (uphill).
/// We maintain two sets of visited cells: one for Pacific and one for Atlantic.
/// The intersection of both sets is our answer.
///
/// NOTE: Reverse DFS and reverse BFS are equivalent-complexity alternatives here (both O(M*N)).
/// This DFS version is labeled `_optimal` and the BFS version `_optimized` only to fit the
/// standard suffix scheme; neither is asymptotically superior. DFS is picked as the default for
/// its compact recursive form; BFS trades that for bounded stack usage.
///
/// Time: O(M*N) - Each cell is visited at most twice (once for each ocean).
/// Space: O(M*N) - Visited sets and recursion stack.
///
/// # Rust Insight
/// Using `&mut HashSet<(usize, usize)>` to pass state is idiomatic when you need
/// a dynamically sized, fast lookup set across multiple recursive calls. Alternatively,
/// a `vec![vec![false; cols]; rows]` matrix is generally faster and uses less memory overhead.
#[must_use]
pub fn pacific_atlantic_optimal(heights: Vec<Vec<i32>>) -> Vec<Vec<i32>> {
    let rows = heights.len();
    if rows == 0 {
        return vec![];
    }
    let cols = heights[0].len();

    let mut pacific_reachable = vec![vec![false; cols]; rows];
    let mut atlantic_reachable = vec![vec![false; cols]; rows];

    fn dfs(
        r: usize,
        c: usize,
        reachable: &mut [Vec<bool>],
        prev_height: i32,
        heights: &[Vec<i32>],
        rows: usize,
        cols: usize,
    ) {
        if reachable[r][c] || heights[r][c] < prev_height {
            return;
        }

        reachable[r][c] = true;

        let directions = [(0, 1), (1, 0), (0, -1), (-1, 0)];
        for (dr, dc) in directions {
            let nr = r as i32 + dr;
            let nc = c as i32 + dc;

            if nr >= 0 && nr < rows as i32 && nc >= 0 && nc < cols as i32 {
                dfs(
                    nr as usize,
                    nc as usize,
                    reachable,
                    heights[r][c],
                    heights,
                    rows,
                    cols,
                );
            }
        }
    }

    // Top and Bottom edges
    for c in 0..cols {
        dfs(
            0,
            c,
            &mut pacific_reachable,
            heights[0][c],
            &heights,
            rows,
            cols,
        );
        dfs(
            rows - 1,
            c,
            &mut atlantic_reachable,
            heights[rows - 1][c],
            &heights,
            rows,
            cols,
        );
    }

    // Left and Right edges
    for r in 0..rows {
        dfs(
            r,
            0,
            &mut pacific_reachable,
            heights[r][0],
            &heights,
            rows,
            cols,
        );
        dfs(
            r,
            cols - 1,
            &mut atlantic_reachable,
            heights[r][cols - 1],
            &heights,
            rows,
            cols,
        );
    }

    // RUST INSIGHT: Here we use iterator combinators instead of nested manual loops to build the final result.
    // This showcases idiomatic Rust, leveraging `flat_map` and `filter_map` for a zero-cost abstraction.
    (0..rows)
        .flat_map(|r| (0..cols).map(move |c| (r, c)))
        .filter_map(|(r, c)| {
            if pacific_reachable[r][c] && atlantic_reachable[r][c] {
                Some(vec![r as i32, c as i32])
            } else {
                None
            }
        })
        .collect()
}

// =========================================================================================
// Optimized Approach: Reverse BFS
// =========================================================================================

/// Optimized approach: Reverse BFS from the Ocean
///
/// Same logic as the optimal reverse-DFS approach, but using Breadth-First Search.
/// BFS avoids deep recursion, which can prevent stack overflows in languages without
/// tail-call optimization or deep default stacks, though Rust's stack is usually fine
/// for standard grid sizes.
///
/// NOTE: This is an equivalent-complexity alternative to the reverse-DFS (`_optimal`) approach
/// above (both O(M*N)); it is labeled `_optimized` only to fit the suffix scheme, and its genuine
/// advantage is bounded (iterative) stack usage.
///
/// Time: O(M*N)
/// Space: O(M*N)
#[must_use]
pub fn pacific_atlantic_optimized(heights: Vec<Vec<i32>>) -> Vec<Vec<i32>> {
    let rows = heights.len();
    if rows == 0 {
        return vec![];
    }
    let cols = heights[0].len();

    let mut pacific_reachable = vec![vec![false; cols]; rows];
    let mut atlantic_reachable = vec![vec![false; cols]; rows];

    let mut pacific_queue = VecDeque::new();
    let mut atlantic_queue = VecDeque::new();

    for c in 0..cols {
        pacific_queue.push_back((0, c));
        pacific_reachable[0][c] = true;

        atlantic_queue.push_back((rows - 1, c));
        atlantic_reachable[rows - 1][c] = true;
    }

    for r in 0..rows {
        // Prevent duplicate corners
        if !pacific_reachable[r][0] {
            pacific_queue.push_back((r, 0));
            pacific_reachable[r][0] = true;
        }
        if !atlantic_reachable[r][cols - 1] {
            atlantic_queue.push_back((r, cols - 1));
            atlantic_reachable[r][cols - 1] = true;
        }
    }

    fn bfs(
        queue: &mut VecDeque<(usize, usize)>,
        reachable: &mut [Vec<bool>],
        heights: &[Vec<i32>],
        rows: usize,
        cols: usize,
    ) {
        let directions = [(0, 1), (1, 0), (0, -1), (-1, 0)];
        while let Some((r, c)) = queue.pop_front() {
            for (dr, dc) in directions {
                let nr = r as i32 + dr;
                let nc = c as i32 + dc;

                if nr >= 0 && nr < rows as i32 && nc >= 0 && nc < cols as i32 {
                    let nr = nr as usize;
                    let nc = nc as usize;

                    if !reachable[nr][nc] && heights[nr][nc] >= heights[r][c] {
                        reachable[nr][nc] = true;
                        queue.push_back((nr, nc));
                    }
                }
            }
        }
    }

    bfs(
        &mut pacific_queue,
        &mut pacific_reachable,
        &heights,
        rows,
        cols,
    );
    bfs(
        &mut atlantic_queue,
        &mut atlantic_reachable,
        &heights,
        rows,
        cols,
    );

    // RUST INSIGHT: Iterator combinators make the filtering logic declarative and concise.
    (0..rows)
        .flat_map(|r| (0..cols).map(move |c| (r, c)))
        .filter_map(|(r, c)| {
            if pacific_reachable[r][c] && atlantic_reachable[r][c] {
                Some(vec![r as i32, c as i32])
            } else {
                None
            }
        })
        .collect()
}

/// Main entry point - uses the optimal reverse-DFS solution.
#[must_use]
pub fn pacific_atlantic(heights: Vec<Vec<i32>>) -> Vec<Vec<i32>> {
    pacific_atlantic_optimal(heights)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force_basic() {
        let heights = vec![
            vec![1, 2, 2, 3, 5],
            vec![3, 2, 3, 4, 4],
            vec![2, 4, 5, 3, 1],
            vec![6, 7, 1, 4, 5],
            vec![5, 1, 1, 2, 4],
        ];
        let mut expected = vec![
            vec![0, 4],
            vec![1, 3],
            vec![1, 4],
            vec![2, 2],
            vec![3, 0],
            vec![3, 1],
            vec![4, 0],
        ];
        let mut result = pacific_atlantic_brute_force(heights);
        expected.sort();
        result.sort();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_optimal_dfs_basic() {
        let heights = vec![
            vec![1, 2, 2, 3, 5],
            vec![3, 2, 3, 4, 4],
            vec![2, 4, 5, 3, 1],
            vec![6, 7, 1, 4, 5],
            vec![5, 1, 1, 2, 4],
        ];
        let mut expected = vec![
            vec![0, 4],
            vec![1, 3],
            vec![1, 4],
            vec![2, 2],
            vec![3, 0],
            vec![3, 1],
            vec![4, 0],
        ];
        let mut result = pacific_atlantic_optimal(heights);
        expected.sort();
        result.sort();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_optimized_bfs_basic() {
        let heights = vec![
            vec![1, 2, 2, 3, 5],
            vec![3, 2, 3, 4, 4],
            vec![2, 4, 5, 3, 1],
            vec![6, 7, 1, 4, 5],
            vec![5, 1, 1, 2, 4],
        ];
        let mut expected = vec![
            vec![0, 4],
            vec![1, 3],
            vec![1, 4],
            vec![2, 2],
            vec![3, 0],
            vec![3, 1],
            vec![4, 0],
        ];
        let mut result = pacific_atlantic_optimized(heights);
        expected.sort();
        result.sort();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_all_approaches_agree() {
        // Cross-implementation agreement across brute force, optimal DFS, and optimized BFS.
        let heights = vec![
            vec![1, 2, 2, 3, 5],
            vec![3, 2, 3, 4, 4],
            vec![2, 4, 5, 3, 1],
            vec![6, 7, 1, 4, 5],
            vec![5, 1, 1, 2, 4],
        ];

        let mut brute = pacific_atlantic_brute_force(heights.clone());
        let mut optimal = pacific_atlantic_optimal(heights.clone());
        let mut optimized = pacific_atlantic_optimized(heights);

        brute.sort();
        optimal.sort();
        optimized.sort();

        assert_eq!(brute, optimal);
        assert_eq!(optimal, optimized);
    }

    #[test]
    fn test_all_1s() {
        let heights = vec![vec![1, 1], vec![1, 1]];
        let mut expected = vec![vec![0, 0], vec![0, 1], vec![1, 0], vec![1, 1]];
        let mut result = pacific_atlantic(heights);
        expected.sort();
        result.sort();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_single_cell() {
        let heights = vec![vec![1]];
        let expected = vec![vec![0, 0]];
        let result = pacific_atlantic(heights);
        assert_eq!(result, expected);
    }
}
