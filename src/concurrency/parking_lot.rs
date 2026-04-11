//! # Parking Lot Mutex Implementation
//!
//! An efficient, user-space Mutex implementation using a global "parking lot" for blocked threads.
//!
//! **Replaces Crates:** `parking_lot`, `spin`
//!
//! **Real-world Usage:**
//! - Core synchronization primitive in high-performance Rust applications.
//! - WebKit's WTF::Lock (which inspired parking_lot).
//! - JVM monitors and Linux futexes use similar concepts.
//!
//! **Why build it yourself?**
//! Building a parking lot mutex demystifies how `std::sync::Mutex` (historically pthreads/sysv)
//! or `parking_lot::Mutex` works under the hood. It teaches you how to decouple the lock's state
//! (which should be as small as possible, e.g., 1 byte) from the wait queue (which can be large
//! but is only needed when there's contention).

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure Diagram:
//
//      Mutex (AtomicU8, 1 byte)
//        │
//        ▼ [On Contention: address of Mutex used as key]
//
//   [Global Parking Lot: Mutex<HashMap<usize, VecDeque<Thread>>>]
//        │
//        ▼
//   [Address 0x123] ──► [Parked Thread A, Parked Thread B]
//   [Address 0x456] ──► [Parked Thread C]
//
// Invariants:
// 1. The Mutex state has 3 values: UNLOCKED (0), LOCKED (1), LOCKED_WITH_PARKED (2).
// 2. A thread must only park itself if it observes the state is LOCKED_WITH_PARKED.
// 3. The lock holder is responsible for unparking a thread if the state is LOCKED_WITH_PARKED.
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ lock (fast)   │ O(1)        │ O(1)        │
// │ lock (slow)   │ O(1)*       │ O(T)        │
// │ unlock (fast) │ O(1)        │ O(1)        │
// │ unlock (slow) │ O(1)*       │ O(1)        │
// └───────────────┴─────────────┴─────────────┘
// * Slow path involves global hash map lookup and OS context switch. Space is O(T) where T is parked threads.
//
// Design Decisions:
// - **Global Parking Lot**: We use a `std::sync::Mutex` to protect the global hash map.
//   - *Tradeoff*: In a highly contended system, the global lock becomes a bottleneck.
//   - *Alternative*: Production `parking_lot` uses an array of buckets (e.g., 256) to shard the global lock.
// - **Thread Parking**: We use `std::thread::park` and `Thread::unpark`.
//   - *Tradeoff*: Simple and available in `std`.
//   - *Alternative*: OS-specific primitives like `futex` on Linux, `WaitOnAddress` on Windows.

use std::cell::UnsafeCell;
use std::collections::{HashMap, VecDeque};
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Mutex as StdMutex, OnceLock};
use std::thread::{self, Thread};

// States for the AtomicU8
const UNLOCKED: u8 = 0;
const LOCKED: u8 = 1;
const LOCKED_WITH_PARKED: u8 = 2;

/// The global parking lot.
/// We use `std::sync::Mutex` here just to bootstrap our own lock, though in reality
/// you'd use a spinlock or OS primitive to protect this.
struct ParkingLot {
    queues: HashMap<usize, VecDeque<Thread>>,
}

impl ParkingLot {
    fn new() -> Self {
        Self {
            queues: HashMap::new(),
        }
    }
}

// Global instance of the parking lot
static GLOBAL_PARKING_LOT: OnceLock<StdMutex<ParkingLot>> = OnceLock::new();

fn global_lot() -> &'static StdMutex<ParkingLot> {
    GLOBAL_PARKING_LOT.get_or_init(|| StdMutex::new(ParkingLot::new()))
}

/// A Mutex that uses a global parking lot to sleep threads when contended.
/// Its size is only exactly what's needed for the atomic state + the inner data.
pub struct Mutex<T> {
    state: AtomicU8,
    data: UnsafeCell<T>,
}

// UNSAFE JUSTIFICATION:
// Mutex<T> safely synchronizes access to `data` using the `state` atomic variable.
// As long as T is Send, it's safe to send Mutex<T> across threads.
// If T is Send, it's also safe to share Mutex<T> across threads (Sync).
unsafe impl<T: Send> Send for Mutex<T> {}
unsafe impl<T: Send> Sync for Mutex<T> {}

impl<T> Mutex<T> {
    /// Creates a new Mutex in the unlocked state.
    pub const fn new(data: T) -> Self {
        Self {
            state: AtomicU8::new(UNLOCKED),
            data: UnsafeCell::new(data),
        }
    }

    /// Acquires the lock, blocking the current thread until it is available.
    pub fn lock(&self) -> MutexGuard<'_, T> {
        // Fast path: try to lock assuming it's unlocked.
        if self
            .state
            .compare_exchange(UNLOCKED, LOCKED, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            return MutexGuard { lock: self };
        }

        // Slow path
        self.lock_slow();
        MutexGuard { lock: self }
    }

    fn lock_slow(&self) {
        let mut spin_count = 0;

        loop {
            // Spin a few times before we decide to park.
            // RUST INSIGHT: Spinning can be faster than sleeping if the lock is held for a short time,
            // but we shouldn't spin indefinitely to avoid wasting CPU cycles.
            let mut state = self.state.load(Ordering::Relaxed);

            if state == UNLOCKED {
                if self
                    .state
                    .compare_exchange_weak(UNLOCKED, LOCKED, Ordering::Acquire, Ordering::Relaxed)
                    .is_ok()
                {
                    return;
                }
                continue;
            }

            if state == LOCKED && spin_count < 100 {
                std::hint::spin_loop();
                spin_count += 1;
                continue;
            }

            // Lock is either LOCKED and we spun enough, or already LOCKED_WITH_PARKED.
            // Transition to LOCKED_WITH_PARKED so the unlocker knows to wake us up.
            if state == LOCKED {
                if let Err(new_state) = self.state.compare_exchange(
                    LOCKED,
                    LOCKED_WITH_PARKED,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    state = new_state;
                    if state == UNLOCKED {
                        continue;
                    }
                } else {
                    state = LOCKED_WITH_PARKED;
                }
            }

            if state == LOCKED_WITH_PARKED {
                let address = self as *const Mutex<T> as usize;
                let current_thread = thread::current();

                {
                    // GOTCHA: We must acquire the global parking lot lock *before* we check the state
                    // again. If we park without checking, we might miss an unlock and sleep forever (lost wakeup).
                    let mut lot = global_lot().lock().unwrap();

                    // Re-check state. If it's UNLOCKED, the lock was released before we got the global lot lock.
                    if self.state.load(Ordering::Relaxed) == UNLOCKED {
                        continue; // Try to acquire the lock again
                    }

                    // Enqueue ourselves, avoiding duplicates if we woke up spuriously
                    let queue = lot.queues.entry(address).or_default();
                    if !queue.iter().any(|t| t.id() == current_thread.id()) {
                        queue.push_back(current_thread);
                    }
                }

                // Park the thread. It will be unparked by `unlock()`.
                // Note: `thread::park` can suffer from spurious wakeups. Since we loop the entire slow path,
                // we prevent pushing duplicates into the wait queue above.
                // PRODUCTION NOTE: A real implementation would loop checking `park` with a token or condition.
                thread::park();
            }
        }
    }

    /// Releases the lock.
    fn unlock(&self) {
        // Fast path: Try to transition from LOCKED to UNLOCKED.
        if self
            .state
            .compare_exchange(LOCKED, UNLOCKED, Ordering::Release, Ordering::Relaxed)
            .is_ok()
        {
            return;
        }

        // Slow path: It's likely LOCKED_WITH_PARKED.
        // First, set it to UNLOCKED.
        self.state.store(UNLOCKED, Ordering::Release);

        // Now, wake up one thread from the parking lot.
        let address = self as *const Mutex<T> as usize;

        let mut lot = global_lot().lock().unwrap();
        if let Some(queue) = lot.queues.get_mut(&address) {
            if let Some(thread) = queue.pop_front() {
                // RUST INSIGHT: `Thread::unpark` is safe to call even if the thread hasn't parked yet.
                // The unpark token is saved, so the thread's next `park` will immediately return.
                thread.unpark();
            }

            // Cleanup empty queues to prevent memory leaks
            if queue.is_empty() {
                lot.queues.remove(&address);
            }
        }
    }
}

/// An RAII guard that releases the lock when dropped.
pub struct MutexGuard<'a, T> {
    lock: &'a Mutex<T>,
}

// UNSAFE JUSTIFICATION:
// Deref allows safe access to the inner data because the Mutex ensures exclusive access.
impl<'a, T> Deref for MutexGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<'a, T> DerefMut for MutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<'a, T> Drop for MutexGuard<'a, T> {
    fn drop(&mut self) {
        self.lock.unlock();
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `parking_lot`: The `parking_lot` crate is incredibly highly optimized. It uses a sharded hash map
//   (bucket array) to reduce contention on the global lock. It also handles fairness, timeouts,
//   and uses OS-specific APIs (futex on Linux, wait-on-address on Windows) to park threads directly
//   without needing an extra Mutex wrapper.
// - `std::sync::Mutex`: Historically wrapped pthread_mutex / SRWLock, but now uses `futex` directly
//   on Linux just like `parking_lot`, though slightly different internals.
//
// What's missing vs. production:
// - **Sharding**: Our global lock is a single `std::sync::Mutex`. High contention on different locks
//   will serialize all threads in the system.
// - **Fairness**: We don't guarantee that the longest waiting thread gets the lock next. A spinning
//   new thread can steal the lock from an unparked thread (barging). Production parking lot handles this.
// - **Memory Footprint**: `std::thread::Thread` handles allocation, whereas raw futex uses only 4 bytes.
//
// Next Steps:
// 1. Shard the `GLOBAL_PARKING_LOT` into an array of 256 mutexes keyed by a hash of the address.
// 2. Implement a `Condvar` that integrates with this parking lot.
// 3. Add timeouts (`lock_timeout`).
//
// Benchmarking Note:
// Use `criterion` to benchmark this against `std::sync::Mutex` and `parking_lot::Mutex`.
// 1. Test uncontended performance (lock immediately followed by unlock) using `std::hint::black_box`.
// 2. Test highly contended performance (many threads incrementing a shared counter).

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_mutex_basic() {
        let mutex = Mutex::new(0);
        {
            let mut guard = mutex.lock();
            *guard += 1;
        }
        {
            let guard = mutex.lock();
            assert_eq!(*guard, 1);
        }
    }

    #[test]
    fn test_mutex_concurrency() {
        let mutex = Arc::new(Mutex::new(0));
        let mut handles = vec![];

        for _ in 0..10 {
            let m = Arc::clone(&mutex);
            handles.push(thread::spawn(move || {
                for _ in 0..1000 {
                    let mut guard = m.lock();
                    *guard += 1;
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let guard = mutex.lock();
        assert_eq!(*guard, 10000);
    }

    #[test]
    fn test_mutex_contention_and_park() {
        // We will force one thread to hold the lock for a long time so others park.
        let mutex = Arc::new(Mutex::new(0));
        let mut handles = vec![];

        // Thread 1 holds lock for 50ms
        let m1 = Arc::clone(&mutex);
        handles.push(thread::spawn(move || {
            let mut guard = m1.lock();
            thread::sleep(Duration::from_millis(50));
            *guard += 1;
        }));

        // Thread 2 waits for lock
        let m2 = Arc::clone(&mutex);
        handles.push(thread::spawn(move || {
            // Wait slightly to ensure Thread 1 gets the lock first
            thread::sleep(Duration::from_millis(10));
            let mut guard = m2.lock();
            *guard += 1;
        }));

        for handle in handles {
            handle.join().unwrap();
        }

        let guard = mutex.lock();
        assert_eq!(*guard, 2);
    }
}
