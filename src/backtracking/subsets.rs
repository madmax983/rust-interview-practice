//! # 78. Subsets
//!
//! **Difficulty: Medium**
//!
//! [LeetCode Problem 78](https://leetcode.com/problems/subsets/)
//!
//! Given an integer array `nums` of unique elements, return all possible subsets (the power set).
//! The solution set must not contain duplicate subsets. Return the solution in any order.
//!
//! ## Why this matters in Rust
//! This problem perfectly demonstrates Rust's varied approaches to building collections.
//! We can solve it using imperative backtracking (managing mutable state and lifetimes),
//! functional iterator combinators (folding over collections), or bit manipulation.
//! It highlights ownership transfer, `Vec::push` vs `Vec::pop` in recursive contexts, and
//! the elegance of `Iterator::fold`.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::backtracking::subsets::subsets;
//!
//! let nums = vec![1, 2, 3];
//! let result = subsets(nums);
//! assert_eq!(result.len(), 8);
//! // Output might be: [[],[1],[2],[1,2],[3],[1,3],[2,3],[1,2,3]]
//! ```
//!
//! ## Constraints
//!
//! - `1 <= nums.length <= 10`
//! - `-10 <= nums[i] <= 10`
//! - All the numbers of `nums` are unique.

/// Brute force approach: Recursive Backtracking
///
/// Note: all three approaches in this file share the same `O(N * 2^N)` asymptotic
/// complexity (the output alone is that large). The tiers reflect constant-factor cost
/// and idiomatic clarity, not asymptotic ranking. This recursive variant carries the most
/// overhead (call-stack management plus per-leaf clones), hence `brute_force`.
///
/// **Strategy**:
/// At each element in the input array, we have two choices:
/// 1. Include the element in the current subset.
/// 2. Exclude the element from the current subset.
///
/// We use recursion to explore both branches. To avoid allocating a new `Vec` for every step,
/// we maintain a single `current` path and push/pop elements as we recurse and backtrack.
///
/// **Time Complexity**: O(N * 2^N) - We generate 2^N subsets, and for each subset, we copy up to N elements into the result.
/// **Space Complexity**: O(N) - The recursion stack and the `current` path vector take O(N) space. The result vector takes O(N * 2^N) space, which is typically excluded from auxiliary space.
///
/// # RUST INSIGHT
/// Passing `&mut Vec<i32>` for `current_path` is idiomatic Rust backtracking. It avoids unnecessary
/// heap allocations that would occur if we passed cloned vectors down the call stack. We only clone
/// when adding a completed path to the final `results`.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn subsets_brute_force(nums: Vec<i32>) -> Vec<Vec<i32>> {
    // 2^N subsets are generated, so we pre-allocate the capacity
    let mut results = Vec::with_capacity(1 << nums.len());
    let mut current_path = Vec::new();

    // We use an inner closure or helper function for recursion.
    // A helper function is typically preferred in Rust for readability and simpler type checking.
    backtrack(&nums, 0, &mut current_path, &mut results);

    results
}

fn backtrack(nums: &[i32], index: usize, current_path: &mut Vec<i32>, results: &mut Vec<Vec<i32>>) {
    // Base case / Leaf node: We reached the end of the array, meaning we have made a decision
    // for every element (include or exclude). Add the current subset to results.
    if index == nums.len() {
        results.push(current_path.clone());
        return;
    }

    // Branch 1: Exclude the current element
    // Move to the next index without adding nums[index] to the current path.
    backtrack(nums, index + 1, current_path, results);

    // Branch 2: Include the current element
    // GOTCHA: We must push, recurse, then pop. The pop is crucial because it restores
    // the state of `current_path` for the caller (the previous level in the recursion tree).
    current_path.push(nums[index]);
    backtrack(nums, index + 1, current_path, results);
    current_path.pop();
}

/// Optimized approach: Iterative / Cascading
///
/// Equivalent `O(N * 2^N)` complexity to the other approaches, but avoids recursion overhead
/// by building the power set iteratively with `Iterator::fold`.
///
/// **Strategy**:
/// Start with an empty subset `[[]]`.
/// For each number in `nums`, take all existing subsets, add the current number to them,
/// and append these new subsets to the result.
///
/// **Time Complexity**: O(N * 2^N)
/// **Space Complexity**: O(N * 2^N) for the result.
///
/// # RUST INSIGHT
/// This approach beautifully avoids recursion entirely. We use `Iterator::fold` to build the
/// result functional-style. The initial state is a vector containing an empty vector.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn subsets_optimized(nums: Vec<i32>) -> Vec<Vec<i32>> {
    nums.into_iter().fold(vec![vec![]], |mut acc, num| {
        // BOLT OPTIMIZATION: Avoid intermediate `.collect::<Vec<_>>()` chains.
        // We know exactly how many new subsets we will add (the current length of `acc`).
        // By pre-allocating the space and pushing directly, we eliminate an unnecessary
        // heap allocation of `Vec<Vec<i32>>` in every iteration step.
        let len = acc.len();
        acc.reserve(len);

        for i in 0..len {
            let mut new_subset = acc[i].clone();
            new_subset.push(num);
            acc.push(new_subset);
        }

        acc
    })
}

/// Optimal approach: Bit Manipulation
///
/// Equivalent `O(N * 2^N)` complexity, but with the lowest constant factor: no recursion and no
/// intermediate cloning of prior subsets. Each subset maps directly to the set bits of an integer
/// mask. Valid because the constraint `N <= 10` fits comfortably in a machine word.
///
/// **Strategy**:
/// A subset can be represented by a binary sequence of length N, where the i-th bit indicates
/// whether the i-th element of `nums` is included in the subset. Since there are 2^N subsets,
/// we can iterate from 0 to 2^N - 1, and for each number, use its bit representation to form a subset.
///
/// **Time Complexity**: O(N * 2^N)
/// **Space Complexity**: O(N * 2^N) for the result.
///
/// # RUST INSIGHT
/// `1 << n` efficiently calculates 2^n. Rust's bitwise operators (`&`, `<<`) make this
/// mathematical mapping direct and performant. `(mask & (1 << i)) != 0` checks if the i-th bit is set.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn subsets_optimal(nums: Vec<i32>) -> Vec<Vec<i32>> {
    let n = nums.len();
    let subset_count = 1 << n; // 2^n
    let mut results = Vec::with_capacity(subset_count);

    for mask in 0..subset_count {
        // BOLT OPTIMIZATION: Mathematically pre-allocate exact capacity using count_ones()
        // to prevent heap reallocations during subset construction.
        let mut subset = Vec::with_capacity(mask.count_ones() as usize);
        for (i, &num) in nums.iter().enumerate() {
            // Check if the i-th bit of `mask` is set
            if (mask & (1 << i)) != 0 {
                subset.push(num);
            }
        }
        results.push(subset);
    }

    results
}

/// Main entry point - uses the optimal (bit manipulation) solution
#[must_use]
pub fn subsets(nums: Vec<i32>) -> Vec<Vec<i32>> {
    subsets_optimal(nums)
}

/// Approach notes (all `O(N * 2^N)`; differ only in constant factor / readability):
/// - **Backtracking** (`subsets_brute_force`): Best when we need to add constraints (like `subsets_with_dup` where we prune branches).
/// - **Functional/Cascading** (`subsets_optimized`): Most readable and idiomatic in Rust when just building combinations.
/// - **Bitwise** (`subsets_optimal`): Fastest due to avoiding recursion overhead, but only works if N is small (<= 64 for `u64`, <= 32 for `i32`).

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    // Helper to verify that two sets of subsets are equivalent regardless of order
    fn assert_subsets_eq(mut actual: Vec<Vec<i32>>, expected: Vec<Vec<i32>>) {
        assert_eq!(actual.len(), expected.len(), "Different number of subsets");

        // Sort inner vectors to normalize them
        for subset in &mut actual {
            subset.sort_unstable();
        }

        let mut expected_normalized = expected;
        for subset in &mut expected_normalized {
            subset.sort_unstable();
        }

        let actual_set: HashSet<Vec<i32>> = actual.into_iter().collect();
        let expected_set: HashSet<Vec<i32>> = expected_normalized.into_iter().collect();

        assert_eq!(actual_set, expected_set);
    }

    #[test]
    fn test_brute_force_basic() {
        let nums = vec![1, 2, 3];
        let expected = vec![
            vec![],
            vec![1],
            vec![2],
            vec![1, 2],
            vec![3],
            vec![1, 3],
            vec![2, 3],
            vec![1, 2, 3],
        ];
        assert_subsets_eq(subsets_brute_force(nums), expected);
    }

    #[test]
    fn test_optimized_basic() {
        let nums = vec![1, 2, 3];
        let expected = vec![
            vec![],
            vec![1],
            vec![2],
            vec![1, 2],
            vec![3],
            vec![1, 3],
            vec![2, 3],
            vec![1, 2, 3],
        ];
        assert_subsets_eq(subsets_optimized(nums), expected);
    }

    #[test]
    fn test_optimal_basic() {
        let nums = vec![1, 2, 3];
        let expected = vec![
            vec![],
            vec![1],
            vec![2],
            vec![1, 2],
            vec![3],
            vec![1, 3],
            vec![2, 3],
            vec![1, 2, 3],
        ];
        assert_subsets_eq(subsets_optimal(nums), expected);
    }

    #[test]
    fn test_all_approaches_agree() {
        // Cross-implementation agreement: all three approaches must produce the same
        // power set (order-independent) for a non-trivial input.
        let nums = vec![4, 1, 7, 2];
        let bf = subsets_brute_force(nums.clone());
        let opt = subsets_optimized(nums.clone());
        let optimal = subsets_optimal(nums);
        assert_subsets_eq(bf.clone(), opt.clone());
        assert_subsets_eq(opt, optimal);
    }

    #[test]
    fn test_empty_input() {
        let expected = vec![vec![]];
        assert_subsets_eq(subsets_brute_force(vec![]), expected.clone());
        assert_subsets_eq(subsets_optimized(vec![]), expected.clone());
        assert_subsets_eq(subsets_optimal(vec![]), expected);
    }

    #[test]
    fn test_single_element() {
        let expected = vec![vec![], vec![0]];
        assert_subsets_eq(subsets_brute_force(vec![0]), expected.clone());
        assert_subsets_eq(subsets_optimized(vec![0]), expected.clone());
        assert_subsets_eq(subsets_optimal(vec![0]), expected);
    }
}
