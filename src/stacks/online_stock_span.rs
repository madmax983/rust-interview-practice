//! # 901. Online Stock Span
//!
//! Design an algorithm that collects daily price quotes for some stock and returns the span of that stock's price for the current day.
//!
//! The span of the stock's price in one day is the maximum number of consecutive days (starting from that day and going backward) for which the stock price was less than or equal to the price of that day.
//!
//! [LeetCode Problem 901](https://leetcode.com/problems/online-stock-span/)
//!
//! ## Why This Matters in Rust
//!
//! This problem is a textbook use case for a stateful **Monotonic Stack**. It demonstrates how to wrap a standard data structure (`Vec`) in a custom struct to maintain internal invariants across method calls.
//! It reinforces ownership semantics, as the `StockSpanner` struct owns the stack and mutates it (`&mut self`) upon each `next` call without external lifetimes.
//!
//! ## Approach
//!
//! **Monotonic Decreasing Stack**
//!
//! We need to find the number of consecutive days in the past where the price was `<= ` the current price.
//! We can keep a stack of tuples: `(price, span)`.
//!
//! When a new price comes in:
//! 1. Initialize `span = 1` (it always spans itself).
//! 2. While the stack is not empty and the top of the stack has a price `<= ` the new price:
//!    - Pop the element from the stack.
//!    - Add the popped element's `span` to our current `span`.
//! 3. Push the new `(price, span)` onto the stack.
//! 4. Return the calculated `span`.
//!
//! Because each element is pushed onto the stack exactly once and popped at most once, the amortized time complexity per `next()` call is O(1).
//!
//! **Time Complexity**: Amortized O(1) per `next()` call. O(N) total across N calls.
//! **Space Complexity**: O(N) in the worst case (e.g., strictly decreasing prices where no elements are ever popped).
//!
//! ## Alternative Approaches
//!
//! - **Brute Force (Array)**: Store all prices in a `Vec`. On each `next` call, iterate backward to count the span. This is O(N) per call, leading to O(N^2) overall. Too slow.
//! - **Alternative Stack Layout**: Instead of storing `(price, span)`, we could store `(price, day_index)` and calculate span based on index differences (like in Daily Temperatures). This is equally valid and idiomatic.

/// A stateful struct that calculates the stock span for daily prices.
#[derive(Debug, Default)]
pub struct StockSpanner {
    // Stores (price, span)
    // RUST INSIGHT: A Vec is the idiomatic choice for a stack in Rust.
    // By storing a tuple `(i32, i32)`, we easily keep the value and its computed property together
    // without the need for a separate struct.
    stack: Vec<(i32, i32)>,
}

impl StockSpanner {
    /// Creates a new, empty `StockSpanner`.
    #[must_use]
    pub fn new() -> Self {
        Self { stack: Vec::new() }
    }

    /// Adds the daily price and returns its span.
    ///
    /// Takes `&mut self` because it modifies the internal stack state.
    pub fn next(&mut self, price: i32) -> i32 {
        let mut span = 1;

        // Maintain a monotonic decreasing stack.
        // GOTCHA: We must loop and pop as long as the incoming price is greater than or equal to
        // the price at the top of the stack.
        //
        // RUST INSIGHT: `self.stack.last()` gives an `Option<&(i32, i32)>`. We destructure the tuple
        // reference. We only pop if the condition is met.
        while let Some(&(top_price, top_span)) = self.stack.last() {
            if top_price <= price {
                span += top_span;
                self.stack.pop();
            } else {
                break;
            }
        }

        self.stack.push((price, span));
        span
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_happy_path() {
        let mut spanner = StockSpanner::new();
        // Prices: [100, 80, 60, 70, 60, 75, 85]
        assert_eq!(spanner.next(100), 1);
        assert_eq!(spanner.next(80), 1);
        assert_eq!(spanner.next(60), 1);
        assert_eq!(spanner.next(70), 2);
        assert_eq!(spanner.next(60), 1);
        assert_eq!(spanner.next(75), 4);
        assert_eq!(spanner.next(85), 6);
    }

    #[test]
    fn test_monotonic_increasing() {
        // If prices always increase, span should accumulate total days so far.
        let mut spanner = StockSpanner::new();
        assert_eq!(spanner.next(10), 1);
        assert_eq!(spanner.next(20), 2);
        assert_eq!(spanner.next(30), 3);
        assert_eq!(spanner.next(40), 4);
    }

    #[test]
    fn test_monotonic_decreasing() {
        // If prices always decrease, span is always 1.
        let mut spanner = StockSpanner::new();
        assert_eq!(spanner.next(40), 1);
        assert_eq!(spanner.next(30), 1);
        assert_eq!(spanner.next(20), 1);
        assert_eq!(spanner.next(10), 1);
    }

    #[test]
    fn test_same_price() {
        // Same prices should also increment span correctly.
        let mut spanner = StockSpanner::new();
        assert_eq!(spanner.next(30), 1);
        assert_eq!(spanner.next(30), 2);
        assert_eq!(spanner.next(30), 3);
        assert_eq!(spanner.next(30), 4);
    }

    #[test]
    fn test_single_element() {
        let mut spanner = StockSpanner::new();
        assert_eq!(spanner.next(42), 1);
    }

    #[test]
    fn test_default() {
        let mut spanner = StockSpanner::default();
        assert_eq!(spanner.next(100), 1);
    }
}
