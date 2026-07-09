//! # 55. Jump Game
//!
//! Difficulty: Medium
//!
//! Link: <https://leetcode.com/problems/jump-game/>
//!
//! You are given an integer array `nums`. You are initially positioned at the array's first index,
//! and each element in the array represents your maximum jump length at that position.
//! Return `true` if you can reach the last index, or `false` otherwise.
//!
//! This problem is a textbook example of Greedy algorithms. In Rust, it provides an excellent
//! opportunity to use iterator combinators like `.enumerate()` and `.try_fold()` for short-circuiting,
//! or a simple `for` loop for clarity, all while maintaining O(1) space.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::arrays::jump_game::can_jump;
//!
//! let nums = vec![2, 3, 1, 1, 4];
//! assert_eq!(can_jump(nums), true);
//! ```
//!
//! ## Constraints
//!
//! - `1 <= nums.length <= 10^4`
//! - `0 <= nums[i] <= 10^5`

use std::cmp;

// =========================================================================================
// Approach 1: Greedy with For Loop (Idiomatic)
// =========================================================================================

/// Greedy Approach: Track Maximum Reachable Index
///
/// We iterate through the array, maintaining the furthest index we can reach (`max_reachable`).
/// If we are currently at an index greater than `max_reachable`, it means we can't even get
/// to this current position, so we return `false`.
/// Otherwise, we update `max_reachable` to be the maximum of its current value and the
/// furthest we can jump from the current position (`i + jump_len`).
///
/// Time: O(N) - We visit each element once.
/// Space: O(1) - We only track a single integer.
///
/// **Rust Insight:**
/// Iterating via `.iter().enumerate()` cleanly provides both the index and the value without
/// risking manual out-of-bounds array indexing or maintaining separate counter variables.
pub fn can_jump(nums: Vec<i32>) -> bool {
    let mut max_reachable = 0;

    for (i, &jump_len) in nums.iter().enumerate() {
        // If we've reached a position beyond our maximum reachable distance, we're stuck.
        if i > max_reachable {
            return false;
        }

        // GOTCHA: Attempting to add an `i32` (`jump_len`) to a `usize` (`i`) without an explicit cast
        // will result in a compile error. Rust intentionally does not silently upcast/downcast integers.
        // RUST INSIGHT: Safe casting using `as usize` is required here because indices
        // in Rust are `usize` while the problem provides jump lengths as `i32`.
        max_reachable = cmp::max(max_reachable, i + jump_len as usize);

        // Short-circuit: if we can already reach the end, no need to process the rest.
        if max_reachable >= nums.len() - 1 {
            return true;
        }
    }

    true
}

// =========================================================================================
// Approach 2: Functional Greedy with Iterator::try_fold
// =========================================================================================

/// Functional Greedy Approach using `.try_fold()`
///
/// This approach achieves the same logic but entirely functionally, eliminating mutable variables.
/// `try_fold` allows us to short-circuit the fold by returning an `Err` when we discover we
/// are stuck (index > max_reachable).
///
/// Time: O(N) - We process elements linearly, stopping early if stuck.
/// Space: O(1) - Purely accumulator state.
///
/// **Rust Insight:**
/// We co-opt `Result` (`Ok`/`Err`) to act as a control-flow mechanism. `Ok(acc)` continues the
/// fold, while `Err(())` breaks it immediately.
pub fn can_jump_functional(nums: Vec<i32>) -> bool {
    let target = nums.len().saturating_sub(1);

    // We fold over the elements. The accumulator is `max_reachable`.
    let result = nums
        .iter()
        .enumerate()
        .try_fold(0, |max_reachable, (i, &jump_len)| {
            if i > max_reachable {
                // We can't reach this index, short-circuit
                std::ops::ControlFlow::Break(false)
            } else {
                let next_reach = cmp::max(max_reachable, i + jump_len as usize);
                if next_reach >= target {
                    // We can reach the target, short-circuit with true
                    std::ops::ControlFlow::Break(true)
                } else {
                    // Continue with the new max_reachable
                    std::ops::ControlFlow::Continue(next_reach)
                }
            }
        });

    match result {
        std::ops::ControlFlow::Break(res) => res,
        std::ops::ControlFlow::Continue(max_reach) => max_reach >= target,
    }
}

// =========================================================================================
// Alternative Approaches
// =========================================================================================
// 1. **Dynamic Programming (Memoization)**: Explore all jump paths, caching reachable indices. Time O(N^2), Space O(N).
//    Inefficient for this problem due to the nested loop required, but conceptually simple.
// 2. **Backwards Greedy**: Start from the end and try to shift the "target" backwards to index 0. Time O(N), Space O(1).
//    Valid and elegant, often functionally identical in performance to forward Greedy.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_can_jump_happy_path() {
        assert!(can_jump(vec![2, 3, 1, 1, 4]));
        assert!(can_jump_functional(vec![2, 3, 1, 1, 4]));
    }

    #[test]
    fn test_can_jump_edge_cases() {
        // Failing path
        assert!(!can_jump(vec![3, 2, 1, 0, 4]));
        assert!(!can_jump_functional(vec![3, 2, 1, 0, 4]));

        // Single element
        assert!(can_jump(vec![0]));
        assert!(can_jump_functional(vec![0]));
    }

    #[test]
    fn test_can_jump_stress() {
        // Vector of 10,000 ones ending in 0.
        let mut nums = vec![1; 10000];
        nums.push(0);
        assert!(can_jump(nums.clone()));
        assert!(can_jump_functional(nums));

        // Vector of 10,000 zeros (except first element). Should fail immediately.
        let mut fail_nums = vec![0; 10000];
        fail_nums[0] = 0;
        assert!(!can_jump(fail_nums.clone()));
        assert!(!can_jump_functional(fail_nums));
    }
}
