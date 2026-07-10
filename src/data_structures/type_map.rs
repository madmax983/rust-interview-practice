//! # Type Map Implementation
//!
//! A type-safe, dynamically-typed map that stores values keyed by their Rust type.
//!
//! **Replaces Crates:** `typemap`, `anymap`, `http::Extensions`
//!
//! **Real-world Usage:**
//! - Web frameworks (Axum, Actix, Hyper) to store request extensions and application state.
//! - Dependency Injection containers to store and retrieve singletons by type.
//! - Game Engines (ECS) to attach arbitrary components to entities.
//!
//! **Why build it yourself?**
//! It demonstrates the power of `std::any::Any` and `TypeId` for dynamic typing in a statically-typed language.
//! You learn how to safely downcast `Box<dyn Any>` back to a concrete type, sidestepping the borrow checker
//! while maintaining memory safety. It shows how "extensions" actually work under the hood in web frameworks.

use std::any::{Any, TypeId};
use std::collections::HashMap;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure:
//
//      TypeId::of::<T>()
//           │
//           ▼
//      ┌──────────┐
//      │ TypeId 1 │ ──► Box<dyn Any> (e.g., Box<DatabasePool>)
//      ├──────────┤
//      │ TypeId 2 │ ──► Box<dyn Any> (e.g., Box<CurrentUser>)
//      ├──────────┤
//      │   ...    │
//      └──────────┘
//
// Invariants:
// 1. The value stored for a `TypeId::of::<T>()` is always of type `T`.
// 2. We can only store one value per type. If we need multiple, the user must wrap them in a struct or collection.
//
// Complexity:
// ┌───────────┬──────────────┬────────┐
// │ Operation │ Time         │ Space  │
// ├───────────┼──────────────┼────────┤
// │ insert    │ O(1) expected│ O(1)   │
// │ get       │ O(1) expected│ O(1)   │
// │ remove    │ O(1) expected│ O(1)   │
// └───────────┴──────────────┴────────┘
// Note: Time complexity includes HashMap lookup which is amortized O(1).
//
// Design Decisions:
// - **Storage**: We use `HashMap<TypeId, Box<dyn Any>>`.
//   - *Tradeoff*: Requires heap allocation for every inserted value (`Box`).
// - **Type Bounds**: Values must be `'static` because `Any` requires `'static`. We cannot store types with non-static references.
// - **Type Bounds**: Values must be `Send + Sync` if the TypeMap itself needs to be sent across threads (common in web servers).
//   We implement a thread-safe version `TypeMap` which requires `Send + Sync`.

/// A trait defining the capabilities of a dynamic type map.
/// Defining this as a trait allows for swappable strategies (e.g. thread-local vs thread-safe).
pub trait DynamicTypeMap {
    fn insert<T: Send + Sync + 'static>(&mut self, val: T) -> Option<T>;
    fn get<T: 'static>(&self) -> Option<&T>;
    fn get_mut<T: 'static>(&mut self) -> Option<&mut T>;
    fn remove<T: 'static>(&mut self) -> Option<T>;
    fn contains<T: 'static>(&self) -> bool;
    fn clear(&mut self);
}

/// A type-safe, dynamically-typed map.
/// This implementation requires values to be `Send + Sync` so it can be safely shared across threads.
#[derive(Default)]
pub struct TypeMap {
    // RUST INSIGHT: `dyn Any + Send + Sync` allows us to downcast safely while preserving thread-safety.
    // GOTCHA: Without `Send + Sync`, the map cannot be shared across threads or used in an async context
    // like Axum or Actix routes.
    map: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

impl TypeMap {
    /// Creates a new, empty `TypeMap`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }
}

impl DynamicTypeMap for TypeMap {
    /// Inserts a value of type `T` into the map.
    /// If a value of this type already existed, it is returned.
    fn insert<T: Send + Sync + 'static>(&mut self, val: T) -> Option<T> {
        // PRODUCTION NOTE: Canonical crates avoid hashing the `TypeId` entirely
        // by using a `NoOpHasher` or `TypeIdHasher` since `TypeId` is already a uniformly distributed 64-bit integer.
        self.map
            .insert(TypeId::of::<T>(), Box::new(val))
            .and_then(|boxed_any| {
                // RUST INSIGHT: Safely downcast the old value.
                // We know this cast will never fail because we enforce the invariant
                // that the value stored at `TypeId::of::<T>()` is always of type `T`.
                boxed_any.downcast::<T>().ok().map(|boxed| *boxed)
            })
    }

    /// Gets a reference to the value of type `T`, if it exists in the map.
    fn get<T: 'static>(&self) -> Option<&T> {
        self.map
            .get(&TypeId::of::<T>())
            .and_then(|boxed_any| boxed_any.downcast_ref::<T>())
    }

    /// Gets a mutable reference to the value of type `T`, if it exists in the map.
    fn get_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.map
            .get_mut(&TypeId::of::<T>())
            .and_then(|boxed_any| boxed_any.downcast_mut::<T>())
    }

    /// Removes a value of type `T` from the map, returning it if it existed.
    fn remove<T: 'static>(&mut self) -> Option<T> {
        self.map
            .remove(&TypeId::of::<T>())
            .and_then(|boxed_any| boxed_any.downcast::<T>().ok().map(|boxed| *boxed))
    }

    /// Checks if the map contains a value of type `T`.
    fn contains<T: 'static>(&self) -> bool {
        self.map.contains_key(&TypeId::of::<T>())
    }

    /// Clears the map, removing all values.
    fn clear(&mut self) {
        self.map.clear();
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `http::Extensions`: `http::Extensions` works exactly like this, using a `HashMap<TypeId, Box<dyn Any + Send + Sync>>`.
// - `typemap`: Provides slightly more flexibility by allowing custom traits as bounds instead of just `Any`, and supporting `Clone`.
//
// Missing vs. Production:
// - **Custom Hashers**: `TypeId` is an opaque `u64` that is already perfectly distributed (it is a hash produced by the compiler).
//   Using a `HashMap` with `DefaultHasher` re-hashes this 64-bit integer, which is unnecessary overhead.
//   Production crates like `anymap` or `http` use a custom `Hasher` (like `NoOpHasher` or `TypeIdHasher`) that simply returns the `u64` directly.
//
// Next Steps:
// 1. Implement a custom `BuildHasher` and `Hasher` that skips hashing to speed up lookups.
// 2. Add an `entry` API similar to `std::collections::HashMap::entry`.
//
// Benchmarking Note:
// To benchmark `TypeMap`, use `criterion` to compare insertions and lookups against a standard `HashMap<String, String>`.
// Compare a `TypeMap` with the default `std::collections::hash_map::DefaultHasher` vs a custom `NoOpHasher`.
// The custom hasher version should significantly outperform standard hashing since `TypeId` is an opaque `u64`.
// Be sure to use `std::hint::black_box()` to prevent the compiler from optimizing away the type cast overhead.

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq)]
    struct Config {
        port: u16,
    }

    #[derive(Debug, PartialEq)]
    struct Connection(String);

    #[test]
    fn test_insert_and_get() {
        let mut map = TypeMap::new();

        map.insert(Config { port: 8080 });
        map.insert(Connection("tcp://localhost".to_string()));

        assert_eq!(map.get::<Config>(), Some(&Config { port: 8080 }));
        assert_eq!(
            map.get::<Connection>(),
            Some(&Connection("tcp://localhost".to_string()))
        );
        assert_eq!(map.get::<String>(), None);
    }

    #[test]
    fn test_insert_overwrite() {
        let mut map = TypeMap::new();

        let old = map.insert(Config { port: 8080 });
        assert_eq!(old, None);

        let old2 = map.insert(Config { port: 9090 });
        assert_eq!(old2, Some(Config { port: 8080 }));
        assert_eq!(map.get::<Config>(), Some(&Config { port: 9090 }));
    }

    #[test]
    fn test_get_mut() {
        let mut map = TypeMap::new();
        map.insert(Config { port: 8080 });

        if let Some(config) = map.get_mut::<Config>() {
            config.port = 8081;
        }

        assert_eq!(map.get::<Config>(), Some(&Config { port: 8081 }));
    }

    #[test]
    fn test_remove() {
        let mut map = TypeMap::new();
        map.insert(Config { port: 8080 });

        let removed = map.remove::<Config>();
        assert_eq!(removed, Some(Config { port: 8080 }));
        assert_eq!(map.get::<Config>(), None);
    }

    #[test]
    fn test_contains() {
        let mut map = TypeMap::new();
        assert!(!map.contains::<Config>());

        map.insert(Config { port: 8080 });
        assert!(map.contains::<Config>());
    }

    #[test]
    fn test_clear() {
        let mut map = TypeMap::new();
        map.insert(Config { port: 8080 });
        map.insert(Connection("tcp://localhost".to_string()));

        map.clear();
        assert!(map.get::<Config>().is_none());
        assert!(map.get::<Connection>().is_none());
    }
}
