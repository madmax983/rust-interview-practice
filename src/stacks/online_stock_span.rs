//! # 901. Online Stock Span
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/online-stock-span/>
//!
//! Why this matters in Rust: This problem perfectly illustrates state management
//! using a custom struct holding a `Vec` as a stack. It demonstrates how to safely
//! and efficiently maintain monotonic properties using Rust's pattern matching
//! (`while let`) on `Option` types returned by stack operations (`last()`, `pop()`).
//!
//! ## Approach
//!
//! The naive approach is to store all prices and count backwards each time `next` is called.
//! This takes O(n) per query, leading to O(n²) overall.
//!
//! The optimal approach uses a **monotonic stack**. We store pairs of `(price, span)`.
//! We maintain the invariant that prices in the stack are strictly decreasing.
//! When a new price comes in, we pop all elements from the stack that are less than
//! or equal to the current price, accumulating their spans. Then we push the new price
//! and its total accumulated span onto the stack.
//!
//! This ensures that every element is pushed and popped at most once, achieving an
//! amortized O(1) time complexity per `next` call, and O(n) space complexity.
//!
//! Idiomatic reasoning: By encapsulating this logic in a `StockSpanner` struct, we
//! hide the internal representation (a `Vec` acting as a stack) from the caller,
//! providing a clean API. We use `while let` to cleanly handle stack iteration without
//! manual bounds checking.
//!
//! ## Alternative Approaches
//!
//! 1.  **Array + Direct Lookbacks**: Store prices and spans in separate arrays, and jump
//!     back using the stored spans. While this avoids explicit stack operations, it's
//!     essentially implementing a stack on top of an array and is less idiomatic and
//!     harder to read than explicitly modeling the data structure.
//! 2.  **Brute Force Lookback**: Just store prices in a `Vec` and iterate backwards for
//!     each query. Easy to implement but fails performance requirements (O(N^2)).

/// A spanner that calculates the span of stock prices.
#[derive(Debug, Default)]
pub struct StockSpanner {
    /// A stack storing tuples of `(price, span)`.
    /// The prices are strictly decreasing from bottom to top of the stack.
    stack: Vec<(i32, i32)>,
}

impl StockSpanner {
    /// Initializes the `StockSpanner` object.
    #[must_use]
    pub const fn new() -> Self {
        Self { stack: Vec::new() }
    }

    /// Processes a new stock price and returns its span.
    ///
    /// # Arguments
    ///
    /// * `price` - The current stock price.
    ///
    /// # Returns
    ///
    /// The span of the stock's price today.
    pub fn next(&mut self, price: i32) -> i32 {
        let mut span = 1;

        // RUST INSIGHT: `while let` with `last()` allows us to safely peek at the top
        // of the stack without borrowing issues, as we only borrow `stack` immutably
        // for the duration of `last()`.
        while let Some(&(top_price, top_span)) = self.stack.last() {
            // GOTCHA: We must pop elements less than OR EQUAL to the current price.
            if top_price <= price {
                // Safely pop the element now that we know we want to consume it.
                self.stack.pop();
                span += top_span;
            } else {
                // The monotonic property is satisfied, stop popping.
                break;
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
    fn test_stock_spanner_happy_path() {
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
    fn test_stock_spanner_decreasing_prices() {
        let mut spanner = StockSpanner::new();
        // Strictly decreasing, span should always be 1
        assert_eq!(spanner.next(50), 1);
        assert_eq!(spanner.next(40), 1);
        assert_eq!(spanner.next(30), 1);
        assert_eq!(spanner.next(20), 1);
        assert_eq!(spanner.next(10), 1);
    }

    #[test]
    fn test_stock_spanner_increasing_prices() {
        let mut spanner = StockSpanner::new();
        // Strictly increasing, span should accumulate
        assert_eq!(spanner.next(10), 1);
        assert_eq!(spanner.next(20), 2);
        assert_eq!(spanner.next(30), 3);
        assert_eq!(spanner.next(40), 4);
        assert_eq!(spanner.next(50), 5);
    }

    #[test]
    fn test_stock_spanner_same_prices() {
        let mut spanner = StockSpanner::new();
        // Same prices, span should accumulate
        assert_eq!(spanner.next(10), 1);
        assert_eq!(spanner.next(10), 2);
        assert_eq!(spanner.next(10), 3);
    }
}
