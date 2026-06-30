//! # 901. Online Stock Span
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/online-stock-span/>
//!
//! Design a class that collects daily price quotes for some stock and returns the span of that stock's price for the current day.
//! The span of the stock's price in one day is the maximum number of consecutive days (starting from that day and going backward)
//! for which the stock price was less than or equal to the price of that day.
//!
//! This problem is a textbook use case for a **Monotonic Stack**. It demonstrates how to maintain
//! a strictly decreasing monotonic stack in a custom struct, using idiomatic Rust patterns like `while let`
//! with `last()` and `pop()` for stack manipulation.
//!
//! ## Approaches
//!
//! 1.  **Monotonic Stack**:
//!     -   We maintain a stack of pairs: `(price, span)`.
//!     -   The stack is strictly decreasing by price.
//!     -   When a new price comes in, we pop all elements from the stack that have a price less than or equal to the new price.
//!     -   We accumulate the span of the popped elements because if the new price is greater than an older price, it is also greater than everything that older price was greater than.
//!     -   Time: Amortized O(1) per `next` call (each element is pushed and popped at most once).
//!     -   Space: O(N) in the worst case (if prices are strictly decreasing).

// =========================================================================================
// Approach 1: Monotonic Stack
// =========================================================================================

/// State for calculating stock spans.
///
/// We use a monotonic stack to keep track of previous prices and their spans.
/// The stack stores tuples of `(price, span)`.
/// It maintains the invariant that prices are strictly decreasing from bottom to top.
pub struct StockSpanner {
    stack: Vec<(i32, i32)>,
}

impl StockSpanner {
    /// Initializes the object with an empty stack.
    #[must_use]
    pub fn new() -> Self {
        Self { stack: Vec::new() }
    }

    /// Returns the span of the stock's price for the given day.
    ///
    /// Time: Amortized O(1) per call
    pub fn next(&mut self, price: i32) -> i32 {
        let mut span = 1;

        // RUST INSIGHT: `while let` with `last()` allows us to inspect the top of the stack
        // without popping it if the condition isn't met.
        // We accumulate the spans of all prices less than or equal to the current price.
        while let Some(&(prev_price, prev_span)) = self.stack.last() {
            if prev_price <= price {
                // GOTCHA: We must `pop()` after confirming the condition.
                // We couldn't `pop()` inside `last()`'s condition because we might need to put it back.
                self.stack.pop();
                span += prev_span;
            } else {
                break;
            }
        }

        self.stack.push((price, span));
        span
    }
}

impl Default for StockSpanner {
    fn default() -> Self {
        Self::new()
    }
}

// =========================================================================================
// Tests
// =========================================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_happy_path() {
        let mut spanner = StockSpanner::new();
        // [100, 80, 60, 70, 60, 75, 85]
        assert_eq!(spanner.next(100), 1);
        assert_eq!(spanner.next(80), 1);
        assert_eq!(spanner.next(60), 1);
        assert_eq!(spanner.next(70), 2); // 70 is > 60
        assert_eq!(spanner.next(60), 1);
        assert_eq!(spanner.next(75), 4); // 75 is > 60, 70, 60
        assert_eq!(spanner.next(85), 6); // 85 is > 75, 60, 70, 60, 80
    }

    #[test]
    fn test_strictly_decreasing() {
        let mut spanner = StockSpanner::new();
        // [5, 4, 3, 2, 1]
        assert_eq!(spanner.next(5), 1);
        assert_eq!(spanner.next(4), 1);
        assert_eq!(spanner.next(3), 1);
        assert_eq!(spanner.next(2), 1);
        assert_eq!(spanner.next(1), 1);
    }

    #[test]
    fn test_strictly_increasing() {
        let mut spanner = StockSpanner::new();
        // [1, 2, 3, 4, 5]
        assert_eq!(spanner.next(1), 1);
        assert_eq!(spanner.next(2), 2);
        assert_eq!(spanner.next(3), 3);
        assert_eq!(spanner.next(4), 4);
        assert_eq!(spanner.next(5), 5);
    }

    #[test]
    fn test_same_prices() {
        let mut spanner = StockSpanner::new();
        // [10, 10, 10]
        assert_eq!(spanner.next(10), 1);
        assert_eq!(spanner.next(10), 2);
        assert_eq!(spanner.next(10), 3);
    }
}
