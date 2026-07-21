//! # 901. Online Stock Span
//!
//! Difficulty: Medium
//! Link: <https://leetcode.com/problems/online-stock-span/>
//!
//! Why this matters in Rust: This problem perfectly demonstrates the power of a monotonic stack
//! to optimize O(n^2) brute force solutions down to amortized O(1) per operation. It showcases
//! idiomatic Rust patterns like custom structs for state management, leveraging `Vec` as a stack,
//! and elegant loop constructs like `while let`.
//!
//! ## Approach
//!
//! We need to calculate the "span" of a stock's price, which is the maximum number of consecutive
//! days (starting from today and going backward) for which the stock price was less than or equal
//! to today's price.
//!
//! **Brute Force:**
//! Store all prices in a simple vector. On each `next(price)` call, iterate backward from the
//! most recent price and count how many are `<= price`.
//! - Time Complexity: O(n) per `next()` call, leading to O(n^2) total for `n` calls.
//! - Space Complexity: O(n) to store all prices.
//!
//! **Optimal (Monotonic Stack):**
//! We can maintain a strictly decreasing stack of `(price, span)` tuples. When a new price comes
//! in, we pop all elements from the stack that are less than or equal to the new price,
//! accumulating their spans into the current day's span.
//! Because every element is pushed and popped at most once, the amortized time complexity is O(1)
//! per call.
//! - Time Complexity: Amortized O(1) per `next()` call (O(n) total for `n` calls).
//! - Space Complexity: O(n) in the worst case (e.g., strictly decreasing prices).
//!
//! Why idiomatic Rust: We use `Vec` to represent the stack, capitalizing on its efficient O(1)
//! `push` and `pop` operations at the end. We use `while let Some(...) = stack.last()` combined
//! with `stack.pop()` for clean and safe stack manipulation without manual index tracking.
//!
//! ## Alternative approaches
//!
//! One alternative is maintaining an array of span jumps, similar to a skip list or tree,
//! which could jump directly to the previous greater element. However, the monotonic stack
//! provides the simplest and most performant solution.

/// Brute force approach: Store all prices and iterate backward.
pub struct StockSpannerBruteForce {
    prices: Vec<i32>,
}

impl StockSpannerBruteForce {
    #[must_use]
    pub const fn new() -> Self {
        Self { prices: Vec::new() }
    }

    // GOTCHA: LeetCode requires a `next(&mut self, price: i32) -> i32` method. This name clashes
    // with the standard `Iterator` trait's `next` method, which takes no arguments and returns `Option<Item>`.
    // We suppress the warning here to maintain signature compatibility for the platform.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self, price: i32) -> i32 {
        self.prices.push(price);
        let mut span = 0;

        // Iterate backward using standard iterator chaining.
        // RUST INSIGHT: `.iter().rev()` is a zero-cost abstraction that creates an iterator
        // yielding elements from back to front without allocating a reversed copy.
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

impl Default for StockSpannerBruteForce {
    fn default() -> Self {
        Self::new()
    }
}

/// Optimal approach: Monotonic stack.
/// Stores tuples of `(price, span)`.
pub struct StockSpannerOptimal {
    stack: Vec<(i32, i32)>,
}

impl StockSpannerOptimal {
    #[must_use]
    pub const fn new() -> Self {
        Self { stack: Vec::new() }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self, price: i32) -> i32 {
        let mut current_span = 1;

        // RUST INSIGHT: `while let Some(&last) = self.stack.last()` safely peeks at the top
        // of the stack. We can then decide whether to pop it. This avoids out-of-bounds panics
        // and cleanly handles the empty stack case.
        while let Some(&(last_price, last_span)) = self.stack.last() {
            if last_price <= price {
                current_span += last_span;
                self.stack.pop(); // Pop elements smaller than or equal to current price
            } else {
                break;
            }
        }

        self.stack.push((price, current_span));
        current_span
    }
}

impl Default for StockSpannerOptimal {
    fn default() -> Self {
        Self::new()
    }
}

/// Main entry point alias for the optimal solution, `StockSpanner`.
pub type StockSpanner = StockSpannerOptimal;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force_happy_path() {
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
    fn test_edge_case_strictly_increasing() {
        let mut spanner_bf = StockSpannerBruteForce::new();
        let mut spanner_opt = StockSpannerOptimal::new();
        let prices = [10, 20, 30, 40, 50];

        for (i, &p) in prices.iter().enumerate() {
            let expected_span = i32::try_from(i + 1).unwrap();
            assert_eq!(spanner_bf.next(p), expected_span);
            assert_eq!(spanner_opt.next(p), expected_span);
        }
    }

    #[test]
    fn test_edge_case_strictly_decreasing() {
        let mut spanner_bf = StockSpannerBruteForce::new();
        let mut spanner_opt = StockSpannerOptimal::new();
        let prices = [50, 40, 30, 20, 10];

        for &p in &prices {
            assert_eq!(spanner_bf.next(p), 1);
            assert_eq!(spanner_opt.next(p), 1);
        }
    }

    #[test]
    fn test_edge_case_same_prices() {
        let mut spanner_opt = StockSpannerOptimal::new();
        assert_eq!(spanner_opt.next(10), 1);
        assert_eq!(spanner_opt.next(10), 2);
        assert_eq!(spanner_opt.next(10), 3);
    }

    #[test]
    fn test_stress_boundary() {
        let mut spanner = StockSpannerOptimal::new();
        // Insert a bunch of elements that should be compressed
        for i in 0..1000 {
            assert_eq!(spanner.next(10), i + 1);
        }
        // One large element compresses them all
        assert_eq!(spanner.next(100), 1001);
    }
}
