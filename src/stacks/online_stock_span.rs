//! # 901. Online Stock Span
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/online-stock-span/>
//!
//! Design a class that collects daily price quotes for some stock and returns the span of that stock's price for the current day.
//!
//! The span of the stock's price today is defined as the maximum number of consecutive days (starting from today and going backward)
//! for which the stock price was less than or equal to today's price.
//!
//! For example, if the price of a stock over the next 7 days were `[100, 80, 60, 70, 60, 75, 85]`, then the stock spans would be `[1, 1, 1, 2, 1, 4, 6]`.
//!
//! This problem perfectly demonstrates the Monotonic Stack pattern and how to build custom structs in Rust to maintain state across method calls.
//!
//! ## Approach
//!
//! A brute force approach would be to store all prices in an array and iterate backwards for each new price. This takes O(N) time per call.
//!
//! An optimal approach uses a **decreasing monotonic stack**. The stack stores tuples of `(price, span)`.
//! When a new `price` comes in, we pop all elements from the stack where `stack_price <= price`, accumulating their spans.
//! Since every element is pushed and popped at most once, the amortized time complexity is O(1) per call.
//!
//! RUST INSIGHT: The `while let` construct combined with `stack.last()` and `stack.pop()` is the most idiomatic and safe way to inspect and remove elements from the top of a `Vec` used as a stack.

/// Optimal approach: Monotonic Stack
///
/// Time: Amortized O(1) per `next` call. Each element is pushed and popped exactly once.
/// Space: O(N) in the worst case (e.g., strictly decreasing prices).
#[derive(Debug, Default)]
pub struct StockSpanner {
    // Stack stores pairs of (price, accumulated_span)
    stack: Vec<(i32, i32)>,
}

impl StockSpanner {
    #[must_use]
    pub fn new() -> Self {
        Self { stack: Vec::new() }
    }

    /// Adds the new price and returns its span.
    pub fn next(&mut self, price: i32) -> i32 {
        let mut span = 1;

        // RUST INSIGHT: `while let Some(...) = self.stack.last()` gives us safe, pattern-matched access to the top element without popping it.
        // We only pop it if the condition `p <= price` is met.
        while let Some(&(p, s)) = self.stack.last() {
            if p <= price {
                span += s;
                self.stack.pop(); // Safe to pop now since we checked the condition
            } else {
                break; // Monotonic property holds, stop popping
            }
        }

        self.stack.push((price, span));
        span
    }
}

// Alternative Approaches:
// 1. **Brute Force Array**: Keep a `Vec<i32>` of all prices. On each `next` call, iterate backward counting how many are <= the current price. Time: O(N) per call, Space: O(N).

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_happy_path() {
        let mut spanner = StockSpanner::new();
        assert_eq!(spanner.next(100), 1);
        assert_eq!(spanner.next(80), 1);
        assert_eq!(spanner.next(60), 1);
        assert_eq!(spanner.next(70), 2); // 60, 70
        assert_eq!(spanner.next(60), 1);
        assert_eq!(spanner.next(75), 4); // 60, 70, 60, 75
        assert_eq!(spanner.next(85), 6); // 80, 60, 70, 60, 75, 85
    }

    #[test]
    fn test_strictly_increasing() {
        let mut spanner = StockSpanner::new();
        assert_eq!(spanner.next(10), 1);
        assert_eq!(spanner.next(20), 2);
        assert_eq!(spanner.next(30), 3);
        assert_eq!(spanner.next(40), 4);
    }

    #[test]
    fn test_strictly_decreasing() {
        let mut spanner = StockSpanner::new();
        assert_eq!(spanner.next(40), 1);
        assert_eq!(spanner.next(30), 1);
        assert_eq!(spanner.next(20), 1);
        assert_eq!(spanner.next(10), 1);
    }

    #[test]
    fn test_same_prices() {
        let mut spanner = StockSpanner::new();
        assert_eq!(spanner.next(30), 1);
        assert_eq!(spanner.next(30), 2);
        assert_eq!(spanner.next(30), 3);
    }
}
