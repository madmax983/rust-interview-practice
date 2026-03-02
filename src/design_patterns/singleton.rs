//! # Singleton Pattern
//!
//! Replaces: **Singleton Pattern** (OOP)
//!
//! Real Rust usage: `std::sync::OnceLock`, `lazy_static` (crate), `tokio::sync::OnceCell`
//!
//! ## Why this pattern exists in Rust
//! In traditional OOP, Singletons are used to ensure a class has only one instance and provides a global point of access to it.
//! In Rust, the classic Singleton is often considered an anti-pattern because:
//! 1. Global mutable state violates Rust's borrowing rules (aliasing XOR mutation).
//! 2. It hides dependencies, making testing and reasoning about code difficult.
//!
//! However, when global state is strictly necessary (e.g., configuration, connection pools, or loggers),
//! Rust provides safe, concurrent primitives to manage it. The preferred alternative is Dependency Injection (passing state explicitly).
//!
//! ## Architecture
//!
//! **Approach 1: Safe Global State (`OnceLock`)**
//! ```text
//! [ static CONFIG: OnceLock<Config> ] --(get_or_init)--> [ Config Instance ]
//! ```
//!
//! **Approach 2: Dependency Injection (Preferred)**
//! ```text
//! [ AppState ] --(owns)--> [ DatabasePool ]
//!       |
//!       +--(passes &DatabasePool)--> [ Request Handler ]
//! ```
//!
//! **Invariants:**
//! - Global state must be thread-safe (`Sync`).
//! - Initialization is performed exactly once, even under concurrent access.
//! - Mutation of global state requires interior mutability (e.g., `Mutex` or `RwLock`).
//!
//! ## When to use
//! - **OnceLock:** For truly global, read-heavy, or write-once data (e.g., application config, env vars, static caches).
//! - **Dependency Injection:** For almost everything else (db connections, services) to keep code modular and testable.
//!
//! ## Anti-patterns
//! - `static mut`: Requires `unsafe` block for every access, highly discouraged due to data race risks.
//!
//! ## Footer
//! The GoF Singleton relies on a private constructor and a static `getInstance()` method. Rust replaces this with module-level `static` variables wrapped in safe synchronization primitives, or ideally, by passing state as parameters (often via traits).

use std::sync::{Mutex, OnceLock};

// ============================================================================
// Approach 1: Safe Global State (OnceLock)
// ============================================================================

/// A global configuration struct.
#[derive(Debug, PartialEq)]
pub struct GlobalConfig {
    pub port: u16,
    pub database_url: String,
}

// OWNERSHIP INSIGHT: `static` items have the `'static` lifetime and live for the entire duration of the program.
// COMPILE-TIME WIN: `OnceLock` forces us to initialize safely and handles concurrent initialization races.
static CONFIG: OnceLock<GlobalConfig> = OnceLock::new();

/// Access the global configuration, initializing it if necessary.
#[must_use]
pub fn get_config() -> &'static GlobalConfig {
    CONFIG.get_or_init(|| {
        // PRODUCTION NOTE: In a real app, this might read from the environment or a file.
        GlobalConfig {
            port: 8080,
            database_url: String::from("postgres://localhost/db"),
        }
    })
}

// ============================================================================
// Approach 1.5: Mutable Global State (OnceLock + Mutex)
// ============================================================================

// TRADEOFF: If the global state must be mutated, we must wrap it in a synchronization primitive like `Mutex` or `RwLock`.
// This introduces potential lock contention and deadlocks.
static MUTABLE_STATE: OnceLock<Mutex<u32>> = OnceLock::new();

/// Increment and return a global counter.
///
/// # Panics
/// Panics if the mutex is poisoned.
#[must_use]
pub fn increment_global_counter() -> u32 {
    let mutex = MUTABLE_STATE.get_or_init(|| Mutex::new(0));
    // GOTCHA: Forgetting to handle the `PoisonError` from a Mutex. In production, decide whether to unwrap (panic on poison) or recover.
    let mut count = mutex.lock().expect("Mutex should not be poisoned");
    *count += 1;
    *count
}

// ============================================================================
// Approach 2: Dependency Injection (The Idiomatic Rust "Singleton")
// ============================================================================

// ANTI-PATTERN: Making the database pool a global static variable.
// Instead, encapsulate it in an application state struct and pass it down.

/// Application state, created once at startup and passed to handlers.
#[derive(Clone)]
pub struct AppState {
    // In real code, this would be a real connection pool (e.g., `sqlx::PgPool`).
    // It's conceptually a singleton for the application's lifecycle, but it's not global.
    pub db_pool: String,
}

impl AppState {
    #[must_use]
    pub fn new(db_url: &str) -> Self {
        Self {
            db_pool: format!("Connected to {db_url}"),
        }
    }
}

/// A handler that takes the required state explicitly.
#[must_use]
pub fn handle_request(state: &AppState) -> String {
    format!("Handling request using: {}", state.db_pool)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_global_config() {
        let config1 = get_config();
        let config2 = get_config();

        // COMPILE-TIME WIN: Both point to the exact same memory location.
        assert_eq!(config1, config2);
        assert_eq!(config1.port, 8080);

        // Ensure pointer equality
        assert!(std::ptr::eq(config1, config2));
    }

    #[test]
    fn test_mutable_global_state() {
        let handles: Vec<_> = (0..10)
            .map(|_| thread::spawn(increment_global_counter))
            .collect();

        for handle in handles {
            let _ = handle.join();
        }

        let final_count = *MUTABLE_STATE
            .get()
            .unwrap()
            .lock()
            .expect("Mutex should not be poisoned");
        // Counter might have been incremented by other tests if run in same process,
        // but it should be at least 10.
        assert!(final_count >= 10);
    }

    #[test]
    fn test_dependency_injection() {
        let state = AppState::new("sqlite://memory");
        let response = handle_request(&state);

        assert_eq!(
            response,
            "Handling request using: Connected to sqlite://memory"
        );
    }
}
