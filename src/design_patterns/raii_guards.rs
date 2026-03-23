// Suppress pedantic and nursery lints for design pattern examples.
#![allow(clippy::pedantic, clippy::nursery, unused)]
//! # RAII Guards (Scope-based Resource Management)
//!
//! Replaces: **try/finally blocks, Context Managers** (Python `with`), **IDisposable** (C#)
//!
//! Real Rust usage: `std::sync::MutexGuard`, `std::fs::File`, `tempfile::TempDir`, `scopeguard` crate
//!
//! ## Why this pattern exists in Rust
//! Rust has deterministic destruction. When a value goes out of scope, its `Drop` implementation is called *immediately* and *guaranteed*.
//! This is the cornerstone of resource safety (memory, file handles, locks) without garbage collection pauses.
//!
//! ## Architecture
//!
//! ```text
//! {
//!     let guard = acquire_resource(); // Acquire
//!     // ... use resource ...
//! } // guard.drop() called automatically // Release
//! ```
//!
//! **Invariants:**
//! - Resources are released exactly when the guard is dropped.
//! - Guards prevent use-after-free and double-free errors.
//! - Guards often implement `Deref` to provide access to the protected resource.
//!
//! ## When to use
//! - Managing system resources (files, sockets, locks).
//! - Ensuring cleanup actions (deleting temp files, rolling back transactions) even during panics.
//! - Creating critical sections.

use std::fs;
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};

// ============================================================================
// Generic Scope Guard (Defer Pattern)
// ============================================================================

/// A guard that executes a closure when it goes out of scope.
///
/// **OWNERSHIP INSIGHT:** This struct owns `T` and the closure `F`.
/// When `ScopeGuard` is dropped, `F` runs with `T` as input.
pub struct ScopeGuard<T, F>
where
    F: FnOnce(T),
{
    // Wrap in Option to extract it during Drop
    data: Option<T>,
    callback: Option<F>,
}

impl<T, F> ScopeGuard<T, F>
where
    F: FnOnce(T),
{
    pub fn new(data: T, callback: F) -> Self {
        ScopeGuard {
            data: Some(data),
            callback: Some(callback),
        }
    }

    /// Consumes the guard without running the callback (defuse).
    pub fn into_inner(mut self) -> T {
        // Take the data out, preventing Drop logic from running on it effectively
        // Actually, we need to prevent `drop` from running the callback.
        let data = self.data.take().unwrap();
        self.callback = None; // Disable callback
        data
    }
}

impl<T, F> Drop for ScopeGuard<T, F>
where
    F: FnOnce(T),
{
    fn drop(&mut self) {
        // Run the callback only if we still have the callback and data
        if let (Some(data), Some(callback)) = (self.data.take(), self.callback.take()) {
            callback(data);
        }
    }
}

// Deref allows access to the inner data while the guard is alive
impl<T, F> Deref for ScopeGuard<T, F>
where
    F: FnOnce(T),
{
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.data.as_ref().unwrap()
    }
}

impl<T, F> DerefMut for ScopeGuard<T, F>
where
    F: FnOnce(T),
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data.as_mut().unwrap()
    }
}

// ============================================================================
// Concrete Example: Temporary Directory
// ============================================================================

/// A temporary directory that is deleted when it goes out of scope.
pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    pub fn new(prefix: &str) -> std::io::Result<Self> {
        // Simplified implementation: create a dir in current dir
        let mut path = std::env::temp_dir();
        path.push(format!("{}_{}", prefix, std::process::id()));
        fs::create_dir_all(&path)?;
        Ok(TempDir { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

// TRADEOFF: Implementing Drop ensures cleanup, but if cleanup fails (e.g. permission error),
// we can't easily return a Result. Usually we log the error and proceed.
// Panicking in Drop is an anti-pattern (double panic = abort).
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
        // Silently ignore errors in Drop, or log them.
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn test_scope_guard_cleanup() {
        let cleaned_up = Arc::new(AtomicBool::new(false));
        let flag = cleaned_up.clone();

        {
            let _guard = ScopeGuard::new((), move |_| {
                flag.store(true, Ordering::SeqCst);
            });
            assert!(!cleaned_up.load(Ordering::SeqCst));
        } // _guard dropped here

        assert!(cleaned_up.load(Ordering::SeqCst));
    }

    #[test]
    fn test_scope_guard_defuse() {
        let cleaned_up = Arc::new(AtomicBool::new(false));
        let flag = cleaned_up.clone();

        {
            let guard = ScopeGuard::new((), move |_| {
                flag.store(true, Ordering::SeqCst);
            });
            guard.into_inner(); // Defuse
        }

        assert!(!cleaned_up.load(Ordering::SeqCst));
    }

    #[test]
    fn test_temp_dir() {
        let path;
        {
            let temp = TempDir::new("rust_pattern_test").unwrap();
            path = temp.path().to_path_buf();
            assert!(path.exists());

            // Create a file inside
            fs::write(path.join("test.txt"), "hello").unwrap();
        } // temp dropped, dir deleted

        assert!(!path.exists());
    }
}
