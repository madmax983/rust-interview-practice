//! # 33. Search in Rotated Sorted Array
//! Difficulty: Medium
//! Link: https://leetcode.com/problems/search-in-rotated-sorted-array/
//!
//! There is an integer array `nums` sorted in ascending order (with distinct values).
//! Prior to being passed to your function, `nums` is possibly rotated at an unknown
//! pivot index `k` (`1 <= k < nums.length`) such that the resulting array is
//! `[nums[k], nums[k+1], ..., nums[n-1], nums[0], nums[1], ..., nums[k-1]]` (0-indexed).
//! For example, `[0,1,2,4,5,6,7]` might be rotated at pivot index 3 and become `[4,5,6,7,0,1,2]`.
//!
//! Given the array `nums` after the possible rotation and an integer `target`,
//! return the index of `target` if it is in `nums`, or `-1` if it is not in `nums`.
//!
//! You must write an algorithm with `O(log n)` runtime complexity.
//!
//! ## Why this matters in Rust
//! This problem is a perfect showcase for Rust's explicit handling of boundary conditions
//! and overflow-safe index arithmetic. By using slice indexing (`usize`) and avoiding implicit
//! type coercion, Rust forces us to correctly manage indices in binary search algorithms,
//! preventing entire classes of off-by-one and buffer overflow errors.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::arrays::search_in_rotated_sorted_array::search;
//!
//! assert_eq!(search(vec![4, 5, 6, 7, 0, 1, 2], 0), 4);
//! assert_eq!(search(vec![4, 5, 6, 7, 0, 1, 2], 3), -1);
//! assert_eq!(search(vec![1], 0), -1);
//! ```
//!
//! ## Constraints
//!
//! - `1 <= nums.length <= 5000`
//! - `-10^4 <= nums[i] <= 10^4`
//! - All values of `nums` are unique.
//! - `nums` is an ascending array that is possibly rotated.
//! - `-10^4 <= target <= 10^4`

/// Brute force approach: Linear Search
/// Time: O(N) - single pass through the array
/// Space: O(1) - no extra space needed
///
/// This approach simply uses Rust's iterator methods to find the target.
/// It doesn't meet the O(log n) requirement but is idiomatic Rust for an O(N) search.
///
/// RUST INSIGHT: `iter().position()` returns an `Option<usize>`, which perfectly
/// encapsulates "found at index" vs "not found". We then map `Some(i)` to `i as i32`
/// and `None` to `-1` using `map_or`.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
#[allow(clippy::cast_possible_wrap)]
pub fn search_brute_force(nums: Vec<i32>, target: i32) -> i32 {
    nums.iter()
        .position(|&x| x == target)
        .map_or(-1, |i| i as i32)
}

/// Optimized approach: Two-pass Binary Search
/// Time: O(log N) - finding pivot is O(log N), then searching the correct half is O(log N)
/// Space: O(1) - only pointer variables
///
/// First, find the index of the smallest element (the pivot point).
/// Then, based on the target value and the values at the array boundaries,
/// perform a standard binary search on either the left or right sorted half.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
#[allow(clippy::cast_possible_wrap)]
pub fn search_optimized(nums: Vec<i32>, target: i32) -> i32 {
    if nums.is_empty() {
        return -1;
    }

    let n = nums.len();
    let mut left = 0;
    let mut right = n - 1;

    // 1. Find the pivot (index of minimum element)
    while left < right {
        let mid = left + (right - left) / 2;
        if nums[mid] > nums[right] {
            // Pivot must be to the right of mid
            left = mid + 1;
        } else {
            // Pivot could be mid or to the left
            right = mid;
        }
    }

    let pivot = left; // pivot is the index of the minimum element
    left = 0;
    right = n - 1;

    // 2. Decide which half to binary search in
    // GOTCHA: We must handle the case where target == nums[pivot] explicitly here,
    // or correctly route the search. The standard way is to check bounds.
    if target >= nums[pivot] && target <= nums[right] {
        left = pivot;
    } else {
        // Safe to subtract 1 since if target is not in the right sorted portion,
        // and pivot > 0, it must be in the left sorted portion. If pivot == 0,
        // the array isn't rotated and target isn't in it, so right becomes underflow?
        // Wait, if pivot == 0, `right` becomes `0 - 1` which panics in Rust!
        // So we must handle `pivot == 0` explicitly.
        if pivot == 0 {
            // The array is fully sorted and not rotated, so search the whole array
            right = n - 1;
        } else {
            right = pivot - 1;
        }
    }

    // 3. Standard binary search
    while left <= right {
        let mid = left + (right - left) / 2;
        if nums[mid] == target {
            return mid as i32;
        } else if nums[mid] < target {
            left = mid + 1;
        } else {
            if mid == 0 {
                break; // Prevent underflow on `right = mid - 1`
            }
            right = mid - 1;
        }
    }

    -1
}

/// Optimal approach: Single-pass Binary Search
/// Time: O(log N) - single binary search
/// Space: O(1) - constant space
///
/// Instead of finding the pivot first, we can do it in one pass.
/// In any rotated sorted array, dividing it in half will result in at least one
/// half being perfectly sorted. We can check which half is sorted, and then check
/// if our target lies within that sorted half's boundaries.
///
/// RUST INSIGHT: Notice how explicit we are with array indices (`usize`).
/// We must be careful with `right = mid - 1` to prevent underflow, so we break
/// or adjust logic to avoid `0usize - 1`.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
#[allow(clippy::cast_possible_wrap)]
pub fn search_optimal(nums: Vec<i32>, target: i32) -> i32 {
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

        // Check if the left half is sorted
        if nums[left] <= nums[mid] {
            // Left half is sorted
            // Check if target is strictly within the left half
            if nums[left] <= target && target < nums[mid] {
                if mid == 0 {
                    break;
                }
                right = mid - 1; // Target is in the left half
            } else {
                left = mid + 1; // Target is in the right half
            }
        } else {
            // Right half is sorted
            // Check if target is strictly within the right half
            if nums[mid] < target && target <= nums[right] {
                left = mid + 1; // Target is in the right half
            } else {
                if mid == 0 {
                    break;
                }
                right = mid - 1; // Target is in the left half
            }
        }
    }

    -1
}

/// Main entry point - uses optimal solution
#[must_use]
pub fn search(nums: Vec<i32>, target: i32) -> i32 {
    search_optimal(nums, target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force() {
        assert_eq!(search_brute_force(vec![4, 5, 6, 7, 0, 1, 2], 0), 4);
        assert_eq!(search_brute_force(vec![4, 5, 6, 7, 0, 1, 2], 3), -1);
        assert_eq!(search_brute_force(vec![1], 0), -1);
        assert_eq!(search_brute_force(vec![1, 3], 3), 1);
    }

    #[test]
    fn test_optimized() {
        assert_eq!(search_optimized(vec![4, 5, 6, 7, 0, 1, 2], 0), 4);
        assert_eq!(search_optimized(vec![4, 5, 6, 7, 0, 1, 2], 3), -1);
        assert_eq!(search_optimized(vec![1], 0), -1);
        assert_eq!(search_optimized(vec![1, 3], 3), 1);
        assert_eq!(search_optimized(vec![3, 1], 1), 1);
        assert_eq!(search_optimized(vec![5, 1, 3], 5), 0);
    }

    #[test]
    fn test_optimal() {
        assert_eq!(search_optimal(vec![4, 5, 6, 7, 0, 1, 2], 0), 4);
        assert_eq!(search_optimal(vec![4, 5, 6, 7, 0, 1, 2], 3), -1);
        assert_eq!(search_optimal(vec![1], 0), -1);
        assert_eq!(search_optimal(vec![1, 3], 3), 1);
        assert_eq!(search_optimal(vec![3, 1], 1), 1);
        assert_eq!(search_optimal(vec![5, 1, 3], 5), 0);
    }

    // Edge cases and stress tests
    #[test]
    fn test_edge_cases() {
        let nums = vec![1, 2, 3, 4, 5, 6]; // Fully sorted
        assert_eq!(search(nums.clone(), 1), 0);
        assert_eq!(search(nums.clone(), 6), 5);
        assert_eq!(search(nums.clone(), 7), -1);

        let nums3 = vec![2, 1]; // Smallest rotated
        assert_eq!(search(nums3.clone(), 1), 1);
        assert_eq!(search(nums3.clone(), 2), 0);
        assert_eq!(search(nums3.clone(), 3), -1);
    }

    #[test]
    fn test_stress() {
        let mut nums = vec![];
        for i in 1000..5000 {
            nums.push(i);
        }
        for i in 0..1000 {
            nums.push(i);
        }

        assert_eq!(search(nums.clone(), 999), 4999);
        assert_eq!(search(nums.clone(), 1000), 0);
        assert_eq!(search(nums.clone(), 2500), 1500);
        assert_eq!(search(nums.clone(), -1), -1);
        assert_eq!(search(nums.clone(), 5000), -1);
    }

    // Cross-implementation agreement test
    #[test]
    fn test_all_approaches_agreement() {
        let arrays: Vec<Vec<i32>> = vec![
            vec![4, 5, 6, 7, 0, 1, 2],
            vec![1],
            vec![1, 3],
            vec![3, 1],
            vec![5, 1, 3],
            vec![1, 2, 3, 4, 5, 6],
            vec![6, 7, 8, 1, 2, 3, 4, 5],
        ];

        for nums in arrays {
            for target in -2..=10 {
                let brute = search_brute_force(nums.clone(), target);
                let optimized = search_optimized(nums.clone(), target);
                let optimal = search_optimal(nums.clone(), target);
                assert_eq!(
                    brute, optimized,
                    "brute vs optimized mismatch for {nums:?}, target={target}"
                );
                assert_eq!(
                    optimized, optimal,
                    "optimized vs optimal mismatch for {nums:?}, target={target}"
                );
            }
        }
    }
}
