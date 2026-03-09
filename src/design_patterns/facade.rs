// Suppress pedantic and nursery lints for design pattern examples.
#![allow(clippy::pedantic, clippy::nursery, unused)]
//! # The Facade / Session Pattern
//!
//! Replaces: **Facade Pattern** (GoF), **Session Beans** (Java)
//!
//! Real Rust usage: `sqlx::Transaction`, `std::net::TcpStream`
//!
//! ## Why this pattern exists in Rust
//! Rust uses lifetimes to bind high-level stateful wrappers (Sessions) to the
//! resources they control (like connection pools or active transactions). This
//! enforces at compile time that the Session cannot outlive the underlying resource,
//! preventing dangling references or double-closes.
//!
//! ## Architecture
//!
//! ```text
//! [ ConnectionPool ] <- borrows - [ DatabaseSession ] -> provides API -> [ Application ]
//! ```
//!
//! **Invariants:**
//! - The `Session` holds a reference (`&'a mut`) to the underlying system.
//! - The `Session` provides a simplified, domain-specific API.
//! - When the `Session` drops, any necessary cleanup (like rolling back an uncommitted
//!   transaction) happens automatically via RAII.
//!
//! ## When to use
//! - When wrapping complex subsystems (like a database connection or a hardware device)
//!   behind a simple interface.
//! - When you need to tie temporary state (like an open transaction) to a specific scope.

// ============================================================================
// Pattern: The Facade / Session
// ============================================================================

/// A dummy subsystem representing a low-level database connection pool.
pub struct ConnectionPool {
    connected: bool,
}

impl ConnectionPool {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self { connected: true }
    }

    /// Low-level method that users shouldn't call directly
    fn execute_raw(&self, query: &str) -> Result<(), String> {
        if !self.connected {
            return Err("Not connected".to_string());
        }
        println!("Executing: {}", query);
        Ok(())
    }
}

/// The Facade/Session that simplifies interaction with the pool.
///
/// **OWNERSHIP INSIGHT:** The lifetime `'a` ties this session to the `ConnectionPool`.
/// The session cannot outlive the pool.
pub struct DatabaseSession<'a> {
    pool: &'a ConnectionPool,
    transaction_open: bool,
}

impl<'a> DatabaseSession<'a> {
    pub fn begin(pool: &'a ConnectionPool) -> Result<Self, String> {
        pool.execute_raw("BEGIN")?;
        Ok(Self {
            pool,
            transaction_open: true,
        })
    }

    /// Domain-specific API, hiding the raw SQL string
    pub fn add_user(&self, username: &str) -> Result<(), String> {
        let query = format!("INSERT INTO users (name) VALUES ('{}')", username);
        self.pool.execute_raw(&query)
    }

    /// Explicitly commit the transaction
    pub fn commit(mut self) -> Result<(), String> {
        self.pool.execute_raw("COMMIT")?;
        self.transaction_open = false; // Prevent rollback on drop
        Ok(())
    }
}

// COMPILE-TIME WIN & RAII: If the session goes out of scope without `commit` being
// called, the `Drop` implementation automatically rolls back, preventing dangling
// or locked transactions.
impl<'a> Drop for DatabaseSession<'a> {
    fn drop(&mut self) {
        if self.transaction_open {
            let _ = self.pool.execute_raw("ROLLBACK");
            println!("Transaction implicitly rolled back on drop.");
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_successful_commit() {
        let pool = ConnectionPool::new();
        let session = DatabaseSession::begin(&pool).unwrap();

        session.add_user("alice").unwrap();

        // Explicitly committing consumes the session and prevents rollback
        assert!(session.commit().is_ok());
    }

    #[test]
    fn test_implicit_rollback() {
        let pool = ConnectionPool::new();
        {
            let session = DatabaseSession::begin(&pool).unwrap();
            session.add_user("bob").unwrap();
            // Session goes out of scope here; `drop` is called, executing "ROLLBACK".
        }
    }
}

// ============================================================================
// Footer
// ============================================================================
//
// Production Note:
// In `sqlx` or `diesel`, transactions are implemented using this exact pattern.
// A `Transaction` is a struct that holds a mutable borrow to the connection
// and automatically rolls back on drop unless explicitly committed.
