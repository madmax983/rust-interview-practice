//! # 39. Combination Sum
//!
//! Difficulty: Medium
//! Link: https://leetcode.com/problems/combination-sum/
//!
//! Given an array of distinct integers `candidates` and a target integer `target`,
//! return a list of all unique combinations of `candidates` where the chosen numbers sum to `target`.
//! You may return the combinations in any order.
//!
//! The same number may be chosen from `candidates` an unlimited number of times.
//! Two combinations are unique if the frequency of at least one of the chosen numbers is different.
//!
//! ## Why this matters in Rust
//!
//! This problem is a textbook example of recursive backtracking. In many garbage-collected languages,
//! a common anti-pattern is to allocate a new list for every recursive call, leading to immense GC pressure.
//! In Rust, this problem elegantly demonstrates the power of mutable references (`&mut Vec<i32>`).
//! We can pass a single `path` buffer up and down the call stack, mutating it via `push()` and `pop()`.
//! The borrow checker guarantees that this single mutable reference is safe, yielding a highly optimized,
//! zero-allocation (during the descent) backtracking traversal.
//!
//! ## Approach
//!
//! **Backtracking / DFS**
//!
//! The core idea is to explore all possible combinations by either including the current candidate
//! (and allowing it to be reused) or moving on to the next candidate.
//!
//! 1.  **Brute Force Backtracking**: Try all possible combinations. We maintain a `path` of numbers chosen so far.
//!     If the sum exceeds the `target`, we backtrack. If it equals the `target`, we record the combination.
//! 2.  **Optimal (Pruned Backtracking)**: By sorting the `candidates` first, we can implement early pruning.
//!     If adding the current candidate exceeds the `target`, we know that adding any subsequent (larger) candidate
//!     will also exceed the `target`, so we can immediately break out of the loop.
//!
//! ## Time and Space Complexity
//!
//! - **Time Complexity**: `O(N ^ (T/M))` where `N` is the number of candidates, `T` is the target value,
//!   and `M` is the minimal value among the candidates. This is a loose upper bound, as the execution tree
//!   can be quite large in the worst case. Sorting adds `O(N log N)`, which is negligible compared to the exponential backtracking.
//! - **Space Complexity**: `O(T/M)` for the recursion stack and the `path` buffer, excluding the space required to hold the output.

/// Brute Force Approach: Standard Backtracking
///
/// This approach blindly explores the state space. It works, but it can waste time
/// exploring branches that are guaranteed to fail (e.g., when the sum already exceeds the target).
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn combination_sum_brute_force(candidates: Vec<i32>, target: i32) -> Vec<Vec<i32>> {
    let mut results = Vec::new();
    let mut path = Vec::new();

    fn backtrack(
        candidates: &[i32],
        target: i32,
        start_idx: usize,
        current_sum: i32,
        path: &mut Vec<i32>,
        results: &mut Vec<Vec<i32>>,
    ) {
        // Base case: we hit the exact target
        if current_sum == target {
            // RUST INSIGHT: We must `.clone()` the path here to store a snapshot
            // in our results vector. `path` itself is reused across calls.
            results.push(path.clone());
            return;
        }

        // Base case: we exceeded the target
        if current_sum > target {
            return;
        }

        // Explore further
        for i in start_idx..candidates.len() {
            let candidate = candidates[i];

            // Choose
            path.push(candidate);

            // Explore
            // GOTCHA: We pass `i` as the `start_idx` (not `i + 1`) because
            // we are allowed to reuse the same element multiple times.
            backtrack(
                candidates,
                target,
                i,
                current_sum + candidate,
                path,
                results,
            );

            // Un-choose (backtrack)
            path.pop();
        }
    }

    backtrack(&candidates, target, 0, 0, &mut path, &mut results);
    results
}

/// Optimal Approach: Backtracking with Early Pruning
///
/// By sorting the candidates first, we can stop exploring a branch as soon as
/// `current_sum + candidate > target`. This is the canonical solution: the pruning
/// makes it strictly less wasteful than the brute-force variant while sharing the same
/// exponential worst-case complexity.
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn combination_sum_optimal(mut candidates: Vec<i32>, target: i32) -> Vec<Vec<i32>> {
    let mut results = Vec::new();

    // RUST INSIGHT: `sort_unstable()` is generally faster than `sort()` and
    // is perfectly fine here since we are dealing with primitive integers (i32)
    // where elements with the same value are indistinguishable anyway.
    candidates.sort_unstable();

    if candidates.is_empty() {
        return results;
    }

    // RUST INSIGHT: We pre-allocate `path` with exact maximum depth `target / candidates[0]`
    // to eliminate heap reallocations. Since we sorted `candidates`, index 0 holds the minimum value.
    // GOTCHA: We must protect against division by zero or negative targets to prevent panic/underflow.
    let max_depth = (target.max(0) / candidates[0].max(1)) as usize;
    let mut path = Vec::with_capacity(max_depth);

    fn backtrack(
        candidates: &[i32],
        target: i32,
        start_idx: usize,
        current_sum: i32,
        path: &mut Vec<i32>,
        results: &mut Vec<Vec<i32>>,
    ) {
        if current_sum == target {
            results.push(path.clone());
            return;
        }

        for i in start_idx..candidates.len() {
            let candidate = candidates[i];

            // Early pruning: Because candidates are sorted, if this candidate
            // pushes us over the target, all subsequent candidates will too.
            if current_sum + candidate > target {
                break;
            }

            // Choose
            path.push(candidate);

            // Explore
            backtrack(
                candidates,
                target,
                i,
                current_sum + candidate,
                path,
                results,
            );

            // Un-choose (backtrack)
            path.pop();
        }
    }

    backtrack(&candidates, target, 0, 0, &mut path, &mut results);
    results
}

/// Main entry point - uses the optimal solution
#[must_use]
pub fn combination_sum(candidates: Vec<i32>, target: i32) -> Vec<Vec<i32>> {
    combination_sum_optimal(candidates, target)
}

// Alternative Approaches:
//
// 1. Dynamic Programming (Bottom-Up):
//    You can build a DP table where `dp[i]` contains all valid combinations that sum to `i`.
//    This avoids recursion but often allocates significantly more intermediate vectors
//    that are eventually discarded, making the backtracking approach generally preferred
//    in languages like Rust where we have fine-grained control over allocations.

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    // Helper to compare nested vectors ignoring order
    fn assert_combinations_eq(actual: Vec<Vec<i32>>, expected: Vec<Vec<i32>>) {
        let actual_set: HashSet<Vec<i32>> = actual
            .into_iter()
            .map(|mut v| {
                v.sort_unstable();
                v
            })
            .collect();
        let expected_set: HashSet<Vec<i32>> = expected
            .into_iter()
            .map(|mut v| {
                v.sort_unstable();
                v
            })
            .collect();
        assert_eq!(actual_set, expected_set);
    }

    #[test]
    fn test_brute_force_happy_path() {
        let candidates = vec![2, 3, 6, 7];
        let target = 7;
        let expected = vec![vec![2, 2, 3], vec![7]];
        assert_combinations_eq(combination_sum_brute_force(candidates, target), expected);
    }

    #[test]
    fn test_optimal_happy_path() {
        let candidates = vec![2, 3, 6, 7];
        let target = 7;
        let expected = vec![vec![2, 2, 3], vec![7]];
        assert_combinations_eq(combination_sum_optimal(candidates, target), expected);
    }

    #[test]
    fn test_all_approaches_agree() {
        // Cross-implementation agreement: both approaches must return the same
        // combinations (order-independent) for a non-trivial input.
        let candidates = vec![2, 3, 5, 7];
        let target = 12;
        let bf = combination_sum_brute_force(candidates.clone(), target);
        let opt = combination_sum_optimal(candidates, target);
        assert_combinations_eq(bf, opt);
    }

    #[test]
    fn test_no_solution() {
        let candidates = vec![2];
        let target = 1;
        let expected: Vec<Vec<i32>> = vec![];
        assert_combinations_eq(combination_sum(candidates, target), expected);
    }

    #[test]
    fn test_boundary_all_same_target() {
        let candidates = vec![2, 3, 5];
        let target = 8;
        let expected = vec![vec![2, 2, 2, 2], vec![2, 3, 3], vec![3, 5]];
        assert_combinations_eq(combination_sum(candidates, target), expected);
    }

    #[test]
    fn test_stress_larger_target() {
        let candidates = vec![2, 3, 5, 7];
        let target = 10;
        let expected = vec![
            vec![2, 2, 2, 2, 2],
            vec![2, 2, 3, 3],
            vec![2, 3, 5],
            vec![3, 7],
            vec![5, 5],
        ];
        assert_combinations_eq(combination_sum(candidates, target), expected);
    }
}
