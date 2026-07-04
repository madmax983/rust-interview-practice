//! # 901. Online Stock Span
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/online-stock-span/>
//!
//! Design an algorithm that collects daily price quotes for some stock and returns the span of that stock's price for the current day.
//! The span of the stock's price in one day is the maximum number of consecutive days (starting from that day and going backward)
//! for which the stock price was less than or equal to the price of that day.
//!
//! This problem perfectly demonstrates why maintaining a strictly decreasing monotonic stack in a custom struct is an elegant
//! approach. It highlights idiomatic Rust patterns like using `while let` with `last()` and `pop()` for safe stack manipulation,
//! avoiding out-of-bounds errors common in index-based loops.
//!
//! ## Examples
//!
//! ```
//! use rust_interview_practice::stacks::online_stock_span::StockSpanner;
//!
//! let mut stock_spanner = StockSpanner::new();
//! assert_eq!(stock_spanner.next(100), 1);
//! assert_eq!(stock_spanner.next(80), 1);
//! assert_eq!(stock_spanner.next(60), 1);
//! assert_eq!(stock_spanner.next(70), 2);
//! assert_eq!(stock_spanner.next(60), 1);
//! assert_eq!(stock_spanner.next(75), 4);
//! assert_eq!(stock_spanner.next(85), 6);
//! ```
//!
//! ## Constraints
//!
//! - `1 <= price <= 10^5`
//! - At most `10^4` calls will be made to `next`.

/// Brute Force approach: Store all prices and iterate backward
///
/// Time: O(n) per `next` call - Worst case, we might scan all previous days.
/// Space: O(n) - We store every single price in a `Vec`.
///
/// The brute force approach simply stores all the incoming prices in a dynamic array.
/// To find the span, it iterates backward from the latest price, counting days as long as
/// the price is less than or equal to the current day's price.
pub struct StockSpannerBruteForce {
    prices: Vec<i32>,
}

impl Default for StockSpannerBruteForce {
    fn default() -> Self {
        Self::new()
    }
}

impl StockSpannerBruteForce {
    #[must_use]
    pub const fn new() -> Self {
        Self { prices: Vec::new() }
    }

    pub fn next(&mut self, price: i32) -> i32 {
        self.prices.push(price);
        let mut span = 0;

        // GOTCHA: Using a reverse iterator is safer and more idiomatic than managing an index variable manually.
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

/// Optimal approach: Monotonic Stack
///
/// Time: O(1) amortized per `next` call - Each element is pushed and popped at most once.
/// Space: O(n) - In the worst case (strictly decreasing prices), we store all prices in the stack.
///
/// Instead of scanning all previous prices, we use a monotonic stack. The stack stores pairs of
/// `(price, span)`. We maintain the invariant that the prices in the stack are strictly decreasing
/// from bottom to top. When a new price arrives, we pop all smaller or equal prices and accumulate
/// their spans, as any future price that is greater than the current price will also be greater than
/// all the popped prices.
pub struct StockSpanner {
    // We store a tuple of (price, span)
    stack: Vec<(i32, i32)>,
}

impl Default for StockSpanner {
    fn default() -> Self {
        Self::new()
    }
}

impl StockSpanner {
    #[must_use]
    pub const fn new() -> Self {
        Self { stack: Vec::new() }
    }

    pub fn next(&mut self, price: i32) -> i32 {
        let mut span = 1;

        // RUST INSIGHT: `while let` with `last()` allows us to safely peek at the top of the stack
        // without popping. The compiler guarantees we only enter the loop if the stack has elements.
        while let Some(&(prev_price, prev_span)) = self.stack.last() {
            if prev_price <= price {
                // We know it's safe to pop because `last()` returned `Some`
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

// Alternative Approaches:
// 1. **Index-based Monotonic Stack**: Instead of storing the span explicitly in the stack, we could store the
//    indices of the prices and compute the span by subtracting the index of the last greater price from the
//    current day's index. This achieves the same time and space complexity.
// 2. **Segment Tree / Fenwick Tree**: While overkill for this problem, range maximum queries could technically
//    be used if we were querying arbitrary ranges instead of just the latest contiguous span.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force() {
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
    fn test_optimal_happy_path() {
        // Standard case matching the problem description
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
    fn test_optimal_edge_case_increasing() {
        // Strictly increasing prices: span should grow exactly by 1 each time
        let mut spanner = StockSpanner::new();
        assert_eq!(spanner.next(10), 1);
        assert_eq!(spanner.next(20), 2);
        assert_eq!(spanner.next(30), 3);
        assert_eq!(spanner.next(40), 4);
    }

    #[test]
    fn test_optimal_edge_case_decreasing() {
        // Strictly decreasing prices: span should always be 1
        let mut spanner = StockSpanner::new();
        assert_eq!(spanner.next(40), 1);
        assert_eq!(spanner.next(30), 1);
        assert_eq!(spanner.next(20), 1);
        assert_eq!(spanner.next(10), 1);
    }

    #[test]
    fn test_optimal_edge_case_same_prices() {
        // Identical prices: span should grow linearly
        let mut spanner = StockSpanner::new();
        assert_eq!(spanner.next(50), 1);
        assert_eq!(spanner.next(50), 2);
        assert_eq!(spanner.next(50), 3);
        assert_eq!(spanner.next(50), 4);
    }

    #[test]
    fn test_stress_boundary_case() {
        // Stress test simulating a long sequence of identical prices followed by a large jump
        let mut spanner_brute = StockSpannerBruteForce::new();
        let mut spanner_opt = StockSpanner::new();

        for _ in 0..1000 {
            assert_eq!(spanner_brute.next(5), spanner_opt.next(5));
        }

        // Final jump larger than all previous ones
        let final_span_brute = spanner_brute.next(10);
        let final_span_opt = spanner_opt.next(10);

        assert_eq!(final_span_brute, 1001);
        assert_eq!(final_span_opt, 1001);
    }
}
