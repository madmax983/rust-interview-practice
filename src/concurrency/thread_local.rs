//! # Thread Local Storage Implementation
//!
//! Implements a custom Thread-Local Storage (TLS) abstraction from scratch.
//!
//! **Replaces Crates:** `std::thread_local!` (standard library macro), `thread_local`
//!
//! **Real-world Usage:**
//! - Global allocators maintaining per-thread memory pools (like jemalloc or mimalloc) to avoid lock contention.
//! - Tracing and logging frameworks passing contextual information (Spans) implicitly.
//! - Random number generators (e.g., `ThreadRng` in the `rand` crate).
//!
//! **Why build it yourself?**
//! `std::thread_local!` relies on OS-level thread-local storage (like `pthread_setspecific` or ELF `.tdata` sections),
//! which is magical and heavily platform-dependent. Building an application-level TLS using a global registry
//! teaches you how to map thread IDs to specific memory regions safely, and how to deal with the primary challenge
//! of TLS: cleaning up memory when a thread exits.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread::{self, ThreadId};

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure:
//
//      Global Registry (Arc<Mutex<HashMap<ThreadId, Box<T>>>>)
//         │
//         ├── Thread A (ID: 1)  ──►  Value A
//         ├── Thread B (ID: 2)  ──►  Value B
//         └── Thread C (ID: 3)  ──►  Value C
//
// Access Flow:
// 1. Thread calls `.with(closure)`.
// 2. Lookup `thread::current().id()` in the shared `HashMap`.
// 3. If present, invoke the closure with a reference. If absent, initialize it using the provided factory function.
//
// Invariants:
// 1. Values are strictly bound to the thread that created them.
// 2. A thread cannot safely access another thread's local data through this interface.
// 3. The data must be dropped when the `ThreadLocal` object itself is dropped (or, ideally, when the thread exits).
// 4. We use a closure-based access pattern (`with`) to prevent returning references that could outlive the lock
//    or lead to mutable aliasing/use-after-free bugs if `clear()` is called.
//
// Complexity:
// ┌───────────┬──────────────┬────────┐
// │ Operation │ Time         │ Space  │
// ├───────────┼──────────────┼────────┤
// │ with      │ O(1)*        │ O(T)   │
// └───────────┴──────────────┴────────┘
// *Time complexity relies on acquiring a `Mutex` to read from the `HashMap`. In high contention, this degrades.
//
// Design Decisions:
// - **Storage**: Global `Mutex<HashMap>`.
//   - *Tradeoff*: Huge bottleneck if accessed frequently by many threads.
//   - *Production Note*: Real OS-level TLS avoids locks entirely by using dedicated CPU registers (e.g., `GS`/`FS` on x86) pointing to thread-local memory.
//   - *Alternative*: The `thread_local` crate uses a lock-free array indexed by atomic thread IDs to provide fast, lock-free access.
// - **Initialization**: Lazy initialization via a `Fn() -> T`.
// - **Cleanup**: Data is cleaned up when `ThreadLocal` is dropped.
//   - *Gotcha*: `std::thread_local!` cleans up data when the *thread* exits. Our implementation cleans up when the *container* drops. To mimic thread-exit cleanup without OS hooks, one must manually call a cleanup routine.

/// A thread-local storage container.
pub struct ThreadLocal<T> {
    // We use a global registry.
    // Box<T> ensures the value doesn't move in memory, though inside a HashMap it's stable enough.
    registry: Arc<Mutex<HashMap<ThreadId, Box<T>>>>,
    init: fn() -> T,
}

impl<T> ThreadLocal<T> {
    /// Creates a new `ThreadLocal` with the given initialization function.
    pub fn new(init: fn() -> T) -> Self {
        Self {
            registry: Arc::new(Mutex::new(HashMap::new())),
            init,
        }
    }

    /// Acquires a reference to the thread-local value, initializing it if necessary,
    /// and executes the provided closure with it.
    ///
    /// # RUST INSIGHT: Safe Access Pattern
    /// To prevent lifetime issues, use-after-free bugs (if `clear` is called while a reference is held),
    /// and mutable aliasing, we enforce a closure-based access pattern.
    /// This ensures the reference cannot outlive the lock guard, matching `std::thread_local!` semantics.
    pub fn with<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&T) -> R,
    {
        let tid = thread::current().id();

        let mut map = self.registry.lock().unwrap();
        map.entry(tid).or_insert_with(|| {
            let value = (self.init)();
            Box::new(value)
        });

        let val_ref = map.get(&tid).unwrap();
        f(val_ref)
    }

    /// Acquires a mutable reference to the thread-local value, initializing it if necessary,
    /// and executes the provided closure with it.
    ///
    /// *Note:* In production, standard TLS only hands out immutable references, forcing users
    /// to use interior mutability (e.g., `RefCell` or `Cell`) if they want to mutate. We provide
    /// `with_mut` here for convenience, but it inherently blocks other threads while executing `f`
    /// because it holds the global `Mutex` write lock!
    pub fn with_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut T) -> R,
    {
        let tid = thread::current().id();

        let mut map = self.registry.lock().unwrap();
        map.entry(tid).or_insert_with(|| {
            let value = (self.init)();
            Box::new(value)
        });

        let val_mut = map.get_mut(&tid).unwrap();
        f(val_mut)
    }

    /// Explicitly removes the current thread's value from the registry.
    /// Useful for mimicking thread-exit cleanup.
    pub fn clear(&self) {
        let tid = thread::current().id();
        let mut map = self.registry.lock().unwrap();
        map.remove(&tid);
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `std::thread_local!`: Provided by the compiler/std. It lowers to LLVM thread-local attributes.
//   It is infinitely faster than this implementation because it translates to a few assembly instructions
//   (reading from a segment register) rather than acquiring a Mutex and hashing a ThreadId.
// - `thread_local` crate: An application-level TLS that allows iterating over *all* threads' local data.
//   It uses a lock-free array mechanism, assigning custom fast IDs to threads instead of using `ThreadId`.
//
// Missing vs. Production:
// - **Performance**: The `Mutex` on every first access (or subsequent access if we didn't use unsafe pointer trickery) is a massive bottleneck.
// - **Thread Exit Hooks**: We leak memory for the duration of the `ThreadLocal` object if threads die
//   without calling `.clear()`. Production TLS hooks into OS thread destructors (e.g., `pthread_key_create`)
//   to drop data when the thread dies.
//
// Next Steps:
// 1. Implement a lock-free array indexed by an atomic counter (assigning custom Thread IDs 0..N).
// 2. Add an iterator to aggregate data across all threads (useful for metrics).

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_thread_local_isolation() {
        let tls: Arc<ThreadLocal<i32>> = Arc::new(ThreadLocal::new(|| 0));

        let t1_tls = Arc::clone(&tls);
        let t1 = thread::spawn(move || {
            t1_tls.with_mut(|val| *val += 10);
            t1_tls.with(|val| assert_eq!(*val, 10));
        });

        let t2_tls = Arc::clone(&tls);
        let t2 = thread::spawn(move || {
            t2_tls.with_mut(|val| *val += 20);
            t2_tls.with(|val| assert_eq!(*val, 20));
        });

        t1.join().unwrap();
        t2.join().unwrap();

        // Main thread should have its own copy initialized to 0
        tls.with(|val| assert_eq!(*val, 0));
    }

    #[test]
    fn test_thread_local_initialization_called_once_per_thread() {
        static INIT_COUNT: AtomicUsize = AtomicUsize::new(0);

        fn init() -> usize {
            INIT_COUNT.fetch_add(1, Ordering::SeqCst)
        }

        let tls: Arc<ThreadLocal<usize>> = Arc::new(ThreadLocal::new(init));

        let t1_tls = Arc::clone(&tls);
        let t1 = thread::spawn(move || {
            let val_copy = t1_tls.with(|val| *val);
            t1_tls.with(|val| assert_eq!(*val, val_copy)); // Subsequent calls don't increment
        });

        let t2_tls = Arc::clone(&tls);
        let t2 = thread::spawn(move || {
            let val_copy = t2_tls.with(|val| *val);
            t2_tls.with(|val| assert_eq!(*val, val_copy));
        });

        t1.join().unwrap();
        t2.join().unwrap();

        assert_eq!(INIT_COUNT.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn test_clear() {
        let tls = ThreadLocal::new(|| 42);
        tls.with(|val| assert_eq!(*val, 42));

        tls.with_mut(|val| *val = 100);
        tls.with(|val| assert_eq!(*val, 100));

        tls.clear();

        // Should re-initialize
        tls.with(|val| assert_eq!(*val, 42));
    }
}
