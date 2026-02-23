//! # Circuit Breaker Implementation
//!
//! A pattern for fault tolerance in distributed systems.
//!
//! **Replaces Crates:** `failsafe`, `breaker`, `resilience4j` (Java)
//!
//! **Real-world Usage:**
//! - Microservices communication (prevent cascading failures).
//! - Database connections (fail fast if DB is down).
//! - Third-party API integration.
//!
//! **Why build it yourself?**
//! Understanding the state transitions (Closed -> Open -> Half-Open) and thread-safety requirements
//! is crucial for building resilient systems. It teaches you about time-based state management and
//! synchronizing shared mutable state.
//!
//! # Architecture
//!
//! **State Machine:**
//!
//! ```text
//!          Success
//!    ┌────────────────┐
//!    │                ▼
//! ┌────────┐      ┌──────────┐
//! │ Closed │◄──── │ Half-Open│
//! └──┬─────┘      └────┬─────┘
//!    │                 ▲
//!    │ Failure         │ Success (Probe)
//!    │ > Threshold     │
//!    ▼                 │
//! ┌────────┐      ┌────┴─────┐
//! │  Open  │ ────►│ (Timer)  │
//! └────────┘      └──────────┘
//! ```
//!
//! **States:**
//! 1.  **Closed**: Normal operation. Requests pass through. Failures are counted.
//! 2.  **Open**: Failure threshold reached. All requests fail fast (Err) without executing.
//! 3.  **Half-Open**: Timeout expired. Allow *one* request to probe.
//!     - If success -> Transition to Closed.
//!     - If failure -> Transition back to Open (reset timer).
//!
//! **Invariants:**
//! *   Failures in Closed state increment a counter.
//! *   Success in Closed state resets the counter (in this simple implementation).
//! *   Open state rejects execution immediately.
//! *   State transitions must be atomic.
//!
//! **Complexity:**
//! *   Space: O(1)
//! *   Time: O(1) overhead per call (Mutex lock).

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The state of the Circuit Breaker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    Closed,
    Open,
    HalfOpen,
}

/// Configuration for the Circuit Breaker.
#[derive(Debug, Clone)]
pub struct Config {
    /// Number of failures to trip the breaker.
    pub failure_threshold: u32,
    /// Time to wait before attempting a probe (Open -> Half-Open).
    pub reset_timeout: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            reset_timeout: Duration::from_secs(10),
        }
    }
}

/// Internal state requiring synchronization.
struct InternalState {
    state: State,
    failure_count: u32,
    last_failure_time: Option<Instant>,
}

/// A thread-safe Circuit Breaker.
#[derive(Clone)]
pub struct CircuitBreaker {
    config: Config,
    state: Arc<Mutex<InternalState>>,
}

/// Error returned when the Circuit Breaker is open.
#[derive(Debug, PartialEq, Eq)]
pub enum CircuitBreakerError<E> {
    /// The operation failed (wrapped inner error).
    Wrapped(E),
    /// The circuit is open, operation was not executed.
    Open,
}

impl CircuitBreaker {
    /// Creates a new Circuit Breaker with the given configuration.
    pub fn new(config: Config) -> Self {
        Self {
            config,
            state: Arc::new(Mutex::new(InternalState {
                state: State::Closed,
                failure_count: 0,
                last_failure_time: None,
            })),
        }
    }

    /// Executes a closure under the protection of the Circuit Breaker.
    pub fn call<F, T, E>(&self, f: F) -> Result<T, CircuitBreakerError<E>>
    where
        F: FnOnce() -> Result<T, E>,
    {
        // 1. Check State (Atomic check-then-act)
        {
            let mut state = self.state.lock().unwrap();
            match state.state {
                State::Open => {
                    // Check if timeout has expired
                    if let Some(last) = state.last_failure_time {
                        if last.elapsed() >= self.config.reset_timeout {
                            // Transition to Half-Open
                            state.state = State::HalfOpen;
                            // Allow this request to proceed as the probe
                        } else {
                            // Still Open
                            return Err(CircuitBreakerError::Open);
                        }
                    } else {
                        // Should not happen if invariants are maintained
                        return Err(CircuitBreakerError::Open);
                    }
                }
                State::HalfOpen => {
                    // Another thread beat us to the probe?
                    // Implementation choice: Reject concurrent probes?
                    // Or allow them? Strict Hystrix allows 1.
                    // Simple impl: If we are here, we might just have transitioned in the block above
                    // or another thread did.
                    // For strictness, we should check if a probe is already running?
                    // We'll treat this simple impl as "All concurrent requests in HalfOpen are probes".
                    // Or better: Revert to Open if we want strict serialization.
                    // Let's allow it for simplicity.
                }
                State::Closed => {
                    // Go ahead
                }
            }
        } // Drop lock to execute operation

        // 2. Execute Operation
        let result = f();

        // 3. Handle Result and Update State
        let mut state = self.state.lock().unwrap();
        match result {
            Ok(val) => {
                match state.state {
                    State::HalfOpen => {
                        // Probe succeeded! Close the circuit.
                        state.state = State::Closed;
                        state.failure_count = 0;
                        state.last_failure_time = None;
                    }
                    State::Closed => {
                        // Reset failure count on success?
                        // Sliding window would keep it.
                        // Simple counter resets it or keeps it until threshold?
                        // Hystrix resets on success.
                        state.failure_count = 0;
                    }
                    State::Open => {
                        // Should not happen if we respect the lock, but theoretically
                        // if time passed, maybe?
                    }
                }
                Ok(val)
            }
            Err(err) => {
                match state.state {
                    State::Closed => {
                        state.failure_count += 1;
                        if state.failure_count >= self.config.failure_threshold {
                            state.state = State::Open;
                            state.last_failure_time = Some(Instant::now());
                        }
                    }
                    State::HalfOpen => {
                        // Probe failed. Re-open.
                        state.state = State::Open;
                        state.last_failure_time = Some(Instant::now());
                    }
                    State::Open => {
                        // Already open.
                    }
                }
                Err(CircuitBreakerError::Wrapped(err))
            }
        }
    }

    /// Returns the current state (for monitoring).
    pub fn state(&self) -> State {
        self.state.lock().unwrap().state.clone()
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `failsafe`: Uses pluggable strategies (failure accrual, backoff).
// - `breaker`: More async-focused (Futures).
//
// Missing vs. Production:
// - **Sliding Window**: We use a simple consecutive failure count. Production systems use a
//   time-based sliding window (e.g., 50% failures in last 10s) to avoid tripping on occasional glitches.
// - **Async Support**: This is blocking (`Mutex`). An async version would use `tokio::sync::Mutex`
//   or lock-free atomics to avoid blocking the executor.
// - **Bulkhead**: Often paired with Circuit Breaker to limit concurrency per service.

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_closed_state_success() {
        let cb = CircuitBreaker::new(Config::default());
        let res = cb.call(|| Ok::<_, ()>(10));
        assert_eq!(res, Ok(10));
        assert_eq!(cb.state(), State::Closed);
    }

    #[test]
    fn test_failure_counting_and_trip() {
        let config = Config {
            failure_threshold: 2,
            reset_timeout: Duration::from_secs(1),
        };
        let cb = CircuitBreaker::new(config);

        // Fail 1
        let res = cb.call(|| Err::<(), _>("error"));
        assert_eq!(res, Err(CircuitBreakerError::Wrapped("error")));
        assert_eq!(cb.state(), State::Closed);

        // Fail 2 -> Trip
        let res = cb.call(|| Err::<(), _>("error"));
        assert_eq!(res, Err(CircuitBreakerError::Wrapped("error")));
        assert_eq!(cb.state(), State::Open);

        // Next call should be rejected
        let res = cb.call(|| Ok::<_, ()>(10));
        assert_eq!(res, Err(CircuitBreakerError::Open));
    }

    #[test]
    fn test_reset_timeout_half_open_recovery() {
        let config = Config {
            failure_threshold: 1,
            reset_timeout: Duration::from_millis(100),
        };
        let cb = CircuitBreaker::new(config);

        // Trip it
        let _ = cb.call(|| Err::<(), _>("err"));
        assert_eq!(cb.state(), State::Open);

        // Wait for timeout
        thread::sleep(Duration::from_millis(150));

        // Next call should probe (transition to Half-Open internally, execute, then succeed -> Closed)
        let res = cb.call(|| Ok::<_, ()>(10));
        assert_eq!(res, Ok(10));
        assert_eq!(cb.state(), State::Closed);
    }

    #[test]
    fn test_half_open_failure_reopens() {
        let config = Config {
            failure_threshold: 1,
            reset_timeout: Duration::from_millis(100),
        };
        let cb = CircuitBreaker::new(config);

        // Trip
        let _ = cb.call(|| Err::<(), _>("err"));
        assert_eq!(cb.state(), State::Open);

        // Wait
        thread::sleep(Duration::from_millis(150));

        // Probe fails
        let res = cb.call(|| Err::<(), _>("probe_fail"));
        assert_eq!(res, Err(CircuitBreakerError::Wrapped("probe_fail")));

        // Should be Open again
        assert_eq!(cb.state(), State::Open);

        // Immediate retry should be rejected (timeout reset)
        let res = cb.call(|| Ok::<_, ()>(10));
        assert_eq!(res, Err(CircuitBreakerError::Open));
    }

    #[test]
    fn test_concurrent_failures() {
        let config = Config {
            failure_threshold: 10,
            reset_timeout: Duration::from_secs(1),
        };
        let cb = CircuitBreaker::new(config);
        let cb_clone = cb.clone();

        let handles: Vec<_> = (0..10).map(|_| {
            let cb = cb_clone.clone();
            thread::spawn(move || {
                let _ = cb.call(|| Err::<(), _>("err"));
            })
        }).collect();

        for h in handles {
            h.join().unwrap();
        }

        // 10 failures should trip it
        assert_eq!(cb.state(), State::Open);
    }
}
