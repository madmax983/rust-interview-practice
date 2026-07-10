//! # 128. Longest Consecutive Sequence
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/longest-consecutive-sequence/>
//!
//! Given an unsorted array of integers `nums`, return the length of the longest consecutive elements sequence.
//!
//! You must write an algorithm that runs in `O(n)` time.
//!
//! This problem perfectly demonstrates Rust's `HashSet` for O(1) lookups and how iterator
//! combinators can elegantly replace imperative while loops. It shows how ownership of the
//! collection (`into_iter()`) or references (`iter()`) affects the API design.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::arrays::longest_consecutive_sequence::longest_consecutive;
//!
//! assert_eq!(longest_consecutive(vec![100, 4, 200, 1, 3, 2]), 4);
//! assert_eq!(longest_consecutive(vec![0, 3, 7, 2, 5, 8, 4, 6, 0, 1]), 9);
//! ```
//!
//! ## Constraints
//!
//! - `0 <= nums.length <= 10^5`
//! - `-10^9 <= nums[i] <= 10^9`

use std::collections::HashSet;

/// Brute force approach: Sort and count.
/// Time: O(n log n) - dominated by sorting.
/// Space: O(1) or O(n) depending on the sorting algorithm (Rust's `slice::sort` is O(n) worst-case space).
///
/// This approach modifies the input to sort it, then uses iterator windows to
/// cleanly count consecutive elements.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
pub fn longest_consecutive_brute_force(mut nums: Vec<i32>) -> i32 {
    if nums.is_empty() {
        return 0;
    }

    // Sort the vector in-place
    nums.sort_unstable();

    let mut max_len = 1;
    let mut current_len = 1;

    // RUST INSIGHT: `.windows(2)` gives an overlapping sliding window of size 2.
    // This allows us to easily compare adjacent elements without dealing with
    // off-by-one index errors (`i` and `i+1`).
    for window in nums.windows(2) {
        if window[0] == window[1] {
            // Skip duplicates
        } else if window[0] + 1 == window[1] {
            // Consecutive elements
            current_len += 1;
            max_len = max_len.max(current_len);
        } else {
            // Sequence broke, reset length
            current_len = 1;
        }
    }

    max_len
}

/// Optimal approach: `HashSet` and Iterator chain.
///
/// Time: O(n) - We iterate through the array to build the set, then iterate through
///       again, only starting a sequence count if the number is the start of a sequence.
///       The inner loop runs at most `n` times total across all iterations.
/// Space: O(n) - The `HashSet` stores at most `n` unique elements.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
// LeetCode signature
// `take_while` bounds the open-ended `(num..)` range, so iteration is finite.
#[allow(clippy::maybe_infinite_iter)]
pub fn longest_consecutive_optimal(nums: Vec<i32>) -> i32 {
    // Collect elements into a HashSet for O(1) lookups.
    // RUST INSIGHT: We use `into_iter()` to consume the vector and move the `i32`s
    // directly into the set, avoiding allocations for references.
    let set: HashSet<i32> = nums.into_iter().collect();
    let mut max_len = 0;

    for &num in &set {
        // Only start counting if `num` is the start of a sequence.
        // If `num - 1` is in the set, it means `num` is part of a sequence that
        // we've already counted or will count later.
        if !set.contains(&(num - 1)) {
            // Start of a sequence!

            // GOTCHA: We could use a `while set.contains(&(current_num + 1))` loop here,
            // but Rust's iterators allow a more functional approach. However, generating
            // an infinite iterator `(num..)` and taking while the condition is true
            // is perfectly idiomatic and prevents manual mutation of `current_num`.
            let current_len = (num..).take_while(|n| set.contains(n)).count();

            // Cast back to i32 to match LeetCode signature, and update max
            // `.try_into().unwrap()` or `as i32` is needed because `.count()` returns `usize`.
            // We use `as i32` because we know the length won't exceed the vector capacity (10^5).
            #[allow(clippy::cast_possible_wrap, clippy::cast_possible_truncation)]
            {
                max_len = max_len.max(current_len as i32);
            }
        }
    }

    max_len
}

/// Main entry point - uses optimal solution
#[must_use]
pub fn longest_consecutive(nums: Vec<i32>) -> i32 {
    longest_consecutive_optimal(nums)
}

// Alternative approaches footer:
// - Union-Find (Disjoint Set): You can map each number to its index and union adjacent
//   numbers. The size of the largest connected component is the answer. This is O(N)
//   amortized but typically slower than HashSet due to map overhead and tree operations.

#[cfg(test)]
mod tests {
    use super::*;

    // Happy Path
    #[test]
    fn test_happy_path() {
        let nums = vec![100, 4, 200, 1, 3, 2];
        assert_eq!(longest_consecutive_brute_force(nums.clone()), 4);
        assert_eq!(longest_consecutive_optimal(nums), 4);
    }

    // Edge Case: Empty array
    #[test]
    fn test_empty() {
        let nums: Vec<i32> = vec![];
        assert_eq!(longest_consecutive_brute_force(nums.clone()), 0);
        assert_eq!(longest_consecutive_optimal(nums), 0);
    }

    // Edge Case: Single element
    #[test]
    fn test_single_element() {
        let nums = vec![42];
        assert_eq!(longest_consecutive_brute_force(nums.clone()), 1);
        assert_eq!(longest_consecutive_optimal(nums), 1);
    }

    // Stress/Boundary case: Duplicates and negatives
    #[test]
    fn test_duplicates_and_negatives() {
        let nums = vec![0, 3, 7, 2, 5, 8, 4, 6, 0, 1, -1, -2];
        assert_eq!(longest_consecutive_brute_force(nums.clone()), 11); // -2 to 8
        assert_eq!(longest_consecutive_optimal(nums), 11);
    }
}
