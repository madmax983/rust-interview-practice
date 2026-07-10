//! # `OnceCell` Implementation
//!
//! Implements a thread-safe initialization primitive that allows a value to be initialized exactly once.
//!
//! **Replaces Crates:** `once_cell`, `lazy_static`
//!
//! **Real-world Usage:**
//! - Global configuration objects.
//! - Compiled regexes created on first use.
//! - Thread-safe singletons without the overhead of a `Mutex` after initialization.
//!
//! **Why build it yourself?**
//! Understanding `OnceCell` demystifies one of the most common concurrency primitives in Rust.
//! It teaches you about `UnsafeCell` for interior mutability, atomic state machines for synchronization,
//! and safe memory publishing across threads without a lock.

use std::cell::UnsafeCell;
use std::hint::spin_loop;
use std::sync::atomic::{AtomicU8, Ordering};

// =========================================================================================
// Architecture
// =========================================================================================
//
// State Machine (AtomicU8):
//
//   [INCOMPLETE] ──(thread A CAS)──► [RUNNING]
//                                        │
//      ▲                                 │ (thread A initializes value)
//      │                                 ▼
//  (panic/err)                       [COMPLETE]
//
// Invariants:
// 1. Only one thread transitions from INCOMPLETE to RUNNING.
// 2. The value in `UnsafeCell` is only mutated when the state is RUNNING.
// 3. The value is only read when the state is COMPLETE.
// 4. If a thread panics during initialization, the state must revert to INCOMPLETE (or poison)
//    so another thread can try. (For simplicity, this implementation might just revert to INCOMPLETE or poison).
//
// Memory Ordering:
// - `Acquire` when reading the state to ensure changes to the value are visible.
// - `Release` when setting the state to COMPLETE to publish the value to other threads.
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ get (hit)     │ O(1)*       │ O(1)        │
// │ get_or_init   │ O(1)**      │ O(1)        │
// └───────────────┴─────────────┴─────────────┘
// * get is a single relaxed atomic load followed by an acquire load if it's COMPLETE.
// ** get_or_init is O(1) after initialization.

const INCOMPLETE: u8 = 0;
const RUNNING: u8 = 1;
const COMPLETE: u8 = 2;
const POISONED: u8 = 3;

/// A thread-safe cell which can be written to only once.
pub struct OnceCell<T> {
    state: AtomicU8,
    value: UnsafeCell<Option<T>>,
}

// RUST INSIGHT: `UnsafeCell` disables `Sync` by default.
// We must explicitly opt back in, but only if `T` itself is `Sync` (for shared access)
// and `Send` (since it might be sent to another thread for dropping).
unsafe impl<T: Sync + Send> Sync for OnceCell<T> {}
unsafe impl<T: Send> Send for OnceCell<T> {}

impl<T> Default for OnceCell<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> OnceCell<T> {
    /// Creates a new, empty `OnceCell`.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: AtomicU8::new(INCOMPLETE),
            value: UnsafeCell::new(None),
        }
    }

    /// Gets the reference to the underlying value.
    /// Returns `None` if the cell is not yet initialized.
    #[must_use]
    pub fn get(&self) -> Option<&T> {
        // RUST INSIGHT: We use `Acquire` to synchronize with the `Release`
        // store that transitions the state to COMPLETE. This ensures we see
        // the initialized data in the UnsafeCell.
        if self.state.load(Ordering::Acquire) == COMPLETE {
            // UNSAFE JUSTIFICATION:
            // State is COMPLETE, meaning initialization is finished. No other thread
            // can write to `value` ever again. Thus, aliasing rules are respected.
            unsafe { (*self.value.get()).as_ref() }
        } else {
            None
        }
    }

    /// Gets the contents of the cell, initializing it with `f` if the cell was empty.
    ///
    /// # Panics
    /// Panics if the initialization function `f` panics.
    pub fn get_or_init<F>(&self, f: F) -> &T
    where
        F: FnOnce() -> T,
    {
        // Fast path: already initialized.
        if let Some(val) = self.get() {
            return val;
        }

        // Slow path: try to initialize.
        self.initialize(f);

        // At this point, it must be initialized.
        // The panic in `initialize` (if `f` panics) would have propagated.
        self.get().unwrap()
    }

    #[cold]
    fn initialize<F>(&self, f: F)
    where
        F: FnOnce() -> T,
    {
        // Try to transition from INCOMPLETE to RUNNING.
        let mut current_state = self.state.load(Ordering::Acquire);

        loop {
            match current_state {
                COMPLETE => return, // Another thread finished initializing.
                POISONED => panic!("OnceCell poisoned by panic in initialization closure."),
                RUNNING => {
                    // Another thread is initializing. Wait for it to finish.
                    // PRODUCTION NOTE: Real implementations use thread parking (`std::thread::park`
                    // or OS futex) to avoid busy-waiting. Spin loops are fine for very short waits.
                    spin_loop();
                    current_state = self.state.load(Ordering::Acquire);
                }
                INCOMPLETE => {
                    // We attempt to claim the initialization right.
                    match self.state.compare_exchange_weak(
                        INCOMPLETE,
                        RUNNING,
                        Ordering::Acquire,
                        Ordering::Relaxed,
                    ) {
                        Ok(_) => {
                            // We successfully claimed the RUNNING state.

                            // GOTCHA: If `f()` panics, we will leave the state as RUNNING,
                            // causing other threads to spin forever (deadlock).
                            // We use a drop guard to set the state to POISONED if a panic occurs.
                            struct PanicGuard<'a, T> {
                                cell: &'a OnceCell<T>,
                                finished: bool,
                            }

                            impl<T> Drop for PanicGuard<'_, T> {
                                fn drop(&mut self) {
                                    if !self.finished {
                                        // The closure panicked.
                                        self.cell.state.store(POISONED, Ordering::Release);
                                    }
                                }
                            }

                            let mut guard = PanicGuard {
                                cell: self,
                                finished: false,
                            };

                            let val = f();

                            // UNSAFE JUSTIFICATION:
                            // We hold the unique RUNNING lock. No other thread can read or write.
                            unsafe {
                                *self.value.get() = Some(val);
                            }

                            guard.finished = true; // Disarm the panic guard

                            // Publish the value to other threads.
                            self.state.store(COMPLETE, Ordering::Release);
                            return;
                        }
                        Err(new_state) => {
                            // CAS failed, someone else claimed it or it completed.
                            current_state = new_state;
                        }
                    }
                }
                _ => unreachable!(),
            }
        }
    }
}

impl<T> Drop for OnceCell<T> {
    fn drop(&mut self) {
        // UNSAFE JUSTIFICATION:
        // We have `&mut self`, meaning we have exclusive access to the whole `OnceCell`.
        // No other threads can be accessing it. It is safe to drop the inner value if it exists.
        // We bypass the AtomicU8 entirely.
        unsafe {
            let _ = (*self.value.get()).take();
        }
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `once_cell`: The canonical crate provides a `sync::OnceCell` using similar atomic synchronization,
//   but also falls back to blocking thread pools or OS futexes (`parking_lot_core`) for true blocking
//   instead of a spin loop.
// - `std::sync::OnceLock`: The standard library now includes `OnceLock` which is effectively `once_cell`.
//
// Missing vs. Production:
// - **OS-Level Blocking**: This uses a spin loop, which burns CPU cycles if the initialization closure
//   takes a long time. Production implementations use wait queues.
// - **`try_insert` / Errors**: Production provides fallible initialization (`get_or_try_init`).
//
// Next Steps:
// 1. Add `get_or_try_init` for fallible closures.
// 2. Use `std::thread::park` and `unpark` to build a real waiting queue.
//
// Benchmarking Note:
// To benchmark `OnceCell` against `std::sync::OnceLock` or `Mutex`, use `criterion`.
// A good benchmark would spawn 100 threads that all hammer `get_or_init` simultaneously
// and measure the total throughput and tail latency of the successful fast-path reads.

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::thread;

    #[test]
    fn test_once_cell_basic() {
        let cell = OnceCell::new();
        assert_eq!(cell.get(), None);

        let v1 = cell.get_or_init(|| 42);
        assert_eq!(v1, &42);
        assert_eq!(cell.get(), Some(&42));

        let v2 = cell.get_or_init(|| 100);
        assert_eq!(v2, &42); // Should not reinitialize
    }

    #[test]
    fn test_once_cell_concurrent() {
        let cell = Arc::new(OnceCell::new());
        let count = Arc::new(AtomicUsize::new(0));

        let mut threads = vec![];
        for _ in 0..10 {
            let cell_clone = Arc::clone(&cell);
            let count_clone = Arc::clone(&count);
            threads.push(thread::spawn(move || {
                let val = cell_clone.get_or_init(|| {
                    count_clone.fetch_add(1, Ordering::SeqCst);
                    // Artificial delay to encourage contention
                    thread::sleep(std::time::Duration::from_millis(10));
                    123
                });
                assert_eq!(val, &123);
            }));
        }

        for t in threads {
            t.join().unwrap();
        }

        // The initialization closure should have run exactly once.
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(cell.get(), Some(&123));
    }

    #[test]
    #[should_panic(expected = "OnceCell poisoned")]
    fn test_once_cell_panic_poisoning() {
        let cell = Arc::new(OnceCell::<String>::new());
        let cell_clone = Arc::clone(&cell);

        let t = thread::spawn(move || {
            cell_clone.get_or_init(|| {
                panic!("Intentional panic during init");
            });
        });

        // Suppress panic output for test clarity if you want, but this is fine
        let _ = t.join();

        // The next attempt should panic with poisoning
        cell.get_or_init(|| "fallback".to_string());
    }

    #[test]
    fn test_drop() {
        struct DropDetector {
            count: Arc<AtomicUsize>,
        }

        impl Drop for DropDetector {
            fn drop(&mut self) {
                self.count.fetch_add(1, Ordering::SeqCst);
            }
        }

        let drop_count = Arc::new(AtomicUsize::new(0));

        {
            let cell = OnceCell::new();
            cell.get_or_init(|| DropDetector {
                count: Arc::clone(&drop_count),
            });
        } // cell dropped here

        assert_eq!(drop_count.load(Ordering::SeqCst), 1);
    }
}
