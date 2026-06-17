//! # 901. Online Stock Span
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/online-stock-span/>
//!
//! Design a class that collects daily price quotes for some stock and returns the span of that stock's price for the current day.
//!
//! The span of the stock's price in one day is the maximum number of consecutive days (starting from that day and going backward)
//! for which the stock price was less than or equal to the price of that day.
//!
//! This problem demonstrates the use of a custom `struct` to manage state (a monotonic stack) across method calls,
//! highlighting idiomatic Rust patterns like `while let` with `last()` or `pop()`.
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

/// A monotonic decreasing stack to efficiently calculate the stock span.
///
/// We store pairs of `(price, span)`.
/// Because we only care about consecutive days where the price was *less than or equal*,
/// if we encounter a price greater than the top of our stack, we can safely "absorb" the
/// previous price and its span into the current day's span, and completely forget about
/// the previous price (since any future price that is greater than the current one will
/// also be greater than the absorbed ones).
#[derive(Default)]
pub struct StockSpanner {
    stack: Vec<(i32, i32)>, // (price, span)
}

impl StockSpanner {
    /// Initializes the object with an empty stack.
    #[must_use]
    pub fn new() -> Self {
        Self { stack: Vec::new() }
    }

    /// Returns the span of the stock's price for the current day.
    ///
    /// Time: O(1) amortized. Each element is pushed and popped at most once.
    /// Space: O(N) in the worst case (strictly decreasing prices).
    pub fn next(&mut self, price: i32) -> i32 {
        let mut span = 1;

        // RUST INSIGHT: `while let Some(...) = self.stack.last()` allows us to peek
        // before popping. However, since we just need to consume elements that meet
        // a condition, we can use a loop and conditionally pop.
        while let Some(&(prev_price, prev_span)) = self.stack.last() {
            if prev_price <= price {
                span += prev_span;
                self.stack.pop(); // Safe to remove because current price eclipses it
            } else {
                break; // Monotonic property restored
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
        assert_eq!(spanner.next(100), 1);
        assert_eq!(spanner.next(80), 1);
        assert_eq!(spanner.next(60), 1);
        assert_eq!(spanner.next(70), 2); // 70 > 60, absorbs 60 (span 1) + self (1) = 2
        assert_eq!(spanner.next(60), 1);
        assert_eq!(spanner.next(75), 4); // 75 > 60, 70, 60. Absorbs spans 1 + 2 + 1 = 4.
        assert_eq!(spanner.next(85), 6); // 85 > 75, 80. Absorbs spans 4 + 1 + 1 = 6.
    }

    #[test]
    fn test_strictly_decreasing() {
        let mut spanner = StockSpanner::new();
        assert_eq!(spanner.next(50), 1);
        assert_eq!(spanner.next(40), 1);
        assert_eq!(spanner.next(30), 1);
        assert_eq!(spanner.next(20), 1);
        assert_eq!(spanner.next(10), 1);
    }

    #[test]
    fn test_strictly_increasing() {
        let mut spanner = StockSpanner::new();
        assert_eq!(spanner.next(10), 1);
        assert_eq!(spanner.next(20), 2);
        assert_eq!(spanner.next(30), 3);
        assert_eq!(spanner.next(40), 4);
        assert_eq!(spanner.next(50), 5);
    }

    #[test]
    fn test_stress_same_values() {
        let mut spanner = StockSpanner::new();
        assert_eq!(spanner.next(10), 1);
        assert_eq!(spanner.next(10), 2);
        assert_eq!(spanner.next(10), 3);
    }
}
