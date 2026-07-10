//! # 33. Search in Rotated Sorted Array
//!
//! Difficulty: Medium
//! Link: https://leetcode.com/problems/search-in-rotated-sorted-array/
//!
//! This problem tests your ability to adapt standard binary search to arrays that
//! are partially sorted. It demonstrates careful boundary checking and understanding
//! of which half of the array is strictly sorted.
//!
//! Note: single canonical implementation; the brute/optimized/optimal progression does not apply here.

/// Approach: Modified Binary Search
///
/// Time Complexity: O(log N) where N is the number of elements.
/// Space Complexity: O(1) auxiliary space.
///
/// Why this is idiomatic Rust:
/// This showcases exhaustive pattern matching or logical flow without relying on
/// deeply nested ternary operators. The logic must cleanly determine if the target
/// falls within the strictly sorted half of the current search window.
pub struct Solution;

impl Solution {
    #[must_use]
    #[allow(clippy::needless_pass_by_value)]
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_possible_wrap)]
    pub fn search(nums: Vec<i32>, target: i32) -> i32 {
        if nums.is_empty() {
            return -1;
        }

        let mut left = 0;
        let mut right = nums.len() - 1;

        while left <= right {
            let mid = left + (right - left) / 2;

            if nums[mid] == target {
                return mid as i32;
            }

            // Determine which half is strictly sorted
            // RUST INSIGHT: Careful with indexing. `mid` might equal `left` or `right`
            // when the search space narrows.
            if nums[left] <= nums[mid] {
                // Left half is sorted
                if target >= nums[left] && target < nums[mid] {
                    // Target is in the sorted left half
                    // Check for underflow before subtracting
                    if mid == 0 {
                        break;
                    }
                    right = mid - 1;
                } else {
                    // Target must be in the right half
                    left = mid + 1;
                }
            } else {
                // Right half is sorted
                if target > nums[mid] && target <= nums[right] {
                    // Target is in the sorted right half
                    left = mid + 1;
                } else {
                    // Target must be in the left half
                    // Check for underflow before subtracting
                    if mid == 0 {
                        break;
                    }
                    right = mid - 1;
                }
            }
        }

        -1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_happy_path() {
        assert_eq!(Solution::search(vec![4, 5, 6, 7, 0, 1, 2], 0), 4);
        assert_eq!(Solution::search(vec![4, 5, 6, 7, 0, 1, 2], 3), -1);
    }

    #[test]
    fn test_edge_cases() {
        assert_eq!(Solution::search(vec![1], 0), -1);
        assert_eq!(Solution::search(vec![1], 1), 0);
        assert_eq!(Solution::search(vec![], 5), -1);
        assert_eq!(Solution::search(vec![3, 1], 1), 1);
        assert_eq!(Solution::search(vec![3, 1], 3), 0);
    }

    #[test]
    fn test_stress_boundaries() {
        assert_eq!(Solution::search(vec![5, 1, 3], 5), 0);
        assert_eq!(Solution::search(vec![5, 1, 3], 1), 1);
        assert_eq!(Solution::search(vec![5, 1, 3], 3), 2);
    }
}
