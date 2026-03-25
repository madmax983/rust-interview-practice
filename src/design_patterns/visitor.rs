// Suppress pedantic and nursery lints for design pattern examples.
#![allow(clippy::pedantic, clippy::nursery, unused)]
//! # Visitor Pattern (Rust-Style)
//!
//! Replaces: **Double Dispatch** (OOP), **Pattern Matching** (Functional)
//!
//! Real Rust usage: `serde::de::Visitor`, `syn::visit::Visit`, `rustc_ast::visit::Visitor`
//!
//! ## Why this pattern exists in Rust
//! In OOP, the Visitor pattern solves the "double dispatch" problem (choosing a function based on two types:
//! the element and the visitor). In Rust, we have two primary ways to traverse structures:
//! 1. **Enum Dispatch (Idiomatic):** A closed set of types (Sum Type) matched exhaustively. Fast, simple, inlineable.
//! 2. **Visitor Trait (Open Set):** An open set of types where the visitor is generic. Essential for serialization (Serde)
//!    where you don't know the data format beforehand.
//!
//! ## Architecture
//!
//! **Approach 1: Enum Dispatch (Closed Set)**
//! ```text
//! enum Expr { Lit(i32), Add(Box<Expr>, Box<Expr>) }
//! impl Expr {
//!     fn eval(&self) -> i32 { match self { ... } }
//! }
//! ```
//!
//! **Approach 2: Visitor Trait (Open Set / Double Dispatch)**
//! ```text
//! trait AstVisitor {
//!     fn visit_literal(&mut self, val: i32);
//!     fn visit_add(&mut self, lhs: &Expr, rhs: &Expr);
//! }
//!
//! impl Expr {
//!     fn accept(&self, visitor: &mut impl AstVisitor) { ... }
//! }
//! ```
//!
//! **Invariants:**
//! - Extensibility is orthogonal. Enums allow adding new operations easily but modifying types is hard. Visitor allows adding new operations easily but adding new types requires updating all visitors.
//!
//! ## When to use
//! - **Enum Dispatch:** When the set of types is fixed (e.g., your own AST). This is 99% of cases.
//! - **Visitor Trait:** When you need to decouple the algorithm from the data structure, or when the set of operations is open-ended
//!   (e.g., a linter where users can add new checks without recompiling the AST).
//!
//! ## Anti-patterns
//! - Using the Visitor Trait when the data structure is fully controlled by your application and won't change independently. Instead, just use Enum Dispatch.

// ============================================================================
// The Data Structure (AST)
// ============================================================================

#[derive(Debug)]
pub enum Expr {
    Literal(i32),
    // Recursive structure using Box
    Add(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
}

// ============================================================================
// Approach 1: Pattern Matching (Internal Visitor)
// ============================================================================

impl Expr {
    // OWNERSHIP INSIGHT: We borrow self recursively.
    // The compiler ensures we cover all variants.
    pub fn eval(&self) -> i32 {
        match self {
            Expr::Literal(val) => *val,
            Expr::Add(lhs, rhs) => lhs.eval() + rhs.eval(),
            Expr::Mul(lhs, rhs) => lhs.eval() * rhs.eval(),
        }
    }
}

// ============================================================================
// Approach 2: Visitor Trait (External Visitor / Double Dispatch)
// ============================================================================

// ANTI-PATTERN: Defaulting to this complex trait approach when Enum dispatch would suffice.
// COMPILE-TIME WIN: Adding a new variant to `Expr` forces updates across all `AstVisitor` implementations.

// The "True" Visitor pattern separates the types.
// This decouples the operation from the structure.

pub trait AstVisitor {
    fn visit_literal(&mut self, value: i32);
    fn visit_add(&mut self, lhs: &Expr, rhs: &Expr);
    fn visit_mul(&mut self, lhs: &Expr, rhs: &Expr);
}

// Dispatcher logic
impl Expr {
    pub fn accept(&self, visitor: &mut impl AstVisitor) {
        // TRADEOFF: Visitor relies heavily on recursive double dispatch, which is often slower than
        // direct enum dispatch due to more virtual calls or monomorphization bloat.
        // GOTCHA: Don't forget that implementing the double dispatch can overflow the stack for deep trees.
        match self {
            Expr::Literal(val) => visitor.visit_literal(*val),
            Expr::Add(lhs, rhs) => visitor.visit_add(lhs, rhs),
            Expr::Mul(lhs, rhs) => visitor.visit_mul(lhs, rhs),
        }
    }
}

// Concrete Visitor: Evaluator (Stateful)
pub struct EvaluatorVisitor {
    result: i32,
}

impl EvaluatorVisitor {
    pub fn new() -> Self {
        EvaluatorVisitor { result: 0 }
    }
}

impl Default for EvaluatorVisitor {
    fn default() -> Self {
        Self::new()
    }
}

impl AstVisitor for EvaluatorVisitor {
    fn visit_literal(&mut self, value: i32) {
        self.result = value;
    }

    fn visit_add(&mut self, lhs: &Expr, rhs: &Expr) {
        // PRODUCTION NOTE: Often visitors will return `Result` or `ControlFlow` rather than mutating `self` directly.
        // Double dispatch:
        // 1. Visit left child (updates self.result)
        lhs.accept(self);
        let left = self.result;

        // 2. Visit right child (updates self.result)
        rhs.accept(self);
        let right = self.result;

        // 3. Combine
        self.result = left + right;
    }

    fn visit_mul(&mut self, lhs: &Expr, rhs: &Expr) {
        lhs.accept(self);
        let left = self.result;
        rhs.accept(self);
        let right = self.result;
        self.result = left * right;
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enum_dispatch() {
        // (1 + 2) * 3
        let ast = Expr::Mul(
            Box::new(Expr::Add(
                Box::new(Expr::Literal(1)),
                Box::new(Expr::Literal(2)),
            )),
            Box::new(Expr::Literal(3)),
        );

        assert_eq!(ast.eval(), 9);
    }

    #[test]
    fn test_visitor_pattern() {
        // (1 + 2) * 3
        let ast = Expr::Mul(
            Box::new(Expr::Add(
                Box::new(Expr::Literal(1)),
                Box::new(Expr::Literal(2)),
            )),
            Box::new(Expr::Literal(3)),
        );

        let mut visitor = EvaluatorVisitor::new();
        ast.accept(&mut visitor);

        assert_eq!(visitor.result, 9);
    }
}

// ============================================================================
// Footer
// ============================================================================
//
// How this pattern appears in std/crates:
// - `serde::de::Visitor`: Decouples the deserialization format from the data type.
// - `syn::visit::Visit`: Allows traversing Rust ASTs without matching every node type.
//
// What the GoF/OOP equivalent is and why it doesn't translate directly:
// In OOP, Visitor avoids runtime type checking via double dispatch. In Rust, enum
// dispatch (pattern matching) natively solves this exact problem with better performance
// and safety, so the actual Visitor pattern is reserved strictly for open sets.
//
// When to reach for this vs simpler alternatives:
// Always default to Enum Dispatch (pattern matching). Only reach for Visitor
// when you are writing a library and users must be able to add operations over your data.
//
// META-PATTERN: "Make illegal states unrepresentable"
// Enum dispatch naturally enforces exhaustive coverage checking, turning unhandled
// data structure variants into compile-time errors.
