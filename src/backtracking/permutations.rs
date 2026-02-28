//! # 46. Permutations
//!
//! **Difficulty: Medium**
//!
//! [LeetCode Problem 46](https://leetcode.com/problems/permutations/)
//!
//! Given an array `nums` of distinct integers, return all the possible permutations. You can return the answer in any order.
//!
//! ## Why this matters in Rust
//! This problem demonstrates a fundamental backtracking pattern: **State Modification**.
//! In Rust, managing the mutable state of the current permutation during recursion requires careful ownership management.
//!
//! It highlights:
//! -   **Mutable References**: Passing `&mut Vec<T>` through recursion.
//! -   **Backtracking**: The "Do, Recurse, Undo" pattern.
//! -   **Swapping**: Using slice `swap` to generate permutations in-place without allocating new vectors for every recursive step.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::backtracking::permutations::permute;
//!
//! let nums = vec![1, 2, 3];
//! let result = permute(nums);
//! assert_eq!(result.len(), 6);
//! // Output: [[1,2,3],[1,3,2],[2,1,3],[2,3,1],[3,1,2],[3,2,1]]
//! ```
//!
//! ## Constraints
//!
//! - `1 <= nums.length <= 6`
//! - `-10 <= nums[i] <= 10`
//! - All the integers of `nums` are unique.

/// Approach: Backtracking with Swaps
///
/// **Strategy**:
/// To generate all permutations of `nums[start..]`:
/// 1. Iterate `i` from `start` to `end`.
/// 2. Swap `nums[start]` with `nums[i]`. This places the i-th element at the `start` position.
/// 3. Recursively generate permutations for `nums[start + 1..]`.
/// 4. Swap back (backtrack) to restore the original order for the next iteration.
///
/// **Time**: O(N * N!) - There are N! permutations, and copying each takes O(N).
/// **Space**: O(N) - Recursion stack depth is N.
///
/// # RUST INSIGHT
/// By passing `&mut nums` and swapping in place, we avoid creating intermediate vectors.
/// We only clone the vector when we reach a base case (a valid permutation).
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn permute_recursive(mut nums: Vec<i32>) -> Vec<Vec<i32>> {
    let mut results = Vec::new();
    let n = nums.len();
    backtrack(n, 0, &mut nums, &mut results);
    results
}

fn backtrack(n: usize, start: usize, nums: &mut Vec<i32>, results: &mut Vec<Vec<i32>>) {
    if start == n {
        results.push(nums.clone());
        return;
    }

    for i in start..n {
        // Place i-th element at 'start'
        // RUST INSIGHT: `swap` takes two indices and swaps the elements at those indices.
        // This is safe because `start` and `i` are valid indices within `nums`.
        // If `start` == `i`, it swaps the element with itself (no-op).
        nums.swap(start, i);

        // Recurse on the sub-problem
        backtrack(n, start + 1, nums, results);

        // Backtrack: restore original state
        // By swapping back, we undo the change made in this iteration, ensuring the next iteration starts from a clean state.
        nums.swap(start, i);
    }
}

/// Alternative Approach: Heap's Algorithm (Iterative)
///
/// Heap's algorithm generates permutations by swapping elements.
/// It can be implemented iteratively to avoid recursion depth limits (though N=6 is small).
///
/// We stick to the recursive swap approach above as it is the standard "backtracking" teaching example.

/// Main entry point
#[must_use]
pub fn permute(nums: Vec<i32>) -> Vec<Vec<i32>> {
    permute_recursive(nums)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    // Helper to check if results match expected set of permutations
    fn check_permutations(nums: Vec<i32>, expected_count: usize) {
        let result = permute(nums.clone());
        assert_eq!(result.len(), expected_count);

        // Convert to HashSet for order-independent comparison
        let result_set: HashSet<Vec<i32>> = result.into_iter().collect();
        assert_eq!(
            result_set.len(),
            expected_count,
            "Duplicate permutations found"
        );

        // Check that all results are valid permutations (contain same elements)
        let expected_elements: HashSet<i32> = nums.into_iter().collect();
        for p in &result_set {
            let p_elements: HashSet<i32> = p.clone().into_iter().collect();
            assert_eq!(p_elements, expected_elements);
        }
    }

    #[test]
    fn test_example_1() {
        let nums = vec![1, 2, 3];
        // 3! = 6
        check_permutations(nums, 6);
    }

    #[test]
    fn test_example_2() {
        let nums = vec![0, 1];
        // 2! = 2
        check_permutations(nums, 2);
    }

    #[test]
    fn test_example_3() {
        let nums = vec![1];
        // 1! = 1
        check_permutations(nums, 1);
    }

    #[test]
    fn test_empty() {
        let nums = vec![];
        let result = permute(nums);
        // Permutation of empty set is a set containing empty set: [[]]
        assert_eq!(result, vec![vec![]]);
    }

    #[test]
    fn test_larger_input() {
        let nums = vec![1, 2, 3, 4];
        // 4! = 24
        check_permutations(nums, 24);
    }
}
