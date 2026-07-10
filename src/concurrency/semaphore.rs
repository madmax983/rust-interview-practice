//! # Semaphore Implementation
//!
//! A synchronization primitive that limits the number of threads that can concurrently access a resource.
//!
//! **Replaces Crates:** `tokio::sync::Semaphore` (async), `std::sync::Semaphore` (removed in Rust 1.0, effectively replaced by Condvar pattern)
//!
//! **Real-world Usage:**
//! - Connection pooling (limiting active DB connections).
//! - Rate limiting (token bucket implementation).
//! - Bounded buffers (producer-consumer).
//!
//! **Why build it yourself?**
//! Rust removed `Semaphore` from the standard library because it can be trivially implemented with `Mutex` and `Condvar`.
//! Building it reinforces your understanding of condition variables and spurious wakeups.

// Lock guards are intentionally held across condvar waits/notifications;
// do not tighten their scope.
#![allow(clippy::significant_drop_tightening)]

use std::sync::{Condvar, Mutex};
use std::time::Duration;

// =========================================================================================
// Architecture
// =========================================================================================
//
// State:
// - `count`: The number of available permits.
//
// Mechanics:
// - `acquire()`: If count > 0, decrement. If count == 0, wait on Condvar.
// - `release()`: Increment count. Notify Condvar.
//
// Invariants:
// 1. Count represents the number of permits available.
// 2. A release always wakes up at least one waiter (if any).

pub struct Semaphore {
    count: Mutex<usize>,
    cond: Condvar,
}

impl Semaphore {
    /// Creates a new semaphore with the initial number of permits.
    #[must_use]
    pub const fn new(permits: usize) -> Self {
        Self {
            count: Mutex::new(permits),
            cond: Condvar::new(),
        }
    }

    /// Acquires a permit, blocking the current thread until one is available.
    ///
    /// # Panics
    ///
    /// Panics if the internal count `Mutex` is poisoned.
    pub fn acquire(&self) {
        let mut count = self.count.lock().unwrap();
        while *count == 0 {
            count = self.cond.wait(count).unwrap();
        }
        *count -= 1;
    }

    /// Tries to acquire a permit without blocking.
    /// Returns `true` if a permit was acquired, `false` otherwise.
    ///
    /// # Panics
    ///
    /// Panics if the internal count `Mutex` is poisoned.
    pub fn try_acquire(&self) -> bool {
        let mut count = self.count.lock().unwrap();
        if *count > 0 {
            *count -= 1;
            true
        } else {
            false
        }
    }

    /// Tries to acquire a permit, blocking for at most `timeout`.
    /// Returns `true` if acquired, `false` if timed out.
    ///
    /// # Panics
    ///
    /// Panics if the internal count `Mutex` is poisoned.
    pub fn try_acquire_timeout(&self, timeout: Duration) -> bool {
        let mut count = self.count.lock().unwrap();
        let start = std::time::Instant::now();
        let mut remaining = timeout;

        while *count == 0 {
            let result = self.cond.wait_timeout(count, remaining).unwrap();
            count = result.0;
            if result.1.timed_out() {
                return false;
            }
            // Update remaining time to handle spurious wakeups correctly
            let elapsed = start.elapsed();
            if elapsed >= timeout {
                return false;
            }
            remaining = timeout.checked_sub(elapsed).unwrap();
        }

        *count -= 1;
        true
    }

    /// Releases a permit, returning it to the semaphore.
    /// This notifies a waiting thread, if any.
    ///
    /// # Panics
    ///
    /// Panics if the internal count `Mutex` is poisoned.
    pub fn release(&self) {
        let mut count = self.count.lock().unwrap();
        *count += 1;
        self.cond.notify_one();
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `tokio::sync::Semaphore`: Async-aware, supports `acquire_many`.
// - `parking_lot`: Uses raw futexes, more efficient than `std::sync::Mutex`.
//
// Missing vs. Production:
// - **Fairness**: This implementation does not guarantee FIFO ordering. A thread can "barge" in
//   and steal a permit before a woken thread gets it (Unfair Semaphore).
// - **Permit Counts**: We don't support `acquire(n)` or `release(n)` for multiple permits.

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_basic_acquire_release() {
        let sem = Semaphore::new(1);
        sem.acquire();
        assert!(!sem.try_acquire());
        sem.release();
        assert!(sem.try_acquire());
    }

    #[test]
    fn test_concurrent_access() {
        let sem = Arc::new(Semaphore::new(0));
        let sem_clone = sem.clone();

        let t = thread::spawn(move || {
            thread::sleep(Duration::from_millis(50));
            sem_clone.release();
        });

        // Should block until release
        sem.acquire();
        t.join().unwrap();
    }

    #[test]
    fn test_try_acquire_timeout() {
        let sem = Semaphore::new(0);
        assert!(!sem.try_acquire_timeout(Duration::from_millis(10)));

        sem.release();
        assert!(sem.try_acquire_timeout(Duration::from_millis(10)));
    }

    #[test]
    fn test_exclusion() {
        let sem = Arc::new(Semaphore::new(1));
        let counter = Arc::new(Mutex::new(0));

        let mut handles = vec![];
        for _ in 0..10 {
            let sem = sem.clone();
            let counter = counter.clone();
            handles.push(thread::spawn(move || {
                sem.acquire();
                let mut c = counter.lock().unwrap();
                *c += 1;
                // Critical section
                thread::sleep(Duration::from_millis(1));
                sem.release();
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(*counter.lock().unwrap(), 10);
    }
}
