//! # Rate Limiter Implementation (Token Bucket)
//!
//! A mechanism to control the rate of traffic sent or received.
//!
//! **Replaces Crates:** `governor`, `ratelimit_meter`
//!
//! **Real-world Usage:**
//! - API Gateways (Kong, Nginx) to prevent abuse.
//! - Microservices to prevent cascading failures (load shedding).
//! - TCP congestion control.
//!
//! **Why build it yourself?**
//! Implementing Token Bucket correctly requires careful handling of time and floating point math.
//! You'll learn how to "lazy refill" tokens only when a request comes in, avoiding background threads.

use std::sync::Mutex;
use std::time::Instant;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Algorithm: Token Bucket (Lazy Refill)
//
// Bucket starts full.
// Tokens drip into the bucket at `refill_rate` (tokens/sec).
// Requests consume `n` tokens.
// If not enough tokens, request is rejected.
//
// Lazy Refill:
// Instead of a background timer adding tokens every tick, we calculate the tokens added
// based on the time delta since the last check.
//
// State:
// - Capacity (max tokens)
// - Current Tokens
// - Refill Rate (tokens/sec)
// - Last Refill Timestamp
//
// Thread Safety:
// - Mutex protects the state.
//
// Invariants:
// 1. Tokens never exceed Capacity.
// 2. Tokens are consumed atomically.

struct BucketState {
    tokens: f64,
    last_refill: Instant,
}

/// A thread-safe Token Bucket Rate Limiter.
pub struct RateLimiter {
    state: Mutex<BucketState>,
    capacity: f64,
    refill_rate: f64,
}

impl RateLimiter {
    /// Creates a new Rate Limiter.
    ///
    /// # Arguments
    /// * `capacity` - Maximum number of tokens the bucket can hold (burst size).
    /// * `refill_rate` - Number of tokens added per second.
    pub fn new(capacity: f64, refill_rate: f64) -> Self {
        Self {
            state: Mutex::new(BucketState {
                tokens: capacity, // Start full
                last_refill: Instant::now(),
            }),
            capacity,
            refill_rate,
        }
    }

    /// Attempts to acquire `tokens` from the bucket.
    /// Returns `true` if successful, `false` if not enough tokens.
    pub fn try_acquire(&self, tokens_needed: f64) -> bool {
        let mut state = self.state.lock().unwrap();
        self.refill(&mut state);

        if state.tokens >= tokens_needed {
            state.tokens -= tokens_needed;
            true
        } else {
            false
        }
    }

    /// Helper to update the token count based on time elapsed.
    fn refill(&self, state: &mut BucketState) {
        let now = Instant::now();
        let elapsed = now.duration_since(state.last_refill);
        let elapsed_secs = elapsed.as_secs_f64();

        // Calculate tokens to add
        let new_tokens = elapsed_secs * self.refill_rate;

        // Update state
        if new_tokens > 0.0 {
            state.tokens = (state.tokens + new_tokens).min(self.capacity);
            state.last_refill = now;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_basic_limit() {
        let limiter = RateLimiter::new(10.0, 1.0); // 10 tokens, 1 per second

        // Should be able to acquire initial capacity
        assert!(limiter.try_acquire(10.0));

        // Should fail immediately after
        assert!(!limiter.try_acquire(1.0));

        // Wait 1.1 second (should get 1 token)
        thread::sleep(Duration::from_millis(1100));
        assert!(limiter.try_acquire(1.0));
    }

    #[test]
    fn test_burst() {
        let limiter = RateLimiter::new(5.0, 1.0);

        // Consume 5
        assert!(limiter.try_acquire(5.0));
        assert!(!limiter.try_acquire(1.0));

        // Wait 2 seconds -> should have 2 tokens
        thread::sleep(Duration::from_secs(2));
        assert!(limiter.try_acquire(1.0));
        assert!(limiter.try_acquire(1.0));
        assert!(!limiter.try_acquire(1.0));
    }

    #[test]
    fn test_refill_cap() {
        let limiter = RateLimiter::new(5.0, 10.0); // Fills fast

        // Wait long enough to overflow if not capped
        thread::sleep(Duration::from_millis(600)); // 6 tokens generated

        // Should only have 5
        assert!(limiter.try_acquire(5.0));
        assert!(!limiter.try_acquire(0.1));
    }
}
