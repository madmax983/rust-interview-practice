// Suppress pedantic and nursery lints for design pattern examples.
#![allow(clippy::pedantic, clippy::nursery, unused)]
//! # The Handle Pattern (Indirection via ID)
//!
//! Replaces: **Pointer/Reference Graph** (C++), **Garbage Collected References** (Java/Python)
//!
//! Real Rust usage: `slotmap`, `thunderdome`, `specs` (ECS), `petgraph` (NodeIndex)
//!
//! ## Why this pattern exists in Rust
//! Rust's ownership rules make self-referential structures (like graphs) difficult.
//! `&'a T` locks the lifetime to a scope. `Rc<RefCell<T>>` incurs runtime overhead and risks cycles.
//! Handles (indices into a collection) decouple lifetime from location, allowing "weak references"
//! that are safe to copy and store.
//!
//! ## Architecture
//!
//! ```text
//! [ Handle(index, generation) ] --> [ Store (Vec<Entry>) ]
//!                                     |
//!                                     +-- [ Entry { gen: 1, payload: Occupied(Data) } ]
//!                                     +-- [ Entry { gen: 2, payload: Free(next) } ]
//! ```
//!
//! **Invariants:**
//! - A handle is only valid if its generation matches the entry's generation.
//! - Accessing a removed item via an old handle returns `None` (safe failure, no Use-After-Free).
//! - No `unsafe` required (bounds checking handles safety).
//!
//! ## When to use
//! - Graphs, Trees with back-references, ECS (Entity Component Systems).
//! - When you need to serialize references (indices are just integers).
//! - To avoid "fighting the borrow checker" with multiple mutable references.

use std::marker::PhantomData;

// ============================================================================
// Core Types
// ============================================================================

/// A strongly-typed handle to a resource in a `Store`.
///
/// **OWNERSHIP INSIGHT:** Handles are `Copy`, lightweight, and don't own the data.
/// They act like "weak references" that never dangle unsafely (access is checked).
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct Handle<T> {
    index: usize,
    generation: u64,
    // COMPILE-TIME WIN: PhantomData prevents mixing handles of different types.
    _marker: PhantomData<T>,
}

// Manually implement Copy/Clone because T might not be Copy/Clone
impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Handle<T> {}

impl<T> Handle<T> {
    fn new(index: usize, generation: u64) -> Self {
        Self {
            index,
            generation,
            _marker: PhantomData,
        }
    }
}

// Internal entry representation
struct Entry<T> {
    generation: u64,
    payload: Payload<T>,
}

enum Payload<T> {
    Free { next_free: Option<usize> },
    Occupied(T),
}

/// A generational arena that stores objects of type `T`.
///
/// Also known as a "SlotMap".
pub struct Store<T> {
    entries: Vec<Entry<T>>,
    free_head: Option<usize>,
    len: usize,
}

impl<T> Store<T> {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            free_head: None,
            len: 0,
        }
    }

    /// Inserts a value and returns a handle to it.
    pub fn insert(&mut self, value: T) -> Handle<T> {
        self.len += 1;

        if let Some(free_idx) = self.free_head {
            // Reuse a slot
            let entry = &mut self.entries[free_idx];

            // Extract next_free from the Free payload
            let next_free = match &entry.payload {
                Payload::Free { next_free } => *next_free,
                Payload::Occupied(_) => unreachable!("Corrupted free list"),
            };

            self.free_head = next_free;

            // PRODUCTION NOTE: `entry.generation` was incremented during `remove`,
            // so it represents the correct generation for this new allocation.

            entry.payload = Payload::Occupied(value);

            Handle::new(free_idx, entry.generation)
        } else {
            // Append new slot
            let idx = self.entries.len();
            self.entries.push(Entry {
                generation: 0,
                payload: Payload::Occupied(value),
            });
            Handle::new(idx, 0)
        }
    }

    /// Gets a reference to the value if the handle is valid.
    pub fn get(&self, handle: Handle<T>) -> Option<&T> {
        let entry = self.entries.get(handle.index)?;

        // GENIUS: The generation check validates the handle in O(1).
        // If the slot was reused, entry.generation will be higher.
        if entry.generation != handle.generation {
            return None;
        }

        match &entry.payload {
            Payload::Occupied(val) => Some(val),
            Payload::Free { .. } => None, // Should be caught by generation check usually
        }
    }

    /// Gets a mutable reference to the value if the handle is valid.
    pub fn get_mut(&mut self, handle: Handle<T>) -> Option<&mut T> {
        let entry = self.entries.get_mut(handle.index)?;

        if entry.generation != handle.generation {
            return None;
        }

        match &mut entry.payload {
            Payload::Occupied(val) => Some(val),
            Payload::Free { .. } => None,
        }
    }

    /// Removes the value, invalidating the handle.
    /// Returns the value if found.
    pub fn remove(&mut self, handle: Handle<T>) -> Option<T> {
        if handle.index >= self.entries.len() {
            return None;
        }

        let entry = &mut self.entries[handle.index];

        if entry.generation != handle.generation {
            return None;
        }

        // Check if already free (double free protection via generation or payload check)
        // If generation matches, it MUST be occupied because we increment generation on free.

        // Move data out
        let data = match std::mem::replace(
            &mut entry.payload,
            Payload::Free {
                next_free: self.free_head,
            },
        ) {
            Payload::Occupied(d) => d,
            Payload::Free { .. } => return None, // Should be unreachable with generation check
        };

        // Update free list
        self.free_head = Some(handle.index);

        // CRITICAL: Increment generation to invalidate all existing handles to this slot
        entry.generation += 1;
        self.len -= 1;

        Some(data)
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl<T> Default for Store<T> {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq)]
    struct Position {
        x: f32,
        y: f32,
    }

    #[derive(Debug, PartialEq)]
    struct Velocity {
        dx: f32,
        dy: f32,
    }

    #[test]
    fn test_crud_operations() {
        let mut positions = Store::<Position>::new();

        let p1 = Position { x: 10.0, y: 20.0 };
        let h1 = positions.insert(p1);

        assert_eq!(positions.len(), 1);

        // Read
        let stored = positions.get(h1).unwrap();
        assert_eq!(stored.x, 10.0);

        // Update
        let stored_mut = positions.get_mut(h1).unwrap();
        stored_mut.x = 15.0;

        assert_eq!(positions.get(h1).unwrap().x, 15.0);

        // Remove
        let removed = positions.remove(h1).unwrap();
        assert_eq!(removed.x, 15.0);
        assert_eq!(positions.len(), 0);

        // Stale access
        assert!(positions.get(h1).is_none());
    }

    #[test]
    fn test_generation_check() {
        let mut store = Store::<i32>::new();

        // 1. Insert A
        let h_a = store.insert(100);

        // 2. Remove A (slot 0 is now free, generation incremented)
        store.remove(h_a);

        // 3. Insert B (reuses slot 0)
        let h_b = store.insert(200);

        // Index should be same
        assert_eq!(h_a.index, h_b.index);
        // Generation should differ
        assert_ne!(h_a.generation, h_b.generation);

        // 4. Access via old handle A should fail
        assert!(store.get(h_a).is_none());
        // Access via new handle B should succeed
        assert_eq!(*store.get(h_b).unwrap(), 200);
    }

    #[test]
    fn test_multiple_types_ecs_lite() {
        // Compile-time check: Handles are typed
        let mut positions = Store::<Position>::new();
        let mut velocities = Store::<Velocity>::new();

        let h_pos = positions.insert(Position { x: 0.0, y: 0.0 });
        let h_vel = velocities.insert(Velocity { dx: 1.0, dy: 1.0 });

        // Uncommenting this should fail to compile:
        // positions.get(h_vel);
        // error[E0308]: mismatched types: expected `Handle<Position>`, found `Handle<Velocity>`

        // Manual "Entity" linking both (in a real ECS, Entity would be a handle to an archetype map)
        struct Entity {
            pos: Handle<Position>,
            vel: Handle<Velocity>,
        }

        let entity = Entity {
            pos: h_pos,
            vel: h_vel,
        };

        // System: Update position based on velocity
        if let (Some(pos), Some(vel)) = (positions.get_mut(entity.pos), velocities.get(entity.vel))
        {
            pos.x += vel.dx;
            pos.y += vel.dy;
        }

        assert_eq!(positions.get(h_pos).unwrap().x, 1.0);
    }
}
