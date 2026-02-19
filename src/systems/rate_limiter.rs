//! # Rate Limiter Implementation
//!
//! Mechanisms to control the rate of traffic sent or received.
//!
//! **Replaces Crates:** `governor`, `ratelimit_meter`
//!
//! **Real-world Usage:**
//! - API Gateways (Kong, Nginx) to prevent abuse.
//! - Microservices to prevent cascading failures (load shedding).
//! - TCP congestion control.
//! - Distributed throttling in cloud systems.
//!
//! **Why build it yourself?**
//! Implementing Token Bucket correctly requires careful handling of time and floating point math.
//! Sliding Window logs teach you about time-series data management.
//! Distributed rate limiting forces you to think about atomicity and race conditions across network boundaries.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// =========================================================================================
// 1. Token Bucket (Single Node)
// =========================================================================================

// Algorithm: Token Bucket (Lazy Refill)
//
// Bucket starts full.
// Tokens drip into the bucket at `refill_rate` (tokens/sec).
// Requests consume `n` tokens.
// If not enough tokens, request is rejected.

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

// =========================================================================================
// 2. Sliding Window Log (Single Node)
// =========================================================================================

// Algorithm: Sliding Window Log
//
// Keeps a log of timestamps for each request.
// When a new request comes in:
// 1. Remove timestamps older than the window.
// 2. Count remaining timestamps.
// 3. If count < limit, add new timestamp and allow.
//
// Pros: Highly accurate.
// Cons: High memory usage (stores one entry per request).

pub struct SlidingWindowRateLimiter {
    // Stores timestamps of successful requests.
    // In a real system, this might be a HashMap<Key, VecDeque<Instant>> for multiple users.
    // Here we implement a single limiter instance.
    log: Mutex<VecDeque<Instant>>,
    window: Duration,
    limit: usize,
}

impl SlidingWindowRateLimiter {
    pub fn new(window: Duration, limit: usize) -> Self {
        Self {
            log: Mutex::new(VecDeque::new()),
            window,
            limit,
        }
    }

    pub fn try_acquire(&self) -> bool {
        let mut log = self.log.lock().unwrap();
        let now = Instant::now();
        let window_start = now.checked_sub(self.window);

        // 1. Remove expired entries only if window start is valid (not before process start)
        if let Some(start) = window_start {
            while let Some(&t) = log.front() {
                if t < start {
                    log.pop_front();
                } else {
                    break;
                }
            }
        }

        // 2. Check limit
        if log.len() < self.limit {
            log.push_back(now);
            true
        } else {
            false
        }
    }
}

// =========================================================================================
// 3. Distributed Rate Limiter
// =========================================================================================

// Abstraction for a distributed store (like Redis).
pub trait RateLimitStore: Send + Sync {
    /// Checks if the request is allowed and updates the count/log atomically.
    ///
    /// # Arguments
    /// * `key` - Unique identifier for the limit bucket (e.g., "ip:127.0.0.1").
    /// * `window` - The time window for the limit.
    /// * `limit` - Max requests in the window.
    ///
    /// Returns `Ok(true)` if allowed, `Ok(false)` if limited.
    fn check_and_update(&self, key: &str, window: Duration, limit: usize) -> Result<bool, String>;
}

/// A local in-memory implementation of the store (for testing/single-node usage).
pub struct MemoryStore {
    // Map of Key -> List of timestamps (Sliding Window Log approach)
    logs: Mutex<HashMap<String, VecDeque<Instant>>>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self {
            logs: Mutex::new(HashMap::new()),
        }
    }
}

impl RateLimitStore for MemoryStore {
    fn check_and_update(&self, key: &str, window: Duration, limit: usize) -> Result<bool, String> {
        let mut logs = self.logs.lock().unwrap();
        let log = logs.entry(key.to_string()).or_insert_with(VecDeque::new);

        let now = Instant::now();
        // Since Instant is monotonic but not absolute, this works for single process.
        // For distributed, we'd use SystemTime or a Redis server time.
        let window_start = now.checked_sub(window);

        // Remove expired
        if let Some(start) = window_start {
            while let Some(&t) = log.front() {
                if t < start {
                    log.pop_front();
                } else {
                    break;
                }
            }
        }

        if log.len() < limit {
            log.push_back(now);
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

/// A Distributed Rate Limiter that uses a backend store.
pub struct DistributedRateLimiter<S: RateLimitStore> {
    store: Arc<S>,
}

impl<S: RateLimitStore> DistributedRateLimiter<S> {
    pub fn new(store: S) -> Self {
        Self {
            store: Arc::new(store),
        }
    }

    pub fn try_acquire(&self, key: &str, window: Duration, limit: usize) -> bool {
        match self.store.check_and_update(key, window, limit) {
            Ok(allowed) => allowed,
            Err(e) => {
                // Fail-open or Fail-closed strategy?
                // Usually fail-open (allow traffic) if Redis is down to avoid outage.
                eprintln!("Rate limiter store error: {}", e);
                true
            }
        }
    }
}

// RUST INSIGHT:
// Implementing a RedisStore would look like this:
//
// impl RateLimitStore for RedisStore {
//     fn check_and_update(&self, key: &str, window: Duration, limit: usize) -> Result<bool, String> {
//         // Use Lua script to ensure atomicity:
//         // local current = redis.call('LLEN', KEYS[1])
//         // if current < limit then
//         //     redis.call('RPUSH', KEYS[1], ARGV[1]) // Push timestamp
//         //     redis.call('PEXPIRE', KEYS[1], ARGV[2]) // Set expiry to window
//         //     return 1
//         // else
//         //     return 0
//         // end
//         // (Note: Real sliding window in Redis usually uses ZSET / Sorted Sets)
//         todo!("Implement using redis crate")
//     }
// }

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_token_bucket_basic() {
        let limiter = RateLimiter::new(10.0, 1.0);
        assert!(limiter.try_acquire(10.0));
        assert!(!limiter.try_acquire(1.0));
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

    #[test]
    fn test_sliding_window_basic() {
        let limiter = SlidingWindowRateLimiter::new(Duration::from_millis(100), 2);
        assert!(limiter.try_acquire());
        assert!(limiter.try_acquire());
        assert!(!limiter.try_acquire()); // Limit reached

        thread::sleep(Duration::from_millis(110));
        assert!(limiter.try_acquire()); // Window slid
    }

    #[test]
    fn test_distributed_memory_store() {
        let store = MemoryStore::new();
        let limiter = DistributedRateLimiter::new(store);

        let key = "user_1";
        let window = Duration::from_millis(100);
        let limit = 2;

        assert!(limiter.try_acquire(key, window, limit));
        assert!(limiter.try_acquire(key, window, limit));
        assert!(!limiter.try_acquire(key, window, limit));

        thread::sleep(Duration::from_millis(110));
        assert!(limiter.try_acquire(key, window, limit));
    }

    #[test]
    fn test_distributed_independent_keys() {
        let store = MemoryStore::new();
        let limiter = DistributedRateLimiter::new(store);

        let window = Duration::from_millis(100);
        let limit = 1;

        assert!(limiter.try_acquire("A", window, limit));
        assert!(!limiter.try_acquire("A", window, limit));

        // B should still be allowed
        assert!(limiter.try_acquire("B", window, limit));
    }
}
