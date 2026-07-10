//! # Barrier Implementation
//!
//! Implements a thread synchronization Barrier from scratch.
//! A barrier enables multiple threads to wait until all threads have reached a certain point of execution
//! before any are allowed to proceed.
//!
//! **Replaces Crates:** `std::sync::Barrier`
//!
//! **Real-world Usage:**
//! - Parallel algorithms with distinct phases (e.g., `MapReduce`, scatter-gather).
//! - Wait for all worker threads to finish initialization before starting a workload.
//! - Graphics rendering pipelines (synchronizing frames across multiple GPU command queues).
//!
//! **Why build it yourself?**
//! While simple in concept, a reusable barrier is tricky to implement correctly. It requires a
//! "generation" counter to prevent a fast thread from leaving the barrier, racing around,
//! and entering the next phase before other threads have even woken up from the first phase.
//! Building this teaches you how to orchestrate complex wake-up conditions using `Condvar`.

// Lock guards are intentionally held across condvar waits/notifications;
// do not tighten their scope.
#![allow(clippy::significant_drop_tightening)]

use std::sync::{Condvar, Mutex};

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure:
//
//      Barrier
//      ├── count (Atomic or Mutex-protected)
//      ├── generation_id (Mutex-protected)
//      └── wait_queue (Condvar)
//
// Flow:
// 1. Thread calls `wait()`.
// 2. Decrement the `count`.
// 3. If `count > 0`, thread blocks on `Condvar` waiting for `generation_id` to change.
// 4. If `count == 0` (last thread):
//    - Reset `count` to the original number of threads.
//    - Increment `generation_id`.
//    - Call `notify_all()` on the `Condvar` to wake up all waiting threads.
//
// Invariants:
// 1. No thread proceeds past `wait()` until `num_threads` have called `wait()`.
// 2. The barrier must be reusable (threads can call `wait()` in a loop).
// 3. Exactly one thread receives `true` from `wait()` (the "leader" thread), useful for
//    single-threaded cleanup between phases.
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ wait          │ O(1)        │ O(1)        │
// └───────────────┴─────────────┴─────────────┘
//
// Design Decisions:
// - **State Protection**: A single `Mutex` protects both the `count` and the `generation_id`.
//   - *Tradeoff*: All threads contend on a single lock. For massive core counts (e.g., >64),
//     a dissemination barrier or tournament barrier (lock-free) is much faster.
// - **Generation Counter**: Crucial for reusability. Without it, a thread could wake up,
//   finish its next task, and re-enter the barrier before the last thread from the *previous*
//   phase even woke up, stealing a spot in the old phase.

/// A trait defining the interface for thread barriers.
/// This allows substituting the simple Mutex-based barrier with lock-free variants.
pub trait BarrierSync {
    fn wait(&self) -> BarrierWaitResult;
}

/// Returned by `Barrier::wait()` when all threads have rendezvoused.
pub struct BarrierWaitResult {
    is_leader: bool,
}

impl BarrierWaitResult {
    /// Returns `true` if this thread is the "leader" (the last thread to arrive at the barrier).
    /// Exactly one thread will receive `true` per barrier cycle.
    #[must_use]
    pub const fn is_leader(&self) -> bool {
        self.is_leader
    }
}

struct BarrierState {
    /// Number of threads still needed to reach the barrier in the current generation.
    count: usize,
    /// The generation ID. Incremented every time the barrier is released.
    generation_id: usize,
}

/// A synchronization primitive that enables multiple threads to synchronize the
/// beginning of some computation.
pub struct Barrier {
    lock: Mutex<BarrierState>,
    cvar: Condvar,
    num_threads: usize,
}

impl Barrier {
    /// Creates a new barrier that can block a given number of threads.
    ///
    /// # Panics
    /// Panics if `num_threads` is 0.
    #[must_use]
    pub fn new(num_threads: usize) -> Self {
        assert!(num_threads > 0, "Barrier requires at least 1 thread");
        Self {
            lock: Mutex::new(BarrierState {
                count: num_threads,
                generation_id: 0,
            }),
            cvar: Condvar::new(),
            num_threads,
        }
    }
}

impl BarrierSync for Barrier {
    /// Blocks the current thread until all threads have rendezvoused here.
    ///
    /// Returns a `BarrierWaitResult` which indicates if the current thread was
    /// the last one to arrive (the leader).
    fn wait(&self) -> BarrierWaitResult {
        let mut state = self.lock.lock().unwrap();

        // Capture the generation we entered in.
        // GOTCHA: We must capture this *before* we potentially sleep. If we don't track
        // generations, a fast thread could wake up, loop around, and steal the count
        // of a slow thread that hasn't woken up yet.
        let local_gen = state.generation_id;

        state.count -= 1;

        if state.count == 0 {
            // We are the last thread to arrive. Time to wake everyone up.
            // Reset the counter for the next use.
            state.count = self.num_threads;
            // Advance the generation to signal waiting threads.
            state.generation_id = state.generation_id.wrapping_add(1);

            // RUST INSIGHT: notify_all() is called while holding the mutex.
            // This is generally fine and often preferred to avoid "missed wakeups" in complex
            // scenarios, though doing it outside the lock can sometimes yield better performance.
            // std::sync::Condvar handles this correctly either way.
            self.cvar.notify_all();

            BarrierWaitResult { is_leader: true }
        } else {
            // We are not the last thread. Wait for the generation to change.
            // RUST INSIGHT: `cvar.wait` handles spurious wakeups by requiring a condition loop.
            // Here, our condition is `state.generation_id == local_gen`. If it's still the same,
            // the barrier hasn't been breached yet, so we go back to sleep.
            while local_gen == state.generation_id {
                state = self.cvar.wait(state).unwrap();
            }

            BarrierWaitResult { is_leader: false }
        }
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `std::sync::Barrier`: Our implementation is functionally identical to the standard library's
//   Barrier. The stdlib also uses a Mutex, Condvar, and generation counter.
//
// Missing vs. Production:
// - **Lock-Free Variants**: For extreme performance on many-core machines, a lock-free barrier
//   (like a Dissemination Barrier or Sense-Reversing Tree Barrier) is used to avoid Mutex contention.
//   This implementation uses a central Mutex which scales poorly beyond a few dozen cores.
//
// Suggested Next Steps / Extensions:
// 1. Implement a `SenseReversingBarrier` to avoid the generation counter allocation.
// 2. Implement a `TreeBarrier` for O(log N) contention instead of O(N) Mutex contention.
// 3. Add a timeout to `wait` using `Condvar::wait_timeout`.
//
// Benchmarking Note:
// To benchmark this against `std::sync::Barrier`, spawn N threads and have them loop `wait()` 10,000 times.
// Use `criterion` to measure the total time. Contention on the central Mutex will be the bottleneck.

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::thread;

    #[test]
    #[should_panic(expected = "Barrier requires at least 1 thread")]
    fn test_barrier_zero_threads() {
        Barrier::new(0);
    }

    #[test]
    fn test_barrier_single_thread() {
        let barrier = Barrier::new(1);
        let res1 = barrier.wait();
        assert!(res1.is_leader());

        // Test reusability
        let res2 = barrier.wait();
        assert!(res2.is_leader());
    }

    #[test]
    fn test_barrier_multiple_threads() {
        const NUM_THREADS: usize = 5;
        let barrier = Arc::new(Barrier::new(NUM_THREADS));
        let count = Arc::new(Mutex::new(0));

        let mut handles = vec![];

        for _ in 0..NUM_THREADS {
            let b = Arc::clone(&barrier);
            let c = Arc::clone(&count);
            handles.push(thread::spawn(move || {
                // Phase 1: Increment counter
                {
                    let mut lock = c.lock().unwrap();
                    *lock += 1;
                }

                // Wait for all threads to finish Phase 1
                let res = b.wait();

                // Phase 2: Verify counter
                {
                    let lock = c.lock().unwrap();
                    // All threads should see the counter exactly at NUM_THREADS
                    assert_eq!(*lock, NUM_THREADS);
                }
                res.is_leader()
            }));
        }

        let mut leader_count = 0;
        for handle in handles {
            let is_leader = handle.join().unwrap();
            if is_leader {
                leader_count += 1;
            }
        }

        // Exactly one thread must be the leader
        assert_eq!(leader_count, 1);
    }

    #[test]
    fn test_barrier_reusability() {
        const NUM_THREADS: usize = 3;
        const NUM_PHASES: usize = 3;

        let barrier = Arc::new(Barrier::new(NUM_THREADS));
        let mut handles = vec![];

        for _ in 0..NUM_THREADS {
            let b = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                let mut leader_count = 0;
                for _ in 0..NUM_PHASES {
                    if b.wait().is_leader() {
                        leader_count += 1;
                    }
                }
                leader_count
            }));
        }

        let mut total_leaders = 0;
        for handle in handles {
            total_leaders += handle.join().unwrap();
        }

        // Across all phases, the total number of leaders must equal the number of phases
        assert_eq!(total_leaders, NUM_PHASES);
    }
}
