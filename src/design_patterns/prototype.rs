//! # Prototype Pattern
//!
//! Replaces: **Prototype Pattern** (OOP)
//!
//! Real Rust usage: `Clone` trait (everywhere), `dyn-clone` crate.
//!
//! ## Why this pattern exists in Rust
//! The Prototype pattern allows creating new objects by copying an existing object (the prototype)
//! rather than going through a complex instantiation process via a constructor.
//!
//! In Rust, this pattern **dissolves entirely** into the built-in `Clone` trait for concrete types.
//! However, a fascinating challenge arises when you want to clone *Trait Objects* (`Box<dyn Trait>`).
//! Rust's `Clone` trait is not "object-safe" because it returns `Self` (the specific concrete type),
//! and the size of `Self` isn't known for a trait object.
//!
//! Therefore, the "Rust Prototype Pattern" is specifically about how to clone trait objects.
//!
//! ## Architecture
//!
//! **Approach 1: Concrete Types (Built-in `Clone`)**
//! ```text
//! #[derive(Clone)]
//! struct MyStruct;
//! ```
//!
//! **Approach 2: Trait Objects (The `BoxClone` Pattern)**
//! ```text
//! trait Prototype: BoxClone { ... }
//! trait BoxClone { fn box_clone(&self) -> Box<dyn Prototype>; }
//!
//! [ Box<dyn Prototype> ] --(box_clone)--> [ New Box<dyn Prototype> ]
//! ```
//!
//! **Invariants:**
//! - Concrete cloning should be inexpensive or explicitly document if it allocates (`Clone` vs `Copy`).
//! - Trait object cloning requires a trampoline method (`box_clone`) that returns a `Box<dyn Trait>`.
//!
//! ## When to use
//! - **Concrete Types:** Always use `#[derive(Clone)]` when a type is purely data and cheap to copy, or when explicit copies are expected.
//! - **Trait Objects:** When you have a collection of heterogeneous objects (e.g., `Vec<Box<dyn Node>>`) and need to clone the entire collection.
//!
//! ## Anti-patterns
//! - Manually implementing a `clone_me()` method on a concrete type instead of just deriving `Clone`.
//! - Trying to use `Clone` as a supertrait for a trait object (`trait MyTrait: Clone`). The compiler will reject this.
//!
//! ## Footer
//! The GoF Prototype pattern is mostly obsolete in Rust due to `Clone`. The only time you need to write custom prototype
//! logic is when dealing with dynamic dispatch. The `dyn-clone` crate automates the boilerplate shown in Approach 2.

// ============================================================================
// Approach 1: Concrete Types (The Built-in Prototype)
// ============================================================================

// COMPILE-TIME WIN: Rust can automatically derive the cloning logic for concrete types.
#[derive(Clone, Debug, PartialEq)]
pub struct Monster {
    pub health: u32,
    pub name: String,
    pub attacks: Vec<String>,
}

impl Monster {
    /// A typical use case of Prototype: cloning a base template and tweaking it.
    #[must_use]
    pub fn clone_with_name(&self, new_name: &str) -> Self {
        let mut new_monster = self.clone();
        new_monster.name = new_name.to_string();
        new_monster
    }
}

// ============================================================================
// Approach 2: Trait Objects (The BoxClone Pattern)
// ============================================================================

/// The main trait we want to use dynamically.
/// Notice it requires `BoxCloneShape` as a supertrait.
pub trait Shape: BoxCloneShape + std::fmt::Debug {
    fn area(&self) -> f64;
}

/// The helper trait that makes cloning trait objects possible.
pub trait BoxCloneShape {
    fn box_clone(&self) -> Box<dyn Shape>;
}

// OWNERSHIP INSIGHT: We implement `BoxCloneShape` for any `T` that implements `Shape` and `Clone`.
// This is a blanket implementation, so implementors of `Shape` just need to derive `Clone`.
impl<T> BoxCloneShape for T
where
    T: 'static + Shape + Clone,
{
    fn box_clone(&self) -> Box<dyn Shape> {
        // Here we clone the concrete type `T`, then box it and return it as a trait object.
        Box::new(self.clone())
    }
}

// Finally, we implement `Clone` for `Box<dyn Shape>` using our helper trait.
impl Clone for Box<dyn Shape> {
    fn clone(&self) -> Self {
        // Deref the Box, call the helper trait method on the inner trait object.
        self.box_clone()
    }
}

// --- Concrete Implementations ---

#[derive(Clone, Debug)]
pub struct Circle {
    pub radius: f64,
}

impl Shape for Circle {
    fn area(&self) -> f64 {
        std::f64::consts::PI * self.radius * self.radius
    }
}

#[derive(Clone, Debug)]
pub struct Rectangle {
    pub width: f64,
    pub height: f64,
}

impl Shape for Rectangle {
    fn area(&self) -> f64 {
        self.width * self.height
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_concrete_prototype() {
        let goblin_template = Monster {
            health: 100,
            name: "Goblin".to_string(),
            attacks: vec!["Club".to_string(), "Bite".to_string()],
        };

        // Clone and modify
        let boss_goblin = goblin_template.clone_with_name("Goblin Boss");

        assert_eq!(boss_goblin.health, 100);
        assert_eq!(boss_goblin.name, "Goblin Boss");
        assert_eq!(boss_goblin.attacks.len(), 2);

        // Ensure the original is untouched
        assert_eq!(goblin_template.name, "Goblin");
    }

    #[test]
    fn test_trait_object_prototype() {
        let mut shapes: Vec<Box<dyn Shape>> = Vec::new();
        shapes.push(Box::new(Circle { radius: 2.0 }));
        shapes.push(Box::new(Rectangle {
            width: 3.0,
            height: 4.0,
        }));

        // COMPILE-TIME WIN: We can now clone the entire vector of trait objects!
        // Without the `BoxCloneShape` pattern, `shapes.clone()` would fail to compile.
        let shapes_clone = shapes.clone();

        assert_eq!(shapes.len(), 2);
        assert_eq!(shapes_clone.len(), 2);

        // Verify the data was actually cloned and functions correctly.
        assert!((shapes_clone[0].area() - 12.566).abs() < 0.01);
        assert!((shapes_clone[1].area() - 12.0).abs() < 0.01);
    }
}
