//! # 153. Find Minimum in Rotated Sorted Array
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/find-minimum-in-rotated-sorted-array>/
//!
//! This problem emphasizes the core binary search paradigm of finding the "pivot" point.
//! It teaches how to converge on a single element rather than searching for a specific target.
//!
//! Note: single canonical implementation; the brute/optimized/optimal progression does not apply here.

/// Approach: Binary Search for Inflection Point
///
/// Time Complexity: O(log N) where N is the number of elements.
/// Space Complexity: O(1) auxiliary space.
///
/// Why this is idiomatic Rust:
/// This shows how to write a loop condition `left < right` to converge on a single
/// element without an early return inside the loop, minimizing bounds checks.
pub struct Solution;

impl Solution {
    #[must_use]
    #[allow(clippy::needless_pass_by_value)]
    pub fn find_min(nums: Vec<i32>) -> i32 {
        if nums.is_empty() {
            return 0; // Return something sensible for empty, though constraints say n >= 1
        }

        let mut left = 0;
        let mut right = nums.len() - 1;

        // If the array is already sorted (0 rotations), the minimum is at the beginning.
        // RUST INSIGHT: Checking nums[left] < nums[right] is an O(1) fast path.
        if nums[left] < nums[right] {
            return nums[left];
        }

        // Loop invariant: The minimum element is always within [left, right]
        while left < right {
            let mid = left + (right - left) / 2;

            // Compare mid with the rightmost element.
            if nums[mid] > nums[right] {
                // The inflection point must be to the right of mid.
                left = mid + 1;
            } else {
                // The inflection point is at mid or to the left of mid.
                // Note we do NOT use mid - 1 because mid itself could be the minimum.
                right = mid;
            }
        }

        // When left == right, we've found the minimum.
        nums[left]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_happy_path() {
        assert_eq!(Solution::find_min(vec![3, 4, 5, 1, 2]), 1);
        assert_eq!(Solution::find_min(vec![4, 5, 6, 7, 0, 1, 2]), 0);
        assert_eq!(Solution::find_min(vec![11, 13, 15, 17]), 11);
    }

    #[test]
    fn test_edge_cases() {
        assert_eq!(Solution::find_min(vec![1]), 1);
        assert_eq!(Solution::find_min(vec![2, 1]), 1);
        assert_eq!(Solution::find_min(vec![1, 2]), 1);
    }

    #[test]
    fn test_stress_boundaries() {
        // Rotated exactly at the middle
        assert_eq!(Solution::find_min(vec![5, 6, 7, 1, 2, 3, 4]), 1);
        // Minimum is the last element
        assert_eq!(Solution::find_min(vec![2, 3, 4, 5, 6, 7, 1]), 1);
    }
}
