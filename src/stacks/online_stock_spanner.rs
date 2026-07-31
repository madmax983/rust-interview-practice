//! # 901. Online Stock Spanner
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/online-stock-spanner/>
//!
//! Design a class that collects daily price quotes for some stock and returns the span of that stock's price for the current day.
//! The span of the stock's price in one day is the maximum number of consecutive days (starting from that day and going backward)
//! for which the stock price was less than or equal to the price of that day.
//!
//! This problem matters in Rust because it perfectly demonstrates the Monotonic Stack pattern
//! and how zero-cost abstractions (like tuples) can be used to neatly bundle state without heap allocations.
//! It also explores stateful structs acting as state machines.
//!
//! ## Approach
//!
//! **Optimal Solution**: Monotonic Stack
//!
//! Instead of looking back at all previous prices every day (which would be O(n^2)), we can maintain
//! a stack of `(price, span)` tuples. The stack remains strictly decreasing in price (a monotonic stack).
//! When a new price comes in, we pop all elements from the stack that have a price less than or equal
//! to the new price, accumulating their spans. We then push the new price and its total accumulated span
//! onto the stack.
//!
//! Time: Amortized O(1) per `next` call. Each element is pushed and popped at most once.
//! Space: O(N) in the worst case (e.g., strictly decreasing prices where nothing gets popped).
//!
//! Why this is idiomatic Rust vs Java/Python:
//! In Java or Python, one might create a `Node` or `Pair` class. In Rust, we simply use a tuple `(i32, i32)`
//! directly in a `Vec`. The `Vec` acts as our contiguous stack. Memory is laid out efficiently without the
//! pointer indirection overhead of an object-based stack.
//!
//! Note: Since this is an object-oriented design problem, we do not follow the typical brute/optimized/optimal
//! function progression. We provide the optimal implementation in the `StockSpanner` struct.

/// A stateful struct that calculates the stock span using a monotonic stack.
#[derive(Default, Debug)]
pub struct StockSpanner {
    // RUST INSIGHT: A Vec of tuples `(price, span)` is perfectly contiguous in memory.
    // It avoids allocations per-item that you would get if using a linked structure.
    stack: Vec<(i32, i32)>,
}

impl StockSpanner {
    /// Creates a new, empty `StockSpanner`.
    #[must_use]
    pub const fn new() -> Self {
        Self { stack: Vec::new() }
    }

    /// Processes the next daily price quote and returns the stock span.
    ///
    /// The span is the maximum number of consecutive days (including today and going backward)
    /// for which the price was less than or equal to today's price.
    pub fn next(&mut self, price: i32) -> i32 {
        let mut span = 1;

        // GOTCHA: We must use a `while let` with `last()` instead of just `pop()` conditionally.
        // Wait, we can use `last()` to check the condition, and if met, `pop()`.
        // Alternatively, we can use a `while let` to peek, and then conditionally pop inside.
        // Rust's `Vec` doesn't have an in-place conditionally-pop method.
        while let Some(&(prev_price, prev_span)) = self.stack.last() {
            if prev_price <= price {
                span += prev_span;
                self.stack.pop();
            } else {
                break;
            }
        }

        self.stack.push((price, span));
        span
    }
}

// =========================================================================================
// Alternative Approaches
// =========================================================================================
//
// 1. **Brute Force (Array)**:
//    - Store every incoming price in a simple `Vec<i32>`.
//    - On each `next(price)` call, iterate backward from the end, incrementing a counter
//      until finding a price strictly greater than the current one.
//    - Time: O(N) per call, O(N^2) total.
//    - Space: O(N) to store prices.
//    - Too slow for LeetCode constraints, but very easy to implement.
//
// 2. **Index-Based Monotonic Stack**:
//    - Store `(price, index)` instead of `(price, span)`.
//    - Maintain a counter for the current day index.
//    - The span is calculated as `current_index - stack_top_index` after popping.
//    - Both approaches are identical in complexity, but the `(price, span)` tuple avoids
//      storing a separate `current_day` field on the struct.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stock_spanner_happy_path() {
        let mut spanner = StockSpanner::new();
        // Prices: [100, 80, 60, 70, 60, 75, 85]
        assert_eq!(spanner.next(100), 1);
        assert_eq!(spanner.next(80), 1);
        assert_eq!(spanner.next(60), 1);
        assert_eq!(spanner.next(70), 2); // 70 > 60
        assert_eq!(spanner.next(60), 1);
        assert_eq!(spanner.next(75), 4); // 75 > 60, 70, 60
        assert_eq!(spanner.next(85), 6); // 85 > 75, 60, 70, 60, 80
    }

    #[test]
    fn test_stock_spanner_strictly_decreasing() {
        let mut spanner = StockSpanner::new();
        // Prices: [5, 4, 3, 2, 1]
        // Since each is smaller, span should always be 1, and no pops happen.
        assert_eq!(spanner.next(5), 1);
        assert_eq!(spanner.next(4), 1);
        assert_eq!(spanner.next(3), 1);
        assert_eq!(spanner.next(2), 1);
        assert_eq!(spanner.next(1), 1);
        assert_eq!(spanner.stack.len(), 5);
    }

    #[test]
    fn test_stock_spanner_strictly_increasing() {
        let mut spanner = StockSpanner::new();
        // Prices: [1, 2, 3, 4, 5]
        // Since each is larger, it absorbs all previous days.
        assert_eq!(spanner.next(1), 1);
        assert_eq!(spanner.next(2), 2);
        assert_eq!(spanner.next(3), 3);
        assert_eq!(spanner.next(4), 4);
        assert_eq!(spanner.next(5), 5);
        assert_eq!(spanner.stack.len(), 1);
    }

    #[test]
    fn test_stock_spanner_same_prices() {
        let mut spanner = StockSpanner::new();
        // Prices: [10, 10, 10]
        assert_eq!(spanner.next(10), 1);
        assert_eq!(spanner.next(10), 2);
        assert_eq!(spanner.next(10), 3);
        assert_eq!(spanner.stack.len(), 1);
    }

    #[test]
    fn test_stock_spanner_stress() {
        let mut spanner = StockSpanner::new();
        // Boundary case: up to 10^4 calls
        for i in 1..=5000 {
            assert_eq!(spanner.next(50), 1);
            assert_eq!(spanner.next(100), 2 * i);
        }
    }
}
