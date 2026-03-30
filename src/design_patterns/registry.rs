// Suppress pedantic and nursery lints for design pattern examples.
#![allow(clippy::pedantic, clippy::nursery, unused)]

//! # Registry Pattern (Type-Map based Registry)
//!
//! Replaces: **Singleton Pattern** (OOP), **Global Service Locators** (OOP), **Dependency Injection Frameworks** (Java/C#)
//!
//! Real Rust usage: `http::Extensions`, `actix_web::web::Data`, `bevy::ecs::world::World`
//!
//! ## Why this pattern exists in Rust
//! In OOP, Singletons and Global Service Locators are common ways to share "one instance" of a service
//! across an application. In Rust, mutable global state is heavily discouraged and requires `unsafe` or
//! complex locking (`static Lazy<Mutex<T>>`).
//!
//! Instead, Rust applications often pass a "Context" or "Registry" object down the call stack.
//! The most powerful version of this is the Type-Map Registry: a dynamically-typed, memory-safe
//! collection keyed by the Rust `TypeId` of the service itself. This eliminates the need for string
//! keys or massive `enum`s, providing O(1) type-safe access.
//!
//! ## Architecture
//!
//! ```text
//! [ Registry ]
//!    |
//!    +-- TypeId::of::<Database>()  -> Box<Any (Database)>
//!    +-- TypeId::of::<Logger>()    -> Box<Any (Logger)>
//! ```
//!
//! **Invariants:**
//! - A registry can only hold one instance of any specific type.
//! - Retrieval guarantees the returned type perfectly matches the requested type (via safe downcasting).
//! - All services in a concurrent registry must be `Send + Sync + 'static`.
//!
//! ## When to use
//! - **Web Frameworks:** To pass shared application state (e.g., db pools, configs) to request handlers.
//! - **Game Engines (ECS):** To store global "Resources" (e.g., Time, Input state).
//! - **Plugin Systems:** To allow plugins to inject their own state into a shared context.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;

// ============================================================================
// The Pattern
// ============================================================================

/// A type-safe registry that stores a single instance of any type.
///
/// **OWNERSHIP INSIGHT:**
/// We store `Arc<dyn Any + Send + Sync>` so that services can be safely
/// shared across multiple threads without transferring ownership or copying.
#[derive(Default, Clone)]
pub struct ServiceRegistry {
    // COMPILE-TIME WIN: By using `TypeId` as the key, we prevent typo-based bugs
    // common in string-keyed Service Locators (e.g. `get("databaes")`).
    services: HashMap<TypeId, Arc<dyn Any + Send + Sync>>,
}

impl ServiceRegistry {
    pub fn new() -> Self {
        Self {
            services: HashMap::new(),
        }
    }

    /// Registers a new service. If a service of this type already exists, it is overwritten.
    ///
    /// The generic parameter `<T>` is constrained to be thread-safe (`Send + Sync`)
    /// and must not contain non-static references (`'static`).
    pub fn register<T: Send + Sync + 'static>(&mut self, service: T) {
        self.services.insert(TypeId::of::<T>(), Arc::new(service));
    }

    /// Retrieves an `Arc` pointer to the requested service, if it exists.
    pub fn get<T: Send + Sync + 'static>(&self) -> Option<Arc<T>> {
        self.services
            .get(&TypeId::of::<T>())
            // TRADEOFF: We must perform a runtime downcast. However, because we control
            // the `insert` logic, we know the downcast will always succeed if the key exists.
            .and_then(|any_service| any_service.clone().downcast::<T>().ok())
    }
}

// ============================================================================
// Example Services
// ============================================================================

pub struct DatabasePool {
    url: String,
}

impl DatabasePool {
    pub fn new(url: &str) -> Self {
        Self {
            url: url.to_string(),
        }
    }

    pub fn execute_query(&self, query: &str) -> String {
        format!("Executing '{query}' on {}", self.url)
    }
}

pub struct Config {
    pub max_connections: u32,
    pub timeout_seconds: u64,
}

// ============================================================================
// Anti-Pattern: String-Keyed Map or Enums
// ============================================================================

// ANTI-PATTERN:
// struct BadRegistry {
//     services: HashMap<String, Box<dyn ServiceTrait>>,
// }
//
// Why this is worse in Rust:
// 1. Requires all services to implement a common trait (`ServiceTrait`), which limits flexibility.
// 2. You have to cast or parse the output back to the specific type you need.
// 3. String keys are prone to typos ("db_pool" vs "dbpool").

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_insert_and_get() {
        let mut registry = ServiceRegistry::new();

        // Register distinct types
        registry.register(DatabasePool::new("postgres://localhost"));
        registry.register(Config {
            max_connections: 100,
            timeout_seconds: 30,
        });

        // Retrieve the DB pool
        // COMPILE-TIME WIN: We don't specify the key string, just the type we want.
        let db = registry.get::<DatabasePool>().expect("Database missing");
        assert_eq!(db.url, "postgres://localhost");

        // Retrieve the Config
        let config = registry.get::<Config>().expect("Config missing");
        assert_eq!(config.max_connections, 100);
    }

    #[test]
    fn test_registry_missing_service() {
        let registry = ServiceRegistry::new();

        // Trying to get a type that wasn't registered returns None
        let db = registry.get::<DatabasePool>();
        assert!(db.is_none());
    }

    #[test]
    fn test_registry_overwrite() {
        let mut registry = ServiceRegistry::new();

        registry.register(Config {
            max_connections: 50,
            timeout_seconds: 10,
        });

        // Overwrite the existing Config
        registry.register(Config {
            max_connections: 999,
            timeout_seconds: 60,
        });

        let config = registry.get::<Config>().unwrap();
        assert_eq!(config.max_connections, 999);
    }
}

// ============================================================================
// Footer
// ============================================================================
//
// How this pattern appears in std/crates:
// - `http::Extensions`: Allows attaching arbitrary typed data to HTTP requests.
// - `actix_web::web::Data`: The standard way to inject state into route handlers.
// - `bevy::ecs`: Uses this pattern heavily to store unique `Resource`s.
//
// What the GoF/OOP equivalent is and why it doesn't translate directly:
// In OOP, Dependency Injection frameworks use Reflection to scan classes and auto-wire
// constructors based on parameter types. Rust's lack of runtime reflection makes this
// impossible. Instead, we explicitly register and retrieve dependencies via the TypeId registry.
//
// When to reach for this vs. simpler alternatives:
// For small applications, passing a simple `struct AppContext { db: Database, conf: Config }`
// is much better (static typing, no runtime overhead). Reach for the Registry pattern when
// you are building a framework, plugin system, or middleware pipeline where the set of
// required types is not known at compile time.
