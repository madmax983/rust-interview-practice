//! # 217. Contains Duplicate
//!
//! Difficulty: Easy
//! Link: <https://leetcode.com/problems/contains-duplicate/>
//!
//! Given an integer array `nums`, return `true` if any value appears at least twice in the array,
//! and return `false` if every element is distinct.
//!
//! This problem is a natural fit for Rust's `std::collections::HashSet` and demonstrates how
//! iterator adapters and basic data structures provide clean, safe, and efficient solutions.
//! It also highlights the trade-off between sorting (O(1) space but O(n log n) time) and hashing
//! (O(n) space but O(n) time).
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::arrays::contains_duplicate::contains_duplicate;
//!
//! assert_eq!(contains_duplicate(vec![1, 2, 3, 1]), true);
//! assert_eq!(contains_duplicate(vec![1, 2, 3, 4]), false);
//! assert_eq!(contains_duplicate(vec![1, 1, 1, 3, 3, 4, 3, 2, 4, 2]), true);
//! ```
//!
//! ## Constraints
//!
//! - `1 <= nums.length <= 10^5`
//! - `-10^9 <= nums[i] <= 10^9`

use std::collections::HashSet;

/// Brute force approach: Nested loops
///
/// Time: O(n²) - For each element, we scan the rest of the array to find a match.
/// Space: O(1) - No extra space allocated.
///
/// This approach tests every possible pair `(i, j)`. It's straightforward but
/// fails with a Time Limit Exceeded (TLE) on large inputs due to O(n²) complexity.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
pub fn contains_duplicate_brute_force(nums: Vec<i32>) -> bool {
    let n = nums.len();

    // RUST INSIGHT: Ranges in Rust are exclusively bounded at the top (`0..n`).
    // If `n` is 0 or 1, the outer loop body won't execute, safely returning `false`.
    for i in 0..n {
        for j in (i + 1)..n {
            if nums[i] == nums[j] {
                return true;
            }
        }
    }

    false
}

/// Optimized approach: Sort and scan adjacent elements
///
/// Time: O(n log n) - Sorting takes O(n log n) time.
/// Space: O(1) or O(n) depending on the sorting algorithm. `sort_unstable` is O(1) auxiliary space.
///
/// By sorting the array first, duplicates are guaranteed to be adjacent.
/// We can then find a duplicate in a single O(n) pass.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
pub fn contains_duplicate_optimized(mut nums: Vec<i32>) -> bool {
    // RUST INSIGHT: `sort_unstable` is generally faster than `sort` for primitive types
    // where element order stability doesn't matter, and it allocates less memory.
    nums.sort_unstable();

    // RUST INSIGHT: The `windows(2)` iterator adapter creates a sliding window of size 2.
    // `.any()` lazily evaluates the predicate and short-circuits upon the first `true`.
    // This entirely replaces the manual `for i in 0..nums.len() - 1` index checking.
    nums.windows(2).any(|w| w[0] == w[1])
}

/// Optimal approach: HashSet (one pass)
///
/// Time: O(n) - Inserting into a HashSet is O(1) on average, giving O(n) total time.
/// Space: O(n) - In the worst case (all elements unique), we store all `n` elements.
///
/// The HashSet is perfect here. We iterate through the array and try to insert each
/// number into the set. If `insert` returns `false`, it means the number was already
/// in the set, proving a duplicate exists.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
pub fn contains_duplicate_optimal(nums: Vec<i32>) -> bool {
    // We pre-allocate the capacity if we expect mostly unique elements,
    // though starting without capacity is fine.
    // HashSet::with_capacity(nums.len()) could prevent re-allocations.
    let mut seen = HashSet::with_capacity(nums.len());

    // GOTCHA: Don't check `.contains()` then `.insert()`. That performs two hash lookups.
    // Instead, `.insert()` itself returns a boolean: true if the value was newly inserted,
    // false if it was already present.
    for &num in &nums {
        if !seen.insert(num) {
            return true;
        }
    }

    false
}

/// Main entry point - uses optimal solution
#[must_use]
pub fn contains_duplicate(nums: Vec<i32>) -> bool {
    contains_duplicate_optimal(nums)
}

// Alternative Approaches:
// 1. **Functional Optimal**: `nums.into_iter().any(|num| !seen.insert(num))`
//    This is slightly more concise but performs identically to the loop-based optimal approach.
// 2. **Length Comparison**: `nums.iter().collect::<HashSet<_>>().len() != nums.len()`
//    This is very concise but slightly less efficient because it cannot short-circuit. It must process the whole array.

#[cfg(test)]
mod tests {
    use super::*;

    // Happy Path tests
    #[test]
    fn test_contains_duplicate_basic() {
        let input = vec![1, 2, 3, 1];
        assert!(contains_duplicate_brute_force(input.clone()));
        assert!(contains_duplicate_optimized(input.clone()));
        assert!(contains_duplicate_optimal(input));
    }

    #[test]
    fn test_all_unique() {
        let input = vec![1, 2, 3, 4];
        assert!(!contains_duplicate_brute_force(input.clone()));
        assert!(!contains_duplicate_optimized(input.clone()));
        assert!(!contains_duplicate_optimal(input));
    }

    // Edge Case tests
    #[test]
    fn test_empty_array() {
        let input: Vec<i32> = vec![];
        assert!(!contains_duplicate_brute_force(input.clone()));
        assert!(!contains_duplicate_optimized(input.clone()));
        assert!(!contains_duplicate_optimal(input));
    }

    #[test]
    fn test_single_element() {
        let input = vec![1];
        assert!(!contains_duplicate_brute_force(input.clone()));
        assert!(!contains_duplicate_optimized(input.clone()));
        assert!(!contains_duplicate_optimal(input));
    }

    // Stress/Boundary tests
    #[test]
    fn test_multiple_duplicates() {
        let input = vec![1, 1, 1, 3, 3, 4, 3, 2, 4, 2];
        assert!(contains_duplicate_brute_force(input.clone()));
        assert!(contains_duplicate_optimized(input.clone()));
        assert!(contains_duplicate_optimal(input));
    }

    #[test]
    fn test_negative_numbers() {
        let input = vec![-1, -2, -3, -1];
        assert!(contains_duplicate_brute_force(input.clone()));
        assert!(contains_duplicate_optimized(input.clone()));
        assert!(contains_duplicate_optimal(input));
    }
}
