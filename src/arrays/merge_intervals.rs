//! # 56. Merge Intervals
//!
//! Given an array of `intervals` where `intervals[i] = [start_i, end_i]`, merge all overlapping intervals,
//! and return an array of the non-overlapping intervals that cover all the intervals in the input.
//!
//! [LeetCode Problem 56](https://leetcode.com/problems/merge-intervals/)
//!
//! ## Why this matters in Rust
//! This problem is a classic example of vector manipulation and sorting. It highlights:
//! -   **Ownership and Borrowing**: Handling nested vectors (`Vec<Vec<i32>>`) requires understanding when to move data versus when to borrow it.
//! -   **Iterators**: Efficiently iterating through sorted data.
//! -   **Zero-cost Abstractions**: Using `sort_unstable_by` and `last_mut` allows us to write high-level, readable code that compiles down to efficient machine instructions.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::arrays::merge_intervals::merge;
//!
//! let intervals = vec![vec![1, 3], vec![2, 6], vec![8, 10], vec![15, 18]];
//! let result = merge(intervals);
//! assert_eq!(result, vec![vec![1, 6], vec![8, 10], vec![15, 18]]);
//!
//! let intervals2 = vec![vec![1, 4], vec![4, 5]];
//! let result2 = merge(intervals2);
//! assert_eq!(result2, vec![vec![1, 5]]);
//! ```
//!
//! ## Constraints
//!
//! -   `1 <= intervals.length <= 10^4`
//! -   `intervals[i].length == 2`
//! -   `0 <= start_i <= end_i <= 10^4`

/// Approach: Sort and Merge
///
/// Time Complexity: O(N log N) dominated by sorting. The linear pass is O(N).
/// Space Complexity: O(N) to store the result (or O(log N) stack space for sorting if we consider input modification in-place).
///
/// We sort the intervals by their start time. This allows us to iterate through the intervals linearly
/// and merge overlapping ones. Since they are sorted, if `current.start <= previous.end`, they overlap.
///
/// We use `sort_unstable_by` because we don't need to preserve the relative order of equal elements (which `sort` guarantees but `sort_unstable` doesn't), and it is generally faster and allocates less memory.
#[must_use]
pub fn merge(mut intervals: Vec<Vec<i32>>) -> Vec<Vec<i32>> {
    if intervals.is_empty() {
        return vec![];
    }

    // RUST INSIGHT: Sorting in-place.
    // We modify the input vector directly. `sort_unstable_by` is faster than `sort_by` when stable sort isn't required.
    // We use a closure `|a, b| a[0].cmp(&b[0])` to sort by the first element (start time).
    intervals.sort_unstable_by(|a, b| a[0].cmp(&b[0]));

    let mut merged: Vec<Vec<i32>> = Vec::with_capacity(intervals.len());

    // Initialize with the first interval.
    // RUST INSIGHT: `into_iter` moves ownership.
    // By consuming `intervals`, we can move the inner vectors directly into `merged` without cloning.
    // This is more efficient than iterating by reference if we don't need the original `intervals` vector afterward.
    let mut iter = intervals.into_iter();
    if let Some(first) = iter.next() {
        merged.push(first);
    }

    for interval in iter {
        // RUST INSIGHT: `last_mut` returns `Option<&mut T>`.
        // This gives us a mutable reference to the last element in `merged` without needing index calculations.
        // The `unwrap()` is safe here because we pushed the first element before the loop.
        let last = merged.last_mut().unwrap();

        // Check for overlap
        // `last` is `&mut Vec<i32>`, so `last[1]` accesses the end time of the previous interval.
        // `interval` is `Vec<i32>` (owned), so `interval[0]` is the start time of the current interval.
        if interval[0] <= last[1] {
            // Overlapping: Merge by updating the end time of the last interval.
            // We use `std::cmp::max` to ensure we cover the full range.
            last[1] = std::cmp::max(last[1], interval[1]);
        } else {
            // Non-overlapping: Push the current interval as a new entry.
            merged.push(interval);
        }
    }

    merged
}

/// Alternative Approach: Fold
///
/// While `fold` is a powerful iterator adaptor, using it here can be slightly more complex due to the need
/// to access the "current last" element of the accumulator. A manual loop is often clearer for this specific logic
/// because `fold` expects the accumulator to be passed by value in each step, which might involve more boilerplate
/// to modify the `Vec` in place efficiently. However, `try_fold` or `fold` with a mutable reference could work.

// GOTCHA: Input validation.
// The problem constraints guarantee `intervals[i].length == 2`.
// In a real-world scenario without these guarantees, we should add checks:
// `if interval.len() < 2 { return Err("Invalid interval"); }`
// Accessing `interval[0]` or `interval[1]` on an empty vector would panic.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_basic() {
        let intervals = vec![vec![1, 3], vec![2, 6], vec![8, 10], vec![15, 18]];
        let expected = vec![vec![1, 6], vec![8, 10], vec![15, 18]];
        assert_eq!(merge(intervals), expected);
    }

    #[test]
    fn test_merge_overlap() {
        let intervals = vec![vec![1, 4], vec![4, 5]];
        let expected = vec![vec![1, 5]];
        assert_eq!(merge(intervals), expected);
    }

    #[test]
    fn test_merge_contained() {
        let intervals = vec![vec![1, 10], vec![2, 5], vec![6, 9]];
        let expected = vec![vec![1, 10]];
        assert_eq!(merge(intervals), expected);
    }

    #[test]
    fn test_merge_unsorted() {
        let intervals = vec![vec![2, 6], vec![1, 3], vec![15, 18], vec![8, 10]];
        let expected = vec![vec![1, 6], vec![8, 10], vec![15, 18]];
        assert_eq!(merge(intervals), expected);
    }

    #[test]
    fn test_merge_single() {
        let intervals = vec![vec![1, 4]];
        let expected = vec![vec![1, 4]];
        assert_eq!(merge(intervals), expected);
    }

    #[test]
    fn test_merge_empty() {
        let intervals: Vec<Vec<i32>> = vec![];
        let expected: Vec<Vec<i32>> = vec![];
        assert_eq!(merge(intervals), expected);
    }

    #[test]
    fn test_merge_touching() {
        let intervals = vec![vec![1, 2], vec![2, 3]];
        let expected = vec![vec![1, 3]];
        assert_eq!(merge(intervals), expected);
    }
}
