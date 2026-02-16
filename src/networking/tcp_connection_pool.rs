//! # TCP Connection Pool Implementation
//!
//! Implements a generic, thread-safe connection pool for managing network resources.
//! It handles connection lifecycle, health checking, and efficient reuse of TCP streams
//! (or any other connection type).
//!
//! **Replaces Crates:** `r2d2`, `bb8`, `deadpool`
//!
//! **Real-world Usage:**
//! - Database clients (Postgres, Redis) to avoid handshake overhead per query.
//! - HTTP clients (keeping sockets open for Keep-Alive).
//! - Microservices communicating over persistent TCP links.
//!
//! **Why build it yourself?**
//! Connection pooling is the single most critical component for system stability under load.
//! By building it, you learn:
//! - How to manage shared mutable state across threads (`Arc<Mutex<State>>`).
//! - How to implement blocking waits with `Condvar` (waiting for a free connection).
//! - The importance of "RAII guards" (`Drop` trait) to automatically return resources.
//! - Handling "thundering herd" problems and fairness in resource acquisition.

use std::fmt;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

// =========================================================================================
// Architecture
// =========================================================================================
//
// Flow:
//
//      Thread A                Pool                  Thread B
//         │                     │                       │
//    get() call ──────────────► │                       │
//         │                [Lock State]                 │
//         │                     │                       │
//         │            <Idle connection?> ── Yes ──────► Return Conn
//         │                     │                       │
//         │                     No                      │
//         │                     ▼                       │
//         │            <Capacity available?> ── Yes ───► Create New
//         │                     │                       │
//         │                     No                      │
//         │                     ▼                       │
//    [Block on Condvar] ◄────── [Wait]                  │
//         │                     │                       │
//         │                     ▲ ◄──────────────── [Return Conn]
//    [Wake up] ───────────────► │                       │
//         │                [Lock State]                 │
//    [Retry Logic] ───────────► │                       │
//         ▼                     ▼                       ▼
//    Return Conn           Update State            (Continue)
//
//
// Invariants:
// 1. `idle_connections.len() <= size`.
// 2. `size <= max_size`.
// 3. A `PooledConnection` always holds a valid permit; dropping it returns the permit.
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ Get (Idle)    │ O(1)        │ O(1)        │
// ├───────────────┼─────────────┼─────────────┤
// │ Get (New)     │ O(Connect)  │ O(1)        │
// ├───────────────┼─────────────┼─────────────┤
// │ Get (Full)    │ O(Wait)     │ O(1)        │
// └───────────────┴─────────────┴─────────────┘
//
// Design Decisions:
// - **`Arc<Mutex<State>>`**: Standard shared-state pattern.
//   - *Tradeoff*: Mutex contention under high load.
//   - *Alternative*: Lock-free queues (like `flume`) or sharded locks (like `ConcurrentHashmap`).
// - **Blocking `get()`**: We use `Condvar` to block the thread until a connection is available.
//   - *Tradeoff*: Simple synchronous API.
//   - *Alternative*: `Future`-based `get()` for async runtimes (requires `Waker` integration).
// - **Lazy Connection**: Connections are created on demand, not pre-filled.
//   - *Tradeoff*: First request latency.
//   - *Alternative*: `min_idle` configuration to keep a baseline of warm connections.

/// A trait which provides connection creation and health checking logic.
pub trait Manager: Send + Sync + 'static {
    /// The connection type this manager creates.
    type Connection: Send + 'static;
    /// The error type returned by the manager.
    type Error: fmt::Debug + Send + 'static;

    /// Creates a new connection.
    fn connect(&self) -> Result<Self::Connection, Self::Error>;

    /// Checks if the connection is still valid.
    /// Returns `true` if valid, `false` if broken.
    fn is_valid(&self, conn: &mut Self::Connection) -> bool;

    /// Optional: Checks if a connection has broken *during* use.
    fn has_broken(&self, _conn: &mut Self::Connection) -> bool {
        false
    }
}

/// A generic connection pool.
pub struct Pool<M: Manager> {
    shared: Arc<Shared<M>>,
}

struct Shared<M: Manager> {
    state: Mutex<State<M>>,
    cond: Condvar,
    manager: M,
    max_size: u32,
    timeout: Option<Duration>,
}

struct State<M: Manager> {
    idle_connections: Vec<M::Connection>,
    /// Total number of connections (idle + active).
    size: u32,
}

/// A smart pointer wrapping a connection from the pool.
pub struct PooledConnection<M: Manager> {
    pool: Pool<M>,
    conn: Option<M::Connection>,
}

// RUST INSIGHT: Clone for Pool
// Deriving `Clone` for `Pool` (which wraps `Arc`) allows us to cheaply pass
// the pool handle to multiple threads. The generic bound `M: Manager` implies `Sized`,
// so we don't need manual implementation.
impl<M: Manager> Clone for Pool<M> {
    fn clone(&self) -> Self {
        Pool {
            shared: Arc::clone(&self.shared),
        }
    }
}

impl<M: Manager> Pool<M> {
    /// Creates a new generic connection pool with the given manager and configuration.
    pub fn new(manager: M, max_size: u32) -> Self {
        Self::new_with_timeout(manager, max_size, Some(Duration::from_secs(30)))
    }

    /// Creates a new pool with a specific timeout.
    pub fn new_with_timeout(manager: M, max_size: u32, timeout: Option<Duration>) -> Self {
        Pool {
            shared: Arc::new(Shared {
                state: Mutex::new(State {
                    idle_connections: Vec::new(),
                    size: 0,
                }),
                cond: Condvar::new(),
                manager,
                max_size,
                timeout,
            }),
        }
    }

    /// Retrieves a connection from the pool.
    ///
    /// This method will:
    /// 1. Return an idle connection if available.
    /// 2. Create a new connection if `size < max_size`.
    /// 3. Block and wait if the pool is full.
    pub fn get(&self) -> Result<PooledConnection<M>, M::Error> {
        let mut state = self.shared.state.lock().unwrap();
        let start = Instant::now();

        loop {
            // 1. Try to reuse an idle connection
            if let Some(mut conn) = state.idle_connections.pop() {
                // RUST INSIGHT: Critical Section Minimization
                // We drop the lock before performing the health check (which might involve network I/O).
                // This prevents blocking other threads while checking validity.
                drop(state);

                if self.shared.manager.is_valid(&mut conn) {
                    return Ok(PooledConnection {
                        pool: self.clone(),
                        conn: Some(conn),
                    });
                } else {
                    // Connection is dead. Update state and retry.
                    state = self.shared.state.lock().unwrap();
                    state.size -= 1;
                    // Notify waiters because a slot just freed up (size decreased)
                    self.shared.cond.notify_one();
                    continue;
                }
            }

            // 2. Try to create a new connection
            if state.size < self.shared.max_size {
                state.size += 1;
                drop(state);

                match self.shared.manager.connect() {
                    Ok(conn) => {
                        return Ok(PooledConnection {
                            pool: self.clone(),
                            conn: Some(conn),
                        });
                    }
                    Err(e) => {
                        // Creation failed. Revert state.
                        state = self.shared.state.lock().unwrap();
                        state.size -= 1;
                        self.shared.cond.notify_one();
                        return Err(e);
                    }
                }
            }

            // 3. Wait for a connection to become available
            if let Some(timeout) = self.shared.timeout {
                let elapsed = start.elapsed();
                let wait_duration = timeout.checked_sub(elapsed).unwrap_or(Duration::ZERO);

                if wait_duration.is_zero() {
                    // We timed out. In a production pool, this would return a PoolError::Timeout.
                    drop(state); // Release lock before panicking to avoid poisoning
                    panic!("Timeout waiting for connection");
                }

                let (new_state, result) = self
                    .shared
                    .cond
                    .wait_timeout(state, wait_duration)
                    .unwrap();
                state = new_state;

                if result.timed_out() {
                    // Check one last time if a connection became available during the race
                    if state.idle_connections.is_empty() && state.size >= self.shared.max_size {
                        drop(state); // Release lock before panicking
                        panic!("Timeout waiting for connection");
                    }
                }
            } else {
                state = self.shared.cond.wait(state).unwrap();
            }
        }
    }
}

// =========================================================================================
// PooledConnection Implementation
// =========================================================================================

impl<M: Manager> Deref for PooledConnection<M> {
    type Target = M::Connection;

    fn deref(&self) -> &Self::Target {
        self.conn.as_ref().unwrap()
    }
}

impl<M: Manager> DerefMut for PooledConnection<M> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.conn.as_mut().unwrap()
    }
}

impl<M: Manager> Drop for PooledConnection<M> {
    fn drop(&mut self) {
        // Take the connection out of the Option
        if let Some(mut conn) = self.conn.take() {
            // Handle lock poisoning gracefully to avoid double panic during unwinding
            let mut state = self.pool.shared.state.lock().unwrap_or_else(|e| e.into_inner());

            // GOTCHA: Double-check validity on return?
            // Some pools do `test_on_checkin`. If it's expensive, we might skip it.
            // Here, we'll just return it. If it's broken, the next `get` will find out.
            // However, if the user marked it as broken (conceptually), we might want to discard it.
            // Our trait has `has_broken`, let's use it.
            // Note: We need `&mut conn` but we are in `drop`.

            // If the manager says it's broken, discard it.
            // We can't easily check `has_broken` here without user interaction?
            // Usually `PooledConnection` has a method `is_broken` the user sets.
            // For now, we assume it's good.

            if state.size <= self.pool.shared.max_size {
                 state.idle_connections.push(conn);
                 // Notify a waiter that a connection is available
                 self.pool.shared.cond.notify_one();
            } else {
                // Should not happen if invariants hold, but if we resized pool down:
                state.size -= 1;
            }
        }
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `r2d2`: The gold standard. Supports generic managers, timeouts, and `test_on_check_out`.
// - `bb8`: Async version of `r2d2` for Tokio.
// - `deadpool`: Another async pool with a different queueing strategy.
//
// Missing vs. Production:
// - **Error Type**: We return `M::Error` or panic on timeout. Production pools have a dedicated `PoolError` enum.
// - **Async Support**: This is strictly blocking.
// - **Reaping**: No background thread to close idle connections after a TTL.
// - **Fairness**: `Condvar` doesn't guarantee FIFO ordering for waiters.
//
// Next Steps:
// 1. Introduce `PoolError` enum to handle Timeouts and Manager errors cleanly.
// 2. Add `min_idle` support to pre-warm the pool.
// 3. Implement a `Reaper` thread to clean up old idle connections.

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;

    #[derive(Debug)]
    struct MockConnection {
        id: usize,
    }

    struct MockManager {
        counter: AtomicUsize,
    }

    impl MockManager {
        fn new() -> Self {
            Self {
                counter: AtomicUsize::new(0),
            }
        }
    }

    impl Manager for MockManager {
        type Connection = MockConnection;
        type Error = ();

        fn connect(&self) -> Result<Self::Connection, Self::Error> {
            let id = self.counter.fetch_add(1, Ordering::SeqCst);
            Ok(MockConnection { id })
        }

        fn is_valid(&self, _conn: &mut Self::Connection) -> bool {
            true
        }
    }

    #[test]
    fn test_pool_creation_and_get() {
        let manager = MockManager::new();
        let pool = Pool::new(manager, 2);

        let conn1 = pool.get().unwrap();
        assert_eq!(conn1.id, 0);

        let conn2 = pool.get().unwrap();
        assert_eq!(conn2.id, 1);

        drop(conn1); // Return 0 to pool

        let conn3 = pool.get().unwrap();
        assert_eq!(conn3.id, 0, "Should reuse connection 0");
    }

    #[test]
    fn test_pool_max_size_blocking() {
        let manager = MockManager::new();
        let pool = Pool::new(manager, 1);

        let conn1 = pool.get().unwrap();

        // Spawn a thread to wait for connection
        let pool_clone = pool.clone();
        let handle = thread::spawn(move || {
            let conn2 = pool_clone.get().unwrap();
            assert_eq!(conn2.id, 0, "Should get the reused connection");
        });

        // Sleep to ensure the thread is blocked
        thread::sleep(Duration::from_millis(100));

        drop(conn1); // Release connection

        handle.join().unwrap();
    }

    #[test]
    #[should_panic(expected = "Timeout waiting for connection")]
    fn test_pool_timeout() {
        let manager = MockManager::new();
        // Pool with 1 connection and 100ms timeout
        let pool = Pool::new_with_timeout(manager, 1, Some(Duration::from_millis(100)));

        let _conn1 = pool.get().unwrap();

        // This should block for 100ms and then panic
        let _conn2 = pool.get().unwrap();
    }
}
