//! # Multi-Producer Single-Consumer (MPSC) Channel
//!
//! Implements a thread-safe channel for sending messages between threads.
//! Supports both bounded (blocking) and unbounded (non-blocking send) variants.
//!
//! **Replaces Crates:** `std::sync::mpsc`, `crossbeam-channel`, `flume`
//!
//! **Real-world Usage:**
//! - Task queues (e.g., in a thread pool).
//! - Event loops (GUI frameworks).
//! - Actor systems (sending messages to actors).
//!
//! **Why build it yourself?**
//! - Understanding `Condvar` and `Mutex` interaction is crucial for concurrency.
//! - Seeing how `Drop` handles cleanup and signals receivers.
//! - Implementing blocking vs non-blocking logic.

use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex};

// =========================================================================================
// Architecture
// =========================================================================================
//
// Shared State:
//   Held by both Senders and Receiver via Arc<Mutex<...>>.
//   Contains the queue, capacity (for bounded), and closed flag.
//
// Synchronization:
//   - `data_available`: Signaled by Senders when data is added. Waited on by Receiver.
//   - `space_available`: Signaled by Receiver when data is removed. Waited on by Senders (if bounded).
//
// Invariants:
// 1. Messages are received in FIFO order.
// 2. If the channel is bounded, `queue.len() <= capacity`.
// 3. Sender blocks if bounded and full.
// 4. Receiver blocks if empty.
// 5. Channel is closed when all Senders are dropped OR the Receiver is dropped.
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ Send          │ O(1)        │ O(1)        │
// ├───────────────┼─────────────┼─────────────┤
// │ Recv          │ O(1)        │ O(1)        │
// ├───────────────┼─────────────┼─────────────┤
// │ Clone Sender  │ O(1)        │ O(1)        │
// └───────────────┴─────────────┴─────────────┘

struct SharedState<T> {
    queue: VecDeque<T>,
    capacity: Option<usize>, // None = Unbounded
    senders: usize,
    receivers: usize, // For MPSC, usually 1, but we track it for correctness
    closed: bool,
}

struct Inner<T> {
    state: Mutex<SharedState<T>>,
    data_available: Condvar,
    space_available: Condvar,
}

/// The transmitting end of a channel.
pub struct Sender<T> {
    inner: Arc<Inner<T>>,
}

/// The receiving end of a channel.
pub struct Receiver<T> {
    inner: Arc<Inner<T>>,
}

// RUST INSIGHT: `Send` and `Sync`
// `Sender` and `Receiver` are `Send` if `T` is `Send`.
// They are `Sync` because they use internal synchronization (`Mutex`).
// However, `std::sync::mpsc::Sender` is `Send` but not `Sync` (cannot be shared between threads without cloning).
// Our `Sender` is `Clone`, so we can clone it to move to other threads.
// `SharedState` is protected by a Mutex, so `T` needs to be `Send` to be moved between threads via the queue.

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        let mut state = self.inner.state.lock().unwrap();
        state.senders += 1;
        drop(state); // Drop lock early

        Sender {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        let mut state = self.inner.state.lock().unwrap();
        state.senders -= 1;
        let no_more_senders = state.senders == 0;

        // If no more senders, we must signal the receiver so it doesn't block forever.
        if no_more_senders {
            state.closed = true;
            self.inner.data_available.notify_all();
        }
    }
}

impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        let mut state = self.inner.state.lock().unwrap();
        state.receivers -= 1;
        state.closed = true;

        // Signal any waiting senders that the channel is closed (they should stop waiting).
        self.inner.space_available.notify_all();
    }
}

impl<T> Sender<T> {
    /// Sends a value into the channel.
    ///
    /// If the channel is bounded and full, this method blocks until space is available.
    /// Returns `Err` if the receiver has hung up.
    pub fn send(&self, t: T) -> Result<(), SendError<T>> {
        let mut state = self.inner.state.lock().unwrap();

        // RUST INSIGHT: Bounded Channel Backpressure
        // If bounded, we must wait for space.
        if let Some(cap) = state.capacity {
            while state.queue.len() >= cap {
                if state.receivers == 0 {
                    // Receiver hung up
                    return Err(SendError(t));
                }

                // GOTCHA: `wait` releases the lock and blocks. When it returns, it re-acquires the lock.
                // We must check the condition again because of spurious wakeups or state changes.
                state = self.inner.space_available.wait(state).unwrap();
            }
        }

        if state.receivers == 0 {
            return Err(SendError(t));
        }

        state.queue.push_back(t);

        // Notify waiting receiver
        self.inner.data_available.notify_one();

        Ok(())
    }
}

impl<T> Receiver<T> {
    /// Receives a value from the channel.
    ///
    /// Blocks if the channel is empty.
    /// Returns `Ok(T)` if a value is received.
    /// Returns `Err` if the channel is empty and all senders have disconnected.
    pub fn recv(&self) -> Result<T, RecvError> {
        let mut state = self.inner.state.lock().unwrap();

        loop {
            if let Some(t) = state.queue.pop_front() {
                // If bounded, notify senders that space is available
                if state.capacity.is_some() {
                    self.inner.space_available.notify_one();
                }
                return Ok(t);
            }

            if state.closed && state.senders == 0 {
                // Channel is empty and closed
                return Err(RecvError);
            }

            // Wait for data
            state = self.inner.data_available.wait(state).unwrap();
        }
    }

    /// Non-blocking receive.
    pub fn try_recv(&self) -> Result<T, TryRecvError> {
        let mut state = self.inner.state.lock().unwrap();

        if let Some(t) = state.queue.pop_front() {
             if state.capacity.is_some() {
                self.inner.space_available.notify_one();
            }
            Ok(t)
        } else if state.senders == 0 {
            Err(TryRecvError::Disconnected)
        } else {
            Err(TryRecvError::Empty)
        }
    }
}

impl<T> Iterator for Receiver<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.recv().ok()
    }
}

#[derive(Debug)]
pub struct SendError<T>(pub T);

#[derive(Debug, PartialEq, Eq)]
pub struct RecvError;

#[derive(Debug, PartialEq, Eq)]
pub enum TryRecvError {
    Empty,
    Disconnected,
}


/// Create an unbounded channel.
pub fn unbounded<T>() -> (Sender<T>, Receiver<T>) {
    let inner = Arc::new(Inner {
        state: Mutex::new(SharedState {
            queue: VecDeque::new(),
            capacity: None,
            senders: 1,
            receivers: 1,
            closed: false,
        }),
        data_available: Condvar::new(),
        space_available: Condvar::new(),
    });

    (
        Sender { inner: Arc::clone(&inner) },
        Receiver { inner },
    )
}

/// Create a bounded channel with capacity `cap`.
pub fn bounded<T>(cap: usize) -> (Sender<T>, Receiver<T>) {
    assert!(cap > 0, "capacity must be > 0");

    let inner = Arc::new(Inner {
        state: Mutex::new(SharedState {
            queue: VecDeque::with_capacity(cap),
            capacity: Some(cap),
            senders: 1,
            receivers: 1,
            closed: false,
        }),
        data_available: Condvar::new(),
        space_available: Condvar::new(),
    });

    (
        Sender { inner: Arc::clone(&inner) },
        Receiver { inner },
    )
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `std::sync::mpsc`: Uses a linked list for unbounded (lock-free optimization) and array for bounded.
// - `crossbeam-channel`: Highly optimized lock-free queues, supports MPMC, and `select!`.
// - Our implementation: Uses a `Mutex` + `VecDeque`. Simple but slower under high contention.
//
// Missing vs. Production:
// - **Lock-free**: Production channels use atomic operations to avoid mutex overhead.
// - **Select**: Waiting on multiple channels at once is not supported.
// - **Zero-copy**: Optimizations to pass ownership without moving memory if possible.

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_simple_send_recv() {
        let (tx, rx) = unbounded();
        tx.send(1).unwrap();
        tx.send(2).unwrap();
        tx.send(3).unwrap();

        assert_eq!(rx.recv(), Ok(1));
        assert_eq!(rx.recv(), Ok(2));
        assert_eq!(rx.recv(), Ok(3));
    }

    #[test]
    fn test_multiple_senders() {
        let (tx, rx) = unbounded();

        for i in 0..10 {
            let tx = tx.clone();
            thread::spawn(move || {
                tx.send(i).unwrap();
            });
        }
        // Drop the original sender so the receiver knows when to stop
        drop(tx);

        let mut results: Vec<i32> = rx.collect();
        results.sort();

        let expected: Vec<i32> = (0..10).collect();
        assert_eq!(results, expected);
    }

    #[test]
    fn test_recv_blocks() {
        let (tx, rx) = unbounded();

        let t = thread::spawn(move || {
            thread::sleep(Duration::from_millis(50));
            tx.send(42).unwrap();
        });

        assert_eq!(rx.recv(), Ok(42));
        t.join().unwrap();
    }

    #[test]
    fn test_bounded_blocks_sender() {
        let (tx, rx) = bounded(1);

        // Fill channel
        tx.send(1).unwrap();

        let tx_clone = tx.clone();
        let t = thread::spawn(move || {
            // This should block until we read from rx
            tx_clone.send(2).unwrap();
        });

        // Give the thread time to block
        thread::sleep(Duration::from_millis(50));

        assert_eq!(rx.recv(), Ok(1));
        // Now sender can proceed

        assert_eq!(rx.recv(), Ok(2));
        t.join().unwrap();
    }

    #[test]
    fn test_disconnect() {
        let (tx, rx) = unbounded::<()>();
        drop(tx);
        assert_eq!(rx.recv(), Err(RecvError));
    }

    #[test]
    fn test_try_recv() {
        let (tx, rx) = unbounded();
        assert_eq!(rx.try_recv(), Err(TryRecvError::Empty));

        tx.send(1).unwrap();
        assert_eq!(rx.try_recv(), Ok(1));

        drop(tx);
        assert_eq!(rx.try_recv(), Err(TryRecvError::Disconnected));
    }
}
