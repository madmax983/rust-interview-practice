//! # Promise/Future Implementation
//!
//! Implements a synchronization primitive for producing and consuming a value asynchronously.
//! The `Promise` is used to provide the value, and the `Future` is used to await it.
//!
//! **Replaces Crates:** The conceptual foundation of `std::future::Future` (pre-async/await) or `oneshot` channels from `tokio` / `futures`.
//!
//! **Real-world Usage:**
//! - Passing the result of a background computation back to the main thread.
//! - One-time event signaling.
//! - Building block for more complex async executors or actor systems.
//!
//! **Why build it yourself?**
//! Understanding how a thread can block and be woken up by another thread is fundamental to concurrency.
//! Building a Promise/Future pair from scratch using `Mutex` and `Condvar` demonstrates exactly how
//! wait queues work under the hood, stripping away the magic of async/await to reveal the raw OS primitives.

use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure:
//
//      Future<T>                        Promise<T>
//          │                               │
//          ▼                               ▼
//      ┌───────────────────────────────────────┐
//      │              Arc<SharedState<T>>      │
//      │ ┌───────────────────────────────────┐ │
//      │ │ Mutex<State<T>>                   │ │
//      │ │ Condvar                           │ │
//      │ └───────────────────────────────────┘ │
//      └───────────────────────────────────────┘
//
// Invariants:
// 1. The inner state is `Empty` until the `Promise` is resolved or dropped.
// 2. The state can only transition exactly once: `Empty` -> `Set(T)` or `Empty` -> `Dropped`.
// 3. The `Condvar` is notified when the state transitions away from `Empty`.
// 4. All state transitions happen under the *same* Mutex to prevent lost wakeups.
//
// Complexity:
// ┌───────────┬────────┬────────┐
// │ Operation │ Time   │ Space  │
// ├───────────┼────────┼────────┤
// │ set       │ O(1)   │ O(1)   │
// │ get       │ O(1)*  │ O(1)   │
// └───────────┴────────┴────────┘
// * Time is O(1) modulo blocking time, which depends on the producer.
//
// Design Decisions:
// - **Shared State**: `Arc<SharedState<T>>` connects the two halves.
//   - *Tradeoff*: Requires allocation (`Arc`).
//   - *Alternative*: Intrusive pointers or specific async executors avoid this for specific tasks.
// - **Blocking**: Uses `Condvar` to block the thread until the value is ready.
//   - *Tradeoff*: Occupies an OS thread. Real `std::future::Future` uses `Waker` to yield to an executor.
// - **Single Mutex**: A single `Mutex<State<T>>` is used to protect both the value and the drop status.
//   This ensures that the condition variable's notification is never missed due to a race condition.

#[derive(Debug, PartialEq, Eq)]
pub enum FutureError {
    /// The Promise was dropped before a value was set.
    PromiseDropped,
}

/// The consumer side trait.
pub trait AsyncFuture<T> {
    /// Blocks the current thread until the value is available.
    ///
    /// # Errors
    /// Returns `Err(FutureError::PromiseDropped)` if the `Promise` was dropped without providing a value.
    fn get(self) -> Result<T, FutureError>;

    /// Blocks for a maximum of `timeout` waiting for the value.
    ///
    /// # Errors
    /// Returns `Err(FutureError::PromiseDropped)` if the promise was dropped.
    /// Returns `Ok(None)` if the timeout elapsed.
    fn get_timeout(self, timeout: Duration) -> Result<Option<T>, FutureError>;
}

/// The producer side trait.
pub trait AsyncPromise<T> {
    /// Resolves the promise with the given value.
    /// Returns `Ok(())` if the value was set, or `Err(value)` if it was already resolved.
    fn set(self, value: T) -> Result<(), T>;
}

enum State<T> {
    Empty,
    Set(T),
    Dropped,
}

struct SharedState<T> {
    state: Mutex<State<T>>,
    cvar: Condvar,
}

/// The consumer side of the channel, used to retrieve the value.
pub struct CondvarFuture<T> {
    shared: Arc<SharedState<T>>,
}

/// The producer side of the channel, used to set the value.
pub struct CondvarPromise<T> {
    shared: Arc<SharedState<T>>,
}

/// Creates a new Promise/Future pair using Condvars.
#[must_use]
pub fn promise<T>() -> (CondvarPromise<T>, CondvarFuture<T>) {
    let shared = Arc::new(SharedState {
        state: Mutex::new(State::Empty),
        cvar: Condvar::new(),
    });

    (
        CondvarPromise {
            shared: Arc::clone(&shared),
        },
        CondvarFuture { shared },
    )
}

impl<T> AsyncPromise<T> for CondvarPromise<T> {
    fn set(self, value: T) -> Result<(), T> {
        let mut guard = self.shared.state.lock().unwrap();

        match std::mem::replace(&mut *guard, State::Set(value)) {
            State::Empty => {
                // We notify while holding the lock to prevent lost wakeups.
                // RUST INSIGHT:
                // Safe and correct usage of Condvar requires modifying the state and calling
                // notify_all while holding the lock that protects the condition.
                self.shared.cvar.notify_all();

                // Disarm Drop implementation
                // We use ManuallyDrop to prevent the drop handler from running and marking it as dropped
                // wait, we consume `self`, so we could just wrap `self` or use a flag.
                // Since `set` consumes `self`, let's just use mem::forget after we drop the guard,
                // or just let drop happen, but update the drop logic to only act if it's Empty.
                // Actually, our drop logic already checks if it's Empty!
                drop(guard);
                // Prevent `Drop` from running and marking it as Dropped, or just rely on the fact that
                // the state is no longer Empty.
                // However, wait, what if `drop` runs after this lock is released?
                // Our `drop` acquires the lock, checks if it's `Empty`, and only then transitions to `Dropped`.
                // Since we just set it to `Set(T)`, `drop` will do nothing.
                Ok(())
            }
            State::Set(val) => {
                // Already resolved (shouldn't happen with our API since set consumes self,
                // but conceptually true if we allowed multiple calls).
                // Revert state
                *guard = State::Set(val);

                // How do we extract the value we just tried to set?
                // We moved it into `State::Set(value)` and replaced it.
                // We actually shouldn't replace it if it's already set. Let's fix this.
                unreachable!("set consumes self, so it cannot be called twice");
            }
            State::Dropped => unreachable!(),
        }
    }
}

impl<T> Drop for CondvarPromise<T> {
    fn drop(&mut self) {
        let mut guard = self.shared.state.lock().unwrap();
        if matches!(*guard, State::Empty) {
            // GOTCHA: Producer is dropping without providing a value!
            // We must wake up the consumer to prevent a deadlock, and we must do it
            // while holding the *same* lock that the consumer waits on to avoid a lost wakeup.
            *guard = State::Dropped;
            self.shared.cvar.notify_all();
        }
    }
}

impl<T> AsyncFuture<T> for CondvarFuture<T> {
    fn get(self) -> Result<T, FutureError> {
        let mut guard = self.shared.state.lock().unwrap();

        loop {
            match std::mem::replace(&mut *guard, State::Empty) {
                State::Set(val) => return Ok(val),
                State::Dropped => {
                    // Put it back to Dropped in case there are multiple waiters (though it's a oneshot)
                    *guard = State::Dropped;
                    return Err(FutureError::PromiseDropped);
                }
                State::Empty => {
                    // Spurious wakeups! Condvars can wake up without a notification.
                    // RUST INSIGHT: `cvar.wait` atomically unlocks the mutex, blocks the thread,
                    // and re-locks the mutex when it wakes up.
                    guard = self.shared.cvar.wait(guard).unwrap();
                }
            }
        }
    }

    fn get_timeout(self, timeout: Duration) -> Result<Option<T>, FutureError> {
        let mut guard = self.shared.state.lock().unwrap();
        let mut timeout_remaining = timeout;
        let start = std::time::Instant::now();

        loop {
            match std::mem::replace(&mut *guard, State::Empty) {
                State::Set(val) => return Ok(Some(val)),
                State::Dropped => {
                    *guard = State::Dropped;
                    return Err(FutureError::PromiseDropped);
                }
                State::Empty => {
                    if timeout_remaining.is_zero() {
                        return Ok(None);
                    }

                    let (new_guard, result) = self
                        .shared
                        .cvar
                        .wait_timeout(guard, timeout_remaining)
                        .unwrap();
                    guard = new_guard;

                    if result.timed_out() {
                        // Check one last time before giving up
                        if let State::Set(_) = *guard {
                            // Let the loop handle it
                        } else if matches!(*guard, State::Dropped) {
                            *guard = State::Dropped;
                            return Err(FutureError::PromiseDropped);
                        } else {
                            return Ok(None);
                        }
                    }

                    // Calculate remaining timeout in case of spurious wakeup
                    let elapsed = start.elapsed();
                    timeout_remaining = timeout.saturating_sub(elapsed);
                }
            }
        }
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `tokio::sync::oneshot`: Does not block an OS thread. Instead, it registers a `Waker` with the
//   async executor, allowing the thread to do other work while waiting.
// - `std::sync::mpsc::channel`: A channel can send multiple values; our primitive is strictly one-shot.
//
// Missing vs. Production:
// - **Non-blocking Polling**: Real futures need a `poll` method that returns `Poll::Ready` or `Poll::Pending`.
// - **Waker Integration**: For async/await compatibility.
//
// PRODUCTION NOTE:
// - **Lock-free implementation**: Production oneshot channels often use atomics (`AtomicU8` state machine)
//   and `UnsafeCell` to avoid the overhead of a `Mutex` entirely on the happy path. This significantly
//   reduces contention when the value is immediately available.
//
// Next Steps:
// 1. Implement `std::future::Future` for our `Future` type to make it `async/.await` compatible.
// 2. Refactor to use atomics instead of `Mutex` for state transitions.

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    // RUST INSIGHT: Benchmarking
    // To benchmark this primitive properly against `std::sync::mpsc::channel`, use `criterion`.
    // Example:
    // b.iter(|| {
    //     let (p, f) = promise::<i32>();
    //     thread::spawn(move || p.set(std::hint::black_box(42)));
    //     std::hint::black_box(f.get());
    // })

    #[test]
    fn test_promise_future_basic() {
        let (promise, future) = promise::<i32>();

        promise.set(42).unwrap();
        assert_eq!(future.get(), Ok(42));
    }

    #[test]
    fn test_promise_future_concurrent() {
        let (promise, future) = promise::<String>();

        let handle = thread::spawn(move || {
            thread::sleep(Duration::from_millis(50));
            promise.set("Hello from thread".to_string()).unwrap();
        });

        assert_eq!(future.get(), Ok("Hello from thread".to_string()));
        handle.join().unwrap();
    }

    #[test]
    fn test_promise_dropped_error() {
        let (promise, future) = promise::<i32>();

        // Drop the promise without setting a value
        drop(promise);

        assert_eq!(future.get(), Err(FutureError::PromiseDropped));
    }

    #[test]
    fn test_promise_dropped_concurrent() {
        let (promise, future) = promise::<i32>();

        let handle = thread::spawn(move || {
            thread::sleep(Duration::from_millis(50));
            drop(promise);
        });

        assert_eq!(future.get(), Err(FutureError::PromiseDropped));
        handle.join().unwrap();
    }

    #[test]
    fn test_future_timeout_success() {
        let (promise, future) = promise::<i32>();

        let handle = thread::spawn(move || {
            thread::sleep(Duration::from_millis(20));
            promise.set(100).unwrap();
        });

        // Wait up to 100ms, should get it in ~20ms
        assert_eq!(
            future.get_timeout(Duration::from_millis(100)),
            Ok(Some(100))
        );
        handle.join().unwrap();
    }

    #[test]
    fn test_future_timeout_expires() {
        let (promise, future) = promise::<i32>();

        let handle = thread::spawn(move || {
            thread::sleep(Duration::from_millis(100));
            promise.set(100).unwrap();
        });

        // Wait only 20ms, should time out
        assert_eq!(future.get_timeout(Duration::from_millis(20)), Ok(None));
        handle.join().unwrap();
    }
}
