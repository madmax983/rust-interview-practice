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

/// Brute force approach: Pairwise merge until stable.
///
/// Time Complexity: O(N^3) worst case - each full pass scans all O(N^2) pairs, and up to O(N)
/// passes may be required before no further merges occur.
/// Space Complexity: O(N) to store the working set of intervals.
///
/// We repeatedly scan every pair of intervals, absorbing any interval that overlaps the current
/// group into it and expanding the group's bounds. We keep sweeping until an entire pass produces
/// no merges, meaning the set is fully coalesced. This ignores the "sort first" insight entirely,
/// making it a naive but correct baseline. We sort the final result only to produce the canonical
/// ascending-by-start ordering.
#[must_use]
pub fn merge_brute_force(intervals: Vec<Vec<i32>>) -> Vec<Vec<i32>> {
    if intervals.is_empty() {
        return vec![];
    }

    let mut current = intervals;

    loop {
        let n = current.len();
        let mut used = vec![false; n];
        let mut next: Vec<Vec<i32>> = Vec::with_capacity(n);
        let mut merged_any = false;

        for i in 0..n {
            if used[i] {
                continue;
            }

            let mut start = current[i][0];
            let mut end = current[i][1];
            used[i] = true;

            // Absorb every remaining interval that overlaps the (growing) group.
            for j in (i + 1)..n {
                if used[j] {
                    continue;
                }

                // Inclusive overlap (touching endpoints count as overlapping).
                if current[j][0] <= end && start <= current[j][1] {
                    start = start.min(current[j][0]);
                    end = end.max(current[j][1]);
                    used[j] = true;
                    merged_any = true;
                }
            }

            next.push(vec![start, end]);
        }

        current = next;

        if !merged_any {
            break;
        }
    }

    // Canonical ordering: sort the coalesced intervals by start time.
    current.sort_unstable_by(|a, b| a[0].cmp(&b[0]));
    current
}

/// Optimal approach: Sort and Merge
///
/// Time Complexity: O(N log N) dominated by sorting. The linear pass is O(N).
/// Space Complexity: O(N) to store the result (or O(log N) stack space for sorting if we consider input modification in-place).
///
/// We sort the intervals by their start time. This allows us to iterate through the intervals linearly
/// and merge overlapping ones. Since they are sorted, if `current.start <= previous.end`, they overlap.
///
/// We use `sort_unstable_by` because we don't need to preserve the relative order of equal elements (which `sort` guarantees but `sort_unstable` doesn't), and it is generally faster and allocates less memory.
///
/// Note: sort-then-merge is the optimal solution for this problem — its cost is bounded below by the
/// O(N log N) sort. There is no meaningfully distinct "optimized" tier between the naive pairwise
/// scan and this approach, so only `_brute_force` and `_optimal` are provided.
#[must_use]
pub fn merge_optimal(mut intervals: Vec<Vec<i32>>) -> Vec<Vec<i32>> {
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

/// Main entry point - uses optimal solution
#[must_use]
pub fn merge(intervals: Vec<Vec<i32>>) -> Vec<Vec<i32>> {
    merge_optimal(intervals)
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
        assert_eq!(merge_brute_force(intervals.clone()), expected);
        assert_eq!(merge_optimal(intervals), expected);
    }

    #[test]
    fn test_merge_overlap() {
        let intervals = vec![vec![1, 4], vec![4, 5]];
        let expected = vec![vec![1, 5]];
        assert_eq!(merge_brute_force(intervals.clone()), expected);
        assert_eq!(merge_optimal(intervals), expected);
    }

    #[test]
    fn test_merge_contained() {
        let intervals = vec![vec![1, 10], vec![2, 5], vec![6, 9]];
        let expected = vec![vec![1, 10]];
        assert_eq!(merge_brute_force(intervals.clone()), expected);
        assert_eq!(merge_optimal(intervals), expected);
    }

    #[test]
    fn test_merge_unsorted() {
        let intervals = vec![vec![2, 6], vec![1, 3], vec![15, 18], vec![8, 10]];
        let expected = vec![vec![1, 6], vec![8, 10], vec![15, 18]];
        assert_eq!(merge_brute_force(intervals.clone()), expected);
        assert_eq!(merge_optimal(intervals), expected);
    }

    #[test]
    fn test_merge_single() {
        let intervals = vec![vec![1, 4]];
        let expected = vec![vec![1, 4]];
        assert_eq!(merge_brute_force(intervals.clone()), expected);
        assert_eq!(merge_optimal(intervals), expected);
    }

    #[test]
    fn test_merge_empty() {
        let intervals: Vec<Vec<i32>> = vec![];
        let expected: Vec<Vec<i32>> = vec![];
        assert_eq!(merge_brute_force(intervals.clone()), expected);
        assert_eq!(merge_optimal(intervals), expected);
    }

    #[test]
    fn test_merge_touching() {
        let intervals = vec![vec![1, 2], vec![2, 3]];
        let expected = vec![vec![1, 3]];
        assert_eq!(merge_brute_force(intervals.clone()), expected);
        assert_eq!(merge_optimal(intervals), expected);
    }

    #[test]
    fn test_main_entry() {
        let intervals = vec![vec![1, 3], vec![2, 6], vec![8, 10], vec![15, 18]];
        assert_eq!(
            merge(intervals),
            vec![vec![1, 6], vec![8, 10], vec![15, 18]]
        );
    }

    #[test]
    fn test_all_approaches_agree() {
        let cases = vec![
            vec![vec![1, 3], vec![2, 6], vec![8, 10], vec![15, 18]],
            vec![vec![1, 4], vec![4, 5]],
            vec![vec![1, 10], vec![2, 5], vec![6, 9]],
            vec![vec![2, 6], vec![1, 3], vec![15, 18], vec![8, 10]],
            vec![vec![1, 4]],
            vec![vec![1, 2], vec![2, 3]],
            vec![vec![5, 6], vec![1, 4], vec![3, 5], vec![10, 12]],
            vec![vec![1, 4], vec![0, 4]],
        ];
        for case in cases {
            let expected = merge_optimal(case.clone());
            assert_eq!(merge_brute_force(case), expected);
        }
    }
}
