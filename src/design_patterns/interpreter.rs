//! # Interpreter Pattern
//!
//! Replaces: **Interpreter Pattern** (OOP / `GoF`)
//!
//! Real Rust usage: `syn` (parsing Rust code), `regex` (interpreting regex AST), `serde_json` (evaluating JSON values).
//!
//! ## Why this pattern exists in Rust
//! In traditional OOP, the Interpreter pattern evaluates sentences in a language by
//! representing the grammar as a class hierarchy (e.g., `AbstractExpression`, `TerminalExpression`,
//! `NonterminalExpression`) and using dynamic dispatch (`interpret()` methods) to evaluate them.
//!
//! In Rust, this pattern **dissolves entirely** into the language's core features:
//! 1. **Enums (Sum Types):** Used to define the Abstract Syntax Tree (AST) concisely.
//! 2. **Pattern Matching:** Used to recursively evaluate the AST with exhaustive compiler checks.
//!
//! Rust's approach avoids the heap allocations and vtable overhead of the OOP approach,
//! while providing stronger compile-time guarantees (you cannot forget to handle a new expression type).
//!
//! ## Architecture
//!
//! **Approach 1: Idiomatic Enum-based AST (Zero-cost abstractions)**
//! ```text
//! [ Client ] --> (builds AST with Enum variants)
//!                      |
//!          [ evaluate(AST) via exhaustive match ]
//! ```
//!
//! **Approach 2: Trait Objects (The OOP translation - Anti-Pattern for closed ASTs)**
//! ```text
//! [ Client ] --> (builds Box<dyn Expression> tree)
//!                      |
//!         [ dynamic dispatch of .interpret() ]
//! ```
//!
//! **Invariants:**
//! - The AST represents a closed, well-defined grammar.
//! - Evaluation is a pure function (or takes explicit mutable context).
//! - All possible grammar states must be handled (enforced by the compiler).
//!
//! ## When to use
//! - When you have a simple language or domain-specific language (DSL) to evaluate.
//! - When the grammar is relatively stable and closed (perfect for enums).
//!
//! ## Anti-patterns
//! - Using `Box<dyn Expression>` for simple, closed ASTs. It scatters the evaluation logic across multiple files
//!   and incurs heavy heap allocation and dynamic dispatch costs.

use std::collections::HashMap;

// ============================================================================
// Approach 1: Idiomatic Rust (Enums + Pattern Matching)
// ============================================================================

// COMPILE-TIME WIN: Enums form a closed set. The compiler knows the exact size
// (with indirection via Box for recursive types) and guarantees exhaustive matching.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    // Terminal expressions
    Number(i32),
    Variable(String),

    // Non-terminal expressions
    // OWNERSHIP INSIGHT: Recursive enum variants must be boxed because Rust
    // needs to know the exact size of the enum at compile time.
    Add(Box<Self>, Box<Self>),
    Subtract(Box<Self>, Box<Self>),
}

/// The context holds the state needed during interpretation, such as variable bindings.
pub type Context = HashMap<String, i32>;

impl Expr {
    /// Evaluates the expression tree recursively.
    #[must_use]
    pub fn evaluate(&self, context: &Context) -> Option<i32> {
        match self {
            Self::Number(n) => Some(*n),
            Self::Variable(name) => context.get(name).copied(),
            Self::Add(left, right) => {
                let l = left.evaluate(context)?;
                let r = right.evaluate(context)?;
                // Safe arithmetic
                l.checked_add(r)
            }
            Self::Subtract(left, right) => {
                let l = left.evaluate(context)?;
                let r = right.evaluate(context)?;
                l.checked_sub(r)
            }
        }
    }
}

// ============================================================================
// Approach 2: Trait Objects (The OOP translation)
// ============================================================================

// ANTI-PATTERN: This is how you'd write it in Java. In Rust, it forces everything
// onto the heap and uses dynamic dispatch for every single node in the AST.
// It also suffers from the Expression Problem: adding new operations (like `print`)
// requires modifying the trait and every implementor.

pub trait Expression {
    fn interpret(&self, context: &Context) -> Option<i32>;
}

pub struct NumExpr(i32);

impl NumExpr {
    #[must_use]
    pub const fn new(val: i32) -> Self {
        Self(val)
    }
}

impl Expression for NumExpr {
    fn interpret(&self, _context: &Context) -> Option<i32> {
        Some(self.0)
    }
}

pub struct AddExpr {
    // TRADEOFF: We must use Box<dyn Expression> because we don't know the exact
    // type of the children at compile time. This means heap allocation.
    left: Box<dyn Expression>,
    right: Box<dyn Expression>,
}

impl AddExpr {
    #[must_use]
    pub fn new(left: Box<dyn Expression>, right: Box<dyn Expression>) -> Self {
        Self { left, right }
    }
}

impl Expression for AddExpr {
    fn interpret(&self, context: &Context) -> Option<i32> {
        let l = self.left.interpret(context)?;
        let r = self.right.interpret(context)?;
        l.checked_add(r)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_idiomatic_interpreter() {
        let mut context = Context::new();
        context.insert("x".to_string(), 10);
        context.insert("y".to_string(), 5);

        // AST: (x + 20) - y
        let ast = Expr::Subtract(
            Box::new(Expr::Add(
                Box::new(Expr::Variable("x".to_string())),
                Box::new(Expr::Number(20)),
            )),
            Box::new(Expr::Variable("y".to_string())),
        );

        let result = ast.evaluate(&context);
        assert_eq!(result, Some(25)); // (10 + 20) - 5 = 25
    }

    #[test]
    fn test_missing_variable() {
        let context = Context::new(); // Empty context

        // AST: z + 5
        let ast = Expr::Add(
            Box::new(Expr::Variable("z".to_string())),
            Box::new(Expr::Number(5)),
        );

        // Should cleanly return None due to the ? operator in evaluate
        let result = ast.evaluate(&context);
        assert_eq!(result, None);
    }

    #[test]
    fn test_trait_object_interpreter() {
        let context = Context::new();

        // AST: 10 + 20
        // Notice the boilerplate required to box everything manually.
        let ast = AddExpr::new(Box::new(NumExpr::new(10)), Box::new(NumExpr::new(20)));

        let result = ast.interpret(&context);
        assert_eq!(result, Some(30));
    }
}

// ============================================================================
// Footer
// ============================================================================
//
// How this pattern appears in std/crates:
// - `syn`: Uses enums extensively to represent the Rust AST during procedural macro execution.
// - `regex_syntax`: Uses enums to represent the parsed regex AST (Hir).
//
// What the GoF/OOP equivalent is and why it doesn't translate directly:
// In OOP, the pattern centers around an interface `Expression` with an `interpret` method.
// Each node type is a class. In Rust, we invert this: the data is an Enum, and the behavior
// is a single recursive function. This is because Rust is expression-oriented and its `match`
// statement is immensely powerful.
//
// When to reach for this vs. simpler alternatives:
// Reach for the Enum approach immediately when you need to represent a tree-like grammar or DSL.
// Reach for the Trait Object approach *only* if the grammar needs to be extensible by downstream
// crates (i.e., users of your library need to add their own AST node types), though even then,
// the Visitor pattern or a Plugin architecture is usually better.
//
// Suggested combinations with other patterns in this collection:
// - **Visitor Pattern**: Often used to traverse the AST to perform different operations (e.g., compile, format, evaluate) without modifying the AST types.
// - **Composite Pattern**: The AST is fundamentally a Composite structure.
//
// PRODUCTION NOTE:
// For very large, deeply recursive ASTs, the call stack might overflow during evaluation.
// In production interpreters, you either need a technique like Trampolining or an explicit
// evaluation stack (`Vec`) to avoid recursion.
//
// GOTCHA:
// Forgetting to box recursive enum variants leads to the "recursive type has infinite size"
// compiler error. Always `Box` the children!
//
// META-PATTERN: "Make illegal states unrepresentable"
// By using strong typing (`Option<i32>`) and exhaustive pattern matching, we ensure that
// evaluating a missing variable or an unsupported operation is caught at compile time
// (by forcing us to handle the `None` case) rather than throwing a runtime `NullPointerException`.
