//! # The Error Enum Hierarchy
//!
//! Replaces: **Exceptions** (Java/Python/C++), **Error Codes** (C)
//!
//! Real Rust usage: `std::io::Error`, `serde_json::Error`, `anyhow` (application), `thiserror` (library)
//!
//! ## Why this pattern exists in Rust
//! Rust treats errors as values (`Result<T, E>`), not control flow exceptions.
//! To make this ergonomic, we need types that can compose (wrap lower-level errors)
//! and convert automatically (via `?` operator and `From` trait).
//!
//! ## Architecture
//!
//! ```text
//! AppError (enum)
//! ├── Config(ConfigError)
//! ├── Database(DatabaseError)
//! │   └── Io(std::io::Error)
//! └── Network(reqwest::Error)
//! ```
//!
//! **Invariants:**
//! - Errors form a directed acyclic graph (DAG) of ownership.
//! - Higher-level errors preserve the "source" (cause) for debugging.
//! - The `?` operator works seamlessly between layers.
//!
//! ## When to use
//! - **Libraries:** Use custom enums (like this pattern) to allow callers to handle specific cases.
//! - **Applications:** Use `anyhow::Result` for top-level code, or this pattern if you need structured recovery.

use std::error::Error;
use std::fmt;
use std::io;

// ============================================================================
// Layer 3: Low-Level Domain Errors
// ============================================================================

#[derive(Debug)]
pub enum DatabaseError {
    ConnectionFailed(String),
    QueryFailed { query: String, reason: String },
    // Wraps an underlying IO error (e.g., socket closed)
    Io(io::Error),
}

// Manual Display implementation
impl fmt::Display for DatabaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DatabaseError::ConnectionFailed(addr) => write!(f, "Failed to connect to DB at {}", addr),
            DatabaseError::QueryFailed { query, reason } => {
                write!(f, "Query failed: '{}', reason: {}", query, reason)
            }
            DatabaseError::Io(err) => write!(f, "Database IO error: {}", err),
        }
    }
}

// Manual Error implementation
impl Error for DatabaseError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            DatabaseError::ConnectionFailed(_) => None,
            DatabaseError::QueryFailed { .. } => None,
            // Forward the source to the underlying error
            DatabaseError::Io(err) => Some(err),
        }
    }
}

// COMPILE-TIME WIN: Implementing From allows `?` to convert io::Error -> DatabaseError
impl From<io::Error> for DatabaseError {
    fn from(err: io::Error) -> Self {
        DatabaseError::Io(err)
    }
}

// ============================================================================
// Layer 2: Application-Level Errors
// ============================================================================

#[derive(Debug)]
pub enum AppError {
    // A domain-specific error
    UserNotFound(u32),
    // Wrapper for database layer errors
    Database(DatabaseError),
    // Wrapper for config/filesystem errors
    ConfigLoad(io::Error),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::UserNotFound(id) => write!(f, "User {} not found", id),
            AppError::Database(err) => write!(f, "Application database error: {}", err),
            AppError::ConfigLoad(err) => write!(f, "Failed to load config: {}", err),
        }
    }
}

impl Error for AppError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AppError::UserNotFound(_) => None,
            // Chain to the next level
            AppError::Database(err) => Some(err),
            AppError::ConfigLoad(err) => Some(err),
        }
    }
}

// Conversions for `?`
impl From<DatabaseError> for AppError {
    fn from(err: DatabaseError) -> Self {
        AppError::Database(err)
    }
}

// Note: We don't implement From<io::Error> directly if `AppError` has multiple variants
// that could wrap io::Error (like Database and ConfigLoad).
// The compiler would complain about ambiguity or we'd have to pick one arbitrarily.
// Instead, we might create specific wrappers or force the callsite to map it.
//
// However, if `ConfigLoad` is the only direct IO error user, we *could*:
// impl From<io::Error> for AppError { fn from(e) -> Self { AppError::ConfigLoad(e) } }
// But it's often safer to be explicit at the callsite for top-level errors.

// ============================================================================
// Logic Simulation
// ============================================================================

// A type alias for brevity
pub type Result<T> = std::result::Result<T, AppError>;

struct UserRepository;

impl UserRepository {
    fn find_user(&self, id: u32) -> std::result::Result<String, DatabaseError> {
        if id == 0 {
            // Simulate IO error via conversion
            let io_err = io::Error::new(io::ErrorKind::ConnectionRefused, "Server down");
            return Err(io_err.into()); // Converts to DatabaseError::Io
        }
        if id == 999 {
            return Ok("Admin".to_string());
        }
        // Simulate domain error (converted to AppError::UserNotFound by caller usually,
        // but here we are in DB layer, so we return Ok(None) or specific DB error.
        // Let's say this method returns String, so it must fail if not found.
        Err(DatabaseError::QueryFailed {
            query: format!("SELECT * FROM users WHERE id={}", id),
            reason: "Row not found".to_string(),
        })
    }
}

fn handle_request(user_id: u32) -> Result<String> {
    let repo = UserRepository;

    // The `?` operator here converts DatabaseError -> AppError::Database
    let user_name = repo.find_user(user_id)?;

    Ok(format!("Hello, {}", user_name))
}

fn load_config() -> Result<()> {
    // Explicit mapping because we decided NOT to impl From<io::Error> for AppError
    // due to ambiguity (it could be DB IO or Config IO).
    std::fs::read_to_string("config.toml")
        .map_err(AppError::ConfigLoad)?;
    Ok(())
}

// ============================================================================
// Footer
// ============================================================================
//
// Production Note:
// In real apps, writing all this boilerplate (Display, Error, From) is tedious.
// Use the `thiserror` crate for libraries to derive these impls:
//
// ```rust
// #[derive(thiserror::Error, Debug)]
// pub enum AppError {
//     #[error("User {0} not found")]
//     UserNotFound(u32),
//     #[error(transparent)]
//     Database(#[from] DatabaseError),
// }
// ```
//
// Use `anyhow` for applications where you don't care about the specific error type,
// just that it failed and you want a stack trace/context.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_chaining() {
        let err = handle_request(0).unwrap_err();

        // Check top level
        match err {
            AppError::Database(db_err) => {
                // Check cause
                match db_err {
                    DatabaseError::Io(io_err) => {
                        assert_eq!(io_err.kind(), io::ErrorKind::ConnectionRefused);
                    }
                    _ => panic!("Expected IO error inside DB error"),
                }
            }
            _ => panic!("Expected Database error"),
        }
    }

    #[test]
    fn test_error_formatting() {
        let err = handle_request(0).unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("Application database error"));
        assert!(msg.contains("Database IO error"));
        assert!(msg.contains("Server down"));
    }

    #[test]
    fn test_source_traversal() {
        let err = handle_request(0).unwrap_err();

        let db_source = err.source().expect("Should have source");
        assert!(db_source.is::<DatabaseError>());

        let io_source = db_source.source().expect("Should have nested source");
        assert!(io_source.is::<io::Error>());
    }
}
