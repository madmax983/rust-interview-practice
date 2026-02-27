//! # Concurrency & Async Patterns
//!
//! Replaces: **Synchronized Methods** (Java), **threading** (Python), **Callbacks** (Node.js)
//!
//! Real Rust usage: `tokio`, `std::sync`, `tower`, `hyper`
//!
//! ## Why these patterns exist in Rust
//! Rust's "Fearless Concurrency" comes from the type system. `Send` and `Sync` traits ensure that data races are
//! compile-time errors. Async/Await in Rust is a zero-cost abstraction over state machines, unlike green threads (Go)
//! or OS threads (Java < 19).
//!
//! ## Architecture
//!
//! 1. **Shared State:** `Arc<Mutex<T>>` - Shared ownership + exclusive access.
//! 2. **State Machine:** `Future` trait - Cooperative multitasking state machine.
//! 3. **Cancellation:** Broadcast channel or `CancellationToken` - Structured teardown.

use std::sync::{Arc, Mutex};

// ============================================================================
// Pattern 1: The Arc<Mutex<T>> Discipline
// ============================================================================

/// A thread-safe counter using shared mutable state.
///
/// **OWNERSHIP INSIGHT:**
/// - `Arc` (Atomic Reference Counted) provides shared ownership across threads.
/// - `Mutex` (Mutual Exclusion) provides exclusive mutable access (`&mut T` from `&Mutex<T>`).
/// - Together, `Arc<Mutex<T>>` allows multiple threads to mutate the same data safely.
#[derive(Debug, Clone)]
pub struct ConcurrentCounter {
    // COMPILE-TIME WIN: You cannot access the inner count without locking the mutex.
    // The compiler prevents data races.
    inner: Arc<Mutex<u32>>,
}

impl ConcurrentCounter {
    pub fn new() -> Self {
        ConcurrentCounter {
            inner: Arc::new(Mutex::new(0)),
        }
    }

    pub fn increment(&self) {
        // Lock scope is critical. We use a block to ensure the lock is released
        // as soon as possible, preventing contention.
        let mut guard = self.inner.lock().unwrap();
        *guard += 1;
        // guard is dropped here, releasing the lock
    }

    pub fn get(&self) -> u32 {
        let guard = self.inner.lock().unwrap();
        *guard
    }
}

impl Default for ConcurrentCounter {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Pattern 2: Async State Machine (Explicit Future)
// ============================================================================

#[cfg(feature = "async-parallel")]
use std::future::Future;
#[cfg(feature = "async-parallel")]
use std::pin::Pin;
#[cfg(feature = "async-parallel")]
use std::task::{Context, Poll, Waker};

/// A Future that completes after a set number of polls.
/// Demonstrates the underlying state machine of `async` functions.
#[cfg(feature = "async-parallel")]
pub struct DelayFuture {
    polls_left: usize,
    waker: Option<Waker>,
}

#[cfg(feature = "async-parallel")]
impl DelayFuture {
    pub fn new(polls: usize) -> Self {
        DelayFuture {
            polls_left: polls,
            waker: None,
        }
    }
}

#[cfg(feature = "async-parallel")]
impl Future for DelayFuture {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.polls_left == 0 {
            Poll::Ready(())
        } else {
            self.polls_left -= 1;

            // Store the waker so we can be woken up later (simulated here)
            self.waker = Some(cx.waker().clone());

            // In a real reactor, we would register with the OS (epoll/kqueue).
            // Here we just wake ourselves up immediately to simulate progress.
            cx.waker().wake_by_ref();

            Poll::Pending
        }
    }
}

// ============================================================================
// Pattern 3: Cancellation & Graceful Shutdown
// ============================================================================

#[cfg(feature = "async-parallel")]
use tokio::sync::broadcast;
#[cfg(feature = "async-parallel")]
use tokio::sync::mpsc;

/// A struct to manage graceful shutdown of multiple tasks.
///
/// Uses a broadcast channel to signal shutdown to all listeners.
#[cfg(feature = "async-parallel")]
pub struct ShutdownManager {
    notify_shutdown: broadcast::Sender<()>,
    shutdown_complete_tx: mpsc::Sender<()>,
    shutdown_complete_rx: mpsc::Receiver<()>,
}

#[cfg(feature = "async-parallel")]
impl ShutdownManager {
    pub fn new() -> Self {
        let (notify_shutdown, _) = broadcast::channel(1);
        let (shutdown_complete_tx, shutdown_complete_rx) = mpsc::channel(1);

        ShutdownManager {
            notify_shutdown,
            shutdown_complete_tx,
            shutdown_complete_rx,
        }
    }

    /// Returns a receiver that triggers when shutdown is requested.
    pub fn subscribe(&self) -> broadcast::Receiver<()> {
        self.notify_shutdown.subscribe()
    }

    /// Returns a handle that tasks should hold until they are done shutting down.
    /// When all handles are dropped, `wait_for_shutdown_complete` will return.
    pub fn get_shutdown_handle(&self) -> mpsc::Sender<()> {
        self.shutdown_complete_tx.clone()
    }

    /// Signals shutdown and waits for all handles to be dropped.
    pub async fn signal_shutdown_and_wait(mut self) {
        // Drop the notification sender to close the channel (optional if we sent a value)
        // or just send the value.
        let _ = self.notify_shutdown.send(());

        // Drop our own handle to the completion channel so we don't wait for ourselves
        drop(self.shutdown_complete_tx);

        // Wait for all other handles to be dropped
        let _ = self.shutdown_complete_rx.recv().await;
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_arc_mutex_counter() {
        let counter = ConcurrentCounter::new();
        let mut handles = vec![];

        for _ in 0..10 {
            let c = counter.clone();
            handles.push(thread::spawn(move || {
                for _ in 0..100 {
                    c.increment();
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(counter.get(), 1000);
    }

    #[cfg(feature = "async-parallel")]
    #[tokio::test]
    async fn test_async_state_machine() {
        let future = DelayFuture::new(3);
        future.await;
        // If it completes, the test passes.
    }

    #[cfg(feature = "async-parallel")]
    #[tokio::test]
    async fn test_graceful_shutdown() {
        let manager = ShutdownManager::new();
        let mut rx_shutdown = manager.subscribe();
        let shutdown_handle = manager.get_shutdown_handle();

        // Simulate a worker task
        let worker = tokio::spawn(async move {
            // Wait for shutdown signal
            let _ = rx_shutdown.recv().await;
            // Simulate cleanup work
            // ...
            // Drop handle to signal completion
            drop(shutdown_handle);
        });

        // Trigger shutdown
        manager.signal_shutdown_and_wait().await;

        // Ensure worker finished
        assert!(worker.await.is_ok());
    }
}
