//! # 11. Container With Most Water
//!
//! **Difficulty: Medium**
//!
//! [LeetCode Problem 11](https://leetcode.com/problems/container-with-most-water/)
//!
//! You are given an integer array `height` of length `n`. There are `n` vertical lines drawn such that the two endpoints of the `i-th` line are `(i, 0)` and `(i, height[i])`.
//!
//! Find two lines that together with the x-axis form a container, such that the container contains the most water.
//!
//! Return the maximum amount of water a container can store.
//!
//! Notice that you may not slant the container.
//!
//! ## Why this matters in Rust
//! This problem is a classic example of the **Two Pointer** technique (Greedy).
//! It demonstrates:
//! -   **Iterators vs Indices**: While Rust iterators are powerful, some algorithms (like two pointers meeting in the middle) are more naturally expressed with explicit indices (`usize`) and `while` loops.
//! -   **Safe Indexing**: Managing bounds checks and ensuring `left < right` invariants.
//! -   **`cmp::min` / `cmp::max`**: Idiomatic usage of standard library comparison functions.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::arrays::container_with_most_water::max_area;
//!
//! let height = vec![1,8,6,2,5,4,8,3,7];
//! assert_eq!(max_area(height), 49);
//! ```
//!
//! ## Constraints
//!
//! - `n == height.length`
//! - `2 <= n <= 10^5`
//! - `0 <= height[i] <= 10^4`

use std::cmp;

/// Brute Force Approach: Check All Pairs
///
/// **Strategy**:
/// Iterate through every possible pair of lines `(i, j)` where `i < j`.
/// Calculate the area `min(height[i], height[j]) * (j - i)` and track the maximum.
///
/// **Time**: O(N^2) - Nested loops.
/// **Space**: O(1) - Constant extra space.
///
/// # RUST INSIGHT
/// While simpler to write, this will Time Limit Exceed (TLE) on large inputs (N=10^5).
#[must_use]
#[allow(clippy::needless_pass_by_value)]
#[allow(clippy::cast_possible_truncation)] // Area fits in i32 given constraints (10^5 * 10^4 = 10^9 < 2*10^9)
#[allow(clippy::cast_possible_wrap)]
pub fn max_area_brute_force(height: Vec<i32>) -> i32 {
    let n = height.len();
    let mut max_water = 0;

    for i in 0..n {
        for j in (i + 1)..n {
            let h = cmp::min(height[i], height[j]);
            let w = (j - i) as i32;
            max_water = cmp::max(max_water, h * w);
        }
    }

    max_water
}

/// Optimal Approach: Two Pointers (Greedy)
///
/// **Strategy**:
/// Start with pointers at the beginning (`left = 0`) and end (`right = n-1`) of the array.
/// The width is maximized at the start. To maximize area, we need to find taller lines.
/// At each step:
/// 1. Calculate area formed by `left` and `right`.
/// 2. Move the pointer pointing to the *shorter* line inward.
///    - Logic: The area is limited by the shorter line. Moving the taller line inward can only reduce width without increasing height (since height is `min(left, right)`). Moving the shorter line gives a chance to find a taller line.
///
/// **Time**: O(N) - Single pass, each element visited at most once.
/// **Space**: O(1) - Constant extra space.
///
/// # RUST INSIGHT
/// We use `usize` for indices to match Rust's slice indexing.
/// `cmp::min` and `cmp::max` are used for readability.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::cast_possible_wrap)]
pub fn max_area_optimal(height: Vec<i32>) -> i32 {
    let mut left = 0;
    let mut right = height.len() - 1;
    let mut max_water = 0;

    while left < right {
        let h_left = height[left];
        let h_right = height[right];

        // Calculate current area
        // GOTCHA: Constraints say `height` can be 0. Area can be 0.
        let current_height = cmp::min(h_left, h_right);
        let current_width = (right - left) as i32;
        let current_area = current_height * current_width;

        max_water = cmp::max(max_water, current_area);

        // Move the shorter line inward
        if h_left < h_right {
            left += 1;
        } else {
            right -= 1;
        }
    }

    max_water
}

/// Main entry point - uses optimal solution
#[must_use]
pub fn max_area(height: Vec<i32>) -> i32 {
    max_area_optimal(height)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_example_1() {
        let height = vec![1, 8, 6, 2, 5, 4, 8, 3, 7];
        // Max area between index 1 (8) and index 8 (7).
        // Width = 8 - 1 = 7. Height = min(8, 7) = 7. Area = 49.
        assert_eq!(max_area(height.clone()), 49);
        assert_eq!(max_area_brute_force(height), 49);
    }

    #[test]
    fn test_example_2() {
        let height = vec![1, 1];
        // Width = 1. Height = 1. Area = 1.
        assert_eq!(max_area(height.clone()), 1);
        assert_eq!(max_area_brute_force(height), 1);
    }

    #[test]
    fn test_increasing() {
        let height = vec![1, 2, 3, 4, 5];
        // Max area between index 1 (2) and 4 (5) -> w=3, h=2 -> 6
        // Or index 0 (1) and 4 (5) -> w=4, h=1 -> 4
        // Or index 2 (3) and 4 (5) -> w=2, h=3 -> 6
        // Or index 3 (4) and 4 (5) -> w=1, h=4 -> 4
        assert_eq!(max_area(height), 6);
    }

    #[test]
    fn test_decreasing() {
        let height = vec![5, 4, 3, 2, 1];
        assert_eq!(max_area(height), 6);
    }

    #[test]
    fn test_large_input() {
        // Can't run brute force on huge input in simple test, but optimal handles it.
        let height = vec![10000; 10000];
        // Max area is first and last: width 9999, height 10000
        // 9999 * 10000 = 99,990,000
        assert_eq!(max_area(height), 99_990_000);
    }
}
