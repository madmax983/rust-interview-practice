//! # 121. Best Time to Buy and Sell Stock
//!
//! Difficulty: Easy
//! Link: <https://leetcode.com/problems/best-time-to-buy-and-sell-stock/>
//!
//! You are given an array `prices` where `prices[i]` is the price of a given stock on the `i`th day.
//! You want to maximize your profit by choosing a single day to buy one stock and choosing a
//! different day in the future to sell that stock.
//!
//! Return the maximum profit you can achieve from this transaction. If you cannot achieve any profit, return 0.
//!
//! This problem perfectly demonstrates how to transform an imperative state-tracking loop into a
//! purely functional `fold`. It shows how Rust's iterators can accumulate state cleanly, avoiding
//! mutable variables in the outer scope while compiling down to optimal machine code.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::arrays::best_time_to_buy_and_sell_stock::max_profit;
//!
//! assert_eq!(max_profit(vec![7, 1, 5, 3, 6, 4]), 5);
//! assert_eq!(max_profit(vec![7, 6, 4, 3, 1]), 0);
//! ```
//!
//! ## Constraints
//!
//! - `1 <= prices.length <= 10^5`
//! - `0 <= prices[i] <= 10^4`

use std::cmp;

/// Brute force approach: Check all pairs.
/// Time: O(n²) - nested loops check every (buy, sell) pair.
/// Space: O(1) - no extra space needed.
///
/// This approach tests every possible transaction. It works, but it's too slow
/// for large inputs.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
pub fn max_profit_brute_force(prices: Vec<i32>) -> i32 {
    let mut max_profit = 0;
    let n = prices.len();

    // Strategy: Try buying on day `i` and selling on day `j` (where j > i)
    for i in 0..n {
        for j in (i + 1)..n {
            let profit = prices[j] - prices[i];
            if profit > max_profit {
                max_profit = profit;
            }
        }
    }

    max_profit
}

/// Optimized approach: Imperative single pass keeping track of minimum.
/// Time: O(n) - one pass through the array.
/// Space: O(1) - uses only two integers.
///
/// By keeping track of the minimum price seen so far, we can calculate the
/// potential profit at any given day. If the current price is less than our
/// tracked minimum, we update the minimum. Otherwise, we see if the profit
/// (current price - min price) beats our `max_profit`.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
pub fn max_profit_optimized(prices: Vec<i32>) -> i32 {
    if prices.is_empty() {
        return 0;
    }

    let mut min_price = prices[0];
    let mut max_profit = 0;

    for &price in &prices {
        if price < min_price {
            // Update the lowest price we've seen so far
            min_price = price;
        } else {
            // Calculate potential profit and update max_profit if it's better
            let current_profit = price - min_price;
            if current_profit > max_profit {
                max_profit = current_profit;
            }
        }
    }

    max_profit
}

/// Custom struct to model the state in our functional approach.
///
/// Using a struct gives our `fold` accumulator semantic meaning instead of just
/// being a generic `(i32, i32)` tuple. This is an idiomatic way to handle complex
/// functional state in Rust.
struct ProfitState {
    min_price: i32,
    max_profit: i32,
}

/// Optimal approach: Functional single pass using iterators and `fold`.
/// Time: O(n) - one pass through the array.
/// Space: O(1) - uses only two integers inside the state struct.
///
/// This perfectly highlights Rust's capabilities:
/// In Java or Python, you would typically write the `max_profit_optimized` version
/// with mutable variables `min_price` and `max_profit` outside the loop. While fine,
/// in Rust we can model this functionally using iterators and `fold` to eliminate
/// mutable variables altogether, shifting state transitions into pure functions.
/// 1. We avoid uninitialized variables or mutable state entirely.
/// 2. `fold` processes the iterator, consuming elements and passing along an accumulator.
/// 3. The compiler completely optimizes away the struct allocation.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // LeetCode signature
pub fn max_profit_optimal(prices: Vec<i32>) -> i32 {
    // RUST INSIGHT: `fold` takes an initial state and a closure. The closure takes the
    // previous state and the next item, returning the new state. It perfectly models
    // state machines without relying on mutable external variables.

    // GOTCHA: It is tempting to use `fold` with a tuple `(min_price, max_profit)`
    // but using a named struct `ProfitState` makes the closure logic self-documenting
    // and prevents mixing up the tuple indices `state.0` and `state.1`.
    let final_state = prices.into_iter().fold(
        ProfitState {
            min_price: i32::MAX, // Start extremely high so the first price becomes the minimum
            max_profit: 0,
        },
        |state, price| ProfitState {
            // Update min_price if the current price is lower
            min_price: cmp::min(state.min_price, price),
            // Update max_profit if selling today is better than the previous best
            max_profit: cmp::max(state.max_profit, price.saturating_sub(state.min_price)),
        },
    );

    final_state.max_profit
}

/// Main entry point - uses optimal solution
#[must_use]
pub fn max_profit(prices: Vec<i32>) -> i32 {
    max_profit_optimal(prices)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Happy path test: there is a clear buying and selling opportunity
    #[test]
    fn test_happy_path() {
        let prices = vec![7, 1, 5, 3, 6, 4];
        let expected = 5; // Buy at 1 (day 2), sell at 6 (day 5)
        assert_eq!(max_profit_brute_force(prices.clone()), expected);
        assert_eq!(max_profit_optimized(prices.clone()), expected);
        assert_eq!(max_profit_optimal(prices), expected);
    }

    // Edge case: Prices only decrease, so no profit is possible
    #[test]
    fn test_decreasing_prices() {
        let prices = vec![7, 6, 4, 3, 1];
        let expected = 0; // No valid transaction
        assert_eq!(max_profit_brute_force(prices.clone()), expected);
        assert_eq!(max_profit_optimized(prices.clone()), expected);
        assert_eq!(max_profit_optimal(prices), expected);
    }

    // Edge case: Empty array and single element array
    #[test]
    fn test_short_arrays() {
        assert_eq!(max_profit_brute_force(vec![]), 0);
        assert_eq!(max_profit_optimized(vec![]), 0);
        assert_eq!(max_profit_optimal(vec![]), 0);

        assert_eq!(max_profit_brute_force(vec![5]), 0);
        assert_eq!(max_profit_optimized(vec![5]), 0);
        assert_eq!(max_profit_optimal(vec![5]), 0);
    }

    // Stress case: Prices stay the same
    #[test]
    fn test_flat_prices() {
        let prices = vec![3, 3, 3, 3, 3];
        let expected = 0;
        assert_eq!(max_profit_brute_force(prices.clone()), expected);
        assert_eq!(max_profit_optimized(prices.clone()), expected);
        assert_eq!(max_profit_optimal(prices), expected);
    }

    // Main function test
    #[test]
    fn test_main_function() {
        assert_eq!(max_profit(vec![7, 1, 5, 3, 6, 4]), 5);
    }
}

// Alternative approaches footer:
// - A 1D Dynamic Programming approach where `dp[i]` represents max profit selling on day `i`.
//   This is an overkill since we only need the minimum price up to day `i` instead of storing an array.
// - Kadane's algorithm variant tracking adjacent differences. It is exactly identical to the `maximum_subarray`
//   problem if we build an array of differences `prices[i] - prices[i-1]`.
