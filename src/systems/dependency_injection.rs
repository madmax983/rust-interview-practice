//! # Dependency Injection (DI) Container
//!
//! An inversion of control (IoC) container from scratch.
//!
//! **Replaces Crates:** `shaku`, `symphony`, `teloc`
//!
//! **Real-world Usage:**
//! - Application bootstrapping to decouple components (e.g., swapping a Postgres repo for an In-Memory repo during tests).
//! - Web frameworks (like Actix or Axum) managing application state and injecting services into handlers.
//! - Complex system architectures where manual wiring of deep dependency graphs becomes unmaintainable.
//!
//! **Why build it yourself?**
//! Rust's strict typing and ownership rules make runtime Dependency Injection challenging.
//! Building a DI container teaches you how to use `std::any::Any` for type erasure,
//! how to manage dynamic lifetimes with `Arc`, and the differences between Singleton and Transient lifecycles in a concurrent environment.

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure:
//
// We use a `HashMap<TypeId, Provider>` where `TypeId` is a unique compiler-generated identifier
// for a given type (usually a Trait Object or Struct), and `Provider` is an enum describing
// how to resolve instances of that type.
//
// Lifecycles:
// 1. `Singleton`: A single instance created once and shared everywhere via `Arc<T>`.
// 2. `Transient`: A new instance is created via a factory closure every time it's requested.
//
// Flow:
//   Container.register::<dyn Logger>( Singleton(Arc::new(ConsoleLogger::new())) )
//                      │
//                      ▼
//   Container.resolve::<dyn Logger>() -> Some(Arc<dyn Logger>)
//
// Invariants:
// 1. Thread-safety: The container itself and all registered services must be `Send + Sync`
//    to be shared across threads.
// 2. Downcasting safety: Since we store values as `Arc<dyn Any + Send + Sync>`, we rely on
//    the `TypeId` key to safely downcast back to the requested type.
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ register      │ O(1)        │ O(1)        │
// │ resolve       │ O(1)        │ O(1)        │
// └───────────────┴─────────────┴─────────────┘
//
// Design Decisions:
// - **Type Erasure (`Any`)**: Rust is statically typed. To store heterogeneous types in a `HashMap`,
//   we must erase their specific types using `Box<dyn Any>` or `Arc<dyn Any>`.
// - **Thread Safety (`Send + Sync`)**: We enforce these bounds because DI containers are often
//   placed in an `Arc` and shared across threads in web servers.
// - **Arc vs Box**: We return `Arc<T>` because Singletons are shared. Using `Arc` simplifies
//   the API (both Transient and Singleton return the same pointer type).

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;

/// A trait defining the interface for a Dependency Injection Container.
/// This allows swapping out the underlying implementation (e.g., for a concurrent or thread-local variant).
pub trait DIContainer {
    /// Registers a Singleton service.
    fn register_singleton<T: Any + Send + Sync>(&mut self, instance: T);

    /// Registers a Transient service using a factory closure.
    fn register_transient<T, F>(&mut self, factory: F)
    where
        T: Any + Send + Sync,
        F: Fn() -> T + Send + Sync + 'static;

    /// Resolves a service by its type `T`.
    fn resolve<T: Any + Send + Sync>(&self) -> Option<Arc<T>>;
}

/// Defines the lifecycle of a registered service.
pub enum Lifecycle {
    /// A single shared instance created upfront.
    Singleton(Arc<dyn Any + Send + Sync>),
    /// A factory closure that creates a new instance on every resolution.
    Transient(Box<dyn Fn() -> Arc<dyn Any + Send + Sync> + Send + Sync>),
}

/// The default Dependency Injection Container implementation.
#[derive(Default)]
pub struct Container {
    services: HashMap<TypeId, Lifecycle>,
}

impl Container {
    /// Creates a new, empty DI Container.
    #[must_use]
    pub fn new() -> Self {
        Self {
            services: HashMap::new(),
        }
    }
}

impl DIContainer for Container {
    /// Registers a Singleton service.
    ///
    /// # RUST INSIGHT: TypeId and 'static
    /// `TypeId::of::<T>()` requires `T` to be `'static`. This ensures we don't store types
    /// that contain references with shorter lifetimes, preventing dangling pointers.
    fn register_singleton<T: Any + Send + Sync>(&mut self, instance: T) {
        let type_id = TypeId::of::<T>();
        // GOTCHA: We must wrap the instance in an Arc, and then cast it to Arc<dyn Any>.
        self.services
            .insert(type_id, Lifecycle::Singleton(Arc::new(instance)));
    }

    /// Registers a Transient service using a factory closure.
    fn register_transient<T, F>(&mut self, factory: F)
    where
        T: Any + Send + Sync,
        F: Fn() -> T + Send + Sync + 'static,
    {
        let type_id = TypeId::of::<T>();
        let wrapped_factory = Box::new(move || {
            let instance: T = factory();
            Arc::new(instance) as Arc<dyn Any + Send + Sync>
        });

        self.services
            .insert(type_id, Lifecycle::Transient(wrapped_factory));
    }

    /// Resolves a service by its type `T`.
    /// Returns `Some(Arc<T>)` if the service is registered, otherwise `None`.
    fn resolve<T: Any + Send + Sync>(&self) -> Option<Arc<T>> {
        let type_id = TypeId::of::<T>();

        let resolved_any = match self.services.get(&type_id)? {
            Lifecycle::Singleton(instance) => {
                // Return a clone of the Arc (cheap reference count increment)
                Arc::clone(instance)
            }
            Lifecycle::Transient(factory) => {
                // Invoke the factory closure to create a new instance
                factory()
            }
        };

        // PRODUCTION NOTE: Safe Downcasting
        // We know the underlying type inside the `Any` matches `T` because we keyed it by `TypeId::of::<T>()`.
        // However, instead of panicking with `.unwrap()`, we gracefully return `.ok()`.
        resolved_any.downcast::<T>().ok()
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `shaku`: Provides macro-based compile-time dependency injection. Our approach is purely
//   runtime-based, meaning missing dependencies cause runtime errors (or `None` returns)
//   rather than compiler errors.
// - `symphony`: Similar runtime DI, but with more advanced features like named dependencies.
//
// What's missing vs. production:
// - **Auto-wiring**: Production crates often use macros (`#[derive(Component)]`) to automatically
//   resolve dependencies inside constructors (e.g., `fn new(db: Arc<Database>) -> Self`).
// - **Interfaces (Traits)**: In Rust, `TypeId::of::<dyn Trait>()` is distinct from `TypeId::of::<Struct>()`.
//   Our simple container registers the concrete struct. To register traits, a more complex approach
//   involving `Box<dyn Trait>` inside `Any` is required, along with macros to generate casting boilerplates.
// - **Cycle Detection**: If two Transient factories depend on each other, they will cause a stack overflow.
//
// Benchmarking Note:
// To measure the overhead of `Any` downcasting versus static dispatch, use `criterion` or `std::hint::black_box`.
// E.g., `let start = Instant::now(); black_box(container.resolve::<MyService>());`
// Compare this against calling a direct constructor `black_box(MyService::new())`.
// The difference highlights the runtime cost (hash map lookup + downcast) of using DI.
//
// Next steps:
// 1. Support interface resolution (registering a struct as a `dyn Trait`).
// 2. Add named resolutions to allow multiple instances of the same type.

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;

    #[derive(Debug, PartialEq, Eq)]
    struct MockService {
        value: i32,
    }

    #[test]
    fn test_register_and_resolve_singleton() {
        let mut container = Container::new();
        let service = MockService { value: 42 };

        container.register_singleton(service);

        let resolved = container.resolve::<MockService>().unwrap();
        assert_eq!(resolved.value, 42);

        // Ensure it's the exact same instance
        let resolved2 = container.resolve::<MockService>().unwrap();
        assert!(Arc::ptr_eq(&resolved, &resolved2));
    }

    #[test]
    fn test_register_and_resolve_transient() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);

        let mut container = Container::new();

        container.register_transient(move || {
            let count = counter_clone.fetch_add(1, Ordering::SeqCst);
            MockService {
                value: count as i32,
            }
        });

        let resolved_1 = container.resolve::<MockService>().unwrap();
        let resolved_2 = container.resolve::<MockService>().unwrap();

        assert_eq!(resolved_1.value, 0);
        assert_eq!(resolved_2.value, 1);

        // They should be different instances
        assert!(!Arc::ptr_eq(&resolved_1, &resolved_2));
    }

    #[test]
    fn test_resolve_missing_dependency() {
        let container = Container::new();
        let result = container.resolve::<MockService>();
        assert!(result.is_none());
    }

    #[test]
    fn test_thread_safety_bounds() {
        // Since `Container` and `MockService` implements `Send` + `Sync`, this should compile.
        let mut container = Container::new();
        container.register_singleton(MockService { value: 99 });
        let container = Arc::new(container);

        let mut handles = vec![];
        for _ in 0..10 {
            let container_clone = Arc::clone(&container);
            handles.push(thread::spawn(move || {
                let resolved = container_clone.resolve::<MockService>().unwrap();
                assert_eq!(resolved.value, 99);
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }
}
