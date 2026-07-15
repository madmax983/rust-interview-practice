//! # 901. Online Stock Span
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/online-stock-span/>
//!
//! Design an algorithm that collects daily price quotes for some stock and returns the span of that stock's price for the current day.
//! The span of the stock's price in one day is the maximum number of consecutive days (starting from that day and going backward) for which the stock price was less than or equal to the price of that day.
//!
//! This problem perfectly demonstrates Rust's ability to encapsulate state within custom structs, using a strictly decreasing monotonic stack to maintain the necessary history while yielding an O(1) amortized time complexity per `next()` call.
//!
//! ## Approach
//!
//! We use a monotonic stack that stores a tuple `(price, span)`.
//! When a new price comes in, we pop all elements from the stack where the price is less than or equal to the new price, accumulating their spans.
//! We then push the new `(price, total_span)` onto the stack and return `total_span`.
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

/// `StockSpanner` encapsulates the state needed to calculate the span of a stock's price.
/// It uses a strictly decreasing monotonic stack to achieve O(1) amortized time complexity per call.
#[derive(Debug, Default)]
pub struct StockSpanner {
    // Stack stores pairs of (price, span)
    stack: Vec<(i32, i32)>,
}

impl StockSpanner {
    /// Initializes the object with an empty stack.
    #[must_use]
    pub fn new() -> Self {
        Self { stack: Vec::new() }
    }

    /// Returns the span of the stock's price for the current day.
    ///
    /// Time Complexity: O(1) amortized - Each element is pushed and popped at most once.
    /// Space Complexity: O(n) - In the worst case (strictly decreasing prices), all prices are stored in the stack.
    pub fn next(&mut self, price: i32) -> i32 {
        let mut span = 1;

        // RUST INSIGHT: `while let Some(...) = ...` combined with `.last()` and `.pop()` is an idiomatic
        // way to conditionally drain from the end of a `Vec` while peeking at the elements first.
        while let Some(&(last_price, last_span)) = self.stack.last() {
            if last_price <= price {
                span += last_span;
                self.stack.pop();
            } else {
                break;
            }
        }

        self.stack.push((price, span));
        span
    }
}

// Alternative Approaches:
// 1. **Brute Force Array**: Keep an array of all prices, and iterate backwards on every `next` call. This is O(n) per call and O(n^2) overall, which will TLE on LeetCode.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_happy_path() {
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
    fn test_edge_case_strictly_increasing() {
        let mut spanner = StockSpanner::new();
        assert_eq!(spanner.next(10), 1);
        assert_eq!(spanner.next(20), 2);
        assert_eq!(spanner.next(30), 3);
        assert_eq!(spanner.next(40), 4);
    }

    #[test]
    fn test_edge_case_strictly_decreasing() {
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
