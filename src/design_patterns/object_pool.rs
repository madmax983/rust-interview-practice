//! # Object Pool Pattern
//!
//! **Replaces:** OOP Object Pool
//! **Real-world usage:** `r2d2` (database connection pools), `deadpool`, `sqlx`.
//!
//! **Why this pattern exists in Rust:**
//! Creating and destroying objects can be expensive (e.g., database connections, large buffers).
//! The Object Pool pattern reuses instances instead of constantly allocating/deallocating.
//!
//! In C++ or Java, users must explicitly request an object from the pool and, crucially,
//! *remember to return it* (e.g., `pool.release(obj)`). Forgetting to return the object causes resource leaks.
//!
//! In Rust, the Object Pool transforms into **RAII Guards**. By combining `Arc<Mutex<Vec<T>>>`
//! with a custom `PoolGuard` that implements the `Drop` trait, the return of the object to the pool
//! is fully automatic and guaranteed by the compiler.
//!
//! ## Architecture
//!
//! ```text
//! Pool<T>
//! ├── Arc<Mutex<Vec<T>>> (Shared storage)
//! └── acquire() -> Option<PoolGuard<T>>
//!
//! PoolGuard<T> (Implements Deref, DerefMut, and Drop)
//! ├── value: Option<T>
//! ├── pool: Arc<Mutex<Vec<T>>> (Reference back to the pool)
//! └── Drop -> Pushes the value back into the pool's Mutex<Vec<T>>
//! ```
//!
//! **Invariants Enforced:**
//! - **Zero Resource Leaks:** The object is guaranteed to return to the pool when the `PoolGuard` goes out of scope, even on panics.
//! - **Exclusive Access:** Only one owner has mutable access to the pulled item at a time (enforced by moving the `T` into the guard).
//!
//! **When to use:** When object creation is very expensive (network connections, OS threads, massive memory allocations).
//! **When it's overkill:** For standard small structs. Rust's allocator (jemalloc/system) is extremely fast; don't pool `String` or small `Vec`s without benchmarking first.
//!
//! ## Anti-Pattern
//!
//! A naive implementation mimics the OOP way, requiring manual release.
//!
//! ```rust
//! // ANTI-PATTERN: Manual release
//! struct NaivePool<T> { items: std::sync::Mutex<Vec<T>> }
//!
//! impl<T> NaivePool<T> {
//!     fn acquire(&self) -> Option<T> { self.items.lock().unwrap().pop() }
//!
//!     // GOTCHA: Users will forget to call this, leading to pool exhaustion.
//!     fn release(&self, item: T) { self.items.lock().unwrap().push(item) }
//! }
//! ```
//!
//! ## Idiomatic Rust Implementation

use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex};

/// The Object Pool that manages reusable instances.
pub struct Pool<T> {
    // Shared state containing the available items.
    // OWNERSHIP INSIGHT: Arc allows multiple threads to share ownership of the pool.
    // Mutex provides interior mutability to push/pop items.
    items: Arc<Mutex<Vec<T>>>,
}

impl<T> Clone for Pool<T> {
    fn clone(&self) -> Self {
        Self {
            items: Arc::clone(&self.items),
        }
    }
}

impl<T> Pool<T> {
    /// Creates a new pool with the given initial items.
    #[must_use]
    pub fn new(initial_items: impl IntoIterator<Item = T>) -> Self {
        Self {
            items: Arc::new(Mutex::new(initial_items.into_iter().collect())),
        }
    }

    /// Acquires an item from the pool, if available.
    /// Returns a `PoolGuard` which automatically returns the item on drop.
    pub fn acquire(&self) -> Option<PoolGuard<T>> {
        let mut items = self.items.lock().unwrap();
        items.pop().map(|item| PoolGuard {
            // We use Option so we can `take()` the value out during Drop.
            value: Some(item),
            pool: Arc::clone(&self.items),
        })
    }

    /// Returns the number of currently available items in the pool.
    pub fn available_count(&self) -> usize {
        self.items.lock().unwrap().len()
    }
}

/// A RAII guard that wraps a borrowed item from the pool.
/// It implements `Deref` and `DerefMut` to act like a smart pointer to `T`.
pub struct PoolGuard<T> {
    value: Option<T>,
    pool: Arc<Mutex<Vec<T>>>,
}

// Implement Deref so we can call methods on the underlying T seamlessly.
impl<T> Deref for PoolGuard<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.value.as_ref().expect("Value is always present before Drop")
    }
}

// Implement DerefMut for mutable access.
impl<T> DerefMut for PoolGuard<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.value.as_mut().expect("Value is always present before Drop")
    }
}

/// The magic happens here: automatically return the item to the pool.
impl<T> Drop for PoolGuard<T> {
    fn drop(&mut self) {
        // COMPILE-TIME WIN: The developer cannot forget to return the item.
        // It happens deterministically when the guard goes out of scope.
        if let Some(item) = self.value.take() {
            let mut items = self.pool.lock().unwrap();
            items.push(item);
        }
    }
}

// PRODUCTION NOTE: Real connection pools (like r2d2) often include a trait
// for checking if a connection is still "healthy" before handing it out,
// and before putting it back. If it's dead, it's discarded and a new one is created.

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[derive(Debug, PartialEq, Eq)]
    struct ExpensiveResource {
        id: usize,
    }

    #[test]
    fn test_pool_acquire_and_automatic_release() {
        let pool = Pool::new(vec![
            ExpensiveResource { id: 1 },
            ExpensiveResource { id: 2 },
        ]);

        assert_eq!(pool.available_count(), 2);

        {
            // Acquire one resource
            let guard1 = pool.acquire().unwrap();
            assert_eq!(guard1.id, 2); // Last in, first out (Vec::pop)
            assert_eq!(pool.available_count(), 1);

            {
                // Acquire second resource
                let guard2 = pool.acquire().unwrap();
                assert_eq!(guard2.id, 1);
                assert_eq!(pool.available_count(), 0);

                // Pool is empty
                assert!(pool.acquire().is_none());

                // guard2 goes out of scope here and is returned.
            }

            assert_eq!(pool.available_count(), 1);
            // guard1 goes out of scope here and is returned.
        }

        // Both are back in the pool.
        assert_eq!(pool.available_count(), 2);
    }

    #[test]
    fn test_pool_thread_safety() {
        let pool = Pool::new((0..5).map(|id| ExpensiveResource { id }));

        // Spawn multiple threads that acquire, do "work", and drop.
        let handles: Vec<_> = (0..5).map(|_| {
            let pool_clone = pool.clone();
            thread::spawn(move || {
                // Wait until we get a resource
                let mut guard = loop {
                    if let Some(g) = pool_clone.acquire() {
                        break g;
                    }
                    thread::yield_now();
                };

                // Mutate it (proving exclusive mutable access)
                guard.id += 100;

                // Automatically returned here.
            })
        }).collect();

        for handle in handles {
            handle.join().unwrap();
        }

        // All 5 resources must be back.
        assert_eq!(pool.available_count(), 5);

        // Check that they were mutated.
        let mut items = vec![];
        while let Some(guard) = pool.acquire() {
            // we have to clone out of the pool to check them all at the end
            // wait, we can't easily clone without trait bound. Just assert id >= 100.
            assert!(guard.id >= 100);
            items.push(guard.id); // store just the ids
        }

        assert_eq!(items.len(), 5);
    }
}

// **Standard Library Equivalents:**
// While `std` doesn't have a generic object pool, the core mechanism—a RAII guard wrapping shared state
// and using `Drop` to release resources—is exactly how `std::sync::MutexGuard` works.
//
// **GoF Equivalent:**
// Object Pool (often related to Flyweight or Singleton).
// In OOP, the focus is on the *Pool manager*. In Rust, the magic is in the *RAII Guard*
// that entirely eliminates the "leak" class of errors common in OOP pooling.
//
// **When to reach for this vs. simpler alternatives:**
// If objects are cheap to create, just create them and let Rust drop them.
// Only use pooling when creation (e.g., establishing a TCP connection) takes significantly more time
// than the lock contention overhead of accessing the shared pool.
//
// **Suggested combinations:**
// - Use with the **Singleton Pattern** if you need a global, shared connection pool across your application.
// - Combine with the **Proxy Pattern** if you want the `PoolGuard` to act as a stand-in that adds logging or validation before dereferencing to the real resource.