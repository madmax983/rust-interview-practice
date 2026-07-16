//! # 901. Online Stock Span
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/online-stock-span/>
//!
//! Design an algorithm that collects daily price quotes for some stock and returns the span of that stock's price for the current day.
//! The span of the stock's price in one day is the maximum number of consecutive days (starting from that day and going backward)
//! for which the stock price was less than or equal to the price of that day.
//!
//! This problem matters in Rust because it perfectly demonstrates how to maintain and encapsulate
//! a strictly decreasing monotonic stack in a custom struct, using idiomatic Rust patterns like `while let`
//! with `last()` and `pop()` for stack manipulation.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::stacks::online_stock_span::StockSpanner;
//!
//! let mut spanner = StockSpanner::new();
//! assert_eq!(spanner.next(100), 1);
//! assert_eq!(spanner.next(80), 1);
//! assert_eq!(spanner.next(60), 1);
//! assert_eq!(spanner.next(70), 2);
//! assert_eq!(spanner.next(60), 1);
//! assert_eq!(spanner.next(75), 4);
//! assert_eq!(spanner.next(85), 6);
//! ```
//!
//! ## Constraints
//!
//! - `1 <= price <= 10^5`
//! - At most `10^4` calls will be made to `next`.

/// The `StockSpanner` uses a Monotonic Stack to efficiently compute the span.
///
/// Time: O(1) amortized per `next` call. While a single call could trigger multiple pops,
/// each price is pushed and popped exactly once over the lifetime of the structure.
/// Space: O(N) where N is the number of calls to `next` (worst-case strictly decreasing prices).
///
/// # Architecture
/// We maintain a stack of tuples `(price, span)`. The stack is kept strictly decreasing
/// in terms of price. When a new price comes in, we pop elements off the stack as long
/// as they are less than or equal to the current price, accumulating their spans.
#[derive(Default)]
pub struct StockSpanner {
    // Stack stores pairs of (price, span)
    stack: Vec<(i32, i32)>,
}

impl StockSpanner {
    #[must_use]
    pub const fn new() -> Self {
        Self { stack: Vec::new() }
    }

    /// Optimized Monotonic Stack implementation
    /// # Panics
    /// This function will not panic. The `unwrap` is protected by a prior `is_some` check.
    pub fn next(&mut self, price: i32) -> i32 {
        let mut span = 1;

        // RUST INSIGHT: `while let` with `last()` combined with `pop()` allows us to safely
        // peek at the top element without moving it out unless the condition is met.
        // GOTCHA: Don't pop immediately in the condition; peek first, then conditionally pop
        // inside the block to avoid losing values that shouldn't be popped.
        while let Some(&(last_price, _last_span)) = self.stack.last() {
            if last_price <= price {
                if let Some((_, popped_span)) = self.stack.pop() {
                    span += popped_span;
                }
            } else {
                break;
            }
        }

        self.stack.push((price, span));
        span
    }
}

// Alternative Approaches:
// 1. Brute Force Array: Store all prices in a Vec and iterate backwards on every `next` call.
//    Time: O(N) per call, leading to O(N^2) total time. Unacceptable for 10^4 calls.

#[cfg(test)]
mod tests {
    use super::*;

    // Happy Path
    #[test]
    fn test_stock_spanner_standard() {
        let mut spanner = StockSpanner::new();
        assert_eq!(spanner.next(100), 1);
        assert_eq!(spanner.next(80), 1);
        assert_eq!(spanner.next(60), 1);
        assert_eq!(spanner.next(70), 2);
        assert_eq!(spanner.next(60), 1);
        assert_eq!(spanner.next(75), 4);
        assert_eq!(spanner.next(85), 6);
    }

    // Edge Cases
    #[test]
    fn test_increasing_prices() {
        let mut spanner = StockSpanner::new();
        assert_eq!(spanner.next(10), 1);
        assert_eq!(spanner.next(20), 2);
        assert_eq!(spanner.next(30), 3);
        assert_eq!(spanner.next(40), 4);
    }

    #[test]
    fn test_decreasing_prices() {
        let mut spanner = StockSpanner::new();
        assert_eq!(spanner.next(40), 1);
        assert_eq!(spanner.next(30), 1);
        assert_eq!(spanner.next(20), 1);
        assert_eq!(spanner.next(10), 1);
    }

    #[test]
    fn test_same_prices() {
        let mut spanner = StockSpanner::new();
        assert_eq!(spanner.next(10), 1);
        assert_eq!(spanner.next(10), 2);
        assert_eq!(spanner.next(10), 3);
    }
}
