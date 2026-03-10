// Suppress pedantic and nursery lints for design pattern examples.
#![allow(clippy::pedantic, clippy::nursery, unused)]
//! # Marker Trait Pattern
//!
//! Replaces: **Runtime Type Checking** (reflection, `instanceof`), **Documentation Only Constraints**
//!
//! Real Rust usage: `std::marker::Send`, `std::marker::Sync`, `std::marker::Sized`, `bytemuck::Pod`
//!
//! ## Why this pattern exists in Rust
//! Rust traits can have zero methods. These are called Marker Traits. They act as "tags" or "labels" for types
//! that the compiler can enforce. By adding a marker trait bound to a generic function, you can restrict
//! the set of types that function accepts at compile time, with zero runtime cost.
//!
//! ## Architecture
//!
//! ```text
//! trait Privileged {} // No methods
//! struct Admin;
//! impl Privileged for Admin {}
//!
//! fn delete_database<T: Privileged>(user: T) { ... }
//! ```
//!
//! **Invariants:**
//! - Zero runtime overhead (traits dissolve after compilation).
//! - Used to categorize types based on properties (thread safety, memory layout, semantic permission).
//! - Often combined with `unsafe` traits to promise invariants to the compiler (e.g., `Send`).
//!
//! ## When to use
//! - To enforce semantic constraints (e.g., `Validated`, `ThreadSafe`).
//! - To enable specific optimizations for certain types (specialization - though unstable feature).
//! - To prevent misuse of APIs by restricting input types.

use std::marker::PhantomData;

// ============================================================================
// Pattern 1: Semantic Permission (Access Control)
// ============================================================================

/// Marker trait indicating a user has administrative privileges.
///
/// **COMPILE-TIME WIN:** Functions requiring this trait will simply not compile
/// if passed a non-privileged user. No runtime `if user.is_admin()` check needed.
pub trait AdminPrivileges {}

pub struct User {
    // Used for example context only
    pub _username: String,
}

pub struct Admin {
    // Used for example context only
    pub _username: String,
}

// Only Admin implements the marker
impl AdminPrivileges for Admin {}

// Function restricted to admins
pub fn nuke_system<T: AdminPrivileges>(_admin: &T) {
    println!("System nuked by authorized user.");
}

// ============================================================================
// Pattern 2: Type Classification (Safe vs Unsafe Data)
// ============================================================================

/// Marker trait for types that are safe to transmit over a network (e.g., no raw pointers).
/// Similar to `Send` but for our domain logic.
pub trait NetworkSafe {}

// Primitive types are safe
impl NetworkSafe for i32 {}
impl NetworkSafe for String {}
impl NetworkSafe for bool {}

// A struct is safe if all its fields are safe (manual implementation here,
// could be derived with proc-macros).
pub struct Message<T> {
    pub _content: T,
}

impl<T: NetworkSafe> NetworkSafe for Message<T> {}

pub struct PacketSender;

impl PacketSender {
    pub fn send<T: NetworkSafe>(&self, _data: T) {
        println!("Sending safe data...");
    }
}

// ============================================================================
// Pattern 3: Zero-Sized Markers in Generics
// ============================================================================

// Combining Marker Traits with PhantomData to tag generic types.
// This is often used in the Typestate pattern (see `typestate.rs`),
// but here we focus on the trait aspect.

pub trait Color {}
pub struct Red;
pub struct Blue;
impl Color for Red {}
impl Color for Blue {}

pub struct Paint<C: Color> {
    _marker: PhantomData<C>,
}

impl<C: Color> Paint<C> {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Paint {
            _marker: PhantomData,
        }
    }
}

// Note: Using `_p` prefixes to avoid unused variable warnings in examples
pub fn mix<C1: Color, C2: Color>(_p1: Paint<C1>, _p2: Paint<C2>) {
    println!("Mixing paint...");
}

// We can constrain mixing logic using traits if we want:
pub trait MixableWith<Other> {}
impl MixableWith<Blue> for Red {} // Red can mix with Blue

pub fn strict_mix<C1, C2>(_p1: Paint<C1>, _p2: Paint<C2>)
where
    C1: Color + MixableWith<C2>,
    C2: Color,
{
    println!("Strict mixing successful.");
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_admin_privileges() {
        let admin = Admin {
            _username: "root".into(),
        };
        let _user = User {
            _username: "guest".into(),
        };

        nuke_system(&admin);

        // COMPILE-TIME WIN:
        // nuke_system(&user); // Error: the trait bound `User: AdminPrivileges` is not satisfied
    }

    #[test]
    fn test_network_safe() {
        let sender = PacketSender;
        sender.send(42);
        sender.send("Hello".to_string());

        struct RawPointer(*const i32); // Not NetworkSafe
        let _ptr = RawPointer(std::ptr::null());

        // COMPILE-TIME WIN:
        // sender.send(_ptr); // Error: `RawPointer` doesn't implement `NetworkSafe`
    }

    #[test]
    fn test_paint_mixing() {
        let red = Paint::<Red>::new();
        let blue = Paint::<Blue>::new();

        strict_mix(red, blue);

        // COMPILE-TIME WIN:
        // let red2 = Paint::<Red>::new();
        // strict_mix(red, red2); // Error: Red is not MixableWith<Red>
    }
}
