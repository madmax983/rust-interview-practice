//! # 42. Trapping Rain Water
//!
//! Given `n` non-negative integers representing an elevation map where the width of
//! each bar is 1, compute how much water it can trap after raining.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::arrays::trapping_rain_water::trap;
//!
//! assert_eq!(trap(vec![0,1,0,2,1,0,1,3,2,1,2,1]), 6);
//! assert_eq!(trap(vec![4,2,0,3,2,5]), 9);
//! ```
//!
//! ## Constraints
//!
//! - n == height.length
//! - 1 <= n <= 2 * 10^4
//! - 0 <= height[i] <= 10^5

/// Brute force approach: For each position, find max left and right
/// Time: O(n²) - for each position, scan left and right
/// Space: O(1) - no extra space needed
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::cast_possible_wrap)]
#[allow(clippy::cast_sign_loss)]
pub fn trap_brute_force(height: Vec<i32>) -> i32 {
    if height.len() < 3 {
        return 0;
    }

    let mut total = 0;

    // Strategy: For each position, water level = min(max_left, max_right)
    // Water trapped at position = max(0, water_level - height[i])
    for i in 1..height.len() - 1 {
        // Skip first and last (can't trap water at edges)

        // Find maximum height to the left
        let max_left = height[..i].iter().copied().max().unwrap_or(0);

        // Find maximum height to the right
        let max_right = height[i + 1..].iter().copied().max().unwrap_or(0);

        // Water level at this position is bounded by the smaller of max_left and max_right
        let water_level = max_left.min(max_right);

        // Water trapped = water level - current height (if positive)
        if water_level > height[i] {
            total += water_level - height[i];
        }
    }

    total
}

/// Optimized approach: Precompute max arrays
/// Time: O(n) - three passes through array
/// Space: O(n) - two arrays to store max values
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::cast_possible_wrap)]
#[allow(clippy::cast_sign_loss)]
pub fn trap_optimized(height: Vec<i32>) -> i32 {
    if height.len() < 3 {
        return 0;
    }

    let n = height.len();

    // Strategy: Precompute max_left and max_right for each position
    // Then calculate water in one pass

    // Build max_left array: max_left[i] = max height from 0 to i
    let mut max_left = vec![0; n];
    max_left[0] = height[0];
    for i in 1..n {
        max_left[i] = max_left[i - 1].max(height[i]);
    }

    // Build max_right array: max_right[i] = max height from i to n-1
    let mut max_right = vec![0; n];
    max_right[n - 1] = height[n - 1];
    for i in (0..n - 1).rev() {
        max_right[i] = max_right[i + 1].max(height[i]);
    }

    // Calculate total water using precomputed arrays
    let mut total = 0;
    for i in 1..n - 1 {
        // Water level is minimum of max_left and max_right
        let water_level = max_left[i].min(max_right[i]);

        // Add water trapped at this position
        if water_level > height[i] {
            total += water_level - height[i];
        }
    }

    total
}

/// Optimal approach: Two pointers
/// Time: O(n) - single pass through array
/// Space: O(1) - only two pointers and variables
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::cast_possible_wrap)]
#[allow(clippy::cast_sign_loss)]
pub fn trap_optimal(height: Vec<i32>) -> i32 {
    if height.len() < 3 {
        return 0;
    }

    let mut left = 0; // Left pointer starts at beginning
    let mut right = height.len() - 1; // Right pointer starts at end

    let mut max_left = 0; // Track max height seen from left
    let mut max_right = 0; // Track max height seen from right

    let mut total = 0;

    // Strategy: Move pointers inward, always processing the shorter side
    // Key insight: water level at shorter side is determined by max on that side
    while left < right {
        if height[left] < height[right] {
            // Left side is shorter, process left position
            if height[left] >= max_left {
                // Current height is new max on left side
                max_left = height[left];
            } else {
                // Water trapped = max_left - current height
                total += max_left - height[left];
            }
            left += 1; // Move left pointer right
        } else {
            // Right side is shorter or equal, process right position
            if height[right] >= max_right {
                // Current height is new max on right side
                max_right = height[right];
            } else {
                // Water trapped = max_right - current height
                total += max_right - height[right];
            }
            right -= 1; // Move right pointer left
        }
    }

    total
}

/// Main entry point - uses optimal solution
#[must_use]
pub fn trap(height: Vec<i32>) -> i32 {
    trap_optimal(height)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Brute force tests
    #[test]
    fn test_brute_force_example_1() {
        assert_eq!(
            trap_brute_force(vec![0, 1, 0, 2, 1, 0, 1, 3, 2, 1, 2, 1]),
            6
        );
    }

    #[test]
    fn test_brute_force_example_2() {
        assert_eq!(trap_brute_force(vec![4, 2, 0, 3, 2, 5]), 9);
    }

    #[test]
    fn test_brute_force_no_water() {
        assert_eq!(trap_brute_force(vec![1, 2, 3, 4, 5]), 0);
    }

    #[test]
    fn test_brute_force_empty() {
        assert_eq!(trap_brute_force(vec![]), 0);
    }

    // Optimized tests
    #[test]
    fn test_optimized_example_1() {
        assert_eq!(trap_optimized(vec![0, 1, 0, 2, 1, 0, 1, 3, 2, 1, 2, 1]), 6);
    }

    #[test]
    fn test_optimized_example_2() {
        assert_eq!(trap_optimized(vec![4, 2, 0, 3, 2, 5]), 9);
    }

    #[test]
    fn test_optimized_no_water() {
        assert_eq!(trap_optimized(vec![1, 2, 3, 4, 5]), 0);
    }

    #[test]
    fn test_optimized_empty() {
        assert_eq!(trap_optimized(vec![]), 0);
    }

    // Optimal tests
    #[test]
    fn test_optimal_example_1() {
        assert_eq!(trap_optimal(vec![0, 1, 0, 2, 1, 0, 1, 3, 2, 1, 2, 1]), 6);
    }

    #[test]
    fn test_optimal_example_2() {
        assert_eq!(trap_optimal(vec![4, 2, 0, 3, 2, 5]), 9);
    }

    #[test]
    fn test_optimal_no_water() {
        assert_eq!(trap_optimal(vec![1, 2, 3, 4, 5]), 0);
    }

    #[test]
    fn test_optimal_empty() {
        assert_eq!(trap_optimal(vec![]), 0);
    }

    // Cross-implementation tests
    #[test]
    fn test_all_approaches_valley() {
        let height = vec![3, 0, 2, 0, 4];
        assert_eq!(trap_brute_force(height.clone()), 7);
        assert_eq!(trap_optimized(height.clone()), 7);
        assert_eq!(trap_optimal(height), 7);
    }

    #[test]
    fn test_all_approaches_descending() {
        let height = vec![5, 4, 3, 2, 1];
        assert_eq!(trap_brute_force(height.clone()), 0);
        assert_eq!(trap_optimized(height.clone()), 0);
        assert_eq!(trap_optimal(height), 0);
    }

    #[test]
    fn test_all_approaches_single_valley() {
        let height = vec![2, 1, 2];
        assert_eq!(trap_brute_force(height.clone()), 1);
        assert_eq!(trap_optimized(height.clone()), 1);
        assert_eq!(trap_optimal(height), 1);
    }

    #[test]
    fn test_all_approaches_complex() {
        let height = vec![5, 2, 1, 2, 1, 5];
        assert_eq!(trap_brute_force(height.clone()), 14);
        assert_eq!(trap_optimized(height.clone()), 14);
        assert_eq!(trap_optimal(height), 14);
    }

    // Main function tests
    #[test]
    fn test_main_example_1() {
        assert_eq!(trap(vec![0, 1, 0, 2, 1, 0, 1, 3, 2, 1, 2, 1]), 6);
    }

    #[test]
    fn test_main_example_2() {
        assert_eq!(trap(vec![4, 2, 0, 3, 2, 5]), 9);
    }
}
