//! # Circuit Breaker Implementation
//!
//! A state machine that prevents an application from repeatedly trying to execute an operation that's likely to fail.
//! It allows the system to fail fast and recover gracefully.
//!
//! **Replaces Crates:** `failsafe`, `breaker`, `sonyflake` (incorrectly cited, but `failsafe` is the main one).
//!
//! **Real-world Usage:**
//! - Microservices communication (preventing cascading failures).
//! - Database connections (failing fast when DB is down).
//! - Third-party API integration.
//!
//! **Why build it yourself?**
//! Implementing a Circuit Breaker teaches you about state machines in concurrent environments.
//! You learn how to balance "failing fast" (Open state) with "self-healing" (Half-Open state),
//! and how to handle time-based transitions safely.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// =========================================================================================
// Architecture
// =========================================================================================
//
// State Machine:
//
//      [Closed] <───────────────┐
//         │                     │
//    Failure Count >= Threshold │ Success
//         │                     │
//         ▼                     │
//       [Open] ────────────────► [Half-Open]
//              Timeout Expired
//
// States:
// - **Closed**: Normal operation. Requests pass through. Failures are counted.
// - **Open**: Service is down. Requests fail immediately (Fail Fast).
// - **Half-Open**: Probation. One request is allowed. If success -> Closed. If fail -> Open.
//
// Invariants:
// 1. In Open state, no requests are executed until timeout expires.
// 2. In Half-Open state, only ONE request is allowed to test the backend.
// 3. State transitions must be atomic.
//
// Thread Safety:
// - Uses `Arc<Mutex<State>>` to share state across threads.
// - RUST INSIGHT: Enums with data (`State::Open(Instant)`) make state-specific data (like timeout) type-safe.

#[derive(Debug, Clone, PartialEq)]
enum State {
    Closed,
    Open(Instant), // Instant when the circuit opened (for timeout calculation)
    HalfOpen,
}

struct InnerState {
    state: State,
    failure_count: usize,
}

pub struct CircuitBreaker {
    state: Arc<Mutex<InnerState>>,
    failure_threshold: usize,
    reset_timeout: Duration,
}

impl CircuitBreaker {
    /// Creates a new Circuit Breaker.
    ///
    /// # Arguments
    /// * `failure_threshold` - Number of failures before opening the circuit.
    /// * `reset_timeout` - Duration to wait before attempting recovery (Open -> Half-Open).
    pub fn new(failure_threshold: usize, reset_timeout: Duration) -> Self {
        Self {
            state: Arc::new(Mutex::new(InnerState {
                state: State::Closed,
                failure_count: 0,
            })),
            failure_threshold,
            reset_timeout,
        }
    }

    /// Executes the given closure within the circuit breaker context.
    ///
    /// The closure must return a `Result<T, E>`.
    /// - `Ok(T)` is considered a success.
    /// - `Err(E)` is considered a failure.
    pub fn call<T, E, F>(&self, f: F) -> Result<T, Error<E>>
    where
        F: FnOnce() -> Result<T, E>,
    {
        // 1. Check State
        {
            let mut inner = self.state.lock().unwrap();
            match inner.state {
                State::Closed => {
                    // Allowed to proceed.
                }
                State::Open(opened_at) => {
                    if opened_at.elapsed() >= self.reset_timeout {
                        // Timeout expired -> Transition to Half-Open
                        inner.state = State::HalfOpen;
                        // RUST INSIGHT: We don't reset failure_count here; strictly speaking,
                        // if Half-Open fails, we go back to Open.
                    } else {
                        // Fail Fast
                        return Err(Error::CircuitOpen);
                    }
                }
                State::HalfOpen => {
                    // Start of a simplistic "Only one request allowed" logic.
                    // In a real implementation, we might want to reject other requests while one is probing.
                    // Here, we let this request be the probe.
                    // If multiple threads hit HalfOpen simultaneously, they might all try.
                    // A better HalfOpen implementation uses a separate "Probe Active" flag or AtomicBool.
                    // For simplicity, we allow concurrent attempts in HalfOpen, first failure closes it back.
                }
            }
        } // Drop lock before executing `f` to avoid holding lock during long operation.

        // 2. Execute Operation
        let result = f();

        // 3. Update State based on result
        let mut inner = self.state.lock().unwrap();
        match result {
            Ok(val) => {
                if let State::HalfOpen = inner.state {
                    // Success in Half-Open -> Reset to Closed
                    inner.state = State::Closed;
                    inner.failure_count = 0;
                } else if let State::Closed = inner.state {
                    // Success in Closed -> Reset failure count (sliding window or consecutive)
                    // Here we implement "consecutive failures", so success resets count.
                    inner.failure_count = 0;
                }
                Ok(val)
            }
            Err(err) => {
                match inner.state {
                    State::Closed => {
                        inner.failure_count += 1;
                        if inner.failure_count >= self.failure_threshold {
                            inner.state = State::Open(Instant::now());
                        }
                    }
                    State::HalfOpen => {
                        // Failure in Half-Open -> Back to Open
                        inner.state = State::Open(Instant::now());
                    }
                    State::Open(_) => {
                        // Should not happen if logic is correct, but possible if race condition
                        // allowed execution before state check (Wait, we check state inside lock).
                        // If we are here, it means we executed `f`, so we must have been Closed or HalfOpen (or Open->HalfOpen).
                        // If another thread flipped us to Open while `f` was running, we just stay Open.
                    }
                }
                Err(Error::OperationFailed(err))
            }
        }
    }

    /// Returns true if the circuit is currently accepting requests (Closed or Half-Open).
    pub fn is_accepting(&self) -> bool {
        let inner = self.state.lock().unwrap();
        match inner.state {
            State::Open(opened_at) => opened_at.elapsed() >= self.reset_timeout,
            _ => true,
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum Error<E> {
    CircuitOpen,
    OperationFailed(E),
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `failsafe`: Offers more advanced policies (success thresholds for Half-Open, backoff strategies).
// - `sonyflake`: This is for ID generation, mistakenly referenced in some contexts as a circuit breaker.
//
// Missing vs. Production:
// - **Concurrency Control in Half-Open**: We allow all threads to proceed if state is Half-Open.
//   Production breakers usually allow only 1 probe request and fail others immediately.
// - **Metrics**: A real breaker would expose metrics (state changes, failure rates) for monitoring.
// - **Async Support**: This implementation blocks on the closure. Async breakers need to return `Future`.

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_closed_state_pass_through() {
        let cb = CircuitBreaker::new(3, Duration::from_secs(1));
        let res: Result<&str, Error<&str>> = cb.call(|| Ok("success"));
        assert_eq!(res, Ok("success"));
    }

    #[test]
    fn test_failure_counting() {
        let cb = CircuitBreaker::new(2, Duration::from_secs(1));

        // 1st failure
        let res: Result<(), Error<&str>> = cb.call(|| Err("fail"));
        assert!(matches!(res, Err(Error::OperationFailed("fail"))));

        // Still accepting
        assert!(cb.is_accepting());

        // 2nd failure -> Open
        let res: Result<(), Error<&str>> = cb.call(|| Err("fail"));
        assert!(matches!(res, Err(Error::OperationFailed("fail"))));

        // Now should be Open (rejecting)
        // Note: is_accepting might return true if timeout is 0, but here it is 1s.
        // Wait, is_accepting checks timeout.
        let inner = cb.state.lock().unwrap();
        if let State::Open(_) = inner.state {
            // Correct
        } else {
            panic!("Should be Open");
        }
    }

    #[test]
    fn test_fail_fast_when_open() {
        let cb = CircuitBreaker::new(1, Duration::from_secs(10));

        // Trip the breaker
        let _: Result<(), Error<&str>> = cb.call(|| Err("boom"));

        // Next call should fail fast
        let res: Result<(), Error<&str>> = cb.call(|| Ok(()));
        assert_eq!(res, Err(Error::CircuitOpen));
    }

    #[test]
    fn test_half_open_recovery() {
        let cb = CircuitBreaker::new(1, Duration::from_millis(100));

        // Trip
        let _: Result<(), Error<&str>> = cb.call(|| Err("boom"));

        // Wait for timeout
        thread::sleep(Duration::from_millis(150));

        // Next call should succeed (Half-Open -> Closed)
        let res: Result<&str, Error<&str>> = cb.call(|| Ok("recovered"));
        assert_eq!(res, Ok("recovered"));

        // Verify Closed
        let inner = cb.state.lock().unwrap();
        assert_eq!(inner.state, State::Closed);
        assert_eq!(inner.failure_count, 0);
    }

    #[test]
    fn test_half_open_failure() {
        let cb = CircuitBreaker::new(1, Duration::from_millis(100));

        // Trip
        let _: Result<(), Error<&str>> = cb.call(|| Err("boom"));

        // Wait
        thread::sleep(Duration::from_millis(150));

        // Call fails again
        let res: Result<(), Error<&str>> = cb.call(|| Err("boom again"));
        assert!(matches!(res, Err(Error::OperationFailed("boom again"))));

        // Verify Open again (and updated timestamp)
        let inner = cb.state.lock().unwrap();
        if let State::Open(_) = inner.state {
            // Good
        } else {
            panic!("Should be Open again");
        }
    }
}
