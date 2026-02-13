//! # Rate Limiter
//!
//! Controls the rate at which actions can be performed.
//! Implements the Token Bucket algorithm with lazy refill.
//!
//! Replaces: `governor`, `ratelimit`
//! Used in: API Gateways (Kong, Nginx), DoS protection, Cloud APIs (AWS Throttling)
//! Why build it: To understand how to smooth out traffic bursts and the mathematics of
//! lazy token refilling without a background thread.
//!
//! ## Architecture
//!
//! The Token Bucket algorithm models a bucket that fills with tokens at a constant rate.
//! Requests consume tokens. If the bucket is empty, the request is denied (or queued, but here denied).
//!
//! ```text
//!       +--------+
//!       |        |  <-- Capacity (max burst)
//!       | Tokens |
//!       |        |
//!       +--------+
//!           ^
//!           | Refill Rate (r tokens/sec)
//!
//! Request -> [Check] -> (Enough tokens?) -> Yes: Consume & Pass
//!                                        -> No: Fail
//! ```
//!
//! ### Invariants
//! 1. `tokens` never exceeds `capacity`.
//! 2. `tokens` is never negative (in this implementation).
//! 3. Refill happens based on time elapsed since `last_refill`.
//!
//! ### Complexity
//!
//! | Operation | Time | Space |
//! |-----------|------|-------|
//! | Check     | O(1) | O(1)  |
//!
//! ### Design Decisions
//!
//! - **Lazy Refill**: Instead of a background thread adding tokens every X ms, we calculate
//!   how many tokens *would* have been added since the last check. This is efficient and precise.
//! - **f64 Precision**: We use `f64` for token counts to handle fractional refills accurately
//!   over small time windows.
//! - **Instant**: Uses `std::time::Instant` for monotonic time tracking.

use std::time::{Duration, Instant};

/// A Token Bucket Rate Limiter.
#[derive(Debug, Clone)]
pub struct RateLimiter {
    /// Maximum number of tokens the bucket can hold.
    capacity: f64,
    /// Current number of tokens in the bucket.
    tokens: f64,
    /// Refill rate in tokens per second.
    refill_rate: f64,
    /// Timestamp of the last refill calculation.
    last_refill: Instant,
}

impl RateLimiter {
    /// Creates a new `RateLimiter`.
    ///
    /// * `capacity`: Maximum burst size (max tokens).
    /// * `refill_rate`: Tokens added per second.
    pub fn new(capacity: f64, refill_rate: f64) -> Self {
        assert!(capacity > 0.0, "Capacity must be positive");
        assert!(refill_rate > 0.0, "Refill rate must be positive");

        Self {
            capacity,
            tokens: capacity, // Start full
            refill_rate,
            last_refill: Instant::now(),
        }
    }

    /// Attempts to consume `amount` tokens.
    ///
    /// Returns `true` if successful (allowed), `false` otherwise (limited).
    pub fn check_n(&mut self, amount: f64) -> bool {
        self.refill();

        if self.tokens >= amount {
            self.tokens -= amount;
            true
        } else {
            false
        }
    }

    /// Attempts to consume 1 token.
    ///
    /// Returns `true` if successful, `false` otherwise.
    pub fn check(&mut self) -> bool {
        self.check_n(1.0)
    }

    /// Refills the bucket based on elapsed time.
    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();

        let new_tokens = elapsed * self.refill_rate;

        if new_tokens > 0.0 {
            // RUST INSIGHT: f64::min to clamp to capacity.
            self.tokens = (self.tokens + new_tokens).min(self.capacity);
            self.last_refill = now;
        }
    }

    /// Returns the current number of tokens (for inspection/metrics).
    pub fn tokens(&self) -> f64 {
        // We assume this is a "peek" and doesn't trigger a refill to keep it const-like?
        // Actually, to get an accurate count, we should calculate what it *would* be.
        // But `tokens` is not &mut self.
        // So we just return the stored value. The "real" value depends on time.
        // Ideally we'd refill here, but that requires mutability.
        // Let's verify if we should support peeking the *effective* tokens.
        // For simplicity, return stored. If user wants up-to-date, they call check (which mutates).
        // Or we could take &mut self here.
        self.tokens
    }
}

// Footer:
// Comparison to canonical crates:
// - `governor`: Uses a sophisticated GCRA (Generic Cell Rate Algorithm) which is more accurate/smooth than Token Bucket.
//   Also supports `async` and multi-threaded operation out of the box.
// - `ratelimit`: Simpler token bucket implementations.
//
// Missing vs Production:
// - Not thread-safe (requires Mutex/Atomic wrappers).
// - No distributed state (Redis/Memcached backend).
// - No wait/block functionality (blocking until tokens are available).

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_rate_limiter_basic() {
        let mut limiter = RateLimiter::new(5.0, 1.0);

        // Should be able to consume 5 immediately
        assert!(limiter.check_n(5.0));

        // Now empty
        assert!(!limiter.check());
    }

    #[test]
    fn test_refill() {
        let mut limiter = RateLimiter::new(1.0, 10.0); // 1 capacity, 10 per sec

        // Consume all
        assert!(limiter.check());
        assert!(!limiter.check());

        // Wait 0.2 seconds (should get 2 tokens, clamped to 1)
        thread::sleep(Duration::from_millis(200));

        // Should be full again
        assert!(limiter.check());
    }

    #[test]
    fn test_partial_refill() {
        // 10 tokens per second. 0.1 tokens per 10ms.
        let mut limiter = RateLimiter::new(10.0, 10.0);
        limiter.check_n(10.0); // Empty it

        thread::sleep(Duration::from_millis(150)); // ~1.5 tokens

        // Should have at least 1 token
        assert!(limiter.check());
        // Remaining ~0.5
        assert!(!limiter.check());
    }
}
