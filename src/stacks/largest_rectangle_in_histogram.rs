//! # 84. Largest Rectangle in Histogram
//!
//! Difficulty: Hard
//! Link: <https://leetcode.com/problems/largest-rectangle-in-histogram/>
//!
//! Given an array of integers `heights` representing the histogram's bar height
//! where the width of each bar is 1, return the area of the largest rectangle in the histogram.
//!
//! This problem perfectly demonstrates Rust's `Vec` as an idiomatic stack, and why
//! avoiding intermediate allocations can lead to extremely high performance. It also showcases
//! how Rust's strict integer arithmetic rules shape boundary calculations.

use std::cmp;

/// Brute force approach: Check all possible sub-arrays and find minimum height.
/// Time: O(n²) - for each starting bar, we scan to the right to find the boundaries.
/// Space: O(1) - no extra space needed.
///
/// This approach iterates through every possible pair of left and right boundaries,
/// keeping track of the minimum height in that range to compute the area.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
pub fn largest_rectangle_area_brute_force(heights: Vec<i32>) -> i32 {
    let mut max_area = 0;
    let n = heights.len();

    for i in 0..n {
        let mut min_height = heights[i];
        for j in i..n {
            min_height = cmp::min(min_height, heights[j]);
            let width = (j - i + 1) as i32;
            max_area = cmp::max(max_area, min_height * width);
        }
    }

    max_area
}

/// Optimized approach: Multi-pass with left and right boundary arrays.
/// Time: O(n) - three passes through the array.
/// Space: O(n) - two arrays to store left and right boundaries.
///
/// By pre-calculating the left and right boundaries for each bar, we can
/// determine the area in O(1) time per bar. We use array lookups to build
/// these boundary arrays efficiently.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
pub fn largest_rectangle_area_optimized(heights: Vec<i32>) -> i32 {
    if heights.is_empty() {
        return 0;
    }

    let n = heights.len();
    let mut less_from_left = vec![0; n];
    let mut less_from_right = vec![0; n];

    less_from_left[0] = -1;
    for i in 1..n {
        let mut p = (i - 1) as i32;
        while p >= 0 && heights[p as usize] >= heights[i] {
            p = less_from_left[p as usize];
        }
        less_from_left[i] = p;
    }

    less_from_right[n - 1] = n as i32;
    // RUST INSIGHT: Iterating backwards is elegantly handled by `.rev()`.
    for i in (0..n - 1).rev() {
        let mut p = (i + 1) as i32;
        while p < n as i32 && heights[p as usize] >= heights[i] {
            p = less_from_right[p as usize];
        }
        less_from_right[i] = p;
    }

    let mut max_area = 0;
    for i in 0..n {
        let width = less_from_right[i] - less_from_left[i] - 1;
        max_area = cmp::max(max_area, heights[i] * width);
    }

    max_area
}

/// Optimal approach: Single pass using a monotonic stack.
/// Time: O(n) - each element is pushed and popped at most once.
/// Space: O(n) - stack stores at most `n` indices.
///
/// This uses a monotonically increasing stack. When we see a bar shorter
/// than the top of the stack, we know we've found the right boundary for
/// the bar at the top of the stack.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
pub fn largest_rectangle_area_optimal(mut heights: Vec<i32>) -> i32 {
    // GOTCHA: We append a 0 to the heights vector. This clever trick forces
    // any remaining bars in the stack to be processed at the end of the iteration,
    // avoiding a separate `while !stack.is_empty()` loop.
    heights.push(0);

    let mut stack: Vec<usize> = Vec::with_capacity(heights.len());
    let mut max_area = 0;

    // RUST INSIGHT: Using `enumerate()` gives us zero-cost index tracking
    // along with the values, avoiding manual loop counters.
    for (i, &h) in heights.iter().enumerate() {
        while let Some(&top_idx) = stack.last() {
            if heights[top_idx] > h {
                // The current bar `h` is strictly smaller than the bar at `top_idx`,
                // meaning `i` is the right boundary for the bar at `top_idx`.
                stack.pop();

                let height = heights[top_idx];

                // If the stack is empty, it means `top_idx` was the shortest bar so far,
                // so its left boundary is 0. Otherwise, its left boundary is the new top of the stack.
                let width = if let Some(&new_top) = stack.last() {
                    i - new_top - 1
                } else {
                    i
                } as i32;

                max_area = cmp::max(max_area, height * width);
            } else {
                break;
            }
        }
        stack.push(i);
    }

    max_area
}

/// Main entry point - uses optimal solution
#[must_use]
pub fn largest_rectangle_area(heights: Vec<i32>) -> i32 {
    largest_rectangle_area_optimal(heights)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Happy path: A standard histogram with varied heights
    #[test]
    fn test_happy_path() {
        let heights = vec![2, 1, 5, 6, 2, 3];
        let expected = 10;
        assert_eq!(
            largest_rectangle_area_brute_force(heights.clone()),
            expected
        );
        assert_eq!(largest_rectangle_area_optimized(heights.clone()), expected);
        assert_eq!(largest_rectangle_area_optimal(heights.clone()), expected);
        assert_eq!(largest_rectangle_area(heights), expected);
    }

    // Edge case: All bars have the same height
    #[test]
    fn test_uniform_heights() {
        let heights = vec![2, 2, 2, 2];
        let expected = 8;
        assert_eq!(
            largest_rectangle_area_brute_force(heights.clone()),
            expected
        );
        assert_eq!(largest_rectangle_area_optimized(heights.clone()), expected);
        assert_eq!(largest_rectangle_area_optimal(heights.clone()), expected);
    }

    // Edge case: Empty array and single element array
    #[test]
    fn test_short_arrays() {
        assert_eq!(largest_rectangle_area_brute_force(vec![]), 0);
        assert_eq!(largest_rectangle_area_optimized(vec![]), 0);
        assert_eq!(largest_rectangle_area_optimal(vec![]), 0);

        assert_eq!(largest_rectangle_area_brute_force(vec![5]), 5);
        assert_eq!(largest_rectangle_area_optimized(vec![5]), 5);
        assert_eq!(largest_rectangle_area_optimal(vec![5]), 5);
    }

    // Stress case: Strictly increasing and strictly decreasing heights
    #[test]
    fn test_monotonic_heights() {
        let increasing = vec![1, 2, 3, 4, 5];
        let expected_inc = 9; // Area formed by [3, 4, 5] is min(3)*3 = 9
        assert_eq!(
            largest_rectangle_area_brute_force(increasing.clone()),
            expected_inc
        );
        assert_eq!(
            largest_rectangle_area_optimized(increasing.clone()),
            expected_inc
        );
        assert_eq!(
            largest_rectangle_area_optimal(increasing.clone()),
            expected_inc
        );

        let decreasing = vec![5, 4, 3, 2, 1];
        let expected_dec = 9; // Area formed by [5, 4, 3] is min(3)*3 = 9
        assert_eq!(
            largest_rectangle_area_brute_force(decreasing.clone()),
            expected_dec
        );
        assert_eq!(
            largest_rectangle_area_optimized(decreasing.clone()),
            expected_dec
        );
        assert_eq!(
            largest_rectangle_area_optimal(decreasing.clone()),
            expected_dec
        );
    }
}

// =========================================================================================
// Alternative approaches footer
// =========================================================================================
// - Divide and Conquer: Find the minimum element in the array, then recursively compute
//   the max area in the left half, right half, and the area covering the minimum element.
//   This takes O(N log N) on average, but O(N^2) in the worst case (sorted array) unless
//   using a Segment Tree to query range minimums, which makes it O(N log N) worst-case
//   but increases code complexity.
