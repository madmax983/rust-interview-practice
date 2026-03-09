// Suppress pedantic and nursery lints for design pattern examples.
#![allow(clippy::pedantic, clippy::nursery, unused)]
//! # Strategy Pattern
//!
//! Replaces: **Strategy Pattern** (OOP)
//!
//! Real Rust usage: `Iterator::map` (closures as strategy), `std::hash::BuildHasher` (traits as strategy)
//!
//! ## Why this pattern exists in Rust
//! In classic OOP, the Strategy pattern defines a family of algorithms, encapsulates each one in a class,
//! and makes them interchangeable at runtime.
//!
//! In Rust, this pattern is incredibly common but rarely uses the heavy "interface + class" boilerplate.
//! Instead, it naturally dissolves into two primary forms:
//! 1. **Closures (Functions as Data):** For simple strategies, higher-order functions taking `Fn`, `FnMut`, or `FnOnce`
//!    are the most idiomatic.
//! 2. **Traits (Static or Dynamic Dispatch):** For complex strategies involving state or multiple methods,
//!    we define a trait. We can then accept it generically (Static Dispatch / Monomorphization) for zero-cost
//!    performance, or via Trait Objects (`Box<dyn Strategy>`) if we need to swap strategies at runtime.
//!
//! ## Architecture
//!
//! **Approach 1: Closures (Functional Strategy)**
//! ```text
//! [ Processor ] --(calls)--> [ Closure: fn(data) -> result ]
//! ```
//!
//! **Approach 2: Traits with Static Dispatch (Zero-Cost Strategy)**
//! ```text
//! [ Processor<S: Strategy> ] --(calls)--> [ S::execute() ]
//! ```
//!
//! **Invariants:**
//! - The strategy encapsulates the varying behavior.
//! - The context (Processor) remains unaware of the strategy's internal implementation.
//! - Static dispatch (Generics) locks the strategy at compile time but runs at native speed.
//!
//! ## When to use
//! - **Closures:** When the strategy is a single, simple function.
//! - **Static Dispatch (Generics):** When performance is critical and strategies don't change at runtime.
//! - **Dynamic Dispatch (`Box<dyn Trait>`):** When you need to swap strategies at runtime or store a heterogeneous collection of them.
//!
//! ## Anti-patterns
//! - Forcing the OOP approach: Creating a `trait Strategy` and implementing it for empty structs
//!   when a simple closure would suffice.

// ============================================================================
// Approach 1: Closures (The Functional Way)
// ============================================================================

/// A context that uses a closure as its strategy.
pub struct DataProcessor {
    data: Vec<i32>,
}

impl DataProcessor {
    #[must_use]
    pub fn new(data: Vec<i32>) -> Self {
        Self { data }
    }

    /// Process the data using the provided closure strategy.
    ///
    /// OWNERSHIP INSIGHT: We take `&mut self` to mutate `data`, and `F` as `FnMut`
    /// in case the strategy itself needs to maintain state.
    pub fn process<F>(&mut self, mut strategy: F)
    where
        F: FnMut(&mut i32),
    {
        for item in &mut self.data {
            strategy(item);
        }
    }
}

// ============================================================================
// Approach 2: Traits (Static Dispatch / Monomorphization)
// ============================================================================

/// The Strategy Trait for more complex behaviors.
pub trait PaymentStrategy {
    fn pay(&self, amount: u32) -> String;
}

// COMPILE-TIME WIN: By using generics, the compiler generates a specific version
// of `Checkout` for each strategy used. There is no virtual method call overhead.
pub struct Checkout<S: PaymentStrategy> {
    amount: u32,
    strategy: S,
}

impl<S: PaymentStrategy> Checkout<S> {
    #[must_use]
    pub const fn new(amount: u32, strategy: S) -> Self {
        Self { amount, strategy }
    }

    #[must_use]
    pub fn complete_payment(&self) -> String {
        self.strategy.pay(self.amount)
    }
}

// ----------------------------------------------------------------------------
// Concrete Strategies
// ----------------------------------------------------------------------------

pub struct CreditCard {
    number: String,
}

impl CreditCard {
    #[must_use]
    pub fn new(number: impl Into<String>) -> Self {
        Self {
            number: number.into(),
        }
    }
}

impl PaymentStrategy for CreditCard {
    fn pay(&self, amount: u32) -> String {
        format!(
            "Paid {amount} using Credit Card ending in {}",
            &self.number[self.number.len().saturating_sub(4)..]
        )
    }
}

pub struct PayPal {
    email: String,
}

impl PayPal {
    #[must_use]
    pub fn new(email: impl Into<String>) -> Self {
        Self {
            email: email.into(),
        }
    }
}

impl PaymentStrategy for PayPal {
    fn pay(&self, amount: u32) -> String {
        format!("Paid {amount} using PayPal account {}", self.email)
    }
}

// ============================================================================
// Approach 3: Traits (Dynamic Dispatch)
// ============================================================================

/// A context that can swap its strategy at runtime.
pub struct DynamicCheckout {
    amount: u32,
    // TRADEOFF: Box<dyn Trait> requires a heap allocation and dynamic dispatch (vtable lookup),
    // but allows swapping the strategy at runtime.
    strategy: Box<dyn PaymentStrategy>,
}

impl DynamicCheckout {
    #[must_use]
    pub fn new(amount: u32, strategy: Box<dyn PaymentStrategy>) -> Self {
        Self { amount, strategy }
    }

    /// Swap the strategy at runtime.
    pub fn set_strategy(&mut self, strategy: Box<dyn PaymentStrategy>) {
        // PRODUCTION NOTE: In highly concurrent systems, replacing a strategy might require
        // synchronization (e.g., `ArcSwap` or `RwLock`) to prevent data races.
        self.strategy = strategy;
    }

    #[must_use]
    pub fn complete_payment(&self) -> String {
        self.strategy.pay(self.amount)
    }
}

// ============================================================================
// Footer
// ============================================================================
//
// How this pattern appears in std/crates:
// - `Iterator::map`, `filter`, `fold`: Closures acting as strategies.
// - `std::hash::BuildHasher`: A trait acting as a strategy for creating hashers in a `HashMap`.
//
// What the GoF/OOP equivalent is and why it doesn't translate directly:
// The OOP Strategy pattern relies heavily on interfaces and class hierarchies. Rust skips the
// boilerplate entirely by using functions as first-class citizens (closures) or traits with
// monomorphization for zero-cost abstraction.
//
// When to reach for this vs. simpler alternatives:
// Always start with a simple closure (`Fn`/`FnMut`). Reach for traits and struct-based strategies
// only when the strategy needs to maintain complex state, implement multiple methods, or when
// you are designing a public API that needs to restrict the types of strategies allowed.
//
// Suggested combinations with other patterns in this collection:
// - **Command Pattern**: A strategy might generate commands to be executed later.
// - **Template Method**: A strategy can implement the specific steps defined in a template method.
//
// GOTCHA:
// When using closures (`FnMut`), be careful about capturing variables from the environment.
// If you return a closure or store it in a struct, you often need to use the `move` keyword
// to take ownership of captured variables, and you might run into lifetime issues requiring `'static`.
//
// META-PATTERN: "Make illegal states unrepresentable"
// By using static dispatch (`Checkout<CreditCard>`), the compiler ensures that a checkout tied
// to a specific strategy cannot accidentally be assigned a different strategy type at runtime.
// The type system enforces the relationship.

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_closure_strategy() {
        let mut processor = DataProcessor::new(vec![1, 2, 3]);

        // Strategy 1: Multiply by 2
        processor.process(|x| *x *= 2);
        assert_eq!(processor.data, vec![2, 4, 6]);

        // Strategy 2: Stateful closure (summing while processing)
        let mut sum = 0;
        processor.process(|x| {
            sum += *x;
            *x = 0;
        });
        assert_eq!(sum, 12);
        assert_eq!(processor.data, vec![0, 0, 0]);
    }

    #[test]
    fn test_static_dispatch_strategy() {
        let cc = CreditCard::new("1234567890123456");
        let checkout1 = Checkout::new(100, cc);
        assert_eq!(
            checkout1.complete_payment(),
            "Paid 100 using Credit Card ending in 3456"
        );

        let paypal = PayPal::new("user@example.com");
        let checkout2 = Checkout::new(50, paypal);
        assert_eq!(
            checkout2.complete_payment(),
            "Paid 50 using PayPal account user@example.com"
        );

        // COMPILE-TIME WIN: checkout1 and checkout2 are fundamentally different types:
        // Checkout<CreditCard> vs Checkout<PayPal>. You cannot accidentally swap them.
    }

    #[test]
    fn test_dynamic_dispatch_strategy() {
        let mut checkout = DynamicCheckout::new(100, Box::new(CreditCard::new("1111222233334444")));
        assert_eq!(
            checkout.complete_payment(),
            "Paid 100 using Credit Card ending in 4444"
        );

        // Swap strategy at runtime
        checkout.set_strategy(Box::new(PayPal::new("dynamic@example.com")));
        assert_eq!(
            checkout.complete_payment(),
            "Paid 100 using PayPal account dynamic@example.com"
        );
    }
}
