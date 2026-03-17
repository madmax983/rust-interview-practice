//! # 75. Sort Colors
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/sort-colors/>
//!
//! Given an array `nums` with `n` objects colored red, white, or blue, sort them in-place
//! so that objects of the same color are adjacent, with the colors in the order red, white, and blue.
//! We will use the integers `0`, `1`, and `2` to represent the color red, white, and blue, respectively.
//!
//! You must solve this problem without using the library's sort function.
//!
//! ## Why This Matters in Rust
//!
//! This problem perfectly demonstrates Rust's powerful and safe `slice::swap` operation.
//! In many languages, implementing the one-pass Dutch National Flag algorithm requires
//! careful manual array indexing and temporary variables. In Rust, `nums.swap(i, j)`
//! securely performs the exchange without violating the single-mutable-borrow rule,
//! sidestepping the borrow checker errors that would occur if you tried to manually assign
//! `let temp = nums[i]; nums[i] = nums[j]; nums[j] = temp;` with multiple mutable references.
//!
//! ## Approach
//!
//! We provide three implementations:
//! 1. **Brute Force (`sort_colors_brute_force`)**: Standard library sort (just to show baseline, though restricted by problem).
//! 2. **Optimized (`sort_colors_optimized`)**: A two-pass counting sort. First pass counts `0`s, `1`s, and `2`s. Second pass overwrites the array.
//! 3. **Optimal (`sort_colors_optimal`)**: The Dutch National Flag algorithm. A one-pass approach using three pointers (`low`, `mid`, `high`) to partition the array into three colored regions dynamically.

/// Brute Force Approach: Standard Library Sort
///
/// Time: O(N log N) - standard unstable sort.
/// Space: O(1) auxiliary space.
///
/// **Note**: The problem explicitly forbids using the built-in sort. This is provided
/// purely as a conceptual baseline or standard fallback.
#[allow(clippy::ptr_arg)]
pub fn sort_colors_brute_force(nums: &mut Vec<i32>) {
    // RUST INSIGHT: `sort_unstable` is generally faster and allocates less memory
    // than `sort` when the stability of equal elements is not required (which it isn't here,
    // as all `0`s are indistinguishable).
    nums.sort_unstable();
}

/// Optimized Approach: Two-Pass Counting Sort
///
/// Time: O(N) - exactly two passes.
/// Space: O(1) - we only store three integer counters.
///
/// This approach counts the frequencies of `0`, `1`, and `2`, and then sequentially
/// overwrites the elements in the vector based on the counts.
#[allow(clippy::ptr_arg)]
pub fn sort_colors_optimized(nums: &mut Vec<i32>) {
    // We could use a hash map, but an array is O(1) and specifically fits the constraint (0, 1, 2).
    let mut counts = [0, 0, 0];

    // First pass: count frequencies
    for &num in nums.iter() {
        if num >= 0 && num <= 2 {
            counts[num as usize] += 1;
        }
    }

    // Second pass: overwrite vector
    let mut index = 0;
    for (color, &count) in counts.iter().enumerate() {
        for _ in 0..count {
            nums[index] = color as i32;
            index += 1;
        }
    }
}

/// Optimal Approach: Dutch National Flag (One-Pass)
///
/// Time: O(N) - exactly one pass.
/// Space: O(1) - modifies the array in-place.
///
/// Uses three pointers to maintain three boundaries:
/// - `[0..low]` contains only `0`s.
/// - `[low..mid]` contains only `1`s.
/// - `[mid..=high]` contains unprocessed elements.
/// - `(high..n]` contains only `2`s.
#[allow(clippy::ptr_arg)]
pub fn sort_colors_optimal(nums: &mut Vec<i32>) {
    // GOTCHA: `high` needs to be able to shrink past 0 if the array is empty
    // or if we swap many 2s to the front. However, we can use `usize` safely
    // as long as we check `mid <= high`. If the array is empty, we just return.
    if nums.is_empty() {
        return;
    }

    let mut low = 0;
    let mut mid = 0;
    let mut high = nums.len() - 1;

    // RUST INSIGHT: We use a `while` loop here because the `mid` pointer advances conditionally.
    // Iterators (`for x in ...`) wouldn't allow us to dynamically modify the bounds (`high`)
    // or conditionally advance the loop counter cleanly.
    while mid <= high {
        match nums[mid] {
            0 => {
                // Swap the 0 to the `low` partition.
                // RUST INSIGHT: `slice::swap` is the safe, idiomatic way to exchange elements.
                // Doing `let t = nums[i]; nums[i] = nums[j]; nums[j] = t;` requires overlapping
                // mutable borrows, which Rust forbids. `swap` performs this safely via raw pointers internally.
                nums.swap(low, mid);
                low += 1;
                mid += 1;
            }
            1 => {
                // It's already in the correct middle partition, just move on.
                mid += 1;
            }
            2 => {
                // Swap the 2 to the `high` partition.
                nums.swap(mid, high);
                // GOTCHA: We don't increment `mid` here! The swapped element from `high`
                // could be a 0, 1, or 2, and it must be evaluated on the next iteration.

                // Prevent `usize` underflow if `high` reaches 0.
                if high == 0 {
                    break;
                }
                high -= 1;
            }
            _ => unreachable!("Constraint violation: colors must be 0, 1, or 2"),
        }
    }
}

/// Main entry point - uses the optimal one-pass approach.
pub fn sort_colors(nums: &mut Vec<i32>) {
    sort_colors_optimal(nums);
}

/// ## Alternative Approaches
///
/// - **Quick Sort / Merge Sort**: The Brute Force approach utilizes standard unstable sort (which is
///   Pattern-Defeating Quicksort in Rust). This is O(N log N) but usually quite fast due to caching.
/// - **Lomuto / Hoare Partitioning**: Variations of Quicksort's partition scheme can be applied,
///   but the Dutch National Flag algorithm (3-way partitioning) is perfectly specialized for exactly three values.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sort_colors_brute_force() {
        let mut nums = vec![2, 0, 2, 1, 1, 0];
        sort_colors_brute_force(&mut nums);
        assert_eq!(nums, vec![0, 0, 1, 1, 2, 2]);
    }

    #[test]
    fn test_sort_colors_optimized() {
        let mut nums = vec![2, 0, 2, 1, 1, 0];
        sort_colors_optimized(&mut nums);
        assert_eq!(nums, vec![0, 0, 1, 1, 2, 2]);
    }

    // Happy Path
    #[test]
    fn test_sort_colors_optimal_mixed() {
        let mut nums = vec![2, 0, 2, 1, 1, 0];
        sort_colors_optimal(&mut nums);
        assert_eq!(nums, vec![0, 0, 1, 1, 2, 2]);
    }

    // Edge Case: All same color
    #[test]
    fn test_sort_colors_optimal_all_ones() {
        let mut nums = vec![1, 1, 1, 1];
        sort_colors_optimal(&mut nums);
        assert_eq!(nums, vec![1, 1, 1, 1]);
    }

    // Edge Case: Already sorted
    #[test]
    fn test_sort_colors_optimal_sorted() {
        let mut nums = vec![0, 0, 1, 1, 2, 2];
        sort_colors_optimal(&mut nums);
        assert_eq!(nums, vec![0, 0, 1, 1, 2, 2]);
    }

    // Edge Case: Reverse sorted
    #[test]
    fn test_sort_colors_optimal_reverse() {
        let mut nums = vec![2, 2, 1, 1, 0, 0];
        sort_colors_optimal(&mut nums);
        assert_eq!(nums, vec![0, 0, 1, 1, 2, 2]);
    }

    // Boundary Case: Empty array
    #[test]
    fn test_sort_colors_optimal_empty() {
        let mut nums: Vec<i32> = vec![];
        sort_colors_optimal(&mut nums);
        assert_eq!(nums, vec![]);
    }

    // Boundary Case: Single element
    #[test]
    fn test_sort_colors_optimal_single() {
        let mut nums = vec![2];
        sort_colors_optimal(&mut nums);
        assert_eq!(nums, vec![2]);
    }

    // Stress Test
    #[test]
    fn test_sort_colors_stress() {
        let mut nums = vec![];
        for _ in 0..10_000 {
            nums.push(2);
            nums.push(0);
            nums.push(1);
        }

        let mut expected = nums.clone();
        expected.sort_unstable();

        sort_colors_optimal(&mut nums);
        assert_eq!(nums, expected);
    }
}
