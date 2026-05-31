//! # 901. Online Stock Span
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/online-stock-span/>
//!
//! Design an algorithm that collects daily price quotes for some stock and returns the span of that
//! stock's price for the current day.
//!
//! The span of the stock's price in one day is the maximum number of consecutive days
//! (starting from that day and going backward) for which the stock price was less than
//! or equal to the price of that day.
//!
//! ## Why This Matters in Rust
//!
//! This problem is a textbook example of using a **monotonic stack**. In Rust, it demonstrates
//! how to manage state across method calls by encapsulating a `Vec` inside a custom `struct`.
//! It also highlights the idiomatic use of `while let` with `last()` or `pop()` for clean,
//! expressive stack manipulation, avoiding the manual index tracking often seen in C++ or Java.
//!
//! ## Approach
//!
//! A naive approach would be to store all prices in a list and, for each new price, iterate
//! backward to count how many consecutive days have a lower or equal price. This is O(n) per
//! operation, making it too slow for large inputs.
//!
//! The optimal approach uses a **decreasing monotonic stack**. The stack stores pairs of
//! `(price, span)`. When a new price arrives:
//! 1. We initialize the current span to 1 (representing the current day).
//! 2. While the stack is not empty and the top price is less than or equal to the new price,
//!    we pop the top element and add its span to our current span.
//! 3. We push the new `(price, span)` pair onto the stack and return the span.
//!
//! This amortizes the cost to O(1) time per `next` call, as each element is pushed and
//! popped at most once over the lifetime of the spanner.
//!
//! We provide two implementations:
//! 1. **Brute Force (`StockSpannerBruteForce`)**: Simple Vec-based history, O(n) per call.
//! 2. **Optimal (`StockSpanner`)**: Monotonic stack approach, amortized O(1) per call.

/// Brute Force Approach
///
/// Time: O(n) per `next()` call, where n is the number of prices seen so far.
/// Space: O(n) to store all historical prices.
#[derive(Default)]
pub struct StockSpannerBruteForce {
    prices: Vec<i32>,
}

impl StockSpannerBruteForce {
    pub fn new() -> Self {
        Self { prices: Vec::new() }
    }

    pub fn next(&mut self, price: i32) -> i32 {
        self.prices.push(price);
        let mut span = 0;

        // Iterate backward using `rev()` on the range.
        for i in (0..self.prices.len()).rev() {
            if self.prices[i] <= price {
                span += 1;
            } else {
                break;
            }
        }

        span
    }
}

/// Optimal Approach: Monotonic Stack
///
/// Time: Amortized O(1) per `next()` call. Each element is pushed and popped at most once.
/// Space: O(n) in the worst case (e.g., strictly decreasing prices).
#[derive(Default)]
pub struct StockSpanner {
    // RUST INSIGHT: We use a tuple `(price, span)` to store both pieces of data together.
    // This is more idiomatic and cache-friendly than parallel arrays.
    stack: Vec<(i32, i32)>,
}

impl StockSpanner {
    pub fn new() -> Self {
        Self { stack: Vec::new() }
    }

    pub fn next(&mut self, price: i32) -> i32 {
        let mut span = 1;

        // RUST INSIGHT: `while let` combined with `last()` allows us to peek at the top
        // of the stack without mutating it yet. This is safer than direct indexing.
        while let Some(&(top_price, top_span)) = self.stack.last() {
            if top_price <= price {
                // GOTCHA: We only pop *after* checking the condition. This avoids
                // accidentally removing an element that is strictly greater.
                self.stack.pop();
                span += top_span;
            } else {
                // The stack is monotonic, so if the top price is greater, all elements
                // below it are also greater (or have already been consumed).
                break;
            }
        }

        self.stack.push((price, span));
        span
    }
}

/// ## Alternative Approaches
///
/// - **Index-based Monotonic Stack**: Instead of storing the span explicitly, you can store
///   `(price, index)`. When a new price comes in at `current_index`, the span is
///   `current_index - stack.last().unwrap_or(-1)`. This uses the same memory but slightly
///   changes the math. The explicit `(price, span)` approach is often easier to reason about.

#[cfg(test)]
mod tests {
    use super::*;

    // Helper macro to test both implementations
    macro_rules! test_spanner {
        ($spanner:ty) => {
            let mut spanner = <$spanner>::new();
            assert_eq!(spanner.next(100), 1);
            assert_eq!(spanner.next(80), 1);
            assert_eq!(spanner.next(60), 1);
            assert_eq!(spanner.next(70), 2);
            assert_eq!(spanner.next(60), 1);
            assert_eq!(spanner.next(75), 4);
            assert_eq!(spanner.next(85), 6);
        };
    }

    #[test]
    fn test_happy_path() {
        test_spanner!(StockSpannerBruteForce);
        test_spanner!(StockSpanner);
    }

    #[test]
    fn test_strictly_decreasing() {
        // Every new price is lower, span is always 1
        let mut spanner = StockSpanner::new();
        assert_eq!(spanner.next(10), 1);
        assert_eq!(spanner.next(9), 1);
        assert_eq!(spanner.next(8), 1);
        assert_eq!(spanner.next(7), 1);
    }

    #[test]
    fn test_strictly_increasing() {
        // Every new price is higher, span keeps growing
        let mut spanner = StockSpanner::new();
        assert_eq!(spanner.next(1), 1);
        assert_eq!(spanner.next(2), 2);
        assert_eq!(spanner.next(3), 3);
        assert_eq!(spanner.next(4), 4);
    }

    #[test]
    fn test_same_prices() {
        let mut spanner = StockSpanner::new();
        assert_eq!(spanner.next(10), 1);
        assert_eq!(spanner.next(10), 2);
        assert_eq!(spanner.next(10), 3);
    }

    #[test]
    fn test_stress_boundary() {
        // A large number of identical prices to ensure no stack overflow or quadratic behavior
        let mut spanner = StockSpanner::new();
        for i in 1..=10_000 {
            assert_eq!(spanner.next(50), i);
        }
    }
}
