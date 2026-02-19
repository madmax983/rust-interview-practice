//! # Rate Limiter Implementation
//!
//! Mechanisms to control the rate of traffic sent or received.
//! This module implements three strategies:
//! 1. **Token Bucket**: Efficient, allows bursts, lazy refill.
//! 2. **Sliding Window Log**: Precise, smooths out bursts, higher memory usage.
//! 3. **Distributed**: Abstracted storage for cluster-wide limiting (e.g., using Redis).
//!
//! **Replaces Crates:** `governor`, `ratelimit_meter`
//!
//! **Real-world Usage:**
//! - API Gateways (Kong, Nginx) to prevent abuse.
//! - Microservices to prevent cascading failures (load shedding).
//! - TCP congestion control.
//!
//! **Why build it yourself?**
//! Implementing Rate Limiters teaches you about:
//! - **Time Management**: Handling monotonic time and durations.
//! - **Concurrency**: Thread-safe state management.
//! - **Distributed Systems**: Handling race conditions (check-and-set) in a shared store.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// =========================================================================================
// Traits
// =========================================================================================

/// A trait for Rate Limiters.
pub trait RateLimiter {
    /// Attempts to acquire `tokens` from the limiter.
    /// Returns `true` if successful, `false` otherwise.
    fn try_acquire(&self, tokens: u32) -> bool;
}

// =========================================================================================
// Strategy 1: Token Bucket (Lazy Refill)
// =========================================================================================
//
// Algorithm:
// Bucket starts full. Tokens drip into the bucket at `refill_rate` (tokens/sec).
// Requests consume `n` tokens. If not enough tokens, request is rejected.
//
// Lazy Refill:
// Instead of a background timer, we calculate tokens added based on the time delta since last check.

struct TokenBucketState {
    tokens: f64,
    last_refill: Instant,
}

/// A thread-safe Token Bucket Rate Limiter.
/// Best for: Allowing bursts but maintaining an average rate.
pub struct TokenBucketRateLimiter {
    state: Mutex<TokenBucketState>,
    capacity: f64,
    refill_rate: f64,
}

impl TokenBucketRateLimiter {
    /// Creates a new Token Bucket Rate Limiter.
    ///
    /// # Arguments
    /// * `capacity` - Maximum number of tokens (burst size).
    /// * `refill_rate` - Tokens added per second.
    pub fn new(capacity: f64, refill_rate: f64) -> Self {
        Self {
            state: Mutex::new(TokenBucketState {
                tokens: capacity,
                last_refill: Instant::now(),
            }),
            capacity,
            refill_rate,
        }
    }

    fn refill(&self, state: &mut TokenBucketState) {
        let now = Instant::now();
        let elapsed = now.duration_since(state.last_refill).as_secs_f64();
        let new_tokens = elapsed * self.refill_rate;

        if new_tokens > 0.0 {
            state.tokens = (state.tokens + new_tokens).min(self.capacity);
            state.last_refill = now;
        }
    }
}

impl RateLimiter for TokenBucketRateLimiter {
    fn try_acquire(&self, tokens_needed: u32) -> bool {
        let mut state = self.state.lock().unwrap();
        self.refill(&mut state);

        let needed = tokens_needed as f64;
        if state.tokens >= needed {
            state.tokens -= needed;
            true
        } else {
            false
        }
    }
}

// =========================================================================================
// Strategy 2: Sliding Window Log
// =========================================================================================
//
// Algorithm:
// Keep a log of timestamps for each request.
// On acquire, remove timestamps older than the window size.
// If count < limit, add new timestamp and allow.
//
// Tradeoff:
// - Precise (no boundary issues like Fixed Window).
// - High memory usage (stores one u64/Instant per request).

/// A thread-safe Sliding Window Log Rate Limiter.
/// Best for: Strict rate enforcement (no bursts beyond limit in window).
pub struct SlidingWindowRateLimiter {
    // We use a Mutex<VecDeque> to store timestamps.
    // In production, for high throughput, this lock contention is bad.
    // Sharding or lock-free structures would be better.
    timestamps: Mutex<VecDeque<Instant>>,
    window: Duration,
    limit: usize,
}

impl SlidingWindowRateLimiter {
    pub fn new(window: Duration, limit: usize) -> Self {
        Self {
            timestamps: Mutex::new(VecDeque::new()),
            window,
            limit,
        }
    }
}

impl RateLimiter for SlidingWindowRateLimiter {
    fn try_acquire(&self, _tokens: u32) -> bool {
        // Note: Sliding Window typically counts "requests", i.e., tokens=1.
        // Supporting variable tokens is harder (need to store weight).
        // For simplicity, we assume tokens=1 here or treat it as 1 request.
        // If tokens > 1, we should probably check if we can add 'tokens' items.

        let mut timestamps = self.timestamps.lock().unwrap();
        let now = Instant::now();
        let cutoff = now - self.window;

        // Remove old entries
        // RUST INSIGHT: VecDeque::pop_front is O(1).
        while let Some(&ts) = timestamps.front() {
            if ts < cutoff {
                timestamps.pop_front();
            } else {
                break;
            }
        }

        if timestamps.len() < self.limit {
            timestamps.push_back(now);
            true
        } else {
            false
        }
    }
}

// =========================================================================================
// Strategy 3: Distributed Rate Limiter
// =========================================================================================
//
// Abstraction for an external storage (like Redis).
// We use a trait `Storage` to allow mocking or real implementation.

/// Trait for the backing store of a distributed limiter.
pub trait Storage: Send + Sync {
    /// Increments the counter for `key` by `value` and returns the new value.
    /// Should also set expiry if the key is new.
    fn incr_and_expire(&self, key: &str, value: usize, window_secs: u64) -> usize;
}

/// A Distributed Rate Limiter using a fixed window counter approach.
/// (Sliding window log is too expensive for distributed storage usually).
pub struct DistributedRateLimiter<S: Storage> {
    storage: Arc<S>,
    key_prefix: String,
    limit: usize,
    window_secs: u64,
}

impl<S: Storage> DistributedRateLimiter<S> {
    pub fn new(storage: Arc<S>, key_prefix: String, limit: usize, window_secs: u64) -> Self {
        Self {
            storage,
            key_prefix,
            limit,
            window_secs,
        }
    }
}

impl<S: Storage> RateLimiter for DistributedRateLimiter<S> {
    fn try_acquire(&self, _tokens: u32) -> bool {
        // Fixed Window: Key depends on current time window.
        // E.g., "prefix:12345678" where suffix is unix_timestamp / window.
        // GOTCHA: Boundary conditions (spike at end of window + start of next).
        // Production usually uses "Sliding Window Counter" (Current + Previous * weight).
        // We implement simple Fixed Window here for demonstration.

        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let window_idx = now_secs / self.window_secs;
        let key = format!("{}:{}", self.key_prefix, window_idx);

        let count = self.storage.incr_and_expire(&key, 1, self.window_secs);

        count <= self.limit
    }
}

// =========================================================================================
// Mock Storage for Testing
// =========================================================================================

#[cfg(test)]
pub mod mocks {
    use super::*;
    use std::collections::HashMap;

    pub struct InMemoryStorage {
        map: Mutex<HashMap<String, usize>>,
    }

    impl InMemoryStorage {
        pub fn new() -> Self {
            Self {
                map: Mutex::new(HashMap::new()),
            }
        }
    }

    impl Storage for InMemoryStorage {
        fn incr_and_expire(&self, key: &str, value: usize, _window_secs: u64) -> usize {
            let mut map = self.map.lock().unwrap();
            let entry = map.entry(key.to_string()).or_insert(0);
            *entry += value;
            *entry
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::mocks::InMemoryStorage;
    use std::thread;

    #[test]
    fn test_token_bucket() {
        let limiter = TokenBucketRateLimiter::new(10.0, 10.0); // 10 capacity, 10/sec refill
        assert!(limiter.try_acquire(10)); // Drain it
        assert!(!limiter.try_acquire(1)); // Empty
        thread::sleep(Duration::from_millis(110)); // Wait 0.1s -> 1 token
        assert!(limiter.try_acquire(1));
    }

    #[test]
    fn test_sliding_window() {
        let limiter = SlidingWindowRateLimiter::new(Duration::from_millis(100), 2);
        assert!(limiter.try_acquire(1));
        assert!(limiter.try_acquire(1));
        assert!(!limiter.try_acquire(1)); // Full

        thread::sleep(Duration::from_millis(110)); // Window passes
        assert!(limiter.try_acquire(1));
    }

    #[test]
    fn test_distributed_mock() {
        let storage = Arc::new(InMemoryStorage::new());
        let limiter = DistributedRateLimiter::new(
            storage.clone(),
            "api_v1".to_string(),
            5,
            1, // 1 sec window
        );

        for _ in 0..5 {
            assert!(limiter.try_acquire(1));
        }
        assert!(!limiter.try_acquire(1)); // Limit reached
    }
}
