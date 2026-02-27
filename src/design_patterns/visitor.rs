//! # The Visitor / Walker Pattern
//!
//! Replaces: **Double Dispatch** (OOP), **Instance Checks** (Java `instanceof`)
//!
//! Real Rust usage: `syn::visit`, `serde::de::Visitor`, `rustc_ast`
//!
//! ## Why this pattern exists in Rust
//! The "Expression Problem" asks: is it easier to add new types or new operations?
//! - **Enum Dispatch (Closed Set):** Easy to add operations (new `match` function), hard to add types (must update enum and all matches).
//! - **Trait Visitor (Open Set):** Easy to add types (impl Trait), hard to add operations (must update Trait and all impls).
//!
//! Rust defaults to Enums because they are zero-cost and data-oriented. However, when the set of types
//! needs to be extensible by downstream crates (like an AST in a library), the Visitor trait becomes necessary.
//!
//! ## Architecture
//!
//! **Approach 1: Enum Dispatch (The Rust Default)**
//! ```text
//! enum Expr { Literal(i64), Binary(...) }
//! fn eval(e: &Expr) -> i64 { match e { ... } }
//! ```
//!
//! **Approach 2: Visitor Trait (The Extensible Way)**
//! ```text
//! trait Visitor<T> { fn visit_literal(&mut self, i: i64) -> T; ... }
//! trait Accept { fn accept<V: Visitor<T>>(&self, v: &mut V) -> T; }
//! ```

// ============================================================================
// Common Definitions (The AST)
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub enum Operator {
    Add,
    Sub,
    Mul,
    Div,
}

// OWNERSHIP INSIGHT:
// We use Box<Expr> for recursive types because Expr has infinite size otherwise.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Literal(i64),
    Binary {
        left: Box<Expr>,
        op: Operator,
        right: Box<Expr>,
    },
    Grouping(Box<Expr>),
}

// Helper to build ASTs easily in tests
impl Expr {
    pub fn lit(i: i64) -> Self {
        Expr::Literal(i)
    }
    pub fn bin(left: Expr, op: Operator, right: Expr) -> Self {
        Expr::Binary {
            left: Box::new(left),
            op,
            right: Box::new(right),
        }
    }
    pub fn group(expr: Expr) -> Self {
        Expr::Grouping(Box::new(expr))
    }
}

// ============================================================================
// Approach 1: Enum Dispatch (Closed Set)
// ============================================================================

/// The "Functional" style.
///
/// **PROS:**
/// - Zero boilerplate.
/// - Compiler enforces exhaustiveness (you can't forget a variant).
/// - Best performance (branches often optimized better than vtables).
///
/// **CONS:**
/// - If you add a variant to `Expr`, you must update *every* match expression in the codebase.
/// - Logic is coupled to the definition of `Expr`.
pub fn eval_recursive(expr: &Expr) -> i64 {
    match expr {
        Expr::Literal(val) => *val,
        Expr::Binary { left, op, right } => {
            let l = eval_recursive(left);
            let r = eval_recursive(right);
            match op {
                Operator::Add => l + r,
                Operator::Sub => l - r,
                Operator::Mul => l * r,
                Operator::Div => l / r, // Panics on zero, simpler for demo
            }
        }
        Expr::Grouping(inner) => eval_recursive(inner),
    }
}

// ============================================================================
// Approach 2: Visitor Trait (Open Set / Double Dispatch)
// ============================================================================

/// The "OOP" style (Double Dispatch).
///
/// **PROS:**
/// - Decouples algorithm (Visitor) from data (Expr).
/// - You can add new Visitors (operations) without recompiling `Expr`.
///
/// **CONS:**
/// - Boilerplate (Accept traits, Visitor traits).
/// - Hard to add new `Expr` variants (breaks the Visitor trait).
/// - Slightly slower due to function call overhead (though generic monomorphization helps).

// The Visitor Interface
// We use a generic return type `R` so visitors can return values.
pub trait Visitor<R> {
    fn visit_literal(&mut self, value: i64) -> R;
    fn visit_binary(&mut self, left: &Expr, op: &Operator, right: &Expr) -> R;
    fn visit_grouping(&mut self, inner: &Expr) -> R;
}

// The Accept Interface
// This enables the "Double Dispatch": Expr calls v.visit_...(self)
pub trait Accept {
    fn accept<R, V: Visitor<R>>(&self, visitor: &mut V) -> R;
}

impl Accept for Expr {
    fn accept<R, V: Visitor<R>>(&self, visitor: &mut V) -> R {
        match self {
            Expr::Literal(v) => visitor.visit_literal(*v),
            Expr::Binary { left, op, right } => visitor.visit_binary(left, op, right),
            Expr::Grouping(inner) => visitor.visit_grouping(inner),
        }
    }
}

// Example Visitor 1: AST Printer (Lisp style)
pub struct AstPrinter;

impl Visitor<String> for AstPrinter {
    fn visit_literal(&mut self, value: i64) -> String {
        value.to_string()
    }

    fn visit_binary(&mut self, left: &Expr, op: &Operator, right: &Expr) -> String {
        let op_str = match op {
            Operator::Add => "+",
            Operator::Sub => "-",
            Operator::Mul => "*",
            Operator::Div => "/",
        };
        let l = left.accept(self);
        let r = right.accept(self);
        format!("({} {} {})", op_str, l, r)
    }

    fn visit_grouping(&mut self, inner: &Expr) -> String {
        // Just print expression, maybe wrap in parens if we wanted
        // explicit grouping, but let's keep it simple.
        format!("(group {})", inner.accept(self))
    }
}

// Example Visitor 2: Evaluator (Re-implementing eval as a Visitor)
pub struct EvalVisitor;

impl Visitor<i64> for EvalVisitor {
    fn visit_literal(&mut self, value: i64) -> i64 {
        value
    }

    fn visit_binary(&mut self, left: &Expr, op: &Operator, right: &Expr) -> i64 {
        let l = left.accept(self);
        let r = right.accept(self);
        match op {
            Operator::Add => l + r,
            Operator::Sub => l - r,
            Operator::Mul => l * r,
            Operator::Div => l / r,
        }
    }

    fn visit_grouping(&mut self, inner: &Expr) -> i64 {
        inner.accept(self)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create (1 + 2) * 3
    fn make_ast() -> Expr {
        Expr::bin(
            Expr::group(Expr::bin(Expr::lit(1), Operator::Add, Expr::lit(2))),
            Operator::Mul,
            Expr::lit(3),
        )
    }

    #[test]
    fn test_enum_dispatch_eval() {
        let expr = make_ast();
        // (1 + 2) * 3 = 9
        assert_eq!(eval_recursive(&expr), 9);
    }

    #[test]
    fn test_visitor_eval() {
        let expr = make_ast();
        let mut evaluator = EvalVisitor;
        assert_eq!(expr.accept(&mut evaluator), 9);
    }

    #[test]
    fn test_visitor_printer() {
        let expr = make_ast();
        let mut printer = AstPrinter;
        // Expected: (* (group (+ 1 2)) 3)
        // Logic:
        // Outermost is Mul
        // Left is Grouping -> Inner is Add -> (+ 1 2) -> (group (+ 1 2))
        // Right is 3
        // Result: (* (group (+ 1 2)) 3)
        assert_eq!(expr.accept(&mut printer), "(* (group (+ 1 2)) 3)");
    }
}
