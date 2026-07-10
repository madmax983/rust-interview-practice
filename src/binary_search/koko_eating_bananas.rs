//! # 875. Koko Eating Bananas
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/koko-eating-bananas>/
//!
//! This problem demonstrates "Binary Search on Answer" - a powerful pattern where we know the
//! bounds of the possible answer, and we use binary search to efficiently find the optimal one.
//! It teaches how to avoid integer overflow and handle custom comparison logic within the search space.
//!
//! Note: single canonical implementation; the brute/optimized/optimal progression does not apply here.

/// Approach: Binary Search on Answer
///
/// Time Complexity: O(N * log M) where N is the number of piles and M is the max pile size.
/// Space Complexity: O(1) auxiliary space.
///
/// Why this is idiomatic Rust:
/// The standard binary search `partition_point` doesn't work as well on a virtual range
/// since we'd need to create a `Range` or iterator over millions of numbers. A custom
/// manual binary search loop allows us to express the `can_eat_all` predicate efficiently
/// without allocating large ranges.
pub struct Solution;

impl Solution {
    /// # Panics
    ///
    /// Panics if `piles` is empty (the `LeetCode` constraints guarantee `piles.len() >= 1`).
    #[must_use]
    #[allow(clippy::needless_pass_by_value)]
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_sign_loss)]
    pub fn min_eating_speed(piles: Vec<i32>, h: i32) -> i32 {
        // Find the maximum pile size to establish our upper bound
        // RUST INSIGHT: iter().max() returns Option<&T>. Since constraints say piles.length >= 1,
        // unwrap() is safe here.
        let max_pile = *piles.iter().max().unwrap();

        // Search space: Koko can eat between 1 and max_pile bananas per hour
        let mut left = 1;
        let mut right = max_pile;
        let mut best_k = right;

        while left <= right {
            let mid = left + (right - left) / 2;

            if Self::can_eat_all(&piles, h, mid) {
                // If she can eat them at speed `mid`, this is a valid answer.
                // Try to find a slower (smaller) speed.
                best_k = mid;
                right = mid - 1;
            } else {
                // Too slow, she needs to eat faster.
                left = mid + 1;
            }
        }

        best_k
    }

    /// Helper to check if Koko can eat all bananas at speed `k` within `h` hours.
    fn can_eat_all(piles: &[i32], h: i32, k: i32) -> bool {
        let mut total_hours: i64 = 0; // Use i64 to prevent overflow during sum
        let k_i64 = i64::from(k);

        // RUST INSIGHT: Iterator combinators make this elegant, but a manual loop is sometimes
        // easier to read for algorithmic problems.
        for &pile in piles {
            let pile_i64 = i64::from(pile);
            // Ceiling division: (a + b - 1) / b
            // GOTCHA: Don't use f64::ceil() as floating point arithmetic can lose precision
            // for very large integers. Integer ceiling division is safer and faster.
            total_hours += (pile_i64 + k_i64 - 1) / k_i64;
        }

        total_hours <= i64::from(h)
    }

    /// Alternative approach using Iterator methods for `can_eat_all`
    /// This is more functional but conceptually does the same thing.
    fn _can_eat_all_iterative(piles: &[i32], h: i32, k: i32) -> bool {
        let k_i64 = i64::from(k);
        let total_hours: i64 = piles
            .iter()
            .map(|&pile| (i64::from(pile) + k_i64 - 1) / k_i64)
            .sum();

        total_hours <= i64::from(h)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_happy_path() {
        assert_eq!(Solution::min_eating_speed(vec![3, 6, 7, 11], 8), 4);
        assert_eq!(Solution::min_eating_speed(vec![30, 11, 23, 4, 20], 5), 30);
        assert_eq!(Solution::min_eating_speed(vec![30, 11, 23, 4, 20], 6), 23);
    }

    #[test]
    fn test_edge_case_one_pile() {
        assert_eq!(Solution::min_eating_speed(vec![10], 9), 2);
    }

    #[test]
    fn test_stress_large_numbers() {
        // Requires using i64 internally to prevent overflow during accumulation
        assert_eq!(
            Solution::min_eating_speed(vec![1_000_000_000], 2),
            500_000_000
        );
        assert_eq!(
            Solution::min_eating_speed(vec![805_306_368, 805_306_368, 805_306_368], 1_000_000_000),
            3
        );
    }
}
