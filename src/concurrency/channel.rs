//! # Multi-Producer Single-Consumer (MPSC) Channel Implementation
//!
//! A thread-safe communication primitive for passing messages between threads.
//!
//! **Replaces Crates:** `std::sync::mpsc`, `crossbeam-channel`, `flume`
//!
//! **Real-world Usage:**
//! - Job queues (thread pools).
//! - Event loops (GUI frameworks).
//! - Actor systems.
//!
//! **Why build it yourself?**
//! Channels are the idiomatic way to communicate in Rust. Building one demystifies `Arc<Mutex<>>`,
//! `Condvar`, and proper signal handling. You'll understand why `send` blocks and how `Drop`
//! coordinates shutdown.

use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Shared State:
// - Queue: Stores messages.
// - Capacity: Max messages (0 for effectively unbounded, but we'll stick to fixed size).
// - Senders Count: To know when all producers are gone.
// - Receiver Active: To know if the consumer is gone (send should fail).
//
// Synchronization:
// - Mutex: Protects the shared state.
// - Condvar (senders): Wait for space available.
// - Condvar (receiver): Wait for data available.
//
// Invariants:
// 1. Messages are FIFO.
// 2. Send blocks if full.
// 3. Recv blocks if empty.
// 4. Send fails if receiver dropped.
// 5. Recv returns None if empty AND all senders dropped.

struct Shared<T> {
    queue: VecDeque<T>,
    capacity: usize,
    senders_count: usize,
    receiver_active: bool,
}

/// The sending end of the channel.
pub struct Sender<T> {
    shared: Arc<Mutex<Shared<T>>>,
    available: Arc<Condvar>,
    received: Arc<Condvar>,
}

/// The receiving end of the channel.
pub struct Receiver<T> {
    shared: Arc<Mutex<Shared<T>>>,
    available: Arc<Condvar>, // Signaled when space is available (item removed)
    received: Arc<Condvar>,  // Signaled when item is added
}

// Errors
#[derive(Debug, PartialEq, Eq)]
pub enum SendError<T> {
    Disconnected(T),
}

#[derive(Debug, PartialEq, Eq)]
pub enum RecvError {
    Empty,
    Disconnected,
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        let mut guard = self.shared.lock().unwrap();
        guard.senders_count += 1;
        drop(guard);

        Self {
            shared: self.shared.clone(),
            available: self.available.clone(),
            received: self.received.clone(),
        }
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        let mut guard = self.shared.lock().unwrap();
        guard.senders_count -= 1;
        let is_last_sender = guard.senders_count == 0;
        drop(guard);

        if is_last_sender {
            // Wake up receiver so it sees there are no more senders
            self.received.notify_all();
        }
    }
}

impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        let mut guard = self.shared.lock().unwrap();
        guard.receiver_active = false;
        drop(guard);

        // Wake up senders so they fail
        self.available.notify_all();
    }
}

impl<T> Sender<T> {
    /// Sends a message into the channel. Blocks if the channel is full.
    ///
    /// # Errors
    ///
    /// Returns `SendError::Disconnected` if the receiver has been dropped.
    ///
    /// # Panics
    ///
    /// Panics if the shared-state `Mutex` is poisoned.
    pub fn send(&self, t: T) -> Result<(), SendError<T>> {
        let mut guard = self.shared.lock().unwrap();

        loop {
            if !guard.receiver_active {
                return Err(SendError::Disconnected(t));
            }

            if guard.queue.len() < guard.capacity {
                guard.queue.push_back(t);
                // Notify receiver that data is available
                self.received.notify_one();
                return Ok(());
            }

            // Channel is full, wait for space.
            guard = self.available.wait(guard).unwrap();
        }
    }
}

impl<T> Receiver<T> {
    /// Receives a message from the channel. Blocks if empty.
    ///
    /// # Errors
    ///
    /// Returns `RecvError::Disconnected` if all senders have been dropped
    /// and the channel is empty.
    ///
    /// # Panics
    ///
    /// Panics if the shared-state `Mutex` is poisoned.
    pub fn recv(&self) -> Result<T, RecvError> {
        let mut guard = self.shared.lock().unwrap();

        loop {
            if let Some(t) = guard.queue.pop_front() {
                // Notify senders that space is available
                self.available.notify_one();
                return Ok(t);
            }

            if guard.senders_count == 0 {
                return Err(RecvError::Disconnected);
            }

            // Channel is empty, wait for data.
            guard = self.received.wait(guard).unwrap();
        }
    }

    /// Non-blocking receive.
    ///
    /// # Errors
    ///
    /// Returns `RecvError::Empty` if no message is currently available, or
    /// `RecvError::Disconnected` if all senders have been dropped.
    ///
    /// # Panics
    ///
    /// Panics if the shared-state `Mutex` is poisoned.
    pub fn try_recv(&self) -> Result<T, RecvError> {
        let mut guard = self.shared.lock().unwrap();

        guard.queue.pop_front().map_or_else(
            || {
                if guard.senders_count == 0 {
                    Err(RecvError::Disconnected)
                } else {
                    Err(RecvError::Empty)
                }
            },
            |t| {
                self.available.notify_one();
                Ok(t)
            },
        )
    }

    /// Receive with timeout.
    ///
    /// # Errors
    ///
    /// Returns `RecvError::Empty` if the timeout elapses before a message
    /// arrives, or `RecvError::Disconnected` if all senders have been dropped.
    ///
    /// # Panics
    ///
    /// Panics if the shared-state `Mutex` is poisoned.
    pub fn recv_timeout(&self, timeout: Duration) -> Result<T, RecvError> {
        let mut guard = self.shared.lock().unwrap();
        let now = std::time::Instant::now();

        loop {
            if let Some(t) = guard.queue.pop_front() {
                self.available.notify_one();
                return Ok(t);
            }

            if guard.senders_count == 0 {
                return Err(RecvError::Disconnected);
            }

            let elapsed = now.elapsed();
            if elapsed >= timeout {
                return Err(RecvError::Empty); // Timeout treated as Empty
            }

            // wait_timeout returns (guard, wait_timeout_result)
            let (new_guard, _) = self
                .received
                .wait_timeout(guard, timeout.checked_sub(elapsed).unwrap())
                .unwrap();
            guard = new_guard;
        }
    }
}

/// Creates a new bounded channel.
///
/// # Panics
///
/// Panics if `capacity` is zero.
#[must_use]
pub fn channel<T>(capacity: usize) -> (Sender<T>, Receiver<T>) {
    assert!(capacity > 0, "Capacity must be greater than 0");

    let shared = Arc::new(Mutex::new(Shared {
        queue: VecDeque::with_capacity(capacity),
        capacity,
        senders_count: 1,
        receiver_active: true,
    }));

    let available = Arc::new(Condvar::new());
    let received = Arc::new(Condvar::new());

    (
        Sender {
            shared: shared.clone(),
            available: available.clone(),
            received: received.clone(),
        },
        Receiver {
            shared,
            available,
            received,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_simple_send_recv() {
        let (tx, rx) = channel(1);
        tx.send(42).unwrap();
        assert_eq!(rx.recv().unwrap(), 42);
    }

    #[test]
    fn test_concurrent_send() {
        let (tx, rx) = channel(10);
        let mut handles = vec![];

        for i in 0..5 {
            let tx = tx.clone();
            handles.push(thread::spawn(move || {
                tx.send(i).unwrap();
            }));
        }

        // Drop original tx so receiver knows when to stop
        drop(tx);

        let mut received = vec![];
        while let Ok(val) = rx.recv() {
            received.push(val);
        }

        received.sort_unstable();
        assert_eq!(received, vec![0, 1, 2, 3, 4]);

        for h in handles {
            h.join().unwrap();
        }
    }

    #[test]
    fn test_bounded_blocking() {
        let (tx, rx) = channel(1);

        // Fill channel
        tx.send(1).unwrap();

        let tx_clone = tx;
        let handle = thread::spawn(move || {
            // This should block until rx receives
            tx_clone.send(2).unwrap();
        });

        // Sleep to ensure thread is blocked
        thread::sleep(std::time::Duration::from_millis(50));

        assert_eq!(rx.recv().unwrap(), 1);
        // Now thread unblocks
        handle.join().unwrap();
        assert_eq!(rx.recv().unwrap(), 2);
    }

    #[test]
    fn test_disconnect_receiver() {
        let (tx, rx) = channel(10);
        drop(rx);
        assert_eq!(tx.send(1), Err(SendError::Disconnected(1)));
    }

    #[test]
    fn test_disconnect_sender() {
        let (tx, rx) = channel(10);
        tx.send(1).unwrap();
        drop(tx);

        assert_eq!(rx.recv(), Ok(1));
        assert_eq!(rx.recv(), Err(RecvError::Disconnected));
    }
}
