//! # Async Mutex Implementation
//!
//! A minimal async-aware mutual exclusion lock.
//! Unlike standard thread-blocking Mutexes, an Async Mutex yields back to the executor
//! when the lock is contested, parking the task instead of the OS thread.
//!
//! **Replaces Crates:** `tokio::sync::Mutex`, `async-lock`
//!
//! **Real-world Usage:**
//! - Protecting shared state (like a database connection pool or global cache) in async
//!   web servers (Axum, Actix-Web) where tasks must yield to allow others to run.
//! - Coordinating asynchronous background jobs.
//!
//! **Why build it yourself?**
//! It demonstrates the core principle of asynchronous programming in Rust: translating
//! physical blocking (sleeping a thread) into logical yielding (returning `Poll::Pending`
//! and storing a `Waker`). It teaches how `UnsafeCell` is used correctly to bypass
//! the borrow checker for interior mutability when manual synchronization guarantees safety.

use std::cell::UnsafeCell;
use std::collections::VecDeque;
use std::future::Future;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::pin::Pin;
use std::sync::{atomic::{AtomicUsize, Ordering}, Mutex};
use std::task::{Context, Poll, Waker};

static NEXT_WAITER_ID: AtomicUsize = AtomicUsize::new(1);

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure:
//
//      AsyncMutex<T>
//      ├── inner: UnsafeCell<T>      (The data being protected)
//      └── state: std::sync::Mutex   (The synchronous lock protecting the async state machine)
//                 ├── locked: bool
//                 └── wakers: VecDeque<Waker>
//
// Flow (Locking):
//      Task A calls `mutex.lock().await`
//      1. Lock the synchronous `state` mutex.
//      2. If `!locked`:
//         - Set `locked = true`, drop the state mutex, return the `AsyncMutexGuard`.
//      3. If `locked`:
//         - Push the current task's `Waker` into `wakers`.
//         - Return `Poll::Pending` (yielding to the executor).
//
// Flow (Unlocking via Drop):
//      Task A drops `AsyncMutexGuard`
//      1. Lock the synchronous `state` mutex.
//      2. Pop the next `Waker` from `wakers`.
//      3. If a waker exists, call `waker.wake()`. The lock *remains* held logically for the next task.
//      4. If no wakers exist, set `locked = false`.
//
// Invariants:
// 1. The synchronous `state` mutex is held ONLY for the briefest moment to check state and queue wakers.
// 2. The inner `UnsafeCell<T>` is only ever accessed when `locked` is logically true.
// 3. Waking a task transfers logical ownership of the lock directly to that task.
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ lock (fast)   │ O(1)        │ O(1)        │
// │ lock (contend)│ O(1) queue  │ O(W) wakers │
// │ unlock        │ O(1)        │ O(1)        │
// └───────────────┴─────────────┴─────────────┘
//
// Design Decisions:
// - **Sync Mutex inside Async Mutex**: We use `std::sync::Mutex` to protect the queue of Wakers.
//   - *Tradeoff*: It technically blocks the thread for nanoseconds. This is standard practice
//     even in `tokio` (which uses a spinlock or cheap OS lock internally for the wait queue).
//   - *Alternative*: Lock-free queue using Atomics, which is wildly complex and prone to ABA.

/// A future that resolves to an `AsyncMutexGuard`.
pub struct MutexAcquire<'a, T> {
    mutex: &'a AsyncMutex<T>,
    id: usize,
}

impl<'a, T> MutexAcquire<'a, T> {
    fn new(mutex: &'a AsyncMutex<T>) -> Self {
        Self {
            mutex,
            id: NEXT_WAITER_ID.fetch_add(1, Ordering::Relaxed),
        }
    }
}

impl<'a, T> Future for MutexAcquire<'a, T> {
    type Output = AsyncMutexGuard<'a, T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut state = self.mutex.state.lock().unwrap();

        if state.locked {
            // RUST INSIGHT: We must handle multiple polls cleanly. We check if our `id`
            // is already in the wait queue. If so, we just update the waker.
            // If not, we push it to the back.
            let mut found = false;
            for (id, waker) in state.wakers.iter_mut() {
                if *id == self.id {
                    // Update the waker in case it has changed (e.g. `tokio::select!`).
                    if !waker.will_wake(cx.waker()) {
                        *waker = cx.waker().clone();
                    }
                    found = true;
                    break;
                }
            }

            if !found {
                state.wakers.push_back((self.id, cx.waker().clone()));
            }

            Poll::Pending
        } else {
            // The lock is available. Claim it immediately.
            state.locked = true;
            // Remove our id from the wakers if we were parked previously.
            state.wakers.retain(|(id, _)| *id != self.id);
            Poll::Ready(AsyncMutexGuard {
                mutex: self.mutex,
                _marker: PhantomData,
            })
        }
    }
}

impl<'a, T> Drop for MutexAcquire<'a, T> {
    fn drop(&mut self) {
        // If the future is dropped before completing, remove its waker from the queue.
        // This prevents the "leak queue space" bug and the deadly cancellation bug where
        // a dropped future is woken and the chain of wakeups dies.
        let mut state = self.mutex.state.lock().unwrap();

        let was_first = state.wakers.front().map(|(id, _)| *id) == Some(self.id);
        state.wakers.retain(|(id, _)| *id != self.id);

        // If this task was at the front of the queue and the lock is NOT held,
        // (meaning the previous holder dropped the lock and woke THIS dropped future),
        // we must wake the next future in the queue so the chain doesn't break.
        if was_first && !state.locked {
            if let Some((_, waker)) = state.wakers.front() {
                waker.wake_by_ref();
            }
        }
    }
}

/// The state of the async mutex.
struct MutexState {
    locked: bool,
    wakers: VecDeque<(usize, Waker)>,
}

/// An asynchronous mutual exclusion primitive.
pub struct AsyncMutex<T> {
    state: Mutex<MutexState>,
    inner: UnsafeCell<T>,
}

// RUST INSIGHT: We must manually implement `Send` and `Sync` because `UnsafeCell` is not `Sync`.
// An AsyncMutex is `Sync` if the inner type `T` is both `Send` (can be moved between threads)
// and `Sync` (can be shared between threads).
unsafe impl<T: Send> Send for AsyncMutex<T> {}
unsafe impl<T: Send> Sync for AsyncMutex<T> {}

impl<T> AsyncMutex<T> {
    /// Creates a new `AsyncMutex` wrapping the given value.
    #[must_use]
    pub fn new(value: T) -> Self {
        Self {
            state: Mutex::new(MutexState {
                locked: false,
                wakers: VecDeque::new(),
            }),
            inner: UnsafeCell::new(value),
        }
    }

    /// Acquires the lock, waiting asynchronously if it is currently held.
    pub fn lock(&self) -> MutexAcquire<'_, T> {
        MutexAcquire::new(self)
    }
}

/// An RAII guard that releases the lock when dropped.
pub struct AsyncMutexGuard<'a, T> {
    mutex: &'a AsyncMutex<T>,
    // RUST INSIGHT: PhantomData ensures `AsyncMutexGuard` correctly inherits `Send` and `Sync`
    // properties from `T` instead of assuming it's `Sync` just because `&AsyncMutex` is `Sync`.
    // We use a mutable pointer phantom to explicitly opt-out of auto-derived `Send`/`Sync`,
    // and then manually implement the correct bounds for the guard.
    _marker: PhantomData<*mut T>,
}

unsafe impl<'a, T: Send> Send for AsyncMutexGuard<'a, T> {}
unsafe impl<'a, T: Sync> Sync for AsyncMutexGuard<'a, T> {}

// RUST INSIGHT: `Deref` and `DerefMut` allow the guard to be used transparently as if
// it were the underlying `T`.
impl<'a, T> Deref for AsyncMutexGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // UNSAFE JUSTIFICATION: The existence of this Guard guarantees exclusive access
        // to the inner value, because the state machine ensures only one task can hold
        // a Guard at a time.
        unsafe { &*self.mutex.inner.get() }
    }
}

impl<'a, T> DerefMut for AsyncMutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // UNSAFE JUSTIFICATION: Same as `deref`.
        unsafe { &mut *self.mutex.inner.get() }
    }
}

impl<'a, T> Drop for AsyncMutexGuard<'a, T> {
    fn drop(&mut self) {
        let mut state = self.mutex.state.lock().unwrap();

        // GOTCHA: We must set `locked = false` before waking the next task.
        // This allows the woken task (or a completely new task) to acquire the lock when it is polled again.
        // This introduces "lock stealing", where a new task can grab the lock before the woken task gets scheduled.
        // While this technically breaks strict FIFO fairness, it is required for this simplified state machine to progress
        // without complex per-future state tracking, and it actually improves overall throughput in real-world scenarios.
        state.locked = false;

        // RUST INSIGHT: We DO NOT remove the waker from the queue here (`pop_front`).
        // If we popped it, and the woken task was cancelled (dropped) before it could run,
        // it wouldn't know it was the "next in line" and wouldn't wake the *following* task,
        // leading to a permanent deadlock.
        // Instead, the task itself removes its waker from the queue when it successfully acquires the lock.
        if let Some((_, waker)) = state.wakers.front() {
            // Wake up the next task in the queue.
            waker.wake_by_ref();
        }
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `tokio::sync::Mutex`: Tokio's implementation is highly optimized. It uses a custom intrusive
//   doubly-linked list for the wait queue (to avoid `VecDeque` allocations) and atomics for fast-path
//   locking (avoiding the `std::sync::Mutex` overhead entirely when uncontended).
//
// Missing vs. Production:
// - **Fairness & Lock Stealing**: Our implementation allows lock stealing (a new task can grab the lock
//   before a woken task is scheduled). Tokio's Mutex is also not perfectly fair by default for performance reasons,
//   but provides mechanisms to prevent starvation under heavy contention.
// - **Cancellation Safety**: If the `MutexAcquire` future is dropped while waiting, its Waker remains
//   in our `VecDeque`. If woken later, it will be a no-op, but it leaks queue space until processed.
//   Production implementations remove the Waker from the queue on Drop.
// - **Try-lock**: Missing a non-asynchronous `try_lock()` method.
//
// Next Steps:
// 1. Implement `Drop` for `MutexAcquire` to remove its waker from the queue (Cancellation safety).
// 2. Implement `try_lock() -> Option<AsyncMutexGuard>`.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::concurrency::async_executor::{new_executor_and_spawner, TimerFuture};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    #[test]
    fn test_async_mutex_uncontended() {
        let (executor, spawner) = new_executor_and_spawner();
        let mutex = Arc::new(AsyncMutex::new(0));

        let m = mutex.clone();
        spawner.spawn(async move {
            let mut guard = m.lock().await;
            *guard += 1;
        });

        drop(spawner);
        executor.run();

        // Check result synchronously since we know the executor is done.
        let val = unsafe { *mutex.inner.get() };
        assert_eq!(val, 1);
    }

    #[test]
    fn test_async_mutex_contended() {
        let (executor, spawner) = new_executor_and_spawner();
        let mutex = Arc::new(AsyncMutex::new(0));

        // Spawn 5 concurrent tasks trying to increment the counter
        for _ in 0..5 {
            let m = mutex.clone();
            spawner.spawn(async move {
                let mut guard = m.lock().await;
                // Artificial delay to force contention and waker queueing
                TimerFuture::new(Duration::from_millis(10)).await;
                *guard += 1;
            });
        }

        drop(spawner);
        executor.run();

        let val = unsafe { *mutex.inner.get() };
        assert_eq!(val, 5);
    }

    #[test]
    fn test_async_mutex_sequential_locking() {
        let (executor, spawner) = new_executor_and_spawner();
        let mutex = Arc::new(AsyncMutex::new(String::new()));
        let order = Arc::new(AtomicUsize::new(0));

        let m1 = mutex.clone();
        let o1 = order.clone();
        spawner.spawn(async move {
            let mut guard = m1.lock().await;
            TimerFuture::new(Duration::from_millis(20)).await;
            guard.push_str("A");
            o1.store(1, Ordering::SeqCst);
        });

        let m2 = mutex.clone();
        let o2 = order.clone();
        spawner.spawn(async move {
            // Task 2 starts shortly after Task 1
            TimerFuture::new(Duration::from_millis(5)).await;
            let mut guard = m2.lock().await;
            guard.push_str("B");
            o2.store(2, Ordering::SeqCst);
        });

        drop(spawner);
        executor.run();

        // The order should be strictly sequential based on locking
        let val = unsafe { &*mutex.inner.get() };
        assert_eq!(val, "AB");
        assert_eq!(order.load(Ordering::SeqCst), 2);
    }
}
