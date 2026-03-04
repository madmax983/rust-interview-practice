//! # Adapter Pattern
//!
//! Replaces: **Adapter Pattern** (OOP / GoF)
//!
//! Real Rust usage: `std::io::BufReader` (adapts `Read`), `std::iter::Map` (adapts `Iterator`), Serde serialization for foreign types.
//!
//! ## Why this pattern exists in Rust
//! In OOP, the Adapter pattern is used to make incompatible interfaces work together, often by subclassing
//! or wrapping objects in a new class hierarchy. In Rust, we rely on Traits rather than class inheritance.
//!
//! Rust handles adaptation in three primary ways:
//! 1. **Wrapper Pattern:** A new struct that contains the adaptee and implements the target trait.
//! 2. **Trait Impl for Foreign Types:** Implementing a local trait for a foreign type directly, effectively acting as an invisible adapter.
//! 3. **Transparent Wrappers (Newtype):** Using a tuple struct to bypass the Orphan Rule when you need to implement a foreign trait for a foreign type.
//!
//! ## Architecture
//!
//! **Approach 1: Wrapper Pattern**
//! ```text
//! [ Client ] --> (Target Trait) --> [ Adapter Struct ] --(owns/borrows)--> [ Adaptee ]
//! ```
//!
//! **Approach 2: Trait Impl for Foreign Types**
//! ```text
//! [ Client ] --> (Target Trait) --> [ Foreign Type (Adaptee) directly ]
//! ```
//!
//! **Approach 3: Transparent Wrappers (Newtype)**
//! ```text
//! [ Client ] --> (Foreign Trait) --> [ Wrapper(ForeignType) ]
//! ```
//!
//! **Invariants:**
//! - Adapters should be lightweight and ideally have zero runtime overhead (Zero-Cost Abstraction).
//! - The target interface is explicitly defined via a Trait.
//!
//! ## When to use
//! - To make a third-party library compatible with your application's trait interfaces.
//! - To implement an external trait (like `std::fmt::Display` or `serde::Serialize`) for a type you don't own, using a Newtype wrapper to satisfy the Orphan Rule.
//!
//! ## Anti-patterns
//! - **Deep Adapter Hierarchies:** Creating deeply nested wrapper types instead of just implementing the needed traits directly where possible.
//! - **Runtime Boxing:** Using `Box<dyn TargetTrait>` when a statically dispatched generic adapter struct would suffice, sacrificing performance.

// ============================================================================
// The Target Interface (Our Domain)
// ============================================================================

pub trait TargetShape {
    fn bounding_box_width(&self) -> f64;
    fn bounding_box_height(&self) -> f64;
}

// ============================================================================
// Approach 1: Wrapper Pattern
// ============================================================================

// An external "foreign" type we don't control
#[derive(Debug, Clone)]
pub struct LegacyRectangle {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

impl LegacyRectangle {
    #[must_use]
    pub const fn new(left: f64, top: f64, right: f64, bottom: f64) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }
}

// OWNERSHIP INSIGHT: The adapter takes ownership (or a borrow) of the adaptee.
// TRADEOFF: Requires explicitly spelling out an adapter struct.
pub struct LegacyRectangleAdapter {
    adaptee: LegacyRectangle,
}

impl LegacyRectangleAdapter {
    #[must_use]
    pub const fn new(adaptee: LegacyRectangle) -> Self {
        Self { adaptee }
    }
}

// COMPILE-TIME WIN: The compiler will inline these calls, making the adapter zero-cost.
impl TargetShape for LegacyRectangleAdapter {
    fn bounding_box_width(&self) -> f64 {
        self.adaptee.right - self.adaptee.left
    }

    fn bounding_box_height(&self) -> f64 {
        self.adaptee.bottom - self.adaptee.top
    }
}

// ============================================================================
// Approach 2: Trait Impl for Foreign Types (The Invisible Adapter)
// ============================================================================

// A different external type
#[derive(Debug, Clone)]
pub struct ForeignCircle {
    pub radius: f64,
}

impl ForeignCircle {
    #[must_use]
    pub const fn new(radius: f64) -> Self {
        Self { radius }
    }
}

// ANTI-PATTERN in OOP: To adapt a foreign class, you *must* wrap it.
// IN RUST: If we own the Target Trait, we can just implement it directly for the foreign type.
// This is the cleanest, most idiomatic way to adapt types in Rust.
impl TargetShape for ForeignCircle {
    fn bounding_box_width(&self) -> f64 {
        self.radius * 2.0
    }

    fn bounding_box_height(&self) -> f64 {
        self.radius * 2.0
    }
}

// ============================================================================
// Approach 3: Transparent Wrappers (Newtype / Bypassing the Orphan Rule)
// ============================================================================

// Sometimes we want to implement a Foreign Trait (like `std::fmt::Display`)
// for a Foreign Type (like `Vec<T>`).
// The "Orphan Rule" strictly forbids this: we must own either the Trait or the Type.
// The "Newtype" transparent wrapper is the idiomatic adapter for this problem.

// FOREIGN TYPE: Vec<i32>
// FOREIGN TRAIT: std::fmt::Display

// Wrapper struct that holds the foreign type
pub struct DisplayableVec<'a>(pub &'a [i32]);

// Now we own `DisplayableVec`, so we can implement the foreign trait `Display` for it!
impl<'a> std::fmt::Display for DisplayableVec<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[")?;
        for (i, val) in self.0.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", val)?;
        }
        write!(f, "]")
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrapper_adapter() {
        let legacy_rect = LegacyRectangle::new(0.0, 10.0, 20.0, 50.0);

        // Wrap the legacy struct in the adapter
        let adapter = LegacyRectangleAdapter::new(legacy_rect);

        // Treat it as a TargetShape
        assert_eq!(adapter.bounding_box_width(), 20.0);
        assert_eq!(adapter.bounding_box_height(), 40.0);
    }

    #[test]
    fn test_invisible_adapter() {
        let circle = ForeignCircle::new(15.0);

        // No wrapper needed! The trait is implemented directly on the foreign type.
        assert_eq!(circle.bounding_box_width(), 30.0);
        assert_eq!(circle.bounding_box_height(), 30.0);

        // We can place both in a heterogeneous collection using trait objects
        let legacy_rect = LegacyRectangle::new(0.0, 10.0, 20.0, 50.0);
        let adapter = LegacyRectangleAdapter::new(legacy_rect);

        let shapes: Vec<&dyn TargetShape> = vec![&adapter, &circle];

        let mut total_width = 0.0;
        for shape in shapes {
            total_width += shape.bounding_box_width();
        }

        assert_eq!(total_width, 50.0);
    }

    #[test]
    fn test_transparent_wrapper() {
        let raw_vec = vec![1, 2, 3, 4, 5];

        // Adapt the vector to implement Display
        let adapter = DisplayableVec(&raw_vec);

        let formatted = format!("{}", adapter);
        assert_eq!(formatted, "[1, 2, 3, 4, 5]");
    }
}

// ============================================================================
// Footer
// ============================================================================
//
// How this pattern appears in std/crates:
// - `std::iter::Map`: A generic struct that takes an iterator and a closure and implements `Iterator` itself, acting as a transparent adapter.
// - `std::io::BufReader`: Takes a type implementing `Read` and provides `BufRead` capabilities.
// - `serde`: Serialization traits are implemented on foreign types using the Newtype pattern (wrapper).
//
// What the GoF/OOP equivalent is and why it doesn't translate directly:
// In OOP, you'd define a Target Interface, create an Adapter Class that implements that interface,
// and have it compose the Adaptee Class. Deep hierarchies and heavy object composition are the norm.
// In Rust, because we have traits, we can skip the wrapper class entirely and just `impl TargetTrait for Adaptee`
// (assuming we own `TargetTrait`). The pattern "dissolves" into the language features.
//
// When to reach for this vs. simpler alternatives:
// Always default to Direct Trait Implementation first. It is the "invisible adapter" and requires zero boilerplate.
// Reach for the Wrapper (Newtype) pattern *only* when the compiler prevents you via the Orphan Rule.
//
// Suggested combinations with other patterns in this collection:
// - **Builder Pattern**: If the adaptation process is complex, use a Builder to construct the adapted type.
// - **Facade Pattern**: If the Adaptee exposes a huge API and you want to adapt it to a smaller, specific subset.
//
// PRODUCTION NOTE:
// In enterprise codebases, Newtypes are heavily used to differentiate between structurally identical types
// (e.g., `struct UserId(u64)` vs `struct OrderId(u64)`) and implementing traits specific to that domain,
// which is fundamentally an application of the transparent wrapper adapter.
//
// GOTCHA:
// When creating a wrapper struct `struct Wrapper<'a>(&'a ForeignType)`, ensure you manage lifetimes properly.
// Often, it's better to take ownership `struct Wrapper(ForeignType)` if you don't need the original object afterwards,
// avoiding lifetime soup in your trait implementations.

// META-PATTERN: "Make illegal states unrepresentable"
// By implementing traits directly (or using newtype wrappers), we encode capabilities in the type system.
// A type simply *cannot* be passed to a function requiring `TargetShape` if the adapter hasn't been applied or implemented,
// catching the error at compile time rather than relying on runtime `instanceof` checks common in some languages.
