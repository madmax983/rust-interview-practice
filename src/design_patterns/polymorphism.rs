//! # Polymorphism: Trait Objects vs. Enum Dispatch
//!
//! Replaces: **Strategy Pattern** (OOP), **Command Pattern** (OOP)
//!
//! Real Rust usage: `serde_json::Value` (Enum), `std::io::Read` (Trait Object), `axum::response::IntoResponse`
//!
//! ## Why this pattern exists in Rust
//! Rust offers two distinct ways to achieve polymorphism:
//! 1. **Enum Dispatch (Static):** A closed set of types (Sum Type). Calls are resolved at compile time (inlined).
//! 2. **Trait Objects (Dynamic):** An open set of types. Calls are resolved at runtime via a vtable (virtual method table).
//!
//! ## Architecture
//!
//! **Enum Dispatch:**
//! ```text
//! enum Shape { Circle(f64), Rect(f64, f64) }
//! // Memory: Tag + Max(Size(Variants))
//! // Dispatch: match self { ... }
//! ```
//!
//! **Trait Object:**
//! ```text
//! struct Circle; impl Shape for Circle;
//! let s: Box<dyn Shape> = Box::new(Circle);
//! // Memory: Pointer to Data + Pointer to VTable
//! // Dispatch: s.vtable.area(s.data)
//! ```
//!
//! ## When to use
//! - **Enum:** When you know all types at compile time. Faster (CPU branch prediction friendly), better cache locality, inlineable.
//! - **Trait Object:** When you need extensibility (plugins, user-defined types) or to reduce compile times (type erasure).

// ============================================================================
// Approach 1: Enum Dispatch (Closed Set)
// ============================================================================

pub enum ShapeEnum {
    Circle { radius: f64 },
    Rectangle { width: f64, height: f64 },
}

impl ShapeEnum {
    pub fn area(&self) -> f64 {
        match self {
            ShapeEnum::Circle { radius } => std::f64::consts::PI * radius * radius,
            ShapeEnum::Rectangle { width, height } => width * height,
        }
    }
}

// ============================================================================
// Approach 2: Trait Objects (Open Set)
// ============================================================================

pub trait ShapeTrait {
    fn area(&self) -> f64;
}

pub struct Circle {
    pub radius: f64,
}

impl ShapeTrait for Circle {
    fn area(&self) -> f64 {
        std::f64::consts::PI * self.radius * self.radius
    }
}

pub struct Rectangle {
    pub width: f64,
    pub height: f64,
}

impl ShapeTrait for Rectangle {
    fn area(&self) -> f64 {
        self.width * self.height
    }
}

// ============================================================================
// Benchmarking / Performance Comparison
// ============================================================================

// OWNERSHIP INSIGHT:
// - Vec<ShapeEnum>: Contiguous memory, cache-friendly. Each element size = sizeof(largest variant) + tag.
// - Vec<Box<dyn ShapeTrait>>: Vector of pointers. Double indirection (Vec -> Box -> Data). Cache misses likely.

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn test_enum_dispatch() {
        let shapes = vec![
            ShapeEnum::Circle { radius: 1.0 },
            ShapeEnum::Rectangle {
                width: 2.0,
                height: 3.0,
            },
        ];

        let total_area: f64 = shapes.iter().map(|s| s.area()).sum();
        assert!((total_area - (std::f64::consts::PI + 6.0)).abs() < 1e-6);
    }

    #[test]
    fn test_trait_dispatch() {
        let shapes: Vec<Box<dyn ShapeTrait>> = vec![
            Box::new(Circle { radius: 1.0 }),
            Box::new(Rectangle {
                width: 2.0,
                height: 3.0,
            }),
        ];

        let total_area: f64 = shapes.iter().map(|s| s.area()).sum();
        assert!((total_area - (std::f64::consts::PI + 6.0)).abs() < 1e-6);
    }

    // Performance Note: Run with `cargo test --release` to see real difference.
    // In debug mode, overhead masks the difference.
    //
    // Typical results: Enum dispatch is 2-5x faster for simple methods like `area` due to inlining.
}
