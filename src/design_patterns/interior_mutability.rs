// Suppress pedantic and nursery lints for design pattern examples.
#![allow(clippy::pedantic, clippy::nursery, unused)]
//! # Interior Mutability Pattern
//!
//! Replaces: **`mutable` keyword** (C++), **unrestricted mutation** (Java/Python)
//!
//! Real Rust usage: `std::cell::Cell`, `std::cell::RefCell`, `std::sync::Mutex`, `std::sync::OnceLock`
//!
//! ## Why this pattern exists in Rust specifically
//! Rust's borrow checker enforces the "Shared XOR Mutable" rule: you can have either many readers (`&T`)
//! OR one writer (`&mut T`), but never both.
//!
//! However, sometimes you need to mutate data even when you only have an immutable reference to the container.
//! This is called **Interior Mutability**. It works by moving the borrow checking from compile time to runtime (`RefCell`)
//! or by restricting operations to `Copy` types (`Cell`).
//!
//! ## Architecture
//!
//! The core primitive is `UnsafeCell<T>`. It is the *only* way to go from `&T` to `*mut T` in safe Rust.
//! All other interior mutability types (`Cell`, `RefCell`, `Mutex`) are built on top of `UnsafeCell`.
//!
//! ```text
//! ┌──────────────────────┐
//! │      Your Type       │
//! ├──────────────────────┤
//! │    UnsafeCell<T>     │ ◄── The magic sauce
//! ├──────────────────────┤
//! │        Data T        │
//! └──────────────────────┘
//! ```
//!
//! ## When to use
//! - Implementing logical immutability but physical mutability (e.g., caching, lazy initialization).
//! - Implementing graph structures where nodes are shared (`Rc<RefCell<T>>`).
//! - Mocking objects where you need to record calls despite an immutable `&self` interface.

use std::cell::UnsafeCell;
use std::ops::{Deref, DerefMut};

// ============================================================================
// Pattern 1: SimpleCell<T> (Copy only)
// ============================================================================

/// A mutable memory location that admits `Copy` types.
///
/// **OWNERSHIP INSIGHT:**
/// `Cell` works because it never gives out a reference to the inner data.
/// It only allows copying data in (`set`) or out (`get`). This avoids aliasing issues entirely.
pub struct SimpleCell<T> {
    // COMPILE-TIME WIN: UnsafeCell opts out of the "immutability" guarantee of &T.
    inner: UnsafeCell<T>,
}

impl<T> SimpleCell<T> {
    pub fn new(value: T) -> Self {
        SimpleCell {
            inner: UnsafeCell::new(value),
        }
    }

    /// Sets the value of the cell.
    pub fn set(&self, val: T) {
        // SAFETY: We are not giving out any references to the inner data,
        // so no one else can be reading it right now (assuming single-threaded).
        // Cell is !Sync, so this is safe.
        unsafe {
            *self.inner.get() = val;
        }
    }

    /// Gets a copy of the value.
    pub fn get(&self) -> T
    where
        T: Copy,
    {
        // SAFETY: We are copying the data, not returning a reference.
        unsafe { *self.inner.get() }
    }
}

// ============================================================================
// Pattern 2: SimpleRefCell<T> (Runtime Borrow Checking)
// ============================================================================

/// A mutable memory location with runtime borrow checking.
///
/// **TRADEOFF:** `RefCell` adds runtime overhead (tracking borrows) and can panic.
/// Use it when you can't prove ownership rules at compile time (e.g., graphs).
pub struct SimpleRefCell<T> {
    value: UnsafeCell<T>,
    // 0: Unborrowed
    // >0: Number of active immutable borrows
    // -1: Active mutable borrow
    borrow_state: SimpleCell<isize>,
}

impl<T> SimpleRefCell<T> {
    pub fn new(value: T) -> Self {
        SimpleRefCell {
            value: UnsafeCell::new(value),
            borrow_state: SimpleCell::new(0),
        }
    }

    pub fn borrow(&self) -> SimpleRef<'_, T> {
        let state = self.borrow_state.get();
        if state < 0 {
            panic!("Already borrowed mutably");
        }
        self.borrow_state.set(state + 1);

        // SAFETY: We checked that there are no mutable borrows.
        SimpleRef {
            cell: self,
            value: unsafe { &*self.value.get() },
        }
    }

    pub fn borrow_mut(&self) -> SimpleRefMut<'_, T> {
        let state = self.borrow_state.get();
        if state != 0 {
            panic!("Already borrowed");
        }
        self.borrow_state.set(-1);

        // SAFETY: We checked that there are no other borrows.
        SimpleRefMut {
            cell: self,
            value: unsafe { &mut *self.value.get() },
        }
    }
}

pub struct SimpleRef<'a, T> {
    cell: &'a SimpleRefCell<T>,
    value: &'a T,
}

impl<'a, T> Deref for SimpleRef<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.value
    }
}

impl<'a, T> Drop for SimpleRef<'a, T> {
    fn drop(&mut self) {
        let state = self.cell.borrow_state.get();
        self.cell.borrow_state.set(state - 1);
    }
}

pub struct SimpleRefMut<'a, T> {
    cell: &'a SimpleRefCell<T>,
    value: &'a mut T,
}

impl<'a, T> Deref for SimpleRefMut<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.value
    }
}

impl<'a, T> DerefMut for SimpleRefMut<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        // Reborrow the mutable reference
        &mut *self.value
    }
}

impl<'a, T> Drop for SimpleRefMut<'a, T> {
    fn drop(&mut self) {
        self.cell.borrow_state.set(0);
    }
}

// ============================================================================
// Pattern 3: OnceInit<T> (Lazy Initialization)
// ============================================================================

#[derive(Clone, Copy, PartialEq, Eq)]
enum InitState {
    Uninitialized,
    Initializing,
    Initialized,
}

/// A synchronization primitive which can be written to only once.
/// Simplified version of `std::sync::OnceLock` or `once_cell::sync::OnceCell`.
///
/// **NOTE:** This implementation uses `SimpleCell` internally, so it is `!Sync`.
/// It cannot be used for global `static` Singletons. Real `OnceLock` uses atomics/mutexes.
pub struct OnceInit<T> {
    inner: UnsafeCell<Option<T>>,
    state: SimpleCell<InitState>,
}

impl<T> OnceInit<T> {
    pub fn new() -> Self {
        OnceInit {
            inner: UnsafeCell::new(None),
            state: SimpleCell::new(InitState::Uninitialized),
        }
    }

    pub fn get_or_init<F>(&self, f: F) -> &T
    where
        F: FnOnce() -> T,
    {
        match self.state.get() {
            InitState::Initialized => {
                // Already initialized, fall through to return reference
            }
            InitState::Initializing => {
                panic!("Recursive initialization detected");
            }
            InitState::Uninitialized => {
                self.state.set(InitState::Initializing);
                // If f() panics, we stay in Initializing state (poisoned).
                let val = f();
                // SAFETY: We are the only one writing because of single-threaded assumption + state check.
                unsafe {
                    *self.inner.get() = Some(val);
                }
                self.state.set(InitState::Initialized);
            }
        }

        // SAFETY: Once Initialized, the value is present and won't move.
        unsafe { (*self.inner.get()).as_ref().unwrap() }
    }
}

impl<T> Default for OnceInit<T> {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Footer
// ============================================================================

// GOTCHA: Interior mutability types are generally !Sync.
// `Cell<T>` and `RefCell<T>` cannot be shared across threads.
// For thread-safe interior mutability, use `Mutex<T>` or `RwLock<T>`.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_cell() {
        let cell = SimpleCell::new(10);
        assert_eq!(cell.get(), 10);
        cell.set(20);
        assert_eq!(cell.get(), 20);
    }

    #[test]
    fn test_simple_refcell() {
        let cell = SimpleRefCell::new(vec![1, 2, 3]);

        {
            let borrowed = cell.borrow();
            assert_eq!(borrowed.len(), 3);
        } // Borrow dropped

        {
            let mut borrowed_mut = cell.borrow_mut();
            borrowed_mut.push(4);
        } // Mutable borrow dropped

        assert_eq!(cell.borrow().len(), 4);
    }

    #[test]
    #[should_panic(expected = "Already borrowed")]
    fn test_refcell_panic_double_borrow_mut() {
        let cell = SimpleRefCell::new(10);
        let _b1 = cell.borrow();
        let _b2 = cell.borrow_mut(); // Should panic
    }

    #[test]
    fn test_once_init() {
        let once = OnceInit::new();
        let val1 = once.get_or_init(|| 10);
        let val2 = once.get_or_init(|| 20); // Should return 10

        assert_eq!(*val1, 10);
        assert_eq!(*val2, 10);
        // Ensure they point to the same memory
        assert!(std::ptr::eq(val1, val2));
    }

    #[test]
    #[should_panic(expected = "Recursive initialization detected")]
    fn test_once_init_reentrancy() {
        let once = OnceInit::new();
        once.get_or_init(|| {
            once.get_or_init(|| 20); // Should panic
            10
        });
    }
}
