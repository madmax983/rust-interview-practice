//! # Connection Pool Implementation
//!
//! Implements a generic, thread-safe synchronous connection pool from scratch.
//! This allows reusing expensive resources (like database connections or network sockets)
//! instead of opening a new one for every request.
//!
//! **Replaces Crates:** `r2d2`, `deadpool` (sync parts), `mobc`
//!
//! **Real-world Usage:**
//! - Database Connection Pooling (PostgreSQL, MySQL).
//! - Redis/Memcached client connection management.
//! - Reusing expensive TLS connections for outgoing HTTP requests.
//!
//! **Why build it yourself?**
//! Building a connection pool teaches you how to manage shared state limits,
//! thread synchronization with `Condvar`, and the powerful RAII (Resource Acquisition Is Initialization)
//! pattern in Rust to automatically return connections to the pool when they are dropped.

use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure:
//
//      Arc<SharedPoolState<T>>
//      ├── Mutex<PoolState<T>>
//      │   ├── idle: Vec<T>           (Idle connections)
//      │   └── active_count: usize    (Number of currently checked out or creating connections)
//      └── Condvar                    (To wake up threads waiting for a connection)
//
// Flow:
//   [Client] -> get() -> Lock Mutex
//                         ├── If idle.pop() -> Return connection.
//                         ├── If active_count < max_size -> Create new, return connection.
//                         └── Else -> Wait on Condvar until a connection is returned.
//
//   [Client drops connection] -> Drop::drop() -> Lock Mutex
//                         ├── push connection back to idle
//                         ├── decrement active_count
//                         └── Condvar.notify_one() -> Wakes up a waiting client.
//
// Invariants:
// 1. `active_count` <= `max_size`.
// 2. The total number of existing connections (`idle.len() + active_count`) <= `max_size`.
//    (Note: a connection checked out to a client is part of `active_count`).
// 3. A dropped connection is always returned to the pool or properly discarded (RAII).
//
// Complexity:
// ┌───────────────┬──────────────┬─────────────┐
// │ Operation     │ Time         │ Space       │
// ├───────────────┼──────────────┼─────────────┤
// │ get()         │ O(1)*        │ O(N)        │
// │ drop()        │ O(1)         │ O(1)        │
// └───────────────┴──────────────┴─────────────┘
// * get() is O(1) if a connection is available. Otherwise, it blocks until a connection is freed.
//
// Design Decisions:
// - **Lock Contention**: The factory closure is called *outside* the mutex lock.
//   This prevents a slow connection establishment from blocking other threads from checking out idle connections.
// - **Smart Pointer**: `PooledConnection<T>` implements `Deref`, `DerefMut`, and `Drop`.
//   This makes it feel like a standard reference, but intercepts the drop to recycle the resource.

/// A trait for managing the lifecycle of a connection.
/// Trait-based interfaces allow swappable strategies (e.g., Postgres, Redis, HTTP).
pub trait ManageConnection: Send + Sync + 'static {
    type Connection: Send + 'static;
    type Error: Send + 'static;

    /// Attempts to create a new connection.
    fn connect(&self) -> Result<Self::Connection, Self::Error>;

    // PRODUCTION NOTE: A real pool trait would also include `is_valid` (ping)
    // and `has_broken` to check if a connection returned to the pool is still good.
}

/// Errors that can occur when interacting with the pool.
#[derive(Debug, PartialEq)]
pub enum PoolError<E> {
    /// The factory function returned an error while creating a connection.
    Factory(E),
    /// A timeout occurred while waiting for a connection.
    Timeout,
}

struct PoolState<T> {
    idle: Vec<T>,
    active_count: usize,
}

struct Shared<M: ManageConnection> {
    state: Mutex<PoolState<M::Connection>>,
    cvar: Condvar,
    max_size: usize,
    manager: M,
}

/// A generic, thread-safe connection pool.
pub struct ConnectionPool<M: ManageConnection> {
    shared: Arc<Shared<M>>,
}

// Clone implementation for the pool, allowing it to be shared across threads easily.
impl<M: ManageConnection> Clone for ConnectionPool<M> {
    fn clone(&self) -> Self {
        Self {
            shared: Arc::clone(&self.shared),
        }
    }
}

// RUST INSIGHT: Panic Safety
// If the `connect()` method panics while we have dropped the lock, `active_count` would remain
// permanently incremented without this guard, eventually deadlocking the pool.
// We use an RAII guard that decrements `active_count` if it is dropped prematurely.
struct ActiveCountGuard<'a, M: ManageConnection> {
    shared: &'a Arc<Shared<M>>,
    commit: bool,
}

impl<'a, M: ManageConnection> ActiveCountGuard<'a, M> {
    fn new(shared: &'a Arc<Shared<M>>) -> Self {
        Self {
            shared,
            commit: false,
        }
    }

    /// Call this when the connection is successfully created and put into a `PooledConnection`.
    /// This prevents the guard from decrementing the count on drop.
    fn commit(mut self) {
        self.commit = true;
    }
}

impl<'a, M: ManageConnection> Drop for ActiveCountGuard<'a, M> {
    fn drop(&mut self) {
        if !self.commit {
            let mut state = self.shared.state.lock().unwrap();
            state.active_count -= 1;
            self.shared.cvar.notify_one();
        }
    }
}

impl<M: ManageConnection> ConnectionPool<M> {
    /// Creates a new connection pool with a maximum size and a connection manager.
    #[must_use]
    pub fn new(max_size: usize, manager: M) -> Self {
        assert!(max_size > 0, "max_size must be greater than 0");
        Self {
            shared: Arc::new(Shared {
                state: Mutex::new(PoolState {
                    idle: Vec::with_capacity(max_size),
                    active_count: 0,
                }),
                cvar: Condvar::new(),
                max_size,
                manager,
            }),
        }
    }

    /// Retrieves a connection from the pool.
    /// Blocks indefinitely until a connection becomes available.
    pub fn get(&self) -> Result<PooledConnection<M>, PoolError<M::Error>> {
        let mut state = self.shared.state.lock().unwrap();

        loop {
            // 1. Try to get an idle connection
            if let Some(conn) = state.idle.pop() {
                state.active_count += 1;
                return Ok(PooledConnection {
                    item: Some(conn),
                    shared: Arc::clone(&self.shared),
                });
            }

            // 2. Try to create a new connection if under the limit
            if state.active_count < self.shared.max_size {
                // Reserve the slot by incrementing active_count *before* calling the manager.
                state.active_count += 1;

                // GOTCHA: We must drop the lock before calling the manager.
                // If we hold the lock while opening a DB connection (which involves network I/O),
                // we block all other threads from returning or getting connections.
                drop(state);

                // Set up the panic guard
                let guard = ActiveCountGuard::new(&self.shared);

                match self.shared.manager.connect() {
                    Ok(conn) => {
                        guard.commit(); // Connection succeeded, disable the guard
                        return Ok(PooledConnection {
                            item: Some(conn),
                            shared: Arc::clone(&self.shared),
                        });
                    }
                    Err(e) => {
                        // The guard will handle decrementing the active_count on drop
                        return Err(PoolError::Factory(e));
                    }
                }
            }

            // 3. Pool is full, wait for a connection to be returned
            state = self.shared.cvar.wait(state).unwrap();
        }
    }

    /// Retrieves a connection from the pool, waiting up to `timeout`.
    pub fn get_timeout(&self, timeout: Duration) -> Result<PooledConnection<M>, PoolError<M::Error>> {
        let mut state = self.shared.state.lock().unwrap();
        let end_time = Instant::now() + timeout;

        loop {
            if let Some(conn) = state.idle.pop() {
                state.active_count += 1;
                return Ok(PooledConnection {
                    item: Some(conn),
                    shared: Arc::clone(&self.shared),
                });
            }

            if state.active_count < self.shared.max_size {
                state.active_count += 1;
                drop(state);

                let guard = ActiveCountGuard::new(&self.shared);

                match self.shared.manager.connect() {
                    Ok(conn) => {
                        guard.commit();
                        return Ok(PooledConnection {
                            item: Some(conn),
                            shared: Arc::clone(&self.shared),
                        });
                    }
                    Err(e) => {
                        return Err(PoolError::Factory(e));
                    }
                }
            }

            let now = Instant::now();
            if now >= end_time {
                return Err(PoolError::Timeout);
            }

            let remaining = end_time - now;
            let (new_state, timeout_result) = self.shared.cvar.wait_timeout(state, remaining).unwrap();
            state = new_state;

            if timeout_result.timed_out() {
                // Final check in case a connection was returned right as we timed out
                if let Some(conn) = state.idle.pop() {
                    state.active_count += 1;
                    return Ok(PooledConnection {
                        item: Some(conn),
                        shared: Arc::clone(&self.shared),
                    });
                }
                return Err(PoolError::Timeout);
            }
        }
    }

    /// Returns the number of connections currently created and available in the pool.
    pub fn idle_count(&self) -> usize {
        let state = self.shared.state.lock().unwrap();
        state.idle.len()
    }

    /// Returns the number of connections currently checked out by clients.
    pub fn active_count(&self) -> usize {
        let state = self.shared.state.lock().unwrap();
        state.active_count
    }
}

/// A smart pointer wrapping a connection from the pool.
/// Automatically returns the connection to the pool when dropped.
pub struct PooledConnection<M: ManageConnection> {
    // Option allows us to take() the connection out if we need to discard it.
    item: Option<M::Connection>,
    shared: Arc<Shared<M>>,
}

impl<M: ManageConnection> PooledConnection<M> {
    /// Discards the connection instead of returning it to the pool.
    /// Useful if the connection is known to be broken (e.g., TCP socket closed).
    pub fn discard(mut self) {
        // Taking the item leaves `None`, so `Drop` won't return it to `idle`.
        self.item.take();
        // The Drop implementation will still run and decrement `active_count`.
    }
}

impl<M: ManageConnection> Deref for PooledConnection<M> {
    type Target = M::Connection;

    fn deref(&self) -> &Self::Target {
        self.item.as_ref().expect("PooledConnection is empty")
    }
}

impl<M: ManageConnection> DerefMut for PooledConnection<M> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.item.as_mut().expect("PooledConnection is empty")
    }
}

// RUST INSIGHT: RAII Pattern
// The Drop trait ensures that no matter how the client finishes using the connection
// (normal return, early return, panic via unwinding), the connection is securely
// placed back into the pool.
impl<M: ManageConnection> Drop for PooledConnection<M> {
    fn drop(&mut self) {
        let mut state = self.shared.state.lock().unwrap();
        if let Some(item) = self.item.take() {
            // Connection is still valid, return to idle pool
            state.idle.push(item);
        }

        // Decrement active_count whether it was discarded or returned
        state.active_count -= 1;

        // Wake up exactly one thread waiting for a connection
        self.shared.cvar.notify_one();
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `r2d2`: A widely used synchronous connection pool. Uses a dedicated trait `ManageConnection`
//   that implements `is_valid` (ping check) and `has_broken` logic.
// - `deadpool`: Primarily an async connection pool.
//
// Missing vs. Production:
// - **Health Checks**: A production pool tests connections (e.g., `SELECT 1`) before handing
//   them out to ensure they haven't timed out or dropped remotely.
// - **Idle Timeouts**: Background threads or eviction logic to close connections that have
//   sat idle for too long, freeing up server resources.
// - **Max Lifetime**: Forcing connections to be re-established after a certain time to prevent
//   long-lived connection state leakage.
//
// Benchmarking Note:
// To benchmark this accurately, compare it against `r2d2` or a simple `Mutex<Vec<Conn>>`
// in a highly concurrent scenario (e.g., using Criterion and 8+ threads).
// The benchmark should simulate quick checkouts and slightly slower `connect()` calls
// to observe how `ActiveCountGuard` and dropping the mutex prevents blocking.
//
// Next Steps:
// 1. Add `is_valid` to `ManageConnection` to run a ping query on `get()`.
// 2. Implement background reaping of old idle connections.
// 3. Port this logic to an asynchronous version using `tokio::sync::Semaphore`.

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;

    struct TestManager<F> {
        factory: F,
    }

    impl<F, T, E> ManageConnection for TestManager<F>
    where
        F: Fn() -> Result<T, E> + Send + Sync + 'static,
        T: Send + 'static,
        E: Send + 'static,
    {
        type Connection = T;
        type Error = E;

        fn connect(&self) -> Result<Self::Connection, Self::Error> {
            (self.factory)()
        }
    }

    fn make_pool<F, T, E>(max_size: usize, factory: F) -> ConnectionPool<TestManager<F>>
    where
        F: Fn() -> Result<T, E> + Send + Sync + 'static,
        T: Send + 'static,
        E: Send + 'static,
    {
        ConnectionPool::new(max_size, TestManager { factory })
    }

    #[test]
    fn test_pool_basic() {
        let pool = make_pool(2, || Ok::<String, ()>("conn".to_string()));

        {
            let conn1 = pool.get().unwrap();
            assert_eq!(*conn1, "conn");
            assert_eq!(pool.active_count(), 1);
            assert_eq!(pool.idle_count(), 0);

            let conn2 = pool.get().unwrap();
            assert_eq!(pool.active_count(), 2);

            // Both connections drop here
        }

        assert_eq!(pool.active_count(), 0);
        assert_eq!(pool.idle_count(), 2); // 2 were returned
    }

    #[test]
    fn test_pool_blocking_and_raii() {
        let pool = make_pool(1, || Ok::<usize, ()>(1));

        let conn1 = pool.get().unwrap();
        assert_eq!(pool.active_count(), 1);

        let pool_clone = pool.clone();

        // Spawn thread that tries to get a connection
        let handle = thread::spawn(move || {
            // This will block until conn1 is dropped
            let conn2 = pool_clone.get().unwrap();
            assert_eq!(*conn2, 1);
        });

        thread::sleep(Duration::from_millis(50));
        assert_eq!(pool.active_count(), 1); // Thread is blocked

        drop(conn1); // Releases the connection back to the pool

        handle.join().unwrap(); // Thread should now complete
        assert_eq!(pool.active_count(), 0);
        assert_eq!(pool.idle_count(), 1);
    }

    #[test]
    fn test_pool_timeout() {
        let pool = make_pool(1, || Ok::<usize, ()>(1));

        let _conn1 = pool.get().unwrap();

        // Try to get another connection, should timeout
        let result = pool.get_timeout(Duration::from_millis(10));
        assert_eq!(result.map(|_| ()).unwrap_err(), PoolError::Timeout);
    }

    #[test]
    fn test_pool_factory_error() {
        let pool = make_pool(2, || Err::<(), &str>("Failed to connect"));

        let result = pool.get();
        assert_eq!(result.map(|_| ()).unwrap_err(), PoolError::Factory("Failed to connect"));
        assert_eq!(pool.active_count(), 0);
    }

    #[test]
    fn test_pool_discard() {
        let counter = Arc::new(AtomicUsize::new(0));
        let c_clone = counter.clone();

        let pool = make_pool(2, move || {
            Ok::<usize, ()>(c_clone.fetch_add(1, Ordering::SeqCst))
        });

        {
            let conn = pool.get().unwrap();
            assert_eq!(*conn, 0);
            conn.discard(); // Do not return to pool
        }

        assert_eq!(pool.active_count(), 0);
        assert_eq!(pool.idle_count(), 0); // Not returned!

        let conn2 = pool.get().unwrap();
        assert_eq!(*conn2, 1); // A new connection was created
    }

    #[test]
    fn test_concurrent_access() {
        let pool = make_pool(3, || Ok::<String, ()>("db_conn".to_string()));
        let mut handles = vec![];

        for _ in 0..10 {
            let pool_clone = pool.clone();
            handles.push(thread::spawn(move || {
                let conn = pool_clone.get().unwrap();
                assert_eq!(*conn, "db_conn");
                thread::sleep(Duration::from_millis(10));
                // Automatically returned
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(pool.active_count(), 0);
        assert_eq!(pool.idle_count(), 3); // Max size reached
    }
}
