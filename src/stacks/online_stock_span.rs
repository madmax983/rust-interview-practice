//! # 901. Online Stock Span
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/online-stock-span/>
//!
//! Design an algorithm that collects daily price quotes for some stock and returns the span of that stock's price for the current day.
//! The span of the stock's price in one day is the maximum number of consecutive days (starting from that day and going backward)
//! for which the stock price was less than or equal to the price of that day.
//!
//! This problem is a textbook use case for a monotonic stack. By implementing it in a custom struct, we demonstrate Rust's type
//! system as a modeling tool. The solution highlights idiomatic Rust patterns like using `while let` with `last()` and `pop()`
//! for safe stack manipulation.
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

/// Represents a single stock price entry in the stack.
/// RUST INSIGHT: We use a small struct to bind the price and its span tightly together,
/// making the code self-documenting and easier to reason about than a generic tuple `(i32, i32)`.
#[derive(Debug, Clone, Copy)]
struct StockEntry {
    price: i32,
    span: i32,
}

/// A monotonic stack implementation for calculating stock spans online.
/// We maintain a strictly decreasing stack of `StockEntry` items.
pub struct StockSpanner {
    stack: Vec<StockEntry>,
}

impl Default for StockSpanner {
    fn default() -> Self {
        Self::new()
    }
}

impl StockSpanner {
    /// Initializes the object with an empty stack.
    #[must_use]
    pub fn new() -> Self {
        Self { stack: Vec::new() }
    }

    /// Returns the span of the stock's price for the current day.
    ///
    /// Time Complexity: Amortized O(1) per call. While the `while let` loop might pop multiple elements,
    /// each element is pushed and popped at most once across all calls to `next`.
    /// Space Complexity: O(N) where N is the number of calls to `next`, in the worst-case scenario
    /// where prices are strictly decreasing.
    pub fn next(&mut self, price: i32) -> i32 {
        let mut span = 1;

        // RUST INSIGHT: `while let` with `self.stack.last()` is an idiomatic way to peek at the
        // top of the stack safely without removing the element. If the stack is empty, `last()` returns `None`
        // and the loop breaks automatically.
        // We only pop when we confirm the price condition is met.
        while let Some(top) = self.stack.last() {
            if top.price <= price {
                // GOTCHA: We unwrap here, but we are guaranteed it won't panic because
                // `last()` just returned `Some`, so the stack is not empty.
                span += self.stack.pop().unwrap().span;
            } else {
                // The stack is monotonic strictly decreasing, so we can stop looking.
                break;
            }
        }

        self.stack.push(StockEntry { price, span });
        span
    }
}

// Alternative Approaches:
// 1. **Brute Force Array**: Keep an array of all prices seen so far, and on every `next()` call,
//    iterate backwards to count the span. This is O(N) time per call, which is too slow.
// 2. **Tuple Stack**: Instead of a custom `StockEntry` struct, use a `Vec<(i32, i32)>`. This is
//    functionally identical but less expressive.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_happy_path() {
        let mut spanner = StockSpanner::new();
        // Prices: [100, 80, 60, 70, 60, 75, 85]
        assert_eq!(spanner.next(100), 1);
        assert_eq!(spanner.next(80), 1);
        assert_eq!(spanner.next(60), 1);
        assert_eq!(spanner.next(70), 2); // 70 > 60, so span is 1 (for 70) + 1 (for 60) = 2
        assert_eq!(spanner.next(60), 1);
        assert_eq!(spanner.next(75), 4); // 75 > 60, 70, 60, so span is 1 + 1 + 2 = 4
        assert_eq!(spanner.next(85), 6); // 85 > 75, 60, 70, 60, 80, so span is 1 + 4 + 1 = 6
    }

    #[test]
    fn test_strictly_decreasing() {
        let mut spanner = StockSpanner::new();
        // In a strictly decreasing sequence, the span is always 1
        assert_eq!(spanner.next(10), 1);
        assert_eq!(spanner.next(9), 1);
        assert_eq!(spanner.next(8), 1);
        assert_eq!(spanner.next(7), 1);
    }

    #[test]
    fn test_strictly_increasing() {
        let mut spanner = StockSpanner::new();
        // In a strictly increasing sequence, the span grows with each step
        assert_eq!(spanner.next(10), 1);
        assert_eq!(spanner.next(20), 2);
        assert_eq!(spanner.next(30), 3);
        assert_eq!(spanner.next(40), 4);
    }

    #[test]
    fn test_same_prices() {
        let mut spanner = StockSpanner::new();
        // Prices are equal, which counts towards the span
        assert_eq!(spanner.next(10), 1);
        assert_eq!(spanner.next(10), 2);
        assert_eq!(spanner.next(10), 3);
        assert_eq!(spanner.next(10), 4);
    }
}
