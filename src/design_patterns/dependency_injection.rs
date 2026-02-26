//! # Trait-Based Dependency Injection
//!
//! Replaces: **DI Containers** (Spring, Guice), **Service Locator Pattern**
//!
//! Real Rust usage: `tower::Service` (HTTP middleware), `axum::extract::State`, `reqwest::Client` (Generic Connectors)
//!
//! ## Why this pattern exists in Rust
//! In languages like Java or C#, dependency injection often relies on runtime reflection and heavy containers
//! to wire components together. Rust's strict type system and lack of reflection make this approach difficult.
//! Instead, Rust uses **Generics** and **Traits** to inject dependencies at compile time.
//! This results in:
//! 1.  **Zero Runtime Overhead:** No container lookup, no virtual method dispatch (unless explicitly using `Box<dyn Trait>`).
//! 2.  **Monomorphization:** The compiler generates specialized code for each concrete type, enabling aggressive inlining.
//! 3.  **Compile-Time Safety:** Missing dependencies or type mismatches are caught by the compiler, not at startup.
//!
//! ## Architecture
//!
//! ```text
//!       +------------+
//!       | BlobService| <--- Depends on S: Storage
//!       +-----+------+
//!             |
//!             v
//!      +------+------+
//!      | trait Storage|
//!      +------+------+
//!             ^
//!             |
//!      +------+------+         +-------------+
//!      | FileStorage |         | MemoryStorage|
//!      +-------------+         +--------------+
//! ```
//!
//! **Invariants:**
//! - `BlobService` does not know the concrete type of `S`.
//! - `S` must satisfy the `Storage` trait bounds.
//! - The decision of which `Storage` to use is made at the composition root (e.g., `main`), not inside the service.
//!
//! ## When to use
//! - **Libraries:** To allow users to plug in their own implementations (e.g., a database driver, a logger).
//! - **Applications:** To decouple business logic from infrastructure (e.g., swapping a real DB for an in-memory mock during tests).
//! - **Performance:** When virtual dispatch (`dyn Trait`) is too slow (though rarely the bottleneck).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::io;

// ============================================================================
// The Abstract Dependency (The Port)
// ============================================================================

/// Defines the contract for storage.
/// Traits are the "Interface" in Rust's DI.
pub trait Storage {
    fn put(&self, key: &str, value: &[u8]) -> io::Result<()>;
    fn get(&self, key: &str) -> io::Result<Option<Vec<u8>>>;
}

// ============================================================================
// Concrete Implementation 1: In-Memory (The Adapter for Testing)
// ============================================================================

/// A storage implementation that keeps data in memory.
/// Perfect for unit tests or fast, ephemeral storage.
#[derive(Default, Clone)] // OWNERSHIP INSIGHT: Clone is cheap here because of Arc
pub struct InMemoryStorage {
    // Interior Mutability pattern allows us to mutate state through shared reference (&self)
    data: Arc<Mutex<HashMap<String, Vec<u8>>>>,
}

impl InMemoryStorage {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Storage for InMemoryStorage {
    fn put(&self, key: &str, value: &[u8]) -> io::Result<()> {
        let mut data = self.data.lock().unwrap();
        data.insert(key.to_string(), value.to_vec());
        Ok(())
    }

    fn get(&self, key: &str) -> io::Result<Option<Vec<u8>>> {
        let data = self.data.lock().unwrap();
        Ok(data.get(key).cloned())
    }
}

// ============================================================================
// Concrete Implementation 2: File System (The Adapter for Production)
// ============================================================================

/// A storage implementation that persists data to disk.
pub struct FileStorage {
    base_path: std::path::PathBuf,
}

impl FileStorage {
    pub fn new(base_path: impl Into<std::path::PathBuf>) -> Self {
        let path = base_path.into();
        // Ensure directory exists
        if let Err(e) = std::fs::create_dir_all(&path) {
            eprintln!("Warning: Failed to create storage dir: {}", e);
        }
        Self { base_path: path }
    }
}

impl Storage for FileStorage {
    fn put(&self, key: &str, value: &[u8]) -> io::Result<()> {
        // TRADEOFF: Simple implementation, no atomicity guarantees here.
        let path = self.base_path.join(key);
        std::fs::write(path, value)
    }

    fn get(&self, key: &str) -> io::Result<Option<Vec<u8>>> {
        let path = self.base_path.join(key);
        match std::fs::read(path) {
            Ok(data) => Ok(Some(data)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }
}

// ============================================================================
// The Consumer (The Service)
// ============================================================================

/// A high-level service that depends on a Storage implementation.
///
/// COMPILE-TIME WIN:
/// This struct is generic over `S`. When used, the compiler generates a specialized
/// version of `BlobService` for the specific `Storage` type used (Monomorphization).
///
/// If we used `Box<dyn Storage>`, we'd have a single struct but pay a small runtime
/// cost for vtable lookups.
pub struct BlobService<S: Storage> {
    storage: S,
}

impl<S: Storage> BlobService<S> {
    // Dependency Injection happens here: we accept ANY type that implements Storage.
    pub fn new(storage: S) -> Self {
        Self { storage }
    }

    pub fn store_profile_picture(&self, user_id: &str, data: &[u8]) -> io::Result<()> {
        let key = format!("user_{}_avatar", user_id);
        // Validations or processing logic would go here
        if data.len() > 1024 * 1024 * 5 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Image too large"));
        }
        self.storage.put(&key, data)
    }

    pub fn get_profile_picture(&self, user_id: &str) -> io::Result<Option<Vec<u8>>> {
        let key = format!("user_{}_avatar", user_id);
        self.storage.get(&key)
    }
}

// ============================================================================
// Footer
// ============================================================================
//
// Production Note:
// In very large applications, excessive generics can lead to:
// 1.  **Code Bloat:** Binary size increases due to monomorphization.
// 2.  **Compile Times:** Slower builds as the compiler generates unique code for each combination.
//
// If this becomes a problem, the standard Rust pattern is to use **Type Erasure**
// at the boundary:
//
// ```rust
// pub struct DynamicBlobService {
//     storage: Box<dyn Storage + Send + Sync>,
// }
// ```
//
// Use generics by default (Zero Cost Abstraction), and switch to `dyn Trait` only if
// you hit specific limits or need runtime polymorphism (e.g., a plugin system).

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blob_service_with_memory() {
        // Setup: Inject the In-Memory implementation
        let storage = InMemoryStorage::new();
        let service = BlobService::new(storage); // Monomorphized as BlobService<InMemoryStorage>

        // Execution
        let user_id = "123";
        let image_data = vec![1, 2, 3, 4];
        service.store_profile_picture(user_id, &image_data).unwrap();

        // Verify
        let retrieved = service.get_profile_picture(user_id).unwrap();
        assert_eq!(retrieved, Some(image_data));
    }

    #[test]
    fn test_blob_service_with_file_system() {
        // Setup: Inject the File System implementation (using a temp dir)
        let temp_dir = std::env::temp_dir().join("rust_di_pattern_test");
        let _ = std::fs::remove_dir_all(&temp_dir); // Cleanup previous run

        let storage = FileStorage::new(&temp_dir);
        let service = BlobService::new(storage); // Monomorphized as BlobService<FileStorage>

        // Execution
        let user_id = "456";
        let image_data = vec![5, 6, 7, 8];
        service.store_profile_picture(user_id, &image_data).unwrap();

        // Verify
        let retrieved = service.get_profile_picture(user_id).unwrap();
        assert_eq!(retrieved, Some(image_data));

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    // This test ensures that we can write tests that DON'T depend on the filesystem,
    // solving the "Global State" / "Side Effect" problem of direct File I/O.
    #[test]
    fn test_mocking_strategy() {
        // We can create a specialized Mock if we want to test error handling specifically
        struct BrokenStorage;
        impl Storage for BrokenStorage {
            fn put(&self, _key: &str, _value: &[u8]) -> io::Result<()> {
                Err(io::Error::new(io::ErrorKind::Other, "Disk full"))
            }
            fn get(&self, _key: &str) -> io::Result<Option<Vec<u8>>> {
                Ok(None)
            }
        }

        let service = BlobService::new(BrokenStorage);
        let result = service.store_profile_picture("user", &[0; 10]);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Disk full");
    }
}
