//! # 152. Maximum Product Subarray
//!
//! **Difficulty:** Medium
//! **Link:** <https://leetcode.com/problems/maximum-product-subarray/>
//!
//! Given an integer array `nums`, find a subarray that has the largest product, and return the product.
//! The test cases are generated so that the answer will fit in a 32-bit integer.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::dynamic_programming::maximum_product_subarray::maximum_product_subarray;
//!
//! assert_eq!(maximum_product_subarray(vec![2, 3, -2, 4]), 6);
//! assert_eq!(maximum_product_subarray(vec![-2, 0, -1]), 0);
//! ```
//!
//! ## Constraints
//!
//! - `1 <= nums.length <= 2 * 10^4`
//! - `-10 <= nums[i] <= 10`
//! - The product of any prefix or suffix of `nums` is guaranteed to fit in a 32-bit integer.
//!
//! ---
//!
//! **Why this matters in Rust:**
//! This problem perfectly illustrates the benefits of Rust's explicit error handling and iterator patterns for DP state accumulation.
//! Handling variables safely (like tracking both the min and max due to negative number multiplication) forces an understanding of variable mutation and safe integer operations.
//!
//! ## Approach
//!
//! The fundamental challenge with products is that a negative number can turn a large positive number into a small negative one,
//! but a subsequent negative number can multiply with that small negative to create an even larger positive product.
//! Thus, unlike `Maximum Subarray` (where you only track the maximum), here we must track **both** the maximum product
//! ending at the current element AND the minimum product ending at the current element.
//!
//! In idiomatic Rust, we can elegantly use `Iterator::fold` to maintain this tuple of state,
//! highlighting how functional patterns can abstract away explicit loops while offering optimal O(1) space complexity.

use std::cmp;

/// Brute force approach: Check all possible subarrays and calculate their products.
///
/// We iterate over all possible starting indices `i` and ending indices `j`, accumulating the product.
///
/// Time: O(N²) - Nested loops over the array.
/// Space: O(1) - Only storing a few integer variables.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature uses owned Vec
pub fn maximum_product_subarray_brute_force(nums: Vec<i32>) -> i32 {
    if nums.is_empty() {
        return 0;
    }

    let mut max_product = i32::MIN;

    for i in 0..nums.len() {
        let mut current_product = 1;
        // RUST INSIGHT: Inclusive/exclusive ranges in Rust make iterating bounds very clear.
        for &num in nums.iter().skip(i) {
            current_product *= num;
            max_product = cmp::max(max_product, current_product);
        }
    }

    max_product
}

/// Optimized approach: Dynamic Programming with arrays (1D DP).
///
/// We maintain two arrays: `max_dp` and `min_dp` where `max_dp[i]` represents
/// the maximum product ending at index `i`, and `min_dp[i]` represents the minimum.
///
/// Time: O(N) - Single pass through the array.
/// Space: O(N) - Storing two DP arrays of length N.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
pub fn maximum_product_subarray_optimized(nums: Vec<i32>) -> i32 {
    if nums.is_empty() {
        return 0;
    }

    let n = nums.len();
    let mut max_dp = vec![0; n];
    let mut min_dp = vec![0; n];

    max_dp[0] = nums[0];
    min_dp[0] = nums[0];
    let mut max_product = nums[0];

    for i in 1..n {
        let val = nums[i];
        let p1 = max_dp[i - 1] * val;
        let p2 = min_dp[i - 1] * val;

        // RUST INSIGHT: `cmp::max` and `cmp::min` provide clear semantic intent compared to `if/else`.
        // We find the max/min of the current value alone (starting a new subarray),
        // or the current value multiplied by previous min or max.
        max_dp[i] = cmp::max(val, cmp::max(p1, p2));
        min_dp[i] = cmp::min(val, cmp::min(p1, p2));

        max_product = cmp::max(max_product, max_dp[i]);
    }

    max_product
}

/// Optimal approach: Dynamic Programming with O(1) space, purely functional.
///
/// We only need the previous step's max and min to compute the current step's max and min.
/// Thus, we can drop the arrays and just use scalar variables. We can elegantly write this
/// using `Iterator::fold`.
///
/// Time: O(N) - Single pass through the array.
/// Space: O(1) - Only state tuple is stored.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
pub fn maximum_product_subarray_optimal(nums: Vec<i32>) -> i32 {
    if nums.is_empty() {
        return 0;
    }

    // GOTCHA: It's important to start with the first element's value, not 1 or 0,
    // to handle arrays of size 1 gracefully and accurately set the baseline.
    let first = nums[0];

    // We use fold over the rest of the elements.
    // The state is a tuple: (current_max, current_min, global_max)
    let (_, _, global_max) = nums.into_iter().skip(1).fold(
        (first, first, first),
        |(curr_max, curr_min, global_max), val| {
            // RUST INSIGHT: Tuple destructuring allows for clean parallel assignment.
            // When multiplying by a negative number, the old min becomes the new max, and vice versa.
            let p1 = curr_max * val;
            let p2 = curr_min * val;

            let new_max = cmp::max(val, cmp::max(p1, p2));
            let new_min = cmp::min(val, cmp::min(p1, p2));
            let new_global = cmp::max(global_max, new_max);

            (new_max, new_min, new_global)
        },
    );

    global_max
}

/// Main entry point - uses optimal solution
#[must_use]
pub fn maximum_product_subarray(nums: Vec<i32>) -> i32 {
    maximum_product_subarray_optimal(nums)
}

// =========================================================================================
// Alternative Approaches
// =========================================================================================
// Prefix & Suffix Products: Another O(N) time and O(1) space approach involves two passes.
// If you traverse left-to-right computing prefix product and right-to-left computing suffix product,
// the maximum product is the maximum across both arrays. Whenever you hit a 0, you reset the running
// product to 1. This naturally handles the "odd number of negatives" problem without needing to track
// mins and maxes explicitly, but is slightly less intuitive to formulate.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force_example_1() {
        assert_eq!(maximum_product_subarray_brute_force(vec![2, 3, -2, 4]), 6);
    }

    #[test]
    fn test_optimized_example_1() {
        assert_eq!(maximum_product_subarray_optimized(vec![2, 3, -2, 4]), 6);
    }

    #[test]
    fn test_optimal_example_1() {
        assert_eq!(maximum_product_subarray_optimal(vec![2, 3, -2, 4]), 6);
    }

    #[test]
    fn test_all_approaches_edge_case_zero() {
        let input = vec![-2, 0, -1];
        assert_eq!(maximum_product_subarray_brute_force(input.clone()), 0);
        assert_eq!(maximum_product_subarray_optimized(input.clone()), 0);
        assert_eq!(maximum_product_subarray_optimal(input.clone()), 0);
    }

    #[test]
    fn test_all_approaches_single_element() {
        let input = vec![-3];
        assert_eq!(maximum_product_subarray_brute_force(input.clone()), -3);
        assert_eq!(maximum_product_subarray_optimized(input.clone()), -3);
        assert_eq!(maximum_product_subarray_optimal(input.clone()), -3);
    }

    #[test]
    fn test_all_approaches_stress_alternating_signs() {
        let input = vec![2, -5, 3, 1, -4, 0, -10, 2, 8, -5, -6];
        // Ensure all approaches yield the same result for an array with multiple zeros and alternating signs.
        let expected = maximum_product_subarray_brute_force(input.clone());
        assert_eq!(maximum_product_subarray_optimized(input.clone()), expected);
        assert_eq!(maximum_product_subarray_optimal(input.clone()), expected);
    }
}
