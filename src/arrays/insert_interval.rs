//! # 57. Insert Interval
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/insert-interval/>
//!
//! You are given an array of non-overlapping intervals `intervals` where `intervals[i] = [start_i, end_i]`
//! represent the start and the end of the `i`th interval and `intervals` is sorted in ascending order by `start_i`.
//! You are also given an interval `newInterval = [start, end]` that represents the start and end of another interval.
//!
//! Insert `newInterval` into `intervals` such that `intervals` is still sorted in ascending order by `start_i`
//! and `intervals` still does not have any overlapping intervals (merge overlapping intervals if necessary).
//!
//! This problem is a natural fit for Rust's `Vec` manipulation and iterators. It demonstrates how to effectively
//! consume collections, pattern match on overlap conditions, and efficiently allocate vectors without unnecessary copies.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::arrays::insert_interval::insert;
//!
//! let intervals = vec![vec![1, 3], vec![6, 9]];
//! let new_interval = vec![2, 5];
//! let result = insert(intervals, new_interval);
//! assert_eq!(result, vec![vec![1, 5], vec![6, 9]]);
//! ```
//!
//! ## Constraints
//!
//! - `0 <= intervals.length <= 10^4`
//! - `intervals[i].length == 2`
//! - `0 <= start_i <= end_i <= 10^5`
//! - `intervals` is sorted by `start_i` in ascending order.
//! - `newInterval.length == 2`
//! - `0 <= start <= end <= 10^5`

use std::cmp;

/// Brute Force approach: Push and Sort
///
/// Time Complexity: O(N log N) - We append the new interval and sort the entire vector.
/// Space Complexity: O(N) - We create a new vector for the result.
///
/// This approach completely ignores the fact that the initial list is already sorted.
/// It just adds the `new_interval` and applies the same logic as the "Merge Intervals" problem.
#[must_use]
pub fn insert_brute_force(intervals: Vec<Vec<i32>>, new_interval: Vec<i32>) -> Vec<Vec<i32>> {
    let mut all_intervals = intervals;
    all_intervals.push(new_interval);

    // Sort intervals by start time
    all_intervals.sort_unstable_by(|a, b| a[0].cmp(&b[0]));

    let mut merged: Vec<Vec<i32>> = Vec::with_capacity(all_intervals.len());

    for interval in all_intervals {
        if let Some(last) = merged.last_mut() {
            if interval[0] <= last[1] {
                // Overlapping: Merge by updating the end time.
                last[1] = cmp::max(last[1], interval[1]);
            } else {
                merged.push(interval);
            }
        } else {
            merged.push(interval);
        }
    }

    merged
}

/// Optimized approach: Linear Scan (Three Phases)
///
/// Time Complexity: O(N) - Single pass through the intervals.
/// Space Complexity: O(N) - To store the merged intervals.
///
/// Since the input is sorted, we can break the logic into three distinct phases:
/// 1. Add all intervals that end before `new_interval` starts.
/// 2. Merge all overlapping intervals with `new_interval`.
/// 3. Add all remaining intervals.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
pub fn insert_optimized(intervals: Vec<Vec<i32>>, new_interval: Vec<i32>) -> Vec<Vec<i32>> {
    let mut result = Vec::with_capacity(intervals.len() + 1);
    let mut i = 0;
    let n = intervals.len();

    let mut current_new = new_interval;

    // Phase 1: Add non-overlapping intervals before the new interval
    while i < n && intervals[i][1] < current_new[0] {
        // RUST INSIGHT: We are cloning here because `intervals` is passed by value but we only take references
        // in this loop structure implicitly, or rather we're indexing. To avoid clone, see `insert_optimal`.
        result.push(intervals[i].clone());
        i += 1;
    }

    // Phase 2: Merge overlapping intervals
    // GOTCHA: It's important to keep checking bounds `i < n` in the while loop.
    while i < n && intervals[i][0] <= current_new[1] {
        current_new[0] = cmp::min(current_new[0], intervals[i][0]);
        current_new[1] = cmp::max(current_new[1], intervals[i][1]);
        i += 1;
    }
    result.push(current_new);

    // Phase 3: Add the remaining intervals
    while i < n {
        result.push(intervals[i].clone());
        i += 1;
    }

    result
}

/// Optimal approach: Iterator-based with Zero-Allocation Moves
///
/// Time Complexity: O(N) - Single pass through the intervals.
/// Space Complexity: O(N) - For the result vector.
///
/// This approach is idiomatic Rust, unlike Python or Java where you might rely on index manipulation. We consume the `intervals` vector via `into_iter()`,
/// which takes ownership of each `Vec<i32>` and avoids any `.clone()` calls.
///
/// # Panics
///
/// Does not panic in practice: the internal `unwrap` runs only inside a matching
/// `Some(_)` branch, so the `Option` is guaranteed to hold a value at that point.
#[must_use]
pub fn insert_optimal(intervals: Vec<Vec<i32>>, new_interval: Vec<i32>) -> Vec<Vec<i32>> {
    // We pre-allocate capacity to avoid dynamic heap reallocations during push.
    let mut result = Vec::with_capacity(intervals.len() + 1);

    // We'll wrap our new_interval in an Option. Once it is inserted into the result,
    // we take it out using `Option::take()` so we know it's handled.
    let mut new_int = Some(new_interval);

    for interval in intervals {
        if let Some(ref mut new_i) = new_int {
            if interval[1] < new_i[0] {
                // Current interval ends before new interval starts
                result.push(interval);
            } else if interval[0] > new_i[1] {
                // Current interval starts after new interval ends.
                // We've passed the new interval, so we can insert it now.
                // RUST INSIGHT: `take()` removes the value from the Option, leaving `None`.
                // This prevents us from inserting it again and signals we're in "Phase 3".
                result.push(new_int.take().unwrap());
                result.push(interval);
            } else {
                // Overlapping! Expand the new interval bounds.
                new_i[0] = cmp::min(new_i[0], interval[0]);
                new_i[1] = cmp::max(new_i[1], interval[1]);
            }
        } else {
            // We've already inserted the new interval, just push the rest.
            result.push(interval);
        }
    }

    // If the new interval was never inserted (it belongs at the very end), push it now.
    if let Some(new_i) = new_int {
        result.push(new_i);
    }

    result
}

/// Main entry point - uses the optimal zero-allocation iterator approach.
#[must_use]
pub fn insert(intervals: Vec<Vec<i32>>, new_interval: Vec<i32>) -> Vec<Vec<i32>> {
    insert_optimal(intervals, new_interval)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_basic() {
        let intervals = vec![vec![1, 3], vec![6, 9]];
        let new_interval = vec![2, 5];
        let expected = vec![vec![1, 5], vec![6, 9]];

        assert_eq!(
            insert_brute_force(intervals.clone(), new_interval.clone()),
            expected
        );
        assert_eq!(
            insert_optimized(intervals.clone(), new_interval.clone()),
            expected
        );
        assert_eq!(insert_optimal(intervals, new_interval), expected);
    }

    #[test]
    fn test_insert_multiple_overlap() {
        let intervals = vec![
            vec![1, 2],
            vec![3, 5],
            vec![6, 7],
            vec![8, 10],
            vec![12, 16],
        ];
        let new_interval = vec![4, 8];
        let expected = vec![vec![1, 2], vec![3, 10], vec![12, 16]];

        assert_eq!(
            insert_brute_force(intervals.clone(), new_interval.clone()),
            expected
        );
        assert_eq!(
            insert_optimized(intervals.clone(), new_interval.clone()),
            expected
        );
        assert_eq!(insert_optimal(intervals, new_interval), expected);
    }

    #[test]
    fn test_insert_empty_intervals() {
        let intervals: Vec<Vec<i32>> = vec![];
        let new_interval = vec![5, 7];
        let expected = vec![vec![5, 7]];

        assert_eq!(
            insert_brute_force(intervals.clone(), new_interval.clone()),
            expected
        );
        assert_eq!(
            insert_optimized(intervals.clone(), new_interval.clone()),
            expected
        );
        assert_eq!(insert_optimal(intervals, new_interval), expected);
    }

    #[test]
    fn test_insert_at_beginning() {
        let intervals = vec![vec![3, 4], vec![6, 9]];
        let new_interval = vec![1, 2];
        let expected = vec![vec![1, 2], vec![3, 4], vec![6, 9]];

        assert_eq!(
            insert_brute_force(intervals.clone(), new_interval.clone()),
            expected
        );
        assert_eq!(
            insert_optimized(intervals.clone(), new_interval.clone()),
            expected
        );
        assert_eq!(insert_optimal(intervals, new_interval), expected);
    }

    #[test]
    fn test_insert_at_end() {
        let intervals = vec![vec![1, 2], vec![3, 4]];
        let new_interval = vec![6, 9];
        let expected = vec![vec![1, 2], vec![3, 4], vec![6, 9]];

        assert_eq!(
            insert_brute_force(intervals.clone(), new_interval.clone()),
            expected
        );
        assert_eq!(
            insert_optimized(intervals.clone(), new_interval.clone()),
            expected
        );
        assert_eq!(insert_optimal(intervals, new_interval), expected);
    }
}

// Alternative Approaches:
// 1. Binary Search: You can use binary search (O(log N)) to find the insertion point for the `new_interval`,
//    and then merge only the overlapping part. While asymptotically faster for the search phase,
//    inserting into or splicing a `Vec` in the middle is still O(N), so the overall time complexity
//    remains O(N). The iterator approach is generally preferred in Rust for its safety and lack of bounds checking.
