//! # Proxy Pattern
//!
//! Replaces: **Proxy Pattern** (OOP)
//!
//! Real Rust usage: `std::sync::MutexGuard`, `std::cell::Ref`, `std::borrow::Cow`, `LazyCell`, `Arc`
//!
//! ## Why this pattern exists in Rust
//! In traditional OOP, a Proxy controls access to another object (the subject), deferring its creation,
//! controlling permissions, or managing concurrent access. It often implements the same interface as the subject.
//!
//! In Rust, the Proxy pattern is deeply ingrained in the language itself via **Smart Pointers** and the `Deref` trait.
//! Rust uses proxies extensively to enforce memory safety and ownership invariants at compile time.
//!
//! ## Architecture
//!
//! **Approach 1: Smart Pointers (Deref Proxy)**
//! ```text
//! [ Proxy Struct ] --(deref)--> [ Subject ]
//! ```
//! The proxy transparently acts like the subject via `Deref`, but can execute code on `Drop` (e.g., releasing a lock).
//!
//! **Approach 2: Protection / Validation Proxy (Newtype)**
//! ```text
//! [ ValidatedEmail ] --(contains)--> [ String ]
//! ```
//! Hides the inner type completely and controls how it can be mutated, upholding domain invariants.
//!
//! **Approach 3: Lazy Initialization Proxy**
//! ```text
//! [ Lazy<T> ] --(on first access)--> initializes and caches [ T ]
//! ```
//!
//! **Invariants:**
//! - A transparent proxy (`Deref`) must not violate the expectations of the underlying type.
//! - A protection proxy must hide the inner value to prevent unauthorized mutation (encapsulation).
//!
//! ## When to use
//! - **Deref/Drop Proxies (RAII Guards):** When you need to manage a resource (locks, file handles, DB transactions).
//! - **Protection Proxies (Newtypes):** When you want to guarantee data validity (e.g., `NonZeroU32`, `ValidJson`).
//! - **Lazy Proxies:** When constructing the subject is expensive and might not be needed.
//!
//! ## Anti-patterns
//! - Implementing `Deref` for types that are *not* smart pointers just to save typing (e.g., inheriting methods). This breaks Rust's explicit composition.
//!
//! ## Footer
//! The GoF Proxy requires explicit interfaces (Traits) and manual forwarding of every method. Rust eliminates
//! this boilerplate with `Deref` for transparent proxies, or uses the Newtype pattern for protection proxies,
//! leaning heavily on the `Drop` trait for resource management (RAII).

use std::ops::{Deref, DerefMut};
use std::sync::OnceLock;

// ============================================================================
// Approach 1: RAII Guard Proxy (Deref + Drop)
// ============================================================================
// This is how `MutexGuard` or `Ref` works under the hood.

/// The subject we want to protect.
pub struct SensitiveData {
    pub value: String,
}

impl SensitiveData {
    pub fn do_work(&self) {
        // Complex operation
    }
}

/// A hypothetical lock that controls access to the data.
pub struct CustomLock {
    data: SensitiveData,
    is_locked: bool, // Simplified for demonstration
}

impl CustomLock {
    #[must_use]
    pub const fn new(data: SensitiveData) -> Self {
        Self {
            data,
            is_locked: false,
        }
    }

    /// Acquires the lock and returns a Proxy to the data.
    pub fn lock(&mut self) -> CustomLockGuard<'_> {
        self.is_locked = true;
        CustomLockGuard { lock: self }
    }
}

/// The Proxy. It holds a reference to the lock and provides access to the data.
pub struct CustomLockGuard<'a> {
    lock: &'a mut CustomLock,
}

// OWNERSHIP INSIGHT: Deref allows the Proxy to seamlessly act like the subject.
impl<'a> Deref for CustomLockGuard<'a> {
    type Target = SensitiveData;

    fn deref(&self) -> &Self::Target {
        &self.lock.data
    }
}

impl<'a> DerefMut for CustomLockGuard<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.lock.data
    }
}

// COMPILE-TIME WIN: The proxy automatically unlocks when it goes out of scope.
impl<'a> Drop for CustomLockGuard<'a> {
    fn drop(&mut self) {
        self.lock.is_locked = false;
        // In a real implementation, you'd notify other waiting threads here.
    }
}

// ============================================================================
// Approach 2: Protection Proxy (Newtype Pattern)
// ============================================================================

/// A protection proxy that ensures a string is a valid email.
/// Notice it does NOT implement `DerefMut` because we don't want external code
/// mutating the inner string and invalidating the email format.
#[derive(Debug, PartialEq)]
pub struct ValidatedEmail(String);

impl ValidatedEmail {
    /// The only way to construct this proxy is through validation.
    /// COMPILE-TIME WIN: Once created, functions requiring a `ValidatedEmail`
    /// don't need to re-validate it.
    pub fn try_new(email: String) -> Result<Self, &'static str> {
        if email.contains('@') && email.contains('.') {
            Ok(Self(email))
        } else {
            Err("Invalid email format")
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// ============================================================================
// Approach 3: Lazy Initialization Proxy
// ============================================================================

/// A proxy that delays the execution of an expensive operation until it is needed.
pub struct LazyProxy<T, F>
where
    F: FnOnce() -> T,
{
    initializer: Option<F>,
    value: OnceLock<T>,
}

impl<T, F> LazyProxy<T, F>
where
    F: FnOnce() -> T,
{
    pub const fn new(initializer: F) -> Self {
        Self {
            initializer: Some(initializer),
            value: OnceLock::new(),
        }
    }

    /// Returns a reference to the initialized value.
    pub fn get_or_init(&mut self) -> &T {
        // TRADEOFF: Requires `&mut self` to take the initializer, or `Cell`/`Mutex` if we wanted `&self`.
        let initializer = self.initializer.take();
        self.value.get_or_init(|| {
            initializer.expect("Initializer should be present on first call")()
        })
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_raii_guard_proxy() {
        let mut my_lock = CustomLock::new(SensitiveData {
            value: "secret".to_string(),
        });

        {
            let mut guard = my_lock.lock();
            assert!(guard.lock.is_locked);

            // Transparent access via Deref!
            guard.do_work();
            assert_eq!(guard.value, "secret");

            // Mutable access via DerefMut
            guard.value = "new secret".to_string();
        } // guard is dropped here

        assert!(!my_lock.is_locked);
        assert_eq!(my_lock.data.value, "new secret");
    }

    #[test]
    fn test_protection_proxy() {
        let valid = ValidatedEmail::try_new("test@example.com".to_string());
        assert!(valid.is_ok());

        let invalid = ValidatedEmail::try_new("bad-email".to_string());
        assert!(invalid.is_err());
    }

    #[test]
    fn test_lazy_proxy() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        let called = Arc::new(AtomicBool::new(false));
        let called_clone = Arc::clone(&called);

        let mut lazy = LazyProxy::new(move || {
            // Expensive initialization
            called_clone.store(true, Ordering::Relaxed);
            vec![1, 2, 3, 4, 5]
        });

        assert!(!called.load(Ordering::Relaxed)); // Not initialized yet

        let data1 = lazy.get_or_init();
        assert!(called.load(Ordering::Relaxed)); // Initialized now
        assert_eq!(data1, &vec![1, 2, 3, 4, 5]);

        // Subsequent calls don't re-run the closure.
        let data2 = lazy.get_or_init();
        assert_eq!(data2, &vec![1, 2, 3, 4, 5]);
    }
}
