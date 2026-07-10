//! # 215. Kth Largest Element in an Array
//!
//! Link: <https://leetcode.com/problems/kth-largest-element-in-an-array/>
//! Difficulty: Medium
//!
//! Given an integer array `nums` and an integer `k`, return the `k`th largest element in the array.
//! Note that it is the `k`th largest element in the sorted order, not the `k`th distinct element.
//!
//! This problem is a natural fit for exploring Rust's `std::collections::BinaryHeap` and slice manipulation
//! algorithms. It demonstrates how to leverage iterators with `Reverse` for min-heaps, and contrasts heap-based
//! approaches with the highly optimized, zero-allocation Quickselect implementation provided by Rust's standard library.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::heaps::kth_largest_element_in_an_array::find_kth_largest;
//!
//! assert_eq!(find_kth_largest(vec![3, 2, 1, 5, 6, 4], 2), 5);
//! assert_eq!(find_kth_largest(vec![3, 2, 3, 1, 2, 4, 5, 5, 6], 4), 4);
//! ```
//!
//! ## Constraints
//!
//! - `1 <= k <= nums.length <= 10^5`
//! - `-10^4 <= nums[i] <= 10^4`

use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// Brute force approach: Sort and index
///
/// The most straightforward approach is to sort the array and return the element at the `k`th position
/// from the end. While simple, it does unnecessary work by fully sorting elements we don't care about.
///
/// Time: O(n log n) - Fully sorts the array.
/// Space: O(1) or O(n) depending on whether the sort is done in-place.
///
/// # Gotcha
/// Using `.sort()` instead of `.sort_unstable()` does more work than needed when we don't care about
/// preserving the original order of equal elements, leading to higher constant factors.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
#[allow(clippy::cast_sign_loss)] // `k` is strictly positive per constraints
pub fn find_kth_largest_brute_force(mut nums: Vec<i32>, k: i32) -> i32 {
    // Sort descending so the kth largest is at index k-1
    // RUST INSIGHT: `sort_unstable_by` is generally faster than `sort_by` for primitives.
    nums.sort_unstable_by(|a, b| b.cmp(a));

    // Convert k to 0-based indexing safely
    // Since k >= 1 is guaranteed by constraints, (k - 1) won't underflow below 0
    let index = (k - 1) as usize;
    nums[index]
}

/// Optimized approach: Min-Heap of size K
///
/// We can maintain a Min-Heap of size `k`. As we iterate through the array, we push elements into the heap.
/// If the heap size exceeds `k`, we pop the smallest element. After processing all elements, the min-heap
/// will contain the `k` largest elements, and the root (the smallest of these) will be the `k`th largest overall.
///
/// Time: O(n log k) - We process `n` elements, each insertion/removal takes O(log k).
/// Space: O(k) - We store at most `k` elements in the heap.
///
/// # Rust Insight
/// Rust's `BinaryHeap` is a Max-Heap by default. To make it a Min-Heap, we wrap the items in `std::cmp::Reverse`.
/// This flips the ordering comparison, changing max-heap semantics to min-heap without custom comparator boilerplate.
///
/// # Panics
///
/// Panics if the heap is empty after processing, which cannot happen when the
/// constraints `1 <= k <= nums.length` hold.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
#[allow(clippy::cast_sign_loss)] // `k` is strictly positive per constraints
pub fn find_kth_largest_optimized(nums: Vec<i32>, k: i32) -> i32 {
    let k_usize = k as usize;
    // Pre-allocate to k to avoid reallocations
    let mut min_heap = BinaryHeap::with_capacity(k_usize + 1);

    for &num in &nums {
        min_heap.push(Reverse(num));

        // Maintain heap size to be exactly k
        if min_heap.len() > k_usize {
            min_heap.pop();
        }
    }

    // The root of the min-heap is the kth largest element.
    // Safe to unwrap because constraints guarantee k <= nums.length.
    min_heap.pop().unwrap().0
}

/// Optimal approach: Quickselect (using standard library)
///
/// Quickselect solves this in O(n) average time by partitioning the array similarly to Quicksort.
/// Rust provides this functionality out-of-the-box with `select_nth_unstable`, making it the most
/// idiomatic and performant solution for in-place selection.
///
/// Time: O(n) average case, O(n^2) worst case.
/// Space: O(1) auxiliary space, modifying the input array in-place.
///
/// # Rust Insight
/// `select_nth_unstable` reorders the slice such that the element at the given index is the one
/// that would be there if the slice were sorted, and elements before/after are partitioned around it.
/// It returns a tuple `(left_slice, median, right_slice)`. This perfectly encapsulates the zero-cost
/// abstraction principle, heavily leveraging mutable slices (`&mut [T]`) for memory safety.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
#[allow(clippy::cast_sign_loss)] // `k` is strictly positive per constraints
pub fn find_kth_largest_optimal(mut nums: Vec<i32>, k: i32) -> i32 {
    // k is 1-indexed for largest, which corresponds to index (len - k) in ascending order
    let target_index = nums.len() - (k as usize);

    // OWNERSHIP INSIGHT: `select_nth_unstable` borrows the slice mutably, partitions it,
    // and returns three things: the sub-slice before the index, a reference to the element
    // at the index, and the sub-slice after the index.
    let (_, &mut kth_largest, _) = nums.select_nth_unstable(target_index);

    kth_largest
}

/// Main entry point - uses optimal solution
#[must_use]
pub fn find_kth_largest(nums: Vec<i32>, k: i32) -> i32 {
    find_kth_largest_optimal(nums, k)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force_example_1() {
        let nums = vec![3, 2, 1, 5, 6, 4];
        assert_eq!(find_kth_largest_brute_force(nums, 2), 5);
    }

    #[test]
    fn test_brute_force_example_2() {
        let nums = vec![3, 2, 3, 1, 2, 4, 5, 5, 6];
        assert_eq!(find_kth_largest_brute_force(nums, 4), 4);
    }

    #[test]
    fn test_optimized_example_1() {
        let nums = vec![3, 2, 1, 5, 6, 4];
        assert_eq!(find_kth_largest_optimized(nums, 2), 5);
    }

    #[test]
    fn test_optimized_example_2() {
        let nums = vec![3, 2, 3, 1, 2, 4, 5, 5, 6];
        assert_eq!(find_kth_largest_optimized(nums, 4), 4);
    }

    #[test]
    fn test_optimal_example_1() {
        let nums = vec![3, 2, 1, 5, 6, 4];
        assert_eq!(find_kth_largest_optimal(nums, 2), 5);
    }

    #[test]
    fn test_optimal_example_2() {
        let nums = vec![3, 2, 3, 1, 2, 4, 5, 5, 6];
        assert_eq!(find_kth_largest_optimal(nums, 4), 4);
    }

    #[test]
    fn test_all_approaches_edge_cases() {
        // Single element
        let input1 = vec![1];
        assert_eq!(find_kth_largest_brute_force(input1.clone(), 1), 1);
        assert_eq!(find_kth_largest_optimized(input1.clone(), 1), 1);
        assert_eq!(find_kth_largest_optimal(input1, 1), 1);

        // Negative numbers
        let input2 = vec![-1, -1];
        assert_eq!(find_kth_largest_brute_force(input2.clone(), 2), -1);
        assert_eq!(find_kth_largest_optimized(input2.clone(), 2), -1);
        assert_eq!(find_kth_largest_optimal(input2, 2), -1);

        // All identical numbers
        let input3 = vec![7, 7, 7, 7, 7];
        assert_eq!(find_kth_largest_brute_force(input3.clone(), 3), 7);
        assert_eq!(find_kth_largest_optimized(input3.clone(), 3), 7);
        assert_eq!(find_kth_largest_optimal(input3, 3), 7);

        // Extremely small and large values
        let input4 = vec![-10000, 10000, 0];
        assert_eq!(find_kth_largest_brute_force(input4.clone(), 2), 0);
        assert_eq!(find_kth_largest_optimized(input4.clone(), 2), 0);
        assert_eq!(find_kth_largest_optimal(input4, 2), 0);
    }
}
