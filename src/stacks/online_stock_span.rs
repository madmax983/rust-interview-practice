//! # 901. Online Stock Span
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/online-stock-span/>
//!
//! Design an algorithm that collects daily price quotes for some stock and returns the span of that stock's price for the current day.
//! The span of the stock's price in one day is the maximum number of consecutive days (starting from that day and going backward) for which the stock price was less than or equal to the price of that day.
//!
//! ## Why This Matters in Rust
//!
//! This problem perfectly illustrates the monotonic stack pattern and how a mutable struct can encapsulate and maintain state across multiple calls.
//! We will also show how we can use an iterator adapter for a functional, lazily evaluated approach in Rust, demonstrating Rust's powerful trait system.
//!
//! ## Approach
//!
//! We need to calculate the span of each day's price as it arrives. A naive approach would scan backward through all previous prices, which gives O(n) time per query.
//! The optimal approach is a **Monotonic Stack**.
//! The stack will store tuples of `(price, span)`.
//! When a new price arrives:
//! 1. Initialize `span = 1`.
//! 2. While the stack is not empty and the price at the top of the stack is less than or equal to the current price:
//!    - Pop the top element and add its span to our `span`.
//! 3. Push `(price, span)` onto the stack.
//! 4. Return `span`.
//!
//! Time complexity: Amortized O(1) per `next` call (each element is pushed and popped at most once). O(N) overall.
//! Space complexity: O(N) in the worst case if prices are strictly decreasing.
//!
//! We implement two solutions:
//! 1. `StockSpanner`: A traditional mutable, stateful struct that matches the `LeetCode` API.
//! 2. `StockSpanIter`: An iterator adapter to lazily yield the spans of a sequence of prices, showing zero-cost abstractions.

/// A stateful struct to calculate the online stock span.
#[derive(Default)]
pub struct StockSpanner {
    // Stores (price, span)
    stack: Vec<(i32, i32)>,
}

impl StockSpanner {
    /// Initializes the object with empty span.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the span of the stock's price for the current day.
    pub fn next(&mut self, price: i32) -> i32 {
        let mut span = 1;

        // RUST INSIGHT: We use `last()` to peek without modifying the stack,
        // and only `pop()` if the condition is met.
        while let Some(&(prev_price, prev_span)) = self.stack.last() {
            if prev_price <= price {
                span += prev_span;
                // GOTCHA: We must pop the element since it's smaller/equal and its span
                // is now absorbed into the current element's span.
                self.stack.pop();
            } else {
                break;
            }
        }

        self.stack.push((price, span));
        span
    }
}

// -----------------------------------------------------------------------------
// Functional / Iterator approach
// -----------------------------------------------------------------------------

/// An iterator adapter that takes an iterator of prices and yields their spans.
pub struct StockSpanIter<I> {
    iter: I,
    stack: Vec<(i32, i32)>,
}

impl<I> StockSpanIter<I> {
    pub const fn new(iter: I) -> Self {
        Self {
            iter,
            stack: Vec::new(),
        }
    }
}

impl<I> Iterator for StockSpanIter<I>
where
    I: Iterator<Item = i32>,
{
    type Item = i32;

    fn next(&mut self) -> Option<Self::Item> {
        // RUST INSIGHT: The `?` operator safely propagates `None` when the inner
        // iterator is exhausted, automatically terminating this iterator.
        let price = self.iter.next()?;
        let mut span = 1;

        while let Some(&(prev_price, prev_span)) = self.stack.last() {
            if prev_price <= price {
                span += prev_span;
                self.stack.pop();
            } else {
                break;
            }
        }

        self.stack.push((price, span));
        Some(span)
    }
}

/// Extension trait to easily chain the `StockSpanIter` adapter.
pub trait StockSpanExt: Iterator<Item = i32> + Sized {
    fn stock_span(self) -> StockSpanIter<Self> {
        StockSpanIter::new(self)
    }
}

// Blanket implementation for any iterator yielding `i32`
impl<I: Iterator<Item = i32>> StockSpanExt for I {}

// ## Alternative Approaches
//
// - **Array with back-pointers**: Instead of a stack of pairs, store all prices in an array,
//   and parallel arrays for spans (or indices of the previous greater element). While this avoids
//   pair allocation and allows random access, a stack of tuples is simpler and generally more memory-efficient
//   for just computing the span since smaller elements are forgotten.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stock_spanner_happy_path() {
        let mut spanner = StockSpanner::new();
        assert_eq!(spanner.next(100), 1);
        assert_eq!(spanner.next(80), 1);
        assert_eq!(spanner.next(60), 1);
        assert_eq!(spanner.next(70), 2);
        assert_eq!(spanner.next(60), 1);
        assert_eq!(spanner.next(75), 4);
        assert_eq!(spanner.next(85), 6);
    }

    #[test]
    fn test_stock_spanner_edge_case_increasing() {
        let mut spanner = StockSpanner::new();
        // Strictly increasing: spans should be 1, 2, 3, 4, 5
        assert_eq!(spanner.next(10), 1);
        assert_eq!(spanner.next(20), 2);
        assert_eq!(spanner.next(30), 3);
        assert_eq!(spanner.next(40), 4);
        assert_eq!(spanner.next(50), 5);
    }

    #[test]
    fn test_stock_spanner_edge_case_decreasing() {
        let mut spanner = StockSpanner::new();
        // Strictly decreasing: spans should always be 1
        assert_eq!(spanner.next(50), 1);
        assert_eq!(spanner.next(40), 1);
        assert_eq!(spanner.next(30), 1);
        assert_eq!(spanner.next(20), 1);
        assert_eq!(spanner.next(10), 1);
    }

    #[test]
    fn test_stock_span_iter() {
        let prices = vec![100, 80, 60, 70, 60, 75, 85];
        let spans: Vec<i32> = prices.into_iter().stock_span().collect();
        assert_eq!(spans, vec![1, 1, 1, 2, 1, 4, 6]);
    }

    #[test]
    fn test_large_input_stress() {
        let mut spanner = StockSpanner::new();
        // Stress test with many identical elements, which should all collapse
        for _ in 0..10_000 {
            spanner.next(50);
        }
        // At the end, the span for a new 50 should be 10001
        assert_eq!(spanner.next(50), 10001);

        // And a larger number collapses everything
        assert_eq!(spanner.next(100), 10002);
    }
}
