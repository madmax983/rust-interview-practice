//! # 901. Online Stock Span
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/online-stock-span/>
//!
//! Design an algorithm that collects daily price quotes for some stock and returns the span of that stock's price for the current day.
//!
//! The span of the stock's price in one day is the maximum number of consecutive days (starting from that day and going backward) for which the stock price was less than or equal to the price of that day.
//!
//! This problem perfectly demonstrates Rust's monotonic stack pattern and ownership model.
//! It highlights how using a `Vec` as a stack can efficiently track historical state while avoiding unnecessary allocations.
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
//!
//! ## Alternative Approaches
//! - **Segment Tree / Fenwick Tree:** If we needed to query arbitrary historical spans or update past prices, a more complex tree structure would be necessary, trading O(1) amortized for O(log N) updates/queries.
//! - **Skip List:** For more generic order statistics and time-based range queries, though over-engineered for just contiguous spans.

/// Brute force approach: Storing all prices and scanning backward.
/// Time: O(N) per `next` call in the worst case (e.g., strictly increasing prices).
/// Space: O(N) where N is the number of prices.
///
/// This approach simply stores every price in a vector and iterates backward
/// to calculate the span for each new price.
#[derive(Default)]
pub struct StockSpannerBruteForce {
    prices: Vec<i32>,
}

impl StockSpannerBruteForce {
    #[must_use]
    pub const fn new() -> Self {
        Self { prices: Vec::new() }
    }

    pub fn next(&mut self, price: i32) -> i32 {
        self.prices.push(price);
        let mut span = 0;

        // RUST INSIGHT: `.iter().rev()` creates a double-ended iterator going backward.
        for &p in self.prices.iter().rev() {
            if p <= price {
                span += 1;
            } else {
                break;
            }
        }

        span
    }
}

/// Optimal approach: Monotonic decreasing stack.
/// Time: O(1) amortized per `next` call (each element is pushed and popped at most once).
/// Space: O(N) in the worst case (e.g., strictly decreasing prices).
///
/// We maintain a stack of `(price, span)` tuples. If the current price is greater than
/// or equal to the price at the top of the stack, we can safely pop the top element
/// and add its span to our current span, because any future price greater than the current
/// price will also be greater than the popped price. This maintains a strictly monotonic
/// decreasing stack.
#[derive(Default)]
pub struct StockSpannerOptimal {
    // Store pairs of (price, span)
    stack: Vec<(i32, i32)>,
}

impl StockSpannerOptimal {
    #[must_use]
    pub const fn new() -> Self {
        Self { stack: Vec::new() }
    }

    pub fn next(&mut self, price: i32) -> i32 {
        let mut current_span = 1;

        // RUST INSIGHT: `while let Some(...) = self.stack.last()` lets us peek at the top
        // of the stack safely without indexing, and pattern match its contents.
        while let Some(&(prev_price, prev_span)) = self.stack.last() {
            if prev_price <= price {
                current_span += prev_span;
                // GOTCHA: We must pop the element since we've accumulated its span.
                // We could also use `.pop()` directly in the while condition if we didn't need to peek.
                self.stack.pop();
            } else {
                break;
            }
        }

        self.stack.push((price, current_span));
        current_span
    }
}

/// Main entry point alias - maps to the optimal implementation.
pub type StockSpanner = StockSpannerOptimal;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force_example() {
        let mut spanner = StockSpannerBruteForce::new();
        assert_eq!(spanner.next(100), 1);
        assert_eq!(spanner.next(80), 1);
        assert_eq!(spanner.next(60), 1);
        assert_eq!(spanner.next(70), 2);
        assert_eq!(spanner.next(60), 1);
        assert_eq!(spanner.next(75), 4);
        assert_eq!(spanner.next(85), 6);
    }

    #[test]
    fn test_optimal_example() {
        let mut spanner = StockSpannerOptimal::new();
        assert_eq!(spanner.next(100), 1);
        assert_eq!(spanner.next(80), 1);
        assert_eq!(spanner.next(60), 1);
        assert_eq!(spanner.next(70), 2);
        assert_eq!(spanner.next(60), 1);
        assert_eq!(spanner.next(75), 4);
        assert_eq!(spanner.next(85), 6);
    }

    #[test]
    fn test_decreasing_prices() {
        let mut spanner_bf = StockSpannerBruteForce::new();
        let mut spanner_opt = StockSpannerOptimal::new();

        let prices = vec![10, 9, 8, 7, 6];
        for price in prices {
            assert_eq!(spanner_bf.next(price), 1);
            assert_eq!(spanner_opt.next(price), 1);
        }
    }

    #[test]
    fn test_increasing_prices() {
        let mut spanner_bf = StockSpannerBruteForce::new();
        let mut spanner_opt = StockSpannerOptimal::new();

        let prices = vec![1, 2, 3, 4, 5];
        for (expected_span, price) in (1..).zip(prices) {
            assert_eq!(spanner_bf.next(price), expected_span);
            assert_eq!(spanner_opt.next(price), expected_span);
        }
    }

    #[test]
    fn test_same_prices() {
        let mut spanner_bf = StockSpannerBruteForce::new();
        let mut spanner_opt = StockSpannerOptimal::new();

        let prices = vec![10, 10, 10, 10];
        for (expected_span, price) in (1..).zip(prices) {
            assert_eq!(spanner_bf.next(price), expected_span);
            assert_eq!(spanner_opt.next(price), expected_span);
        }
    }
}
