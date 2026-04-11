//! # 153. Find Minimum in Rotated Sorted Array
//! Difficulty: Medium
//! Link: https://leetcode.com/problems/find-minimum-in-rotated-sorted-array/
//!
//! Suppose an array of length `n` sorted in ascending order is rotated between `1` and `n` times.
//! For example, the array `nums = [0,1,2,4,5,6,7]` might become:
//! - `[4,5,6,7,0,1,2]` if it was rotated 4 times.
//! - `[0,1,2,4,5,6,7]` if it was rotated 7 times.
//!
//! Notice that rotating an array `[a[0], a[1], a[2], ..., a[n-1]]` 1 time results in the array
//! `[a[n-1], a[0], a[1], a[2], ..., a[n-2]]`.
//!
//! Given the sorted rotated array `nums` of unique elements, return the minimum element of this array.
//!
//! You must write an algorithm that runs in `O(log n)` time.
//!
//! ## Why this matters in Rust
//! This problem emphasizes safe array indexing and iterative binary search. In Rust, incorrect boundary
//! checks like `0usize - 1` will panic in debug mode, effectively preventing buffer underflows. It demonstrates
//! how idiomatic loop constructs (`while left < right`) paired with slice lengths keep array access secure without bounds-checking overhead.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::arrays::find_minimum_in_rotated_sorted_array::find_min;
//!
//! assert_eq!(find_min(vec![3, 4, 5, 1, 2]), 1);
//! assert_eq!(find_min(vec![4, 5, 6, 7, 0, 1, 2]), 0);
//! assert_eq!(find_min(vec![11, 13, 15, 17]), 11);
//! ```
//!
//! ## Constraints
//!
//! - `n == nums.length`
//! - `1 <= n <= 5000`
//! - `-5000 <= nums[i] <= 5000`
//! - All the integers of `nums` are unique.
//! - `nums` is sorted and rotated between `1` and `n` times.

/// Brute force approach: Linear Search
/// Time: O(N) - single pass through the array
/// Space: O(1) - no extra space needed
///
/// This approach simply uses Rust's `Iterator::min` to find the target.
/// It doesn't meet the O(log N) requirement but is perfectly idiomatic Rust for an O(N) search.
///
/// RUST INSIGHT: `iter().min()` returns an `Option<&T>`, handling the empty iterator case gracefully.
/// Since the problem guarantees `1 <= n`, we can safely unwrap or map it.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
pub fn find_min_brute_force(nums: Vec<i32>) -> i32 {
    // `.copied()` converts `Option<&i32>` to `Option<i32>`.
    // We unwrap since `nums` is guaranteed to be non-empty by constraints.
    nums.iter().min().copied().unwrap_or(0)
}

/// Optimized approach: Standard Binary Search
/// Time: O(log N) - binary search
/// Space: O(1) - only integer pointers
///
/// We maintain `left` and `right` pointers. The key insight is comparing the middle element
/// with the rightmost element to determine which half is unsorted (and thus contains the pivot).
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
pub fn find_min_optimized(nums: Vec<i32>) -> i32 {
    let mut left = 0;
    // GOTCHA: `nums.len() - 1` can underflow if `nums` is empty. The problem constraints guarantee `n >= 1`.
    let mut right = nums.len() - 1;

    while left < right {
        let mid = left + (right - left) / 2;

        if nums[mid] > nums[right] {
            // The minimum must be strictly to the right of mid, because mid is greater than the rightmost element.
            left = mid + 1;
        } else {
            // The minimum is either at mid or to the left of mid.
            right = mid;
        }
    }

    // left and right converge to the index of the minimum element.
    nums[left]
}

/// Optimal approach: Binary Search using Slice References
/// Time: O(log N) - single binary search
/// Space: O(1) - constant space
///
/// Instead of working with indices on the entire vector, we can use a slice reference `&[i32]`
/// and narrow it down. This is an alternative idiomatic Rust pattern that avoids index math
/// complexity and explicitly relies on slice sizes.
///
/// RUST INSIGHT: By borrowing the vector as a slice `&[i32]`, we show that the function
/// does not need ownership of the data. Although LeetCode forces `Vec<i32>` as the argument,
/// internally operating on a slice is more flexible and idiomatic.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
pub fn find_min_optimal(nums: Vec<i32>) -> i32 {
    let mut slice: &[i32] = &nums;

    while slice.len() > 1 {
        let right = slice.len() - 1;
        let mid = right / 2;

        // If the slice is already sorted (no rotation in this sub-slice)
        if slice[0] < slice[right] {
            return slice[0];
        }

        if slice[mid] >= slice[0] {
            // Left half is sorted, pivot is in the right half
            slice = &slice[mid + 1..];
        } else {
            // Pivot is in the left half, including mid
            slice = &slice[..=mid];
        }
    }

    slice[0]
}

/// Main entry point - uses optimized solution (as it's often more readable and standard than slice manipulation)
#[must_use]
pub fn find_min(nums: Vec<i32>) -> i32 {
    find_min_optimized(nums)
}

// =========================================================================================
// Alternative Approaches Footer
// =========================================================================================
// 1. Recursive Binary Search: We could implement binary search recursively, but it risks a stack overflow
//    in languages without Guaranteed Tail Call Optimization (like Rust, currently). The iterative approach
//    is strictly better here.
// 2. Early Exit: If `nums[left] < nums[right]` at any point in the `optimized` loop, the subarray is completely sorted.
//    We could instantly `return nums[left]`. This provides a slight real-world speedup for partially sorted data.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force() {
        assert_eq!(find_min_brute_force(vec![3, 4, 5, 1, 2]), 1);
        assert_eq!(find_min_brute_force(vec![4, 5, 6, 7, 0, 1, 2]), 0);
        assert_eq!(find_min_brute_force(vec![11, 13, 15, 17]), 11);
    }

    #[test]
    fn test_optimized() {
        assert_eq!(find_min_optimized(vec![3, 4, 5, 1, 2]), 1);
        assert_eq!(find_min_optimized(vec![4, 5, 6, 7, 0, 1, 2]), 0);
        assert_eq!(find_min_optimized(vec![11, 13, 15, 17]), 11);
    }

    #[test]
    fn test_optimal() {
        assert_eq!(find_min_optimal(vec![3, 4, 5, 1, 2]), 1);
        assert_eq!(find_min_optimal(vec![4, 5, 6, 7, 0, 1, 2]), 0);
        assert_eq!(find_min_optimal(vec![11, 13, 15, 17]), 11);
    }

    #[test]
    fn test_edge_cases() {
        // Single element
        assert_eq!(find_min(vec![1]), 1);

        // Two elements, rotated
        assert_eq!(find_min(vec![2, 1]), 1);

        // Two elements, not rotated
        assert_eq!(find_min(vec![1, 2]), 1);
    }

    #[test]
    fn test_stress() {
        let mut nums = Vec::with_capacity(5000);
        // Add 1000..5000
        for i in 1000..5000 {
            nums.push(i);
        }
        // Add 0..1000 (so the minimum is 0, at index 4000)
        for i in 0..1000 {
            nums.push(i);
        }

        assert_eq!(find_min(nums), 0);
    }
}
