//! # Parking Lot Mutex Implementation
//!
//! An efficient, user-space Mutex implementation using a global "parking lot" for blocked threads.
//!
//! **Replaces Crates:** `parking_lot`, `spin`
//!
//! **Real-world Usage:**
//! - Core synchronization primitive in high-performance Rust applications.
//! - `WebKit`'s `WTF::Lock` (which inspired `parking_lot`).
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

// The global parking-lot lock is intentionally held across enqueue/dequeue and
// hand-off critical sections; do not tighten its scope.
#![allow(clippy::significant_drop_tightening)]

use std::cell::UnsafeCell;
use std::collections::{HashMap, VecDeque};
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex as StdMutex, OnceLock};
use std::thread::{self, Thread};

// States for the AtomicU8
const UNLOCKED: u8 = 0;
const LOCKED: u8 = 1;
const LOCKED_WITH_PARKED: u8 = 2;

/// A single parked waiter.
///
/// `granted` is the direct-handoff signal: the thread releasing the lock sets it
/// to `true` (while holding the lot lock) immediately before unparking this
/// waiter. When the waiter wakes it inspects `granted` to learn whether it now
/// owns the lock (a real handoff) or merely woke spuriously. Because ownership is
/// transferred without the lock ever returning to the `UNLOCKED` state, no other
/// thread can barge in and strand a queued waiter.
struct Waiter {
    thread: Thread,
    granted: Arc<AtomicBool>,
}

/// The global parking lot.
/// We use `std::sync::Mutex` here just to bootstrap our own lock, though in reality
/// you'd use a spinlock or OS primitive to protect this.
struct ParkingLot {
    queues: HashMap<usize, VecDeque<Waiter>>,
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
        let address = std::ptr::from_ref::<Self>(self) as usize;
        let current_thread = thread::current();
        // Our personal handoff flag. `unlock` sets this to `true` (under the lot
        // lock) when it hands us the lock directly, so a spurious `park` wakeup
        // can be told apart from a real grant.
        let granted = Arc::new(AtomicBool::new(false));
        // Whether we currently have an entry in the global parking queue.
        let mut enqueued = false;

        loop {
            // Spin a few times before we decide to park.
            // RUST INSIGHT: Spinning can be faster than sleeping if the lock is held for a short time,
            // but we shouldn't spin indefinitely to avoid wasting CPU cycles.
            let state = self.state.load(Ordering::Relaxed);

            if state == UNLOCKED {
                // The lock is free; try to grab it. We acquire with plain LOCKED:
                // if there are still other waiters in the queue, `unlock` finds
                // them via the queue (not via the state bits), so a bit-1
                // acquisition can never strand them.
                if self
                    .state
                    .compare_exchange_weak(UNLOCKED, LOCKED, Ordering::Acquire, Ordering::Relaxed)
                    .is_ok()
                {
                    // We grabbed the lock ourselves. If we were still enqueued
                    // (woke spuriously and then acquired), remove our own entry so
                    // a later `unlock` cannot hand the lock to a phantom waiter.
                    if enqueued {
                        Self::dequeue_self(address, &current_thread);
                    }
                    return;
                }
                continue;
            }

            if state == LOCKED && spin_count < 100 && !enqueued {
                std::hint::spin_loop();
                spin_count += 1;
                continue;
            }

            // The lock is held (LOCKED or LOCKED_WITH_PARKED). Publish that there
            // are waiters, enqueue ourselves, and park.
            {
                // GOTCHA: acquire the global parking-lot lock *before* re-checking
                // the state, so our "enqueue + park" is ordered against `unlock`'s
                // "dequeue + hand off", both of which run under this same lock.
                let mut lot = global_lot().lock().unwrap();

                // Mark the lock as having parked waiters. If it turns out to be
                // free again, retry the acquire instead of parking.
                //
                // Ok: transitioned to LOCKED_WITH_PARKED.
                // Err(UNLOCKED): became free again; go grab it instead of parking.
                // Err(_): already LOCKED_WITH_PARKED, nothing to do.
                if self.state.compare_exchange(
                    LOCKED,
                    LOCKED_WITH_PARKED,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) == Err(UNLOCKED)
                {
                    continue;
                }

                // Enqueue ourselves, avoiding duplicates if we woke up spuriously.
                let queue = lot.queues.entry(address).or_default();
                if !queue.iter().any(|w| w.thread.id() == current_thread.id()) {
                    queue.push_back(Waiter {
                        thread: current_thread.clone(),
                        granted: Arc::clone(&granted),
                    });
                }
                enqueued = true;
            }

            // Park until `unlock` hands us the lock (or a spurious wakeup occurs).
            // `thread::park` can wake spuriously, so we must consult `granted`.
            thread::park();

            if granted.load(Ordering::Acquire) {
                // Direct handoff: `unlock` transferred ownership to us without ever
                // releasing the lock to the UNLOCKED state, and already removed us
                // from the queue. We own the lock; return.
                return;
            }
            // Spurious wakeup (or we were re-parked). We are still enqueued; loop
            // and either acquire (if the lock became free) or park again.
        }
    }

    /// Removes this thread's own entry from the parking queue for `address`.
    ///
    /// Called when we acquired the lock ourselves while still enqueued (e.g. after
    /// a spurious wakeup). Keeping enqueue/dequeue symmetric guarantees the queue
    /// only ever holds genuinely-waiting threads, so `unlock`'s hand-off can never
    /// be wasted on a stale entry.
    fn dequeue_self(address: usize, current_thread: &Thread) {
        let mut lot = global_lot().lock().unwrap();
        if let Some(queue) = lot.queues.get_mut(&address) {
            queue.retain(|w| w.thread.id() != current_thread.id());
            if queue.is_empty() {
                lot.queues.remove(&address);
            }
        }
    }

    /// Releases the lock.
    fn unlock(&self) {
        // Fast path: no parked waiters (state is exactly LOCKED), so release
        // directly without touching the global parking lot.
        if self
            .state
            .compare_exchange(LOCKED, UNLOCKED, Ordering::Release, Ordering::Relaxed)
            .is_ok()
        {
            return;
        }

        // Slow path: state is LOCKED_WITH_PARKED, so waiters may be queued.
        //
        // CORRECTNESS (direct handoff): everything below runs under the lot lock,
        // which is the same lock a parking thread holds while it re-checks the
        // state and enqueues itself. We must decide "hand off vs. fully release"
        // atomically with respect to that, otherwise a waiter could enqueue in a
        // window where we have already stopped looking (a lost wakeup that strands
        // it forever: state == UNLOCKED, non-empty queue, no holder).
        //
        // If there is a waiter, we transfer ownership to it *without* ever
        // returning the lock to the UNLOCKED state: we leave the state LOCKED
        // (or LOCKED_WITH_PARKED if more waiters remain), set the waiter's
        // `granted` flag, and unpark it. Because the lock never looks free, no
        // other thread can win a fast-path acquisition and skip over the queue.
        let address = std::ptr::from_ref::<Self>(self) as usize;

        let mut lot = global_lot().lock().unwrap();

        if let Some(queue) = lot.queues.get_mut(&address) {
            if let Some(waiter) = queue.pop_front() {
                let more = !queue.is_empty();
                if !more {
                    lot.queues.remove(&address);
                }
                // Hand the lock to `waiter`. Keep it held: LOCKED_WITH_PARKED if
                // other waiters remain (so the next unlock also drains the queue),
                // otherwise plain LOCKED.
                self.state.store(
                    if more { LOCKED_WITH_PARKED } else { LOCKED },
                    Ordering::Release,
                );
                waiter.granted.store(true, Ordering::Release);
                // `Thread::unpark` never blocks, so calling it under the lot lock
                // is fine and keeps the grant and the wakeup atomic.
                waiter.thread.unpark();
                return;
            }
            // Queue present but empty: clean it up and fall through to release.
            lot.queues.remove(&address);
        }

        // No waiters: actually release the lock.
        self.state.store(UNLOCKED, Ordering::Release);
    }
}

/// An RAII guard that releases the lock when dropped.
pub struct MutexGuard<'a, T> {
    lock: &'a Mutex<T>,
}

// UNSAFE JUSTIFICATION:
// Deref allows safe access to the inner data because the Mutex ensures exclusive access.
impl<T> Deref for MutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<T> DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<T> Drop for MutexGuard<'_, T> {
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
// - **Fairness**: We use *direct handoff* -- `unlock` transfers ownership to the head of the wait
//   queue without ever returning the lock to the UNLOCKED state, so the FIFO order is honored and
//   a fresh thread cannot barge past a parked waiter. This also closes the lost-wakeup window where
//   a barging fast-path acquire+release could strand a queued waiter. (Production `parking_lot`
//   deliberately allows *some* barging for throughput, using occasional fair handoff to bound
//   starvation.)
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

    /// Regression test for the lost-wakeup deadlock.
    ///
    /// Root cause: `unlock` released the lock to `UNLOCKED` and *then* woke one queued
    /// waiter. When more than one thread was parked, a fresh thread (or the just-woken one)
    /// could win the fast-path acquire (`CAS(UNLOCKED, LOCKED)`) while other waiters were
    /// still queued, then release via the fast-path unlock (`CAS(LOCKED, UNLOCKED)`) without
    /// ever consulting the queue -- permanently stranding those waiters (observed as
    /// state == UNLOCKED, a non-empty queue, and no lock holder). The fix hands the lock
    /// directly to the head of the queue under the parking-lot lock, so the lock is never
    /// observable as free while waiters remain and no thread can barge past them.
    ///
    /// This drives many threads into simultaneous parking and checks completion against a
    /// deadline, so any regression fails fast instead of hanging CI.
    #[test]
    fn test_no_lost_wakeup_deadlock() {
        use std::sync::mpsc;

        const THREADS: usize = 8;
        const ITERS: usize = 2000;

        let mutex = Arc::new(Mutex::new(0usize));
        let (done_tx, done_rx) = mpsc::channel();
        let mut handles = vec![];

        for _ in 0..THREADS {
            let m = Arc::clone(&mutex);
            let tx = done_tx.clone();
            handles.push(thread::spawn(move || {
                for _ in 0..ITERS {
                    let mut guard = m.lock();
                    *guard += 1;
                }
                // Signal completion so the main thread can enforce a deadline
                // without blocking forever on `join`.
                let _ = tx.send(());
            }));
        }
        drop(done_tx);

        // Wait for every worker to report completion, bounded by a deadline.
        // recv_timeout returns Err on timeout, turning a deadlock into a fast failure.
        for _ in 0..THREADS {
            done_rx
                .recv_timeout(Duration::from_secs(30))
                .expect("deadlock detected: worker thread did not finish within 30s");
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let guard = mutex.lock();
        assert_eq!(*guard, THREADS * ITERS);
    }
}
