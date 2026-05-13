//! # 875. Koko Eating Bananas
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/koko-eating-bananas/>
//!
//! Koko loves to eat bananas. There are `n` piles of bananas, the `i`-th pile has `piles[i]` bananas.
//! The guards have gone and will come back in `h` hours.
//!
//! Koko can decide her bananas-per-hour eating speed of `k`. Each hour, she chooses some pile of
//! bananas and eats `k` bananas from that pile. If the pile has less than `k` bananas, she eats all
//! of them instead and will not eat any more bananas during this hour.
//!
//! Koko likes to eat slowly but still wants to finish eating all the bananas before the guards return.
//!
//! Return the minimum integer `k` such that she can eat all the bananas within `h` hours.
//!
//! This is a classic **"Binary Search on Answer"** pattern. It demonstrates how to apply binary
//! search not just to find elements in an array, but to search a virtual monotonic search space.
//! In Rust, it's a great opportunity to explore iterator adapters and overflow-safe math.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::binary_search::koko_eating_bananas::min_eating_speed;
//!
//! let piles = vec![3, 6, 7, 11];
//! let h = 8;
//! assert_eq!(min_eating_speed(piles, h), 4);
//! ```
//!
//! ## Constraints
//!
//! - `1 <= piles.length <= 10^4`
//! - `piles.length <= h <= 10^9`
//! - `1 <= piles[i] <= 10^9`

/// Calculates the total hours required to eat all bananas at speed `k`.
///
/// # RUST INSIGHT: Iterator Adapters
/// Instead of a manual `for` loop, we can use `iter()` with `map()` and `sum()`.
/// This is idiomatic Rust, zero-cost, and often easier to parallelize later if needed.
fn hours_to_eat(piles: &[i32], k: i32) -> u64 {
    // GOTCHA: `piles[i]` can be up to 10^9, and `piles.length` up to 10^4.
    // The sum of hours could easily exceed the maximum value of `i32` (approx 2*10^9).
    // Therefore, we accumulate the total hours as `u64` to prevent overflow panics.
    piles
        .iter()
        .map(|&pile| {
            // Equivalent to ceil(pile / k). In integer math: (pile + k - 1) / k.
            // Safe to cast to u64 because piles[i] is positive.
            ((pile as u64 + k as u64 - 1) / k as u64)
        })
        .sum()
}

// ============================================================================
// Brute Force Approach
// ============================================================================

/// Brute force approach: Try every possible speed starting from 1.
///
/// We know the minimum speed is 1, and the maximum speed is the size of the largest pile
/// (since eating faster than the largest pile still takes 1 hour for that pile).
/// We simply iterate `k` from 1 upwards until we find a speed that works.
///
/// Time: `O(M * N)` where `M` is the maximum pile size and `N` is the number of piles.
/// Space: `O(1)`
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
pub fn min_eating_speed_brute_force(piles: Vec<i32>, h: i32) -> i32 {
    let mut k = 1;
    // RUST INSIGHT: `loop` is an explicit infinite loop in Rust.
    // It signals intent clearer than `while true`, and the compiler analyzes it differently.
    loop {
        if hours_to_eat(&piles, k) <= h as u64 {
            return k;
        }
        k += 1;
    }
}

// ============================================================================
// Optimal Approach (Binary Search)
// ============================================================================

/// Optimal approach: Binary Search on the Answer.
///
/// The search space for speed `k` is monotonic: if she can eat all bananas at speed `k`,
/// she can definitely eat them at speed `k + 1`. If she cannot at speed `k`, she cannot
/// at speed `k - 1`. This allows us to use binary search on the range `[1, max(piles)]`.
///
/// Time: `O(N log M)` where `N` is the number of piles and `M` is the maximum pile size.
/// Space: `O(1)`
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
pub fn min_eating_speed_optimal(piles: Vec<i32>, h: i32) -> i32 {
    let mut left = 1;
    // RUST INSIGHT: `iter().copied().max()` finds the maximum efficiently without allocating.
    // `unwrap_or(1)` ensures we don't panic on an empty slice, even though constraints say N >= 1.
    let mut right = piles.iter().copied().max().unwrap_or(1);
    let mut result = right;

    while left <= right {
        let mid = left + (right - left) / 2; // Prevents overflow, although not strictly needed for i32 here

        if hours_to_eat(&piles, mid) <= h as u64 {
            // Speed `mid` works. Record it, but try to find a smaller working speed.
            result = mid;
            right = mid - 1;
        } else {
            // Speed `mid` is too slow. We must eat faster.
            left = mid + 1;
        }
    }

    result
}

/// Main entry point - uses the optimal solution.
#[must_use]
pub fn min_eating_speed(piles: Vec<i32>, h: i32) -> i32 {
    min_eating_speed_optimal(piles, h)
}

// ============================================================================
// Alternative Approaches
// ============================================================================
// 1. **Mathematical Approach**: For this specific problem, binary search is the optimal
//    and expected approach. There is no O(1) mathematical formula because the ceiling
//    function applied to each pile creates non-linear constraints.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force() {
        assert_eq!(min_eating_speed_brute_force(vec![3, 6, 7, 11], 8), 4);
        assert_eq!(min_eating_speed_brute_force(vec![30, 11, 23, 4, 20], 5), 30);
        assert_eq!(min_eating_speed_brute_force(vec![30, 11, 23, 4, 20], 6), 23);
    }

    #[test]
    fn test_optimal() {
        assert_eq!(min_eating_speed_optimal(vec![3, 6, 7, 11], 8), 4);
        assert_eq!(min_eating_speed_optimal(vec![30, 11, 23, 4, 20], 5), 30);
        assert_eq!(min_eating_speed_optimal(vec![30, 11, 23, 4, 20], 6), 23);
    }

    #[test]
    fn test_edge_cases() {
        // Only one pile
        assert_eq!(min_eating_speed(vec![10], 2), 5);
        assert_eq!(min_eating_speed(vec![10], 10), 1);

        // Exact time equal to number of piles (must eat largest pile in 1 hour)
        assert_eq!(min_eating_speed(vec![1, 2, 3, 4, 5], 5), 5);

        // Abundant time (can eat at minimum speed 1)
        assert_eq!(min_eating_speed(vec![1, 2, 3], 10), 1);
    }

    #[test]
    fn test_stress_boundary() {
        // GOTCHA: This test ensures we handle potential u32/i32 overflow in `hours_to_eat`.
        // Summing these hours at speed 1 would yield 2,000,000,000, which barely fits in i32
        // but can easily overflow if constraints were slightly higher. `u64` protects us.
        let piles = vec![1_000_000_000, 1_000_000_000];
        let h = 2_000_000_000;
        assert_eq!(min_eating_speed(piles, h), 1);
    }

    #[test]
    fn test_cross_implementation() {
        let inputs = vec![
            (vec![3, 6, 7, 11], 8),
            (vec![30, 11, 23, 4, 20], 5),
            (vec![30, 11, 23, 4, 20], 6),
            (vec![1, 1, 1], 3),
        ];

        for (piles, h) in inputs {
            let brute = min_eating_speed_brute_force(piles.clone(), h);
            let optimal = min_eating_speed_optimal(piles.clone(), h);
            assert_eq!(brute, optimal, "Mismatch for piles={:?} h={}", piles, h);
        }
    }
}
