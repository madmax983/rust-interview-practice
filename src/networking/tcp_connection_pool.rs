//! # TCP Connection Pool Implementation
//!
//! A generic connection pool for managing TCP (or other) connections.
//!
//! **Replaces Crates:** `r2d2`, `bb8`, `deadpool`
//!
//! **Real-world Usage:**
//! - Database clients (`PostgreSQL`, `MySQL`, Redis) to avoid handshake overhead.
//! - HTTP clients (Keep-Alive connections).
//!
//! **Why build it yourself?**
//! Connection pooling is critical for performance. Building one shows you how to manage resources
//! with `Drop` guards, handle "checkout" semantics, and coordinate threads waiting for resources
//! using `Condvar`. You also learn about "validity checks" (pings) and lifecycle management.

use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Shared State:
// - `idle_connections`: A stack (Vec) of idle connections ready to be reused.
// - `num_connections`: Total active connections (idle + checked out).
// - `config`: Max pool size, timeouts.
//
// Traits:
// - `ConnectionManager`: Abstraction for creating and checking connections.
//
// Lifecycle:
// - `get()`: Checkout a connection.
//   - If idle available: pop and return.
//   - If not idle but space available: create new.
//   - If full: wait on Condvar.
// - `Drop` of `PooledConnection`: Return connection to idle list and notify Condvar.
//
// Invariants:
// 1. `num_connections` <= `max_size`.
// 2. `idle_connections.len()` <= `num_connections`.

/// Trait for managing the lifecycle of connections.
pub trait ConnectionManager {
    type Connection: Send + 'static;
    type Error: std::fmt::Debug + Send + 'static;

    /// Creates a new connection.
    ///
    /// # Errors
    /// Returns [`Self::Error`] if a new connection cannot be established.
    fn connect(&self) -> Result<Self::Connection, Self::Error>;

    /// Checks if the connection is still valid (e.g., ping).
    ///
    /// # Errors
    /// Returns [`Self::Error`] if the connection is no longer valid.
    fn is_valid(&self, conn: &mut Self::Connection) -> Result<(), Self::Error>;

    /// Checks if the connection has broken (fast check).
    fn has_broken(&self, conn: &mut Self::Connection) -> bool;
}

#[derive(Clone, Copy)]
pub struct PoolConfig {
    pub max_size: usize,
    pub connection_timeout: Duration,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_size: 10,
            connection_timeout: Duration::from_secs(30),
        }
    }
}

struct SharedPool<M: ConnectionManager> {
    config: PoolConfig,
    manager: M,
    state: Mutex<PoolState<M>>,
    cond: Condvar,
}

struct PoolState<M: ConnectionManager> {
    idle_connections: Vec<M::Connection>,
    num_connections: usize,
}

/// A generic connection pool.
pub struct Pool<M: ConnectionManager> {
    shared: Arc<SharedPool<M>>,
}

// Manual Clone implementation to avoid M: Clone bound
impl<M: ConnectionManager> Clone for Pool<M> {
    fn clone(&self) -> Self {
        Self {
            shared: self.shared.clone(),
        }
    }
}

/// A smart pointer wrapping a connection checked out from the pool.
pub struct PooledConnection<M: ConnectionManager> {
    pool: Pool<M>,
    conn: Option<M::Connection>,
}

#[derive(Debug)]
pub enum PoolError<E> {
    Manager(E),
    Timeout,
}

impl<M: ConnectionManager> Pool<M> {
    /// Creates a new connection pool backed by the given manager and config.
    ///
    /// # Panics
    /// Panics if `config.max_size` is zero.
    pub fn new(manager: M, config: PoolConfig) -> Self {
        assert!(config.max_size > 0, "max_size must be positive");
        Self {
            shared: Arc::new(SharedPool {
                config,
                manager,
                state: Mutex::new(PoolState {
                    idle_connections: Vec::with_capacity(config.max_size),
                    num_connections: 0,
                }),
                cond: Condvar::new(),
            }),
        }
    }

    /// Checks out a connection from the pool, blocking until one is available
    /// or the configured timeout elapses.
    ///
    /// # Errors
    /// Returns [`PoolError::Timeout`] if no connection becomes available within
    /// the timeout, or [`PoolError::Manager`] if creating a new connection fails.
    ///
    /// # Panics
    /// Panics if the internal state lock is poisoned.
    // The state guard is deliberately released and re-acquired around user I/O
    // (validation/creation); it cannot be tightened without breaking that dance.
    #[allow(clippy::significant_drop_tightening)]
    pub fn get(&self) -> Result<PooledConnection<M>, PoolError<M::Error>> {
        let mut state = self.shared.state.lock().unwrap();
        let start = std::time::Instant::now();
        let timeout = self.shared.config.connection_timeout;

        loop {
            // 1. Try to reuse an idle connection
            if let Some(mut conn) = state.idle_connections.pop() {
                // RUST INSIGHT: Drop Lock for Validation
                // We drop the lock before calling user code (is_valid) to avoid holding the lock
                // during potentially slow I/O operations. This improves concurrency.
                drop(state);

                if matches!(self.shared.manager.is_valid(&mut conn), Ok(())) {
                    return Ok(PooledConnection {
                        pool: self.clone(),
                        conn: Some(conn),
                    });
                }
                // Connection is invalid. We must re-acquire lock to update state.
                state = self.shared.state.lock().unwrap();
                state.num_connections -= 1;
                // We freed a slot, so notify waiting threads.
                self.shared.cond.notify_one();
                continue;
            }

            // 2. If no idle, try to create new
            if state.num_connections < self.shared.config.max_size {
                // Reserve the slot
                state.num_connections += 1;
                // Drop lock for connection creation
                drop(state);

                match self.shared.manager.connect() {
                    Ok(conn) => {
                        return Ok(PooledConnection {
                            pool: self.clone(),
                            conn: Some(conn),
                        });
                    }
                    Err(e) => {
                        // Creation failed. Re-acquire lock to rollback reservation.
                        state = self.shared.state.lock().unwrap();
                        state.num_connections -= 1;
                        self.shared.cond.notify_all();
                        return Err(PoolError::Manager(e));
                    }
                }
            }

            // 3. Pool is full, wait.
            let elapsed = start.elapsed();
            let Some(remaining) = timeout.checked_sub(elapsed) else {
                return Err(PoolError::Timeout);
            };

            // RUST INSIGHT: Condvar Wait
            // `wait_timeout` atomically releases the lock and blocks.
            // When it returns (or times out), it re-acquires the lock.
            let (new_state, result) = self.shared.cond.wait_timeout(state, remaining).unwrap();

            state = new_state;

            if result.timed_out() {
                return Err(PoolError::Timeout);
            }
        }
    }

    fn return_connection(&self, conn: M::Connection) {
        let mut state = self.shared.state.lock().unwrap();

        if state.idle_connections.len() < self.shared.config.max_size {
            state.idle_connections.push(conn);
            self.shared.cond.notify_one();
        } else {
            // Should not happen if invariants are maintained, but if it does (e.g., bug),
            // we drop the connection (it goes out of scope).
            state.num_connections -= 1;
        }
    }
}

// RUST INSIGHT: Implementing Deref allows PooledConnection to be used exactly like the inner Connection.
// This is "Coercion" magic.
impl<M: ConnectionManager> Deref for PooledConnection<M> {
    type Target = M::Connection;

    fn deref(&self) -> &Self::Target {
        self.conn.as_ref().unwrap()
    }
}

impl<M: ConnectionManager> DerefMut for PooledConnection<M> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.conn.as_mut().unwrap()
    }
}

impl<M: ConnectionManager> Drop for PooledConnection<M> {
    fn drop(&mut self) {
        // Take the connection out of the Option so we can move it back to the pool
        if let Some(conn) = self.conn.take() {
            // We return logic to the pool. The pool does not validate on return (common optimization),
            // relying on `get` to validate on checkout.
            self.pool.return_connection(conn);
        }
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `r2d2`: Synchronous, robust, widely used. Similar design.
// - `bb8`: Async version of `r2d2`.
// - `deadpool`: Async, uses a queue.
//
// Missing vs. Production:
// - **Async Support**: This is blocking.
// - **Reaping**: No background thread to close idle connections after TTL.
// - **Fairness**: `Condvar` doesn't guarantee FIFO.
// - **Health Checks**: We check on checkout. Production pools often background check.

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;

    struct FakeConnection(usize);

    struct FakeManager {
        count: AtomicUsize,
    }

    impl ConnectionManager for FakeManager {
        type Connection = FakeConnection;
        type Error = ();

        fn connect(&self) -> Result<Self::Connection, Self::Error> {
            let id = self.count.fetch_add(1, Ordering::SeqCst);
            Ok(FakeConnection(id))
        }

        fn is_valid(&self, _conn: &mut Self::Connection) -> Result<(), Self::Error> {
            Ok(())
        }

        fn has_broken(&self, _conn: &mut Self::Connection) -> bool {
            false
        }
    }

    #[test]
    fn test_pool_limit() {
        let manager = FakeManager {
            count: AtomicUsize::new(0),
        };
        let config = PoolConfig {
            max_size: 2,
            ..Default::default()
        };
        let pool = Pool::new(manager, config);

        let _c1 = pool.get().unwrap();
        let _c2 = pool.get().unwrap();

        // This should timeout because pool is full (size 2)
        let config_timeout = PoolConfig {
            max_size: 2,
            connection_timeout: Duration::from_millis(50),
        };
        let pool_timeout = Pool::new(
            FakeManager {
                count: AtomicUsize::new(0),
            },
            config_timeout,
        );
        // Ensure distinct pool state for this test
        // Wait, creating a new pool creates new state. Yes.

        // Wait, FakeManager count is distinct? Yes.
        let _c3 = pool_timeout.get().unwrap();
        let _c4 = pool_timeout.get().unwrap();

        assert!(matches!(pool_timeout.get(), Err(PoolError::Timeout)));
    }

    #[test]
    fn test_pool_reuse() {
        let manager = FakeManager {
            count: AtomicUsize::new(0),
        };
        let config = PoolConfig {
            max_size: 1,
            ..Default::default()
        };
        let pool = Pool::new(manager, config);

        {
            let c1 = pool.get().unwrap();
            assert_eq!(c1.0, 0);
        } // c1 dropped, returned to pool

        {
            let c2 = pool.get().unwrap();
            assert_eq!(c2.0, 0); // Should be the same connection
        }
    }

    #[test]
    fn test_concurrent_access() {
        let manager = FakeManager {
            count: AtomicUsize::new(0),
        };
        let config = PoolConfig {
            max_size: 5,
            ..Default::default()
        };
        let pool = Pool::new(manager, config);
        let mut handles = vec![];

        for _ in 0..10 {
            let pool = pool.clone();
            handles.push(thread::spawn(move || {
                let _conn = pool.get().unwrap();
                thread::sleep(Duration::from_millis(10));
                // conn dropped
            }));
        }

        for h in handles {
            h.join().unwrap();
        }
    }

    // New test for broken manager
    struct BrokenManager {
        should_fail_connect: bool,
        should_fail_valid: bool,
    }

    impl ConnectionManager for BrokenManager {
        type Connection = ();
        type Error = String;

        fn connect(&self) -> Result<Self::Connection, Self::Error> {
            if self.should_fail_connect {
                Err("Connect Failed".to_string())
            } else {
                Ok(())
            }
        }

        fn is_valid(&self, _conn: &mut Self::Connection) -> Result<(), Self::Error> {
            if self.should_fail_valid {
                Err("Invalid Connection".to_string())
            } else {
                Ok(())
            }
        }

        fn has_broken(&self, _conn: &mut Self::Connection) -> bool {
            false
        }
    }

    #[test]
    fn test_broken_manager() {
        // Test 1: Connect failure
        let manager = BrokenManager {
            should_fail_connect: true,
            should_fail_valid: false,
        };
        let pool = Pool::new(
            manager,
            PoolConfig {
                max_size: 1,
                ..Default::default()
            },
        );

        let res = pool.get();
        assert!(matches!(res, Err(PoolError::Manager(_))));

        // Ensure pool state is recovered (num_connections should be 0)
        let state = pool.shared.state.lock().unwrap();
        assert_eq!(state.num_connections, 0);
        drop(state);

        // Test 2: Validation failure
        let manager_valid = BrokenManager {
            should_fail_connect: false,
            should_fail_valid: true,
        };
        let pool_valid = Pool::new(
            manager_valid,
            PoolConfig {
                max_size: 1,
                ..Default::default()
            },
        );

        // First get succeeds (creates new)
        // But checking `is_valid` happens on reuse.
        // So:
        // 1. Get conn (create)
        // 2. Return conn
        // 3. Get conn (reuse -> check valid -> fail -> discard -> create new)

        let c1 = pool_valid.get().unwrap();
        drop(c1); // Returned to pool

        // Reuse
        // This will check is_valid -> fail.
        // Then loop checks num_connections (0).
        // Then tries to create new (succeeds).
        let c2 = pool_valid.get();
        assert!(c2.is_ok());
    }
}
