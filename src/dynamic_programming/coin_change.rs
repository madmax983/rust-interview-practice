//! # 322. Coin Change
//!
//! You are given an integer array `coins` representing coins of different denominations and an integer `amount` representing a total amount of money.
//! Return the fewest number of coins that you need to make up that amount. If that amount of money cannot be made up by any combination of the coins, return `-1`.
//!
//! You may assume that you have an infinite number of each kind of coin.
//!
//! - Difficulty: Medium
//! - `LeetCode`: <https://leetcode.com/problems/coin-change/>
//!
//! ## Why this matters in Rust
//! This problem is a classic introduction to Dynamic Programming (DP) and highlights several Rust-specific patterns:
//! 1.  **Vector Initialization**: The `vec![value; len]` macro is idiomatic for creating DP tables.
//! 2.  **Type Safety**: Iterating through amounts requires `usize` indices, while the problem uses `i32` for values. Rust forces explicit casting (`as usize`), which makes index usage deliberate and prevents subtle overflow bugs common in C/C++.
//! 3.  **Option vs Sentinels**: While idiomatic Rust usually prefers `Option<T>` for "no value", classic DP algorithms often use a sentinel value (like `amount + 1` or `i32::MAX`) to simplify `min` logic inside tight loops and avoid the overhead of unwrapping `Option`.
//!
//! ## Approach
//!
//! The core idea is to build the solution for `amount` from smaller subproblems:
//! `dp[i] = min(dp[i - coin]) + 1` for all `coin` in `coins`.
//!
//! We explore three implementations:
//! 1.  **Brute Force**: Naive recursion. Tries all combinations.
//! 2.  **Top-Down DP**: Recursion with Memoization (`HashMap`).
//! 3.  **Bottom-Up DP**: Iterative Tabulation using a `Vec`. This is the standard competitive programming solution.

use std::collections::HashMap;

/// Brute Force Approach: Recursive DFS
///
/// We try every coin for the current amount recursively.
///
/// - **Time Complexity**: O(n^S), where S is the amount and n is the number of coins. Extremely slow because it recomputes the same subproblems exponentially.
/// - **Space Complexity**: O(S) for the recursion stack in the worst case (e.g., all 1-value coins).
///
/// # RUST INSIGHT
/// In Rust, strict type checking prevents us from accidentally using `amount` (i32) as an index.
/// While beneficial, it requires frequent `as` casting when working with array-based DP.
#[allow(clippy::needless_pass_by_value)]
#[must_use]
pub fn coin_change_brute_force(coins: &[i32], amount: i32) -> i32 {
    if amount == 0 {
        return 0;
    }
    if amount < 0 {
        return -1;
    }

    let mut min_coins = i32::MAX;

    for &coin in coins {
        let res = coin_change_brute_force(coins, amount - coin);

        // If the subproblem returned -1, it's invalid.
        if res >= 0 && res < min_coins {
            min_coins = res + 1;
        }
    }

    if min_coins == i32::MAX { -1 } else { min_coins }
}

/// Optimized Approach: Top-Down DP (Memoization)
///
/// We store the result of each subproblem (amount) in a `HashMap` to avoid re-computation.
///
/// - **Time Complexity**: O(S * n). Each state (amount) is computed once, and for each state, we iterate through `n` coins.
/// - **Space Complexity**: O(S). The `HashMap` stores `S` entries, and recursion depth is at most `S`.
#[must_use]
pub fn coin_change_optimized(coins: &[i32], amount: i32) -> i32 {
    let mut memo = HashMap::new();
    coin_change_memo(coins, amount, &mut memo)
}

fn coin_change_memo(coins: &[i32], amount: i32, memo: &mut HashMap<i32, i32>) -> i32 {
    if amount == 0 {
        return 0;
    }
    if amount < 0 {
        return -1;
    }

    // Check memoization table
    if let Some(&res) = memo.get(&amount) {
        return res;
    }

    let mut min_coins = i32::MAX;

    for &coin in coins {
        let res = coin_change_memo(coins, amount - coin, memo);

        if res >= 0 && res < min_coins {
            min_coins = res + 1;
        }
    }

    let result = if min_coins == i32::MAX { -1 } else { min_coins };

    // RUST INSIGHT: `insert` takes ownership of the key/value. Since they are Copy types (i32),
    // they are implicitly copied, so we don't need to clone.
    memo.insert(amount, result);
    result
}

/// Optimal Approach: Bottom-Up DP (Tabulation)
///
/// We build a table `dp` where `dp[i]` is the minimum coins for amount `i`.
/// We initialize the array with a sentinel value (`amount + 1`) representing "infinity".
///
/// - **Time Complexity**: O(S * n).
/// - **Space Complexity**: O(S). We use a vector of size `amount + 1`.
///
/// # GOTCHA
/// A common mistake is using `i32::MAX` as the sentinel. If we do `dp[i-coin] + 1`,
/// `i32::MAX + 1` overflows and panics in debug mode (or wraps in release).
/// `amount + 1` is a safe sentinel because the maximum possible answer is `amount` (using coins of value 1).
/// Note: We assume `amount` is small enough that `amount + 1` does not overflow `i32`.
#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::cast_sign_loss)]
#[must_use]
pub fn coin_change_optimal(coins: &[i32], amount: i32) -> i32 {
    if amount < 0 {
        return -1;
    }
    let amount_usize = amount as usize;

    // RUST INSIGHT: `vec!` macro initializes memory efficiently.
    // We use `amount + 1` as a sentinel for "infinity" to avoid overflow issues with `i32::MAX + 1`.
    let max_val = amount + 1;
    let mut dp = vec![max_val; amount_usize + 1];

    // Base case: 0 coins needed to make amount 0
    dp[0] = 0;

    for i in 1..=amount_usize {
        for &coin in coins {
            // Check if coin can be used (avoid underflow)
            if coin as usize <= i {
                // dp[i] = min(dp[i], dp[i - coin] + 1)
                let sub_res = dp[i - (coin as usize)];
                if sub_res != max_val {
                    dp[i] = std::cmp::min(dp[i], sub_res + 1);
                }
            }
        }
    }

    if dp[amount_usize] > amount {
        -1
    } else {
        dp[amount_usize]
    }
}

/// Main entry point
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
pub fn coin_change(coins: Vec<i32>, amount: i32) -> i32 {
    coin_change_optimal(&coins, amount)
}

// Alternative Approaches:
// 1. **BFS (Breadth-First Search)**:
//    Treat amounts as nodes in a graph. BFS guarantees the shortest path (minimum coins) in an unweighted graph.
//    However, for large amounts, the queue can grow very large. DP is usually more memory efficient and cache-friendly.
// 2. **Result<i32, CannotMakeAmountError>**:
//    In a real Rust library, we would likely return a `Result` or `Option` instead of `-1`.
//    `-1` is a legacy pattern from C/Java competitive programming.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force_basic() {
        let coins = vec![1, 2, 5];
        let amount = 11;
        assert_eq!(coin_change_brute_force(&coins, amount), 3); // 5 + 5 + 1
    }

    #[test]
    fn test_optimized_basic() {
        let coins = vec![1, 2, 5];
        let amount = 11;
        assert_eq!(coin_change_optimized(&coins, amount), 3);
    }

    #[test]
    fn test_optimal_basic() {
        let coins = vec![1, 2, 5];
        let amount = 11;
        assert_eq!(coin_change_optimal(&coins, amount), 3);
    }

    #[test]
    fn test_impossible() {
        let coins = vec![2];
        let amount = 3;
        assert_eq!(coin_change_optimal(&coins, amount), -1);
    }

    #[test]
    fn test_zero_amount() {
        let coins = vec![1];
        let amount = 0;
        assert_eq!(coin_change_optimal(&coins, amount), 0);
    }

    #[test]
    fn test_large_amount() {
        // This would timeout with brute force
        let coins = vec![1, 2, 5];
        let amount = 100;
        assert_eq!(coin_change_optimal(&coins, amount), 20); // 5 * 20
    }

    #[test]
    fn test_cross_verify() {
        let coins = vec![1, 3, 4];
        let amount = 6;
        // Brute force
        assert_eq!(coin_change_brute_force(&coins, amount), 2); // 3 + 3
        // Optimized
        assert_eq!(coin_change_optimized(&coins, amount), 2);
        // Optimal
        assert_eq!(coin_change_optimal(&coins, amount), 2);
    }
}
