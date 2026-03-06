//! # Flyweight Pattern
//!
//! Replaces: **Flyweight Pattern** (OOP)
//!
//! Real Rust usage: `string-interner` (crate), `petgraph` (indices instead of pointers), `smol_str`
//!
//! ## Why this pattern exists in Rust
//! In OOP languages, the Flyweight pattern reduces memory usage by sharing as much data as possible with similar objects.
//! It often relies on object references and a factory that caches created objects.
//!
//! In Rust, building complex webs of shared references (`Rc<T>`, `Arc<T>`, or `&'a T`) quickly leads to
//! borrow-checker friction, lifetime hell, or reference-counting overhead.
//!
//! The idiomatic Rust translation of Flyweight avoids widespread shared pointers entirely. Instead, it relies on:
//! 1. **Indices/Arenas:** Storing the shared "intrinsic" state in a central collection (like a `Vec`) and giving
//!    objects a lightweight index (`usize` or `u32`) into that collection.
//! 2. **Interning:** For strings or other immutable data, using an interner to map values to unique IDs.
//!
//! ## Architecture
//!
//! ```text
//! [ extrinsic state 1 ] \                    /-> [ intrinsic state A ]
//! [ extrinsic state 2 ] --(index: usize)--> |    [ intrinsic state B ]
//! [ extrinsic state 3 ] /                    \-> [ intrinsic state C ]
//!    (Lightweight)                            (Heavy, stored in Vec/Arena)
//! ```
//!
//! **Invariants:**
//! - Intrinsic (shared) state is immutable once created.
//! - The central repository (the "Context" or "Arena") outlives all entities that hold indices into it.
//!
//! ## When to use
//! - When creating thousands/millions of objects that share large chunks of data (e.g., textures in a game, formatting in a text editor).
//! - When you need to build graph-like structures but want to avoid `Rc<RefCell<T>>`.
//!
//! ## Anti-patterns
//! - Using `Rc<RefCell<HeavyState>>` everywhere just to share data. It adds runtime overhead and makes the code harder to reason about.
//!
//! ## Footer
//! The GoF Flyweight pattern is fundamentally about memory optimization via sharing. Rust achieves this much more cleanly
//! and with better cache locality by separating the data into flat arrays (SoA - Struct of Arrays style) and passing
//! simple indices by value. This is a foundational pattern in high-performance Rust, especially in game development (ECS).

use std::collections::HashMap;
use std::sync::Arc;

// ============================================================================
// Approach 1: Arena / Index-based Flyweight (The "Rust" Way)
// ============================================================================

/// Intrinsic, heavy, shared state.
/// For example, in a game, this could be a heavy 3D mesh and texture data.
#[derive(Debug, PartialEq)]
pub struct TreeModel {
    pub mesh_data: Vec<u8>,
    pub texture_id: u32,
}

/// A lightweight handle (index) to the intrinsic state.
// COMPILE-TIME WIN: `u32` is trivially `Copy`, entirely sidestepping borrow checker issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ModelHandle(usize);

/// The central repository (Arena) that owns the heavy state.
#[derive(Default)]
pub struct ModelRegistry {
    models: Vec<TreeModel>,
    // Optional: caching to reuse existing models if they are identical.
    // In a real scenario, you might hash the TreeModel or use a specific key.
}

impl ModelRegistry {
    /// Registers a model and returns a lightweight handle to it.
    pub fn register(&mut self, model: TreeModel) -> ModelHandle {
        // OWNERSHIP INSIGHT: The registry takes full ownership of the heavy data.
        let index = self.models.len();
        self.models.push(model);
        ModelHandle(index)
    }

    /// Retrieves the heavy state using the lightweight handle.
    #[must_use]
    pub fn get(&self, handle: ModelHandle) -> Option<&TreeModel> {
        self.models.get(handle.0)
    }
}

/// The extrinsic state, specific to each instance, combined with the lightweight handle.
#[derive(Debug)]
pub struct TreeInstance {
    pub x: f32,
    pub y: f32,
    pub height: f32,
    pub model: ModelHandle,
}

// ============================================================================
// Approach 2: String Interning (Arc<str>)
// ============================================================================

// If you need to share strings across many objects, `Arc<str>` is a common
// lightweight flyweight. It avoids copying the string data and provides O(1) cloning.

/// A simple string interner.
#[derive(Default)]
pub struct StringInterner {
    // Stores the interned strings.
    map: HashMap<String, Arc<str>>,
}

impl StringInterner {
    /// Interns a string, returning an `Arc<str>` pointing to the shared allocation.
    pub fn intern(&mut self, s: &str) -> Arc<str> {
        // TRADEOFF: `Arc` introduces atomic reference counting overhead upon cloning,
        // but it allows sharing the string across threads without lifetimes.
        if let Some(arc) = self.map.get(s) {
            return Arc::clone(arc);
        }

        let arc: Arc<str> = Arc::from(s);
        self.map.insert(s.to_string(), Arc::clone(&arc));
        arc
    }
}

// ANTI-PATTERN:
// struct BadTree {
//     x: f32,
//     y: f32,
//     // This forces every tree to heap-allocate and copy the string!
//     species_name: String,
//     // This requires complex lifetimes or Rc everywhere.
//     model: Rc<TreeModel>,
// }

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arena_flyweight() {
        let mut registry = ModelRegistry::default();

        let oak_model = TreeModel {
            mesh_data: vec![1, 2, 3],
            texture_id: 10,
        };
        let pine_model = TreeModel {
            mesh_data: vec![4, 5, 6],
            texture_id: 20,
        };

        let oak_handle = registry.register(oak_model);
        let pine_handle = registry.register(pine_model);

        // We can create millions of these very cheaply.
        let mut forest = Vec::new();
        for i in 0..1000 {
            forest.push(TreeInstance {
                x: i as f32,
                y: 0.0,
                height: 10.0,
                model: if i % 2 == 0 { oak_handle } else { pine_handle },
            });
        }

        assert_eq!(forest.len(), 1000);

        // Retrieve the heavy data via the handle
        let first_tree_model = registry.get(forest[0].model).unwrap();
        assert_eq!(first_tree_model.texture_id, 10);
    }

    #[test]
    fn test_string_interner() {
        let mut interner = StringInterner::default();

        let s1 = interner.intern("Hello, Flyweight!");
        let s2 = interner.intern("Hello, Flyweight!");
        let s3 = interner.intern("Different string");

        // COMPILE-TIME WIN: `Arc::ptr_eq` guarantees they point to the exact same memory allocation.
        assert!(Arc::ptr_eq(&s1, &s2));
        assert!(!Arc::ptr_eq(&s1, &s3));

        // The actual value is accessible
        assert_eq!(&*s1, "Hello, Flyweight!");
    }
}
