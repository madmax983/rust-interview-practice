//! # Entity Component System (ECS) Implementation
//!
//! A minimal Entity Component System that separates data (Components) from logic (Systems)
//! and identifies objects by unique IDs (Entities).
//!
//! **Replaces Crates:** `bevy_ecs`, `specs`, `hecs`, `legion`
//!
//! **Real-world Usage:**
//! - Game Engines (Bevy, Amethyst, Unity DOTS).
//! - Physics simulations and particle systems.
//! - High-performance data processing where cache locality matters.
//!
//! **Why build it yourself?**
//! ECS forces you to rethink object-oriented design. Instead of "Array of Structs" (`AoS`),
//! ECS uses "Struct of Arrays" (`SoA`) for cache efficiency. Building one in Rust teaches
//! you how to use `std::any::Any` to store heterogeneous collections (a `HashMap` of different
//! component types) safely and downcast them back to concrete types at runtime.

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure:
//
//      World
//      ├── next_entity: usize
//      └── storages: HashMap<TypeId, Box<dyn StorageTrait>>
//                            │
//                            ▼
//                     VecStorage<Position> ──► [ Pos, None, Pos, ... ] (indexed by Entity ID)
//                     VecStorage<Velocity> ──► [ Vel, Vel,  None, ...]
//
// Components:
// - `Entity`: Just a unique ID (usize).
// - `Component`: Any Rust struct (e.g., `Position { x: f32, y: f32 }`).
// - `Storage`: An array aligned to Entity IDs. If Entity 2 has a Position, `positions[2] = Some(Pos)`.
//
// Invariants:
// 1. **Entity Uniqueness**: Every spawned entity gets a strictly increasing, unique ID.
// 2. **Type Safety**: The `Any` downcasting must never fail because the `TypeId` key in the
//    HashMap guarantees the type of the underlying `Box<dyn StorageTrait>`.
// 3. **Storage Alignment**: Storage vectors grow as needed to accommodate the highest Entity ID.
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ Spawn Entity  │ O(1)        │ O(1)        │
// │ Add Component │ O(1)*       │ O(N)        │
// │ Query 1 Comp  │ O(N)        │ O(1)        │
// └───────────────┴─────────────┴─────────────┘
// * Amortized O(1) due to Vec reallocation. N = Max Entity ID.
//
// Design Decisions:
// - **Sparse Sets vs. Flat Arrays**: We use flat `Vec<Option<T>>` for simplicity (SoA).
//   - *Tradeoff*: High memory overhead if components are sparse (many `None`s).
//   - *Alternative*: Sparse Sets (used by EnTT, Bevy) keep components contiguous in memory.
// - **Querying**: Basic iterator filtering.
//   - *Tradeoff*: We iterate over all possible entity IDs and check for `Some()`.
//   - *Alternative*: Archetype-based storage (like Bevy) group entities with the same components
//     together for true contiguous iteration.
// - **Borrowing Rules**: We use runtime borrow checking (`RefCell`) to allow multiple systems to
//   borrow different storages simultaneously, or we just hand out vectors. For this minimal ECS,
//   we will return `Vec<&T>` directly.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::fmt::Debug;

/// Entity represents a unique ID in the system.
pub type Entity = usize;

/// `StorageTrait` allows the World to hold multiple typed Storage containers generically.
///
/// RUST INSIGHT: `Any` gives us runtime reflection-like capabilities, allowing downcasting
/// from a trait object to a concrete type if the `TypeId` matches.
pub trait StorageTrait: Any {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// A contiguous array of a specific component type aligned by Entity ID.
#[derive(Debug, Default)]
pub struct VecStorage<T> {
    components: Vec<Option<T>>,
}

impl<T: 'static> StorageTrait for VecStorage<T> {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl<T> VecStorage<T> {
    pub fn insert(&mut self, entity: Entity, component: T) {
        if entity >= self.components.len() {
            // Resize vector to accommodate the entity ID. Fill gaps with None.
            self.components.resize_with(entity + 1, || None);
        }
        self.components[entity] = Some(component);
    }

    #[must_use]
    pub fn get(&self, entity: Entity) -> Option<&T> {
        self.components.get(entity)?.as_ref()
    }

    pub fn get_mut(&mut self, entity: Entity) -> Option<&mut T> {
        self.components.get_mut(entity)?.as_mut()
    }
}

/// The World acts as the central registry for entities and their components.
#[derive(Default)]
pub struct World {
    next_entity: Entity,
    storages: HashMap<TypeId, Box<dyn StorageTrait>>,
}

impl World {
    /// Creates a new, empty World.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Spawns a new entity into the World and returns its unique ID.
    pub const fn spawn(&mut self) -> Entity {
        let id = self.next_entity;
        self.next_entity += 1;
        id
    }

    /// Helper to get or create storage for a specific component type.
    fn ensure_storage<T: 'static>(&mut self) -> &mut VecStorage<T> {
        let type_id = TypeId::of::<T>();
        let storage = self.storages.entry(type_id).or_insert_with(|| {
            // RUST INSIGHT: We box the newly created VecStorage
            // because `storages` takes `Box<dyn StorageTrait>`.
            Box::new(VecStorage::<T> { components: Vec::new() })
        });

        // UNSAFE JUSTIFICATION: This downcast is safe because we only ever
        // insert `VecStorage<T>` when the key is `TypeId::of::<T>()`.
        storage
            .as_any_mut()
            .downcast_mut::<VecStorage<T>>()
            .expect("Type mismatch in storage map")
    }

    /// Attaches a component to an entity. Overwrites if one already exists.
    ///
    /// # Panics
    ///
    /// Panics if the `entity` ID has not been spawned yet or if there's a type mismatch.
    pub fn insert_component<T: 'static>(&mut self, entity: Entity, component: T) {
        assert!(entity < self.next_entity, "Entity does not exist");
        let storage = self.ensure_storage::<T>();
        storage.insert(entity, component);
    }

    /// Retrieves an immutable reference to a component on an entity.
    ///
    /// # Panics
    ///
    /// Panics if there's a type mismatch in the internal storage map.
    #[must_use]
    pub fn get_component<T: 'static>(&self, entity: Entity) -> Option<&T> {
        let type_id = TypeId::of::<T>();
        let storage = self.storages.get(&type_id)?;

        let vec_storage = storage
            .as_any()
            .downcast_ref::<VecStorage<T>>()
            .expect("Type mismatch in storage map");

        vec_storage.get(entity)
    }

    /// Retrieves a mutable reference to a component on an entity.
    ///
    /// # Panics
    ///
    /// Panics if there's a type mismatch in the internal storage map.
    pub fn get_component_mut<T: 'static>(&mut self, entity: Entity) -> Option<&mut T> {
        let type_id = TypeId::of::<T>();
        let storage = self.storages.get_mut(&type_id)?;

        let vec_storage = storage
            .as_any_mut()
            .downcast_mut::<VecStorage<T>>()
            .expect("Type mismatch in storage map");

        vec_storage.get_mut(entity)
    }

    /// Returns a vector of tuples representing entities that have component `T`.
    /// This is the simplest possible query mechanism.
    ///
    /// # Panics
    ///
    /// Panics if there's a type mismatch in the internal storage map.
    #[must_use]
    pub fn query<T: 'static>(&self) -> Vec<(Entity, &T)> {
        let type_id = TypeId::of::<T>();
        self.storages.get(&type_id).map_or_else(Vec::new, |storage| {
            let vec_storage = storage
                .as_any()
                .downcast_ref::<VecStorage<T>>()
                .expect("Type mismatch in storage map");

            vec_storage
                .components
                .iter()
                .enumerate()
                .filter_map(|(id, comp)| comp.as_ref().map(|c| (id, c)))
                .collect()
        })
    }

    /// Returns a vector of tuples representing entities that have both component `T` and `U`.
    /// GOTCHA: In Rust, you cannot borrow `self.storages` mutably twice simultaneously for
    /// a mutable query. Hence we only implement an immutable join here for simplicity.
    /// A robust ECS uses `std::cell::RefCell` or `AtomicRefCell` inside the `HashMap`
    /// instead of a direct `Box<dyn StorageTrait>` to allow interior mutability.
    ///
    /// # Panics
    ///
    /// Panics if there's a type mismatch in the internal storage map.
    #[must_use]
    pub fn query_two<T: 'static, U: 'static>(&self) -> Vec<(Entity, &T, &U)> {
        // Collect T components
        let t_storage = self.storages.get(&TypeId::of::<T>());
        let u_storage = self.storages.get(&TypeId::of::<U>());

        if t_storage.is_none() || u_storage.is_none() {
            return Vec::new();
        }

        let t_vec = t_storage.unwrap().as_any().downcast_ref::<VecStorage<T>>().unwrap();
        let u_vec = u_storage.unwrap().as_any().downcast_ref::<VecStorage<U>>().unwrap();

        let mut results = Vec::new();

        // The length of iteration is the minimum length of the two underlying Vecs
        let len = std::cmp::min(t_vec.components.len(), u_vec.components.len());

        for id in 0..len {
            if let (Some(t_comp), Some(u_comp)) = (t_vec.components[id].as_ref(), u_vec.components[id].as_ref()) {
                results.push((id, t_comp, u_comp));
            }
        }

        results
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `bevy_ecs`: Extremely advanced. Uses "Archetypes" (grouping entities by their exact combination
//   of components) rather than SoA, allowing highly optimized, contiguous iteration.
// - `specs`: Uses `BitSet` to quickly find entities that match a query, rather than linear scans.
//
// Missing vs. Production:
// - **Mutable Queries**: We return immutable references. To return `(&mut T, &mut U)`, the ECS needs
//   interior mutability (e.g., `RefCell<Box<dyn StorageTrait>>`) so `borrow_mut()` can dynamically check.
// - **Archetypes / Sparse Sets**: Flat `Vec<Option<T>>` wastes memory if component distribution is sparse.
// - **System Scheduling**: No mechanism to schedule systems based on read/write dependency graphs.

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

    #[derive(Debug, PartialEq)]
    struct Health(i32);

    #[test]
    fn test_spawn_and_get() {
        let mut world = World::new();

        let e0 = world.spawn();
        let e1 = world.spawn();

        world.insert_component(e0, Position { x: 0.0, y: 0.0 });
        world.insert_component(e1, Position { x: 10.0, y: 10.0 });
        world.insert_component(e1, Velocity { dx: 1.0, dy: 1.0 });

        assert_eq!(world.get_component::<Position>(e0), Some(&Position { x: 0.0, y: 0.0 }));
        assert_eq!(world.get_component::<Position>(e1), Some(&Position { x: 10.0, y: 10.0 }));
        assert_eq!(world.get_component::<Velocity>(e1), Some(&Velocity { dx: 1.0, dy: 1.0 }));
        assert_eq!(world.get_component::<Velocity>(e0), None);
    }

    #[test]
    #[should_panic(expected = "Entity does not exist")]
    fn test_insert_invalid_entity() {
        let mut world = World::new();
        world.insert_component(99, Position { x: 0.0, y: 0.0 });
    }

    #[test]
    fn test_mutation() {
        let mut world = World::new();
        let e = world.spawn();
        world.insert_component(e, Health(100));

        if let Some(health) = world.get_component_mut::<Health>(e) {
            health.0 -= 10;
        }

        assert_eq!(world.get_component::<Health>(e), Some(&Health(90)));
    }

    #[test]
    fn test_queries() {
        let mut world = World::new();

        let e0 = world.spawn(); // Pos
        let e1 = world.spawn(); // Pos, Vel
        let e2 = world.spawn(); // Pos, Vel, Health
        let e3 = world.spawn(); // Vel

        world.insert_component(e0, Position { x: 0.0, y: 0.0 });

        world.insert_component(e1, Position { x: 1.0, y: 0.0 });
        world.insert_component(e1, Velocity { dx: 1.0, dy: 0.0 });

        world.insert_component(e2, Position { x: 2.0, y: 0.0 });
        world.insert_component(e2, Velocity { dx: 2.0, dy: 0.0 });
        world.insert_component(e2, Health(100));

        world.insert_component(e3, Velocity { dx: 3.0, dy: 0.0 });

        let pos_query = world.query::<Position>();
        assert_eq!(pos_query.len(), 3); // e0, e1, e2

        let vel_query = world.query::<Velocity>();
        assert_eq!(vel_query.len(), 3); // e1, e2, e3

        let health_query = world.query::<Health>();
        assert_eq!(health_query.len(), 1); // e2

        let pos_vel_query = world.query_two::<Position, Velocity>();
        assert_eq!(pos_vel_query.len(), 2); // e1, e2

        // Verify query_two results
        let e1_result = pos_vel_query.iter().find(|(id, _, _)| *id == e1).unwrap();
        assert_eq!(e1_result.1, &Position { x: 1.0, y: 0.0 });
        assert_eq!(e1_result.2, &Velocity { dx: 1.0, dy: 0.0 });
    }

    #[test]
    fn test_benchmark_stub() {
        // A simple benchmark-like test to show O(N) iteration
        let mut world = World::new();
        for _ in 0..10_000 {
            let e = world.spawn();
            world.insert_component(e, Position { x: 0.0, y: 0.0 });
        }

        let start = std::time::Instant::now();
        let query = world.query::<Position>();
        let elapsed = start.elapsed();

        assert_eq!(query.len(), 10_000);
        // Ensure it runs in a reasonable time (sub-millisecond usually)
        assert!(elapsed < std::time::Duration::from_millis(50));
    }
}
