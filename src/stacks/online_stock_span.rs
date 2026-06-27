//! # 901. Online Stock Span
//!
//! Difficulty: Medium
//! Link: https://leetcode.com/problems/online-stock-span/
//!
//! Why this matters in Rust: This problem demonstrates the use of a custom `struct` to manage
//! state (a monotonic stack) across method calls, highlighting idiomatic Rust patterns like
//! `while let` with `last()` or `pop()` for safe stack manipulation.
//!
//! ## Approach
//!
//! We need to calculate the span of a stock's price for the current day. The span is defined
//! as the maximum number of consecutive days (starting from today and going backward) for which
//! the stock price was less than or equal to today's price.
//!
//! We can efficiently solve this using a monotonic stack. The stack stores pairs of `(price, span)`.
//! When a new price comes in, we pop all elements from the stack that are less than or equal to
//! the current price, accumulating their spans. This works because any future price that is greater
//! than the current price will also be greater than all the popped prices, and the current price's
//! accumulated span encapsulates them perfectly.
//!
//! Time Complexity: Amortized O(1) per `next` call. Each price is pushed and popped at most once.
//! Space Complexity: O(N) where N is the number of calls to `next` (in the worst case of strictly
//! decreasing prices).
//!
//! ## Alternative Approaches
//!
//! * A brute-force approach would store all prices in an array and iterate backwards for every
//!   call to `next`. This would result in O(N) time per call, leading to O(N^2) total time.

/// Custom struct to manage state across method calls
pub struct StockSpanner {
    // Stores pairs of (price, span)
    stack: Vec<(i32, i32)>,
}

impl StockSpanner {
    pub fn new() -> Self {
        StockSpanner { stack: Vec::new() }
    }

    pub fn next(&mut self, price: i32) -> i32 {
        let mut span = 1;

        // RUST INSIGHT: `while let` is perfect here to cleanly peek at the last element
        // without panicking on an empty stack.
        // GOTCHA: We must use `.last()` to peek before popping. If we pop and the condition
        // fails, we'd have to push it back, which is less efficient.
        while let Some(&(prev_price, prev_span)) = self.stack.last() {
            if prev_price <= price {
                // If the previous price is smaller, it's subsumed by the current price
                span += prev_span;
                self.stack.pop();
            } else {
                // If we encounter a strictly larger price, the monotonic property guarantees
                // we stop here.
                break;
            }
        }

        self.stack.push((price, span));
        span
    }
}

// Ensure the struct meets default initialization expectations if used generically
impl Default for StockSpanner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stock_spanner_happy_path() {
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
    fn test_stock_spanner_edge_cases() {
        // Strictly increasing sequence
        let mut spanner1 = StockSpanner::new();
        assert_eq!(spanner1.next(10), 1);
        assert_eq!(spanner1.next(20), 2);
        assert_eq!(spanner1.next(30), 3);

        // Strictly decreasing sequence
        let mut spanner2 = StockSpanner::new();
        assert_eq!(spanner2.next(30), 1);
        assert_eq!(spanner2.next(20), 1);
        assert_eq!(spanner2.next(10), 1);
    }

    #[test]
    fn test_stock_spanner_stress() {
        // Same price repeatedly
        let mut spanner = StockSpanner::new();
        for i in 1..=100 {
            assert_eq!(spanner.next(50), i);
        }
    }
}
