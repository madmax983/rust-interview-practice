//! # Spin-based Mutex Implementation
//!
//! A mutual exclusion primitive useful for protecting shared data.
//! This implementation uses a spinlock approach.
//!
//! **Replaces Crates:** `std::sync::Mutex`, `spin::Mutex`
//!
//! **Real-world Usage:**
//! - Protecting shared variables in concurrent systems.
//! - Operating system kernels (where thread blocking is not possible).
//! - Microcontrollers and embedded systems without an OS scheduler.
//!
//! **Why build it yourself?**
//! Implementing a basic Mutex reveals how hardware atomics (`AtomicBool`) combined with
//! compiler fences (`Ordering::Acquire` / `Ordering::Release`) create memory safety.
//! It also demonstrates Rust's interior mutability pattern via `UnsafeCell` and how to use
//! a `Drop` guard (`MutexGuard`) to tie lock release to scope exit, preventing leaked locks.

use std::cell::UnsafeCell;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicBool, Ordering};

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure:
//
//      Mutex<T>
//         │
//         ▼
//      ┌───────────────┐
//      │ is_locked:    │ ──► AtomicBool (false = unlocked, true = locked)
//      │ AtomicBool    │
//      ├───────────────┤
//      │ data:         │ ──► UnsafeCell<T> (holds the actual data, allows interior mutability)
//      │ UnsafeCell<T> │
//      └───────────────┘
//
// Invariants:
// 1. Only one thread can have `is_locked` set to `true` at a time.
// 2. Data inside `UnsafeCell` is only accessed when `is_locked` is `true`.
// 3. Releasing the lock (`is_locked` = false) happens automatically when the guard is dropped.
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ Lock          │ O(1)*       │ O(1)        │
// │ Unlock (Drop) │ O(1)        │ O(1)        │
// └───────────────┴─────────────┴─────────────┘
// * In a spinlock, lock time depends on contention. Can spin infinitely if the lock holder deadlocks.

/// A mutual exclusion primitive useful for protecting shared data.
pub struct Mutex<T> {
    is_locked: AtomicBool,
    // RUST INSIGHT:
    // UnsafeCell is the *only* way to mutate data through an immutable reference (`&self`) in safe Rust.
    // It opts out of the compiler's strict alias analysis for this specific memory location.
    data: UnsafeCell<T>,
}

// UNSAFE JUSTIFICATION:
// Mutex<T> can be sent across threads safely as long as the underlying data T can be sent across threads.
// Mutex<T> can be shared across threads safely (Sync) as long as T can be sent across threads (Send).
// Why `T: Send` for `Sync`? Because thread A might lock the Mutex, mutate T, and then unlock it.
// Thread B might then lock the Mutex, effectively transferring ownership/mutation of T from A to B.
unsafe impl<T: Send> Sync for Mutex<T> {}
unsafe impl<T: Send> Send for Mutex<T> {}

impl<T> Mutex<T> {
    /// Creates a new mutex in an unlocked state ready for use.
    pub const fn new(data: T) -> Self {
        Self {
            is_locked: AtomicBool::new(false),
            data: UnsafeCell::new(data),
        }
    }

    /// Acquires a mutex, blocking the current thread until it is able to do so.
    /// This function will block the local thread until it is available to acquire the mutex.
    /// Upon returning, the thread is the only thread with the mutex held. An RAII guard is returned to allow scoped unlock of the lock.
    pub fn lock(&self) -> MutexGuard<'_, T> {
        // Spin until we successfully swap `false` (unlocked) to `true` (locked).
        // PRODUCTION NOTE:
        // A naive spinlock constantly executes `compare_exchange_weak`, causing intense
        // cache-line invalidation traffic. A more optimized approach is to loop on a cheap
        // relaxed `load()` until it appears unlocked *before* attempting the atomic swap.
        loop {
            // Fast path: attempt to acquire the lock.
            // GOTCHA: `compare_exchange_weak` inside a loop is generally preferred over `compare_exchange`
            // on some architectures (like ARM) because it can fail spuriously but is faster on success.
            if self.is_locked.compare_exchange_weak(
                false,
                true,
                Ordering::Acquire,
                Ordering::Relaxed,
            ).is_ok() {
                break;
            }

            // Spin while the lock is held, avoiding expensive cache invalidations.
            while self.is_locked.load(Ordering::Relaxed) {
                // RUST INSIGHT:
                // `std::hint::spin_loop()` tells the CPU we are in a busy-wait loop.
                // It prevents pipeline stalls, reduces power consumption, and allows other
                // hyperthreads on the same core to run faster.
                std::hint::spin_loop();
            }
        }

        MutexGuard { mutex: self }
    }
}

/// An RAII implementation of a "scoped lock" of a mutex.
/// When this structure is dropped (falls out of scope), the lock will be unlocked.
///
/// The data protected by the mutex can be accessed through this guard via its `Deref` and `DerefMut` implementations.
pub struct MutexGuard<'a, T> {
    mutex: &'a Mutex<T>,
}

impl<T> Deref for MutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // UNSAFE JUSTIFICATION:
        // We know this is safe because the existence of this MutexGuard implies
        // the current thread has successfully set `is_locked` to true. No other thread
        // can have a MutexGuard, ensuring exclusive access.
        unsafe { &*self.mutex.data.get() }
    }
}

impl<T> DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // UNSAFE JUSTIFICATION:
        // See Deref justification. Exclusive lock means exclusive mutability is safe.
        unsafe { &mut *self.mutex.data.get() }
    }
}

impl<T> Drop for MutexGuard<'_, T> {
    fn drop(&mut self) {
        // RUST INSIGHT:
        // Ordering::Release ensures that all memory operations (writes to the data)
        // that happened *before* the unlock are visible to any thread that subsequently
        // acquires the lock (which uses Ordering::Acquire).
        self.mutex.is_locked.store(false, Ordering::Release);
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to `std::sync::Mutex`:
// - `std::sync::Mutex` uses OS-level primitives (like `futex` on Linux or SRW locks on Windows)
//   to actually put the thread to sleep when blocked, rather than spinning and wasting CPU cycles.
// - `std::sync::Mutex` handles "poisoning" — if a thread panics while holding the lock, the lock
//   is poisoned to prevent other threads from seeing potentially inconsistent state.
//
// Missing vs Production:
// - Thread sleeping (OS integration).
// - Poisoning mechanism.
// - `try_lock` method.
//
// Next Steps:
// - Implement poisoning by adding a second atomic flag or state enum.
// - Add a `try_lock` method that returns an `Option<MutexGuard>`.
//
// Benchmarking Note:
// To benchmark this spinlock against `std::sync::Mutex` or `parking_lot::Mutex`,
// spawn N threads contending on the same lock and use `criterion` to measure throughput
// of short and long critical sections using `std::hint::black_box`.

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_mutex_basic() {
        let m = Mutex::new(0);
        {
            let mut guard = m.lock();
            *guard += 1;
        } // unlocked here

        let guard = m.lock();
        assert_eq!(*guard, 1);
    }

    #[test]
    fn test_mutex_multithreaded() {
        let m = Arc::new(Mutex::new(0));
        let mut handles = vec![];

        for _ in 0..10 {
            let m_clone = m.clone();
            handles.push(thread::spawn(move || {
                for _ in 0..1000 {
                    let mut guard = m_clone.lock();
                    *guard += 1;
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let guard = m.lock();
        assert_eq!(*guard, 10000);
    }
}
