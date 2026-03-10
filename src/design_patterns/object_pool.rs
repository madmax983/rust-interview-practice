// Suppress pedantic and nursery lints for design pattern examples.
#![allow(clippy::pedantic, clippy::nursery, unused)]

//! # Object Pool Pattern
//!
//! Replaces: **Object Pool Pattern** (OOP / Performance Optimization)
//!
//! Real Rust usage: `sqlx::Pool`, thread pools, `slab` allocator, `bb8`
//!
//! ## Why this pattern exists in Rust
//! In OOP, an Object Pool holds onto expensive objects (e.g., database connections)
//! so they can be reused without reallocation or setup costs. The danger in C++/Java
//! is that the user must explicitly return the object to the pool, risking leaks.
//!
//! In Rust, **this pattern transforms into RAII Guards**. We create a `PoolGuard` struct
//! that wraps the borrowed object. When the guard goes out of scope, its `Drop`
//! implementation automatically returns the object to the pool, completely eliminating
//! the "forgot to return to pool" class of bugs.
//!
//! ## Architecture
//!
//! **Approach: Thread-Safe Pool with RAII Guards**
//! ```text
//! [ Pool ] <---- (owns Vec<T>)
//!    |
//!    +-- get() --> [ PoolGuard (Deref -> T) ]
//!                       |
//!                       V
//!                  (Drop called) ---> returns T to [ Pool ]
//! ```
//!
//! **Invariants:**
//! - Objects are never copied or cloned during retrieval or return.
//! - The `PoolGuard` must not outlive the `Pool` itself, enforced by `Arc` and `Drop`.
//!
//! ## When to use
//! - Managing expensive resources (e.g., sockets, connections, heavy allocations).
//! - Creating predictable memory usage paths in real-time or embedded systems.
//!
//! ## Anti-patterns
//! - Forcing the user to manually call `.return_to_pool()`, which bypasses Rust's safety guarantees.

use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex};

// ============================================================================
// Approach: Thread-Safe Object Pool
// ============================================================================

/// The underlying structure that manages the pool of reusable objects.
///
/// **OWNERSHIP INSIGHT:**
/// We wrap the internal state in an `Arc<Mutex>` so multiple threads can request
/// objects from the pool simultaneously. The objects (`T`) must implement `Send`.
#[derive(Clone)]
pub struct ObjectPool<T> {
    // The items waiting in the pool.
    items: Arc<Mutex<Vec<T>>>,
}

impl<T> ObjectPool<T> {
    pub fn new(initial_items: impl IntoIterator<Item = T>) -> Self {
        Self {
            items: Arc::new(Mutex::new(initial_items.into_iter().collect())),
        }
    }

    /// Borrows an object from the pool, blocking if the mutex is locked.
    /// Returns an `Option` if the pool is empty.
    pub fn get(&self) -> Option<PoolGuard<T>> {
        let mut items = self.items.lock().unwrap();
        items.pop().map(|item| PoolGuard {
            item: Some(item),
            pool: self.items.clone(),
        })
    }

    /// Return the current size of the pool.
    pub fn available(&self) -> usize {
        self.items.lock().unwrap().len()
    }
}

// ============================================================================
// The RAII Guard
// ============================================================================

/// A smart pointer that automatically returns the object to the pool when dropped.
pub struct PoolGuard<T> {
    // We wrap the item in Option so we can `take()` it out during Drop without violating ownership.
    item: Option<T>,
    pool: Arc<Mutex<Vec<T>>>,
}

impl<T> Deref for PoolGuard<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // Safe to unwrap because it is only set to None during Drop.
        self.item.as_ref().unwrap()
    }
}

impl<T> DerefMut for PoolGuard<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.item.as_mut().unwrap()
    }
}

impl<T> Drop for PoolGuard<T> {
    fn drop(&mut self) {
        // COMPILE-TIME WIN: RAII ensures we can NEVER forget to return the object.
        // It is physically impossible to leak the object back into the wild
        // or drop it entirely, unless the thread panics while holding the lock.
        if let (Some(item), Ok(mut pool)) = (self.item.take(), self.pool.lock()) {
            // Return to pool
            pool.push(item);
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[derive(Debug, PartialEq, Eq, Clone)]
    struct DbConnection {
        id: usize,
    }

    impl DbConnection {
        pub fn new(id: usize) -> Self {
            Self { id }
        }
    }

    #[test]
    fn test_object_pool_reuse() {
        let pool = ObjectPool::new(vec![DbConnection::new(1), DbConnection::new(2)]);
        assert_eq!(pool.available(), 2);

        {
            let mut conn1 = pool.get().unwrap();
            assert_eq!(conn1.id, 2); // Pops from the end
            assert_eq!(pool.available(), 1);

            // Mutate the object via DerefMut
            conn1.id = 99;

            let conn2 = pool.get().unwrap();
            assert_eq!(conn2.id, 1);
            assert_eq!(pool.available(), 0);

            // Pool is empty
            assert!(pool.get().is_none());

            // conn1 and conn2 go out of scope here
        }

        // Drop has been called, returning items to the pool
        assert_eq!(pool.available(), 2);

        // Verify the mutated item was returned
        let mut restored_conn = pool.get().unwrap();
        assert!(restored_conn.id == 99 || restored_conn.id == 1);
    }

    #[test]
    fn test_multithreaded_pool() {
        let pool = ObjectPool::new((0..5).map(DbConnection::new));
        let mut handles = vec![];

        for _ in 0..10 {
            let pool_clone = pool.clone();
            handles.push(thread::spawn(move || {
                // Try to get a connection. We might fail if all 5 are checked out.
                if let Some(conn) = pool_clone.get() {
                    // Simulate work
                    let _val = conn.id;
                    // Automatically returned at end of scope
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // All items should be returned regardless of how threads scheduled
        assert_eq!(pool.available(), 5);
    }
}

// ============================================================================
// Footer
// ============================================================================
//
// How this pattern appears in std/crates:
// - `sqlx` and `diesel` use this exact `PoolGuard` approach for database connections.
// - `slab` provides pre-allocated storage where indices act as "pool handles."
//
// What the GoF/OOP equivalent is and why it doesn't translate directly:
// In Java/C#, you must use a `try/finally` block to manually call `pool.release(obj)`.
// Rust leverages the `Drop` trait and the borrow checker to enforce this automatically.
