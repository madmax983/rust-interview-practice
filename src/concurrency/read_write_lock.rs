//! # Reader-Writer Lock Implementation
//!
//! A synchronization primitive that allows multiple concurrent readers or a single exclusive writer.
//! This implementation prioritizes writers to avoid write starvation.
//!
//! **Replaces Crates:** `std::sync::RwLock`, `parking_lot::RwLock`
//!
//! **Real-world Usage:**
//! - Database systems (concurrent reads, exclusive writes).
//! - In-memory caches (frequent reads, occasional updates).
//! - Configuration updates in running services.
//!
//! **Why build it yourself?**
//! Standard `RwLock` implementations can vary in their starvation policy (OS-dependent).
//! Building one reveals the tricky coordination required between readers and writers using
//! basic primitives like `Mutex` and `Condvar`. You'll learn about "Writer Starvation" vs "Reader Starvation".

// Lock guards are intentionally held across condvar waits/notifications;
// do not tighten their scope.
#![allow(clippy::significant_drop_tightening)]

use std::cell::UnsafeCell;
use std::ops::{Deref, DerefMut};
use std::sync::{Condvar, Mutex};

// =========================================================================================
// Architecture
// =========================================================================================
//
// State:
// - `readers`: Number of active readers.
// - `writer_active`: Boolean, true if a writer holds the lock.
// - `writers_waiting`: Number of writers waiting for the lock.
//
// Synchronization:
// - `Mutex<State>`: Protects the state.
// - `Condvar` (readers): Readers wait on this if a writer is active or waiting.
// - `Condvar` (writers): Writers wait on this if readers are active or another writer is active.
//
// Invariants:
// 1. If `writer_active` is true, `readers` must be 0.
// 2. If `readers > 0`, `writer_active` must be false.
// 3. Data access is only allowed when holding a guard.

struct State {
    readers: usize,
    writer_active: bool,
    writers_waiting: usize,
}

/// A writer-preferred Reader-Writer Lock.
pub struct RwLock<T> {
    data: UnsafeCell<T>,
    state: Mutex<State>,
    reader_cond: Condvar,
    writer_cond: Condvar,
}

// UNSAFE JUSTIFICATION:
// - `RwLock` coordinates access to `UnsafeCell`.
// - It enforces exclusive access for `WriteGuard` (mutable reference).
// - It enforces shared access for `ReadGuard` (immutable reference).
// - `T` must be `Send` because threads can access it.
// - `T` must be `Sync` because multiple readers can access it simultaneously.
unsafe impl<T: Send + Sync> Sync for RwLock<T> {}
unsafe impl<T: Send> Send for RwLock<T> {}

pub struct ReadGuard<'a, T> {
    lock: &'a RwLock<T>,
}

pub struct WriteGuard<'a, T> {
    lock: &'a RwLock<T>,
}

impl<T> RwLock<T> {
    /// Creates a new `RwLock`.
    pub const fn new(t: T) -> Self {
        Self {
            data: UnsafeCell::new(t),
            state: Mutex::new(State {
                readers: 0,
                writer_active: false,
                writers_waiting: 0,
            }),
            reader_cond: Condvar::new(),
            writer_cond: Condvar::new(),
        }
    }

    /// Acquires a shared lock for reading.
    /// Blocks if a writer is active or waiting (Writer Preference).
    ///
    /// # Panics
    ///
    /// Panics if the internal state `Mutex` is poisoned.
    pub fn read(&self) -> ReadGuard<'_, T> {
        let mut state = self.state.lock().unwrap();

        // Writer Preference: Wait if a writer is active OR waiting.
        // This prevents new readers from starving pending writers.
        while state.writer_active || state.writers_waiting > 0 {
            state = self.reader_cond.wait(state).unwrap();
        }

        state.readers += 1;
        ReadGuard { lock: self }
    }

    /// Acquires an exclusive lock for writing.
    /// Blocks if any readers or a writer are active.
    ///
    /// # Panics
    ///
    /// Panics if the internal state `Mutex` is poisoned.
    pub fn write(&self) -> WriteGuard<'_, T> {
        let mut state = self.state.lock().unwrap();

        state.writers_waiting += 1;

        // Wait until no readers and no active writer.
        while state.readers > 0 || state.writer_active {
            state = self.writer_cond.wait(state).unwrap();
        }

        state.writers_waiting -= 1;
        state.writer_active = true;

        WriteGuard { lock: self }
    }
}

impl<T> Deref for ReadGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        // SAFETY: The guard ensures we have shared access.
        unsafe { &*self.lock.data.get() }
    }
}

impl<T> Drop for ReadGuard<'_, T> {
    fn drop(&mut self) {
        let mut state = self.lock.state.lock().unwrap();
        state.readers -= 1;

        if state.readers == 0 {
            // Last reader exits, wake up one writer.
            // We prioritize writers, so we notify `writer_cond`.
            self.lock.writer_cond.notify_one();
        }
    }
}

impl<T> Deref for WriteGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        // SAFETY: The guard ensures we have exclusive access.
        unsafe { &*self.lock.data.get() }
    }
}

impl<T> DerefMut for WriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: The guard ensures we have exclusive access.
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<T> Drop for WriteGuard<'_, T> {
    fn drop(&mut self) {
        let mut state = self.lock.state.lock().unwrap();
        state.writer_active = false;

        if state.writers_waiting > 0 {
            // Wake up one writer if any are waiting.
            self.lock.writer_cond.notify_one();
        } else {
            // No writers waiting, wake up all readers.
            self.lock.reader_cond.notify_all();
        }
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `std::sync::RwLock`: Uses OS primitives (pthreads/SRWLock). Policy is platform-dependent (often reader-preferred or fair).
// - `parking_lot::RwLock`: Uses efficient user-space primitives (parking) and guarantees fairness (no starvation).
//
// Missing vs. Production:
// - **Fairness**: This implementation is strictly writer-preferred. It can starve readers if writers are continuous.
// - **Poisoning**: Standard Mutexes/RwLocks handle thread panics by "poisoning" the lock. We unwrap and panic.
// - **Optimizations**: We use a `Mutex` to protect the state. Production implementations use atomic operations and only park threads when necessary (fast path / slow path).

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_rwlock_basic_read_write() {
        let lock = RwLock::new(5);
        {
            let r1 = lock.read();
            assert_eq!(*r1, 5);
            let r2 = lock.read();
            assert_eq!(*r2, 5);
        } // Readers dropped

        {
            let mut w = lock.write();
            *w = 10;
        } // Writer dropped

        assert_eq!(*lock.read(), 10);
    }

    #[test]
    fn test_writer_preference() {
        let lock = Arc::new(RwLock::new(0));
        let lock_clone = lock.clone();

        // Start a reader that holds the lock for a bit
        let r1 = lock.read();

        // Spawn a writer thread that will block waiting for r1
        let writer_thread = thread::spawn(move || {
            let mut w = lock_clone.write();
            *w += 1;
        });

        // Sleep to ensure writer is waiting
        thread::sleep(Duration::from_millis(50));

        // Attempt to start another reader.
        // If writer preference is working, this reader should block until the writer is done.
        // We can't easily assert "blocking", but we can check the order of operations implicitly or use timeouts.
        // But here, we just verify correctness.

        let lock_clone2 = lock.clone();
        let reader_thread = thread::spawn(move || {
            let r = lock_clone2.read();
            assert_eq!(*r, 1); // Should see the write
        });

        // Drop r1, allowing writer to proceed
        drop(r1);

        writer_thread.join().unwrap();
        reader_thread.join().unwrap();
    }
}
