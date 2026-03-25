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
//! ## When to use
//! - **Enum Dispatch:** When the set of types is fixed (e.g., your own AST). This is 99% of cases.
//! - **Visitor Trait:** When you need to decouple the algorithm from the data structure, or when the set of operations is open-ended
//!   (e.g., a linter where users can add new checks without recompiling the AST).
//!
//! ## Anti-patterns
//! - Forcing the Visitor trait pattern when a simple `match` statement on an enum would suffice.

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
    // ANTI-PATTERN: Using a full Visitor trait when a simple match on a closed enum works perfectly.
    // OWNERSHIP INSIGHT: We borrow self recursively (`&self`).
    // COMPILE-TIME WIN: The compiler ensures we cover all variants in the `match` block exhaustively.
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

// The "True" Visitor pattern separates the types.
// This decouples the operation from the structure.

// TRADEOFF: The Visitor trait decouples logic from data, but at the cost of significantly
// more boilerplate and indirection compared to standard enum pattern matching.
pub trait AstVisitor {
    fn visit_literal(&mut self, value: i32);
    fn visit_add(&mut self, lhs: &Expr, rhs: &Expr);
    fn visit_mul(&mut self, lhs: &Expr, rhs: &Expr);
}

// Dispatcher logic
impl Expr {
    // PRODUCTION NOTE: In complex systems (like `syn`), `accept` often takes `&mut self` or consumes `self`,
    // and returns a `Result` to allow for fallible traversal.
    pub fn accept(&self, visitor: &mut impl AstVisitor) {
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
        // GOTCHA: Stateful visitors require careful management of intermediate results.
        // Overwriting `self.result` recursively means we must extract it immediately after returning.

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
// How this pattern appears in std/major crates:
// - **Serde:** `serde::de::Visitor` for deserializing arbitrary data formats.
// - **Syn/Rustc:** `syn::visit::Visit` and `rustc_ast::visit::Visitor` for AST traversal.
//
// What the GoF/OOP equivalent is and why it doesn't translate directly:
// The OOP Visitor pattern exists to implement "double dispatch" when classes form an inheritance hierarchy.
// Rust natively solves this for closed sets using Enums and Pattern Matching, which is the preferred approach
// for 99% of use cases.
//
// When to reach for this vs simpler alternatives:
// - Always default to Enum + `match` (Approach 1).
// - Reach for the Visitor Trait (Approach 2) only when the data types are an open set, or when
//   you are building a framework (like Serde) where users need to provide custom traversal logic.
