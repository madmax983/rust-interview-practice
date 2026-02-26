//! # Trait-Based Dependency Injection
//!
//! Replaces: **Dependency Injection Container** (Spring, Guice), **Mocking Frameworks** (Mockito)
//!
//! Real Rust usage: `tower::Service`, `sqlx::Executor`, `embedded-hal` drivers
//!
//! ## Why this pattern exists in Rust
//! In languages like Java or C#, dependency injection often relies on reflection and runtime configuration (DI Containers).
//! Rust's strict type system and lack of runtime reflection make compile-time injection via Generics or Trait Objects preferred.
//! This approach has zero runtime overhead (monomorphization) and catches wiring errors at compile time.
//!
//! ## Architecture
//!
//! ```text
//! [ UserParams ] --(use)--> [ UserService<R> ] --(owns)--> [ R: UserRepository ]
//!                                                                    ^
//!                                                                    |
//!                                           +------------------------+--------------------------+
//!                                           |                                                   |
//!                                  [ PostgresRepository ]                             [ InMemoryRepository ]
//!                                    (Production)                                        (Testing)
//! ```
//!
//! **Invariants:**
//! - The `UserService` does not know the concrete type of its repository, only that it implements `UserRepository`.
//! - Dependencies are provided at creation time (Constructor Injection).
//! - Ownership of the dependency is typically passed to the consumer (`UserService` owns `R`).
//!
//! ## When to use
//! - When you need to swap implementations for testing (mocking).
//! - When supporting multiple backends (e.g., Postgres, Redis, InMemory).
//! - To decouple high-level logic from low-level details (Ports and Adapters / Hexagonal Architecture).

use std::collections::HashMap;

// ============================================================================
// The Port (Trait Definition)
// ============================================================================

// OWNERSHIP INSIGHT: Traits define behavior. By accepting `&self` or `&mut self`,
// we dictate whether the implementation needs exclusive access to its state.
pub trait UserRepository {
    fn find_user(&self, id: u32) -> Option<String>;
    fn save_user(&mut self, id: u32, name: String) -> Result<(), String>;
}

// ============================================================================
// The Adapter (Concrete Implementation 1: In-Memory / Mock)
// ============================================================================

/// A thread-safe in-memory repository, useful for testing.
#[derive(Default)]
pub struct InMemoryRepository {
    users: HashMap<u32, String>,
}

impl InMemoryRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

impl UserRepository for InMemoryRepository {
    fn find_user(&self, id: u32) -> Option<String> {
        self.users.get(&id).cloned()
    }

    fn save_user(&mut self, id: u32, name: String) -> Result<(), String> {
        self.users.insert(id, name);
        Ok(())
    }
}

// ============================================================================
// The Adapter (Concrete Implementation 2: "Production" / File)
// ============================================================================

/// A stub for a file-based or database repository.
pub struct FileRepository {
    path: String,
}

impl FileRepository {
    pub fn new(path: impl Into<String>) -> Self {
        FileRepository { path: path.into() }
    }
}

impl UserRepository for FileRepository {
    fn find_user(&self, id: u32) -> Option<String> {
        // In a real app, we'd read from `self.path`
        println!("Reading user {} from file {}", id, self.path);
        None // Stub implementation
    }

    fn save_user(&mut self, id: u32, name: String) -> Result<(), String> {
        // In a real app, we'd write to `self.path`
        println!("Writing user {}='{}' to file {}", id, name, self.path);
        Ok(())
    }
}

// ============================================================================
// The Domain Service (Consumer)
// ============================================================================

// COMPILE-TIME WIN: We use generics `R: UserRepository` instead of `Box<dyn UserRepository>`.
// This allows the compiler to monomorphize (generate a specialized version of)
// `UserService` for each repository type, enabling inlining and static dispatch.
pub struct UserService<R: UserRepository> {
    repository: R,
}

impl<R: UserRepository> UserService<R> {
    // OWNERSHIP INSIGHT: We take ownership of the repository (`R`).
    // The service now owns its dependency and controls its lifetime.
    pub fn new(repository: R) -> Self {
        UserService { repository }
    }

    pub fn register(&mut self, id: u32, name: String) -> Result<String, String> {
        if self.repository.find_user(id).is_some() {
            return Err("User already exists".to_string());
        }

        self.repository.save_user(id, name.clone())?;
        Ok(format!("Welcome, {}!", name))
    }

    pub fn get_profile(&self, id: u32) -> String {
        match self.repository.find_user(id) {
            Some(name) => format!("Profile: {}", name),
            None => "User not found".to_string(),
        }
    }
}

// ============================================================================
// Dynamic Dispatch Variant (Optional)
// ============================================================================

// Sometimes we *do* want dynamic dispatch (e.g., storing heterogeneous services).
// We can use `Box<dyn UserRepository>` or `Arc<dyn UserRepository>`.
pub struct DynamicUserService {
    // TRADEOFF: Indirect calls (vtable) and allocation overhead, but faster compilation
    // and flexibility to swap implementations at runtime.
    repository: Box<dyn UserRepository + Send + Sync>,
}

impl DynamicUserService {
    pub fn new(repository: Box<dyn UserRepository + Send + Sync>) -> Self {
        DynamicUserService { repository }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_service_with_memory_repo() {
        // 1. Setup the dependency
        let repo = InMemoryRepository::new();

        // 2. Inject into service
        let mut service = UserService::new(repo);

        // 3. Test business logic
        assert!(service.register(1, "Alice".to_string()).is_ok());
        assert_eq!(service.get_profile(1), "Profile: Alice");

        // 4. Verify state (via public API or inspection if possible)
        assert!(service.register(1, "Alice".to_string()).is_err());
    }

    #[test]
    fn test_user_service_with_file_repo() {
        // COMPILE-TIME WIN: The compiler generates a *different* `UserService` struct layout
        // for `FileRepository` than for `InMemoryRepository`.
        let repo = FileRepository::new("/tmp/users.db");
        let mut service = UserService::new(repo);

        // Since it's a stub, it returns None
        assert_eq!(service.get_profile(1), "User not found");
        // And save works (prints to stdout)
        assert!(service.register(1, "Bob".to_string()).is_ok());
    }
}
