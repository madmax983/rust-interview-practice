//! # 15. Three Sum
//!
//! Given an integer array `nums`, return all the triplets `[nums[i], nums[j], nums[k]]`
//! such that `i != j`, `i != k`, and `j != k`, and `nums[i] + nums[j] + nums[k] == 0`.
//!
//! Notice that the solution set must not contain duplicate triplets.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::arrays::three_sum::three_sum;
//!
//! let result = three_sum(vec![-1, 0, 1, 2, -1, -4]);
//! // Result contains [[-1, -1, 2], [-1, 0, 1]] in some order
//! assert_eq!(result.len(), 2);
//! ```
//!
//! ## Constraints
//!
//! - 3 <= nums.length <= 3000
//! - -10^5 <= nums[i] <= 10^5

/// Brute force approach: Check all triplets
/// Time: O(n³) - three nested loops
/// Space: O(1) - excluding output array
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
pub fn three_sum_brute_force(nums: Vec<i32>) -> Vec<Vec<i32>> {
    let n = nums.len();
    let mut result = Vec::new();
    let mut seen = std::collections::HashSet::new(); // Track unique triplets

    // Strategy: Try every possible triplet (i, j, k) where i < j < k
    for i in 0..n {
        for j in (i + 1)..n {
            for k in (j + 1)..n {
                // Check if this triplet sums to zero
                if nums[i] + nums[j] + nums[k] == 0 {
                    // Sort the triplet to avoid duplicates
                    let mut triplet = vec![nums[i], nums[j], nums[k]];
                    triplet.sort_unstable();

                    // Only add if we haven't seen this triplet before
                    if seen.insert(triplet.clone()) {
                        result.push(triplet);
                    }
                }
            }
        }
    }

    result
}

/// Optimized approach: Sort + binary search
/// Time: O(n² log n) - O(n log n) sort + O(n²) pairs × O(log n) search
/// Space: O(1) - excluding output array
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
pub fn three_sum_optimized(nums: Vec<i32>) -> Vec<Vec<i32>> {
    if nums.len() < 3 {
        return vec![];
    }

    let mut nums = nums;
    nums.sort_unstable(); // Sort array first
    let mut result = Vec::new();

    // Strategy: Fix first element, then binary search for the other two
    for i in 0..nums.len() - 2 {
        // Skip duplicates for first element
        if i > 0 && nums[i] == nums[i - 1] {
            continue;
        }

        // For each second element
        for j in (i + 1)..nums.len() - 1 {
            // Skip duplicates for second element
            if j > i + 1 && nums[j] == nums[j - 1] {
                continue;
            }

            let target = -(nums[i] + nums[j]); // What we need for third element

            // Binary search for target in remaining elements
            if let Ok(k) = nums[(j + 1)..].binary_search(&target) {
                let k = j + 1 + k; // Adjust index to full array
                result.push(vec![nums[i], nums[j], nums[k]]);
            }
        }
    }

    result
}

/// Optimal approach: Sort + two pointers
/// Time: O(n²) - O(n log n) sort + O(n²) two pointer scan
/// Space: O(1) - excluding output array
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
pub fn three_sum_optimal(nums: Vec<i32>) -> Vec<Vec<i32>> {
    if nums.len() < 3 {
        return vec![];
    }

    let mut nums = nums;
    nums.sort_unstable(); // Sort to enable two-pointer technique
    let mut result = Vec::new();

    // Strategy: Fix first element, use two pointers for the other two
    for i in 0..nums.len() - 2 {
        // Optimization: if first element is positive, we can't make zero
        if nums[i] > 0 {
            break;
        }

        // Skip duplicates for first element
        if i > 0 && nums[i] == nums[i - 1] {
            continue;
        }

        // Two pointers for remaining elements
        let mut left = i + 1; // Start after current element
        let mut right = nums.len() - 1; // Start from end

        while left < right {
            let sum = nums[i] + nums[left] + nums[right];

            if sum == 0 {
                // Found a triplet!
                result.push(vec![nums[i], nums[left], nums[right]]);

                // Skip duplicates for left pointer
                while left < right && nums[left] == nums[left + 1] {
                    left += 1;
                }

                // Skip duplicates for right pointer
                while left < right && nums[right] == nums[right - 1] {
                    right -= 1;
                }

                // Move both pointers
                left += 1;
                right -= 1;
            } else if sum < 0 {
                // Sum too small, move left pointer right (increase sum)
                left += 1;
            } else {
                // Sum too large, move right pointer left (decrease sum)
                right -= 1;
            }
        }
    }

    result
}

/// Main entry point - uses optimal solution
#[must_use]
pub fn three_sum(nums: Vec<i32>) -> Vec<Vec<i32>> {
    three_sum_optimal(nums)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn normalize_result(mut result: Vec<Vec<i32>>) -> Vec<Vec<i32>> {
        for triplet in &mut result {
            triplet.sort_unstable();
        }
        result.sort();
        result
    }

    // Brute force tests
    #[test]
    fn test_brute_force_example_1() {
        let result = normalize_result(three_sum_brute_force(vec![-1, 0, 1, 2, -1, -4]));
        let expected = normalize_result(vec![vec![-1, -1, 2], vec![-1, 0, 1]]);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_brute_force_empty() {
        let result = three_sum_brute_force(vec![0, 1, 1]);
        assert_eq!(result, vec![] as Vec<Vec<i32>>);
    }

    #[test]
    fn test_brute_force_all_zeros() {
        let result = three_sum_brute_force(vec![0, 0, 0]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], vec![0, 0, 0]);
    }

    // Optimized tests
    #[test]
    fn test_optimized_example_1() {
        let result = normalize_result(three_sum_optimized(vec![-1, 0, 1, 2, -1, -4]));
        let expected = normalize_result(vec![vec![-1, -1, 2], vec![-1, 0, 1]]);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_optimized_empty() {
        let result = three_sum_optimized(vec![0, 1, 1]);
        assert_eq!(result, vec![] as Vec<Vec<i32>>);
    }

    #[test]
    fn test_optimized_all_zeros() {
        let result = three_sum_optimized(vec![0, 0, 0]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], vec![0, 0, 0]);
    }

    // Optimal tests
    #[test]
    fn test_optimal_example_1() {
        let result = normalize_result(three_sum_optimal(vec![-1, 0, 1, 2, -1, -4]));
        let expected = normalize_result(vec![vec![-1, -1, 2], vec![-1, 0, 1]]);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_optimal_empty() {
        let result = three_sum_optimal(vec![0, 1, 1]);
        assert_eq!(result, vec![] as Vec<Vec<i32>>);
    }

    #[test]
    fn test_optimal_all_zeros() {
        let result = three_sum_optimal(vec![0, 0, 0]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], vec![0, 0, 0]);
    }

    // Cross-implementation tests
    #[test]
    fn test_all_approaches_negative() {
        let nums = vec![-2, 0, 1, 1, 2];
        let r1 = normalize_result(three_sum_brute_force(nums.clone()));
        let r2 = normalize_result(three_sum_optimized(nums.clone()));
        let r3 = normalize_result(three_sum_optimal(nums));

        assert_eq!(r1, r2);
        assert_eq!(r2, r3);
        assert_eq!(r1.len(), 2); // [-2, 0, 2] and [-2, 1, 1]
    }

    #[test]
    fn test_all_approaches_duplicates() {
        let nums = vec![-1, -1, -1, 0, 1, 1, 1, 2];
        let r1 = normalize_result(three_sum_brute_force(nums.clone()));
        let r2 = normalize_result(three_sum_optimized(nums.clone()));
        let r3 = normalize_result(three_sum_optimal(nums));

        assert_eq!(r1, r2);
        assert_eq!(r2, r3);
    }

    // Main function test
    #[test]
    fn test_main_example_1() {
        let result = normalize_result(three_sum(vec![-1, 0, 1, 2, -1, -4]));
        let expected = normalize_result(vec![vec![-1, -1, 2], vec![-1, 0, 1]]);
        assert_eq!(result, expected);
    }
}
