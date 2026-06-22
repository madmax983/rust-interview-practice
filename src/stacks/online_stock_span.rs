//! # 901. Online Stock Span
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/online-stock-span/>
//!
//! Design a class that collects daily price quotes for some stock and returns the span
//! of that stock's price for the current day.
//!
//! The span of the stock's price in one day is the maximum number of consecutive days
//! (starting from that day and going backward) for which the stock price was less than
//! or equal to the price of that day.
//!
//! This problem naturally fits a monotonic stack. In Rust, it provides an excellent
//! opportunity to practice state management within a `struct` across method calls,
//! and idiomatic stack manipulation using `Vec` and `while let`.
//!
//! ## Examples
//!
//! ```
//! // Omitted for brevity, see tests.
//! ```
//!
//! ## Constraints
//!
//! - `1 <= price <= 10^5`
//! - At most `10^4` calls will be made to `next`.

/// The `StockSpanner` manages the state of the monotonic stack.
///
/// We store tuples of `(price, span)`. The stack maintains prices in strictly
/// decreasing order. If we see a new price that is greater than or equal to the
/// price at the top of the stack, we pop the top element and accumulate its span.
///
/// Time: Amortized O(1) per `next` call. Each element is pushed and popped at most once.
/// Space: O(N) where N is the number of calls to `next`, in the worst case (strictly decreasing prices).
#[derive(Default)]
pub struct StockSpanner {
    // Stack stores (price, span)
    stack: Vec<(i32, i32)>,
}

impl StockSpanner {
    /// Initializes the object with an empty stack.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the span of the stock's price for the given day.
    ///
    /// RUST INSIGHT: We use `while let` with `self.stack.last()` to peek at the top
    /// of the stack. Because `pop()` takes mutable access, we can't easily chain it
    /// directly in a condition without borrowing issues if we also wanted to just peek.
    /// However, checking the condition first and then popping is safe and clear.
    pub fn next(&mut self, price: i32) -> i32 {
        let mut span = 1;

        while let Some(&(top_price, top_span)) = self.stack.last() {
            if top_price <= price {
                self.stack.pop();
                span += top_span;
            } else {
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
    fn test_duplicates() {
        let mut spanner = StockSpanner::new();
        assert_eq!(spanner.next(10), 1);
        assert_eq!(spanner.next(10), 2);
        assert_eq!(spanner.next(10), 3);
    }
}
