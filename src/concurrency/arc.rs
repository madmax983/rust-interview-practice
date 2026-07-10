//! # Atomic Reference Count Implementation
//!
//! A thread-safe reference-counting pointer. 'Arc' stands for 'Atomically Reference Counted'.
//!
//! **Replaces Crates:** `std::sync::Arc`, `triomphe` (simplified)
//!
//! **Real-world Usage:**
//! - Shared state in multi-threaded environments (e.g., config data).
//! - Complex graph-like data structures across threads.
//! - Foundation for other concurrency primitives.
//!
//! **Why build it yourself?**
//! Building `Arc` teaches you about manual memory management in Rust, specifically using
//! `NonNull` to hold a raw pointer and using `AtomicUsize` for thread-safe lock-free synchronization.
//! It clarifies the role of `Send` and `Sync` auto-traits.

use std::marker::PhantomData;
use std::ops::Deref;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicUsize, Ordering};

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure:
//
//      Arc<T> (Pointer)
//         │
//         ▼
//      ┌───────────────┐
//      │ ArcInner<T>   │ (Heap Allocated)
//      ├───────────────┤
//      │ count: Atomic │
//      │ data: T       │
//      └───────────────┘
//
// Invariants:
// 1. The memory is valid as long as `count > 0`.
// 2. Cloning an `Arc` increments the atomic count.
// 3. Dropping an `Arc` decrements the atomic count. If it reaches 0, the inner memory is deallocated.
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ Clone         │ O(1)        │ O(1)        │
// │ Drop          │ O(1)*       │ O(1)        │
// │ Deref         │ O(1)        │ O(1)        │
// └───────────────┴─────────────┴─────────────┘
// * Drops the inner value when the last Arc is dropped.

/// The heap-allocated inner state shared by all `Arc` instances.
struct ArcInner<T> {
    count: AtomicUsize,
    data: T,
}

/// A thread-safe reference-counting pointer.
pub struct Arc<T> {
    ptr: NonNull<ArcInner<T>>,
    // RUST INSIGHT:
    // PhantomData is needed to tell the compiler that we logically own a `T`.
    // This affects dropck (drop checker) and auto-traits like Send/Sync.
    _marker: PhantomData<ArcInner<T>>,
}

// UNSAFE JUSTIFICATION:
// An `Arc<T>` can be sent across threads if and only if `T` is `Sync` and `Send`.
// `Sync` is required because multiple `Arc`s can exist on different threads, all dereferencing to the same `T` concurrently.
// `Send` is required because the last `Arc` might be dropped on a different thread than it was created,
// meaning `T` could be dropped on that other thread.
unsafe impl<T: Sync + Send> Send for Arc<T> {}
unsafe impl<T: Sync + Send> Sync for Arc<T> {}

impl<T> Arc<T> {
    /// Creates a new `Arc` containing the given data.
    ///
    /// # Panics
    ///
    /// Panics if the underlying allocation returned by `Box::into_raw` is null,
    /// which cannot happen for a successful allocation.
    pub fn new(data: T) -> Self {
        // PRODUCTION NOTE:
        // A production `Arc` also supports `Weak` pointers, which requires a separate `weak_count` atomic.
        // We omit `Weak` for simplicity, focusing on the strong reference counting mechanics.

        let inner = Box::new(ArcInner {
            count: AtomicUsize::new(1),
            data,
        });

        // Box::into_raw consumes the Box and returns a raw pointer, leaking the allocation.
        // We now take manual responsibility for freeing this memory.
        let ptr = NonNull::new(Box::into_raw(inner)).expect("Box::into_raw cannot return null");

        Self {
            ptr,
            _marker: PhantomData,
        }
    }
}

impl<T> Clone for Arc<T> {
    fn clone(&self) -> Self {
        let inner = unsafe { self.ptr.as_ref() };

        // RUST INSIGHT:
        // Ordering::Relaxed is sufficient for incrementing the count.
        // We don't need any special synchronization here because we already have a valid `Arc` (this one),
        // meaning the count is at least 1 and the memory cannot be freed while we are cloning.
        // GOTCHA:
        // Using `fetch_add` returns the *previous* value. We don't care about it here.
        inner.count.fetch_add(1, Ordering::Relaxed);

        Self {
            ptr: self.ptr,
            _marker: PhantomData,
        }
    }
}

impl<T> Drop for Arc<T> {
    fn drop(&mut self) {
        let inner = unsafe { self.ptr.as_ref() };

        // RUST INSIGHT:
        // Ordering::Release is required when decrementing. It ensures that all memory accesses
        // prior to the drop on *this* thread become visible to the thread that eventually frees the memory.
        // GOTCHA:
        // If we used Relaxed here, another thread might free the memory before all writes to the
        // data from this thread are visible, leading to a data race if the Drop implementation of T relies on them.
        if inner.count.fetch_sub(1, Ordering::Release) != 1 {
            return; // We were not the last one.
        }

        // RUST INSIGHT:
        // Ordering::Acquire is needed *only* by the thread that actually frees the memory.
        // This pairs with the `Release` above, ensuring we see all modifications made by all other
        // threads before they dropped their Arcs.
        // `atomic::fence(Ordering::Acquire)` achieves this without an unnecessary read.
        std::sync::atomic::fence(Ordering::Acquire);

        // We are the last one. Re-construct the Box and let it drop naturally.
        // UNSAFE JUSTIFICATION:
        // We know we are the last `Arc` because `fetch_sub` returned 1. Therefore, no other
        // pointers to this memory exist, making it safe to take ownership and drop it.
        unsafe {
            drop(Box::from_raw(self.ptr.as_ptr()));
        }
    }
}

impl<T> Deref for Arc<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        let inner = unsafe { self.ptr.as_ref() };
        &inner.data
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to `std::sync::Arc`:
// - `std::sync::Arc` includes `Weak` pointers, which requires a second atomic counter (`weak_count`).
// - Production `Arc` handles `isize::MAX` overflows on the counter (which would require leaking the Arc to prevent safety issues).
// - `std::sync::Arc` provides `make_mut` and `get_mut` methods using `is_unique` checks.
//
// Missing vs Production:
// - `Weak` pointer support.
// - Overflow panic logic for the count.
// - `CoerceUnsized` and `DispatchFromDyn` for trait object support (requires nightly).
//
// Next Steps:
// - Implement a `Weak` pointer and the corresponding `weak_count`.
//
// Benchmarking Note:
// To benchmark `Arc` vs `std::sync::Arc`, use `criterion` to measure the overhead of
// highly contended atomic increments/decrements across multiple threads.
// e.g., `b.iter(|| std::hint::black_box(arc.clone()))`

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;

    #[test]
    fn test_arc_basic() {
        let arc = Arc::new(42);
        assert_eq!(*arc, 42);

        let arc2 = arc.clone();
        assert_eq!(*arc2, 42);
    }

    #[test]
    fn test_arc_drop() {
        static DROP_COUNT: AtomicUsize = AtomicUsize::new(0);

        struct DropDetector;
        impl Drop for DropDetector {
            fn drop(&mut self) {
                DROP_COUNT.fetch_add(1, Ordering::SeqCst);
            }
        }

        {
            let arc1 = Arc::new(DropDetector);
            let arc2 = arc1.clone();

            assert_eq!(DROP_COUNT.load(Ordering::SeqCst), 0);
            drop(arc1);
            assert_eq!(DROP_COUNT.load(Ordering::SeqCst), 0); // Still 1 ref
            drop(arc2);
            assert_eq!(DROP_COUNT.load(Ordering::SeqCst), 1); // 0 refs, dropped
        }
    }

    #[test]
    fn test_arc_multithreaded() {
        let arc = Arc::new(AtomicUsize::new(0));
        let mut handles = vec![];

        for _ in 0..10 {
            let arc_clone = arc.clone();
            handles.push(thread::spawn(move || {
                for _ in 0..1000 {
                    arc_clone.fetch_add(1, Ordering::SeqCst);
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(arc.load(Ordering::SeqCst), 10000);
    }
}
