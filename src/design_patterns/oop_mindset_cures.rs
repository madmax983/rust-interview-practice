// Suppress pedantic and nursery lints for design pattern examples.
#![allow(clippy::pedantic, clippy::nursery, unused)]
//! # Rust Design Patterns — From First Principles
//!
//! Replaces/Transforms: **Strategy**, **Template Method**, **State (OOP)**
//!
//! Real Rust usage: `std::iter::Iterator` (Template Method), `std::thread::spawn` (Strategy via closures), `hyper` HTTP protocols (Session Types)
//!
//! ## Why this pattern exists in Rust specifically
//! Half of the classic OOP patterns exist because languages like Java/C++ lack sum types,
//! trait objects, or ownership semantics. In Rust, some patterns dissolve entirely
//! (e.g., Strategy becomes simply passing closures), some transform into something better
//! (e.g., Template Method becomes trait default methods with no inheritance required),
//! and some genuinely new ones emerge that have no equivalent elsewhere (e.g., Session Types).
//!
//! ## Meta-Pattern: Make Illegal States Unrepresentable
//! Every pattern in this collection should be evaluated against this principle. Rust's deepest
//! design pattern is moving invariants from runtime to compile time. If a pattern doesn't do this,
//! question whether it's earning its complexity.
//!
//! ## Architecture
//!
//! **Category 1: Dissolves Entirely (Strategy -> Closures)**
//! ```text
//! [ OOP Strategy ] --> Interface `SortStrategy` + Classes `QuickSort`, `MergeSort` + `Context` class.
//! [ Rust Approach ] --> fn sort_with<F: Fn(&T) -> bool>(strategy: F) -> No classes needed.
//! ```
//!
//! **Category 2: Transforms into Something Better (Template Method -> Trait Defaults)**
//! ```text
//! [ OOP Template ] --> Abstract Base Class defines `process()`, subclasses override `step1()`.
//! [ Rust Approach ] --> Trait defines `process()` with default impl, implementors define `step1()`. No inheritance.
//! ```
//!
//! **Category 3: Genuinely New (Session Types)**
//! ```text
//! [ OOP State ] --> object.authenticate(); object.send_data(); // Runtime crash if out of order
//! [ Rust Session ] --> UnauthenticatedChannel --auth()--> AuthenticatedChannel --send()--> DataSentChannel
//! ```
//!
//! **Invariants Enforced:**
//! - **Session Types:** Compile-time guarantee that protocol steps are executed in exactly the correct order.
//! - **Traits/Closures:** Eliminates heap allocations associated with OOP Strategy interfaces via monomorphization.
//!
//! **When to use:**
//! - Use closures instead of Strategy when the "strategy" is just a function.
//! - Use Traits with default methods instead of Template Method class inheritance.
//! - Use Session Types when communicating over a protocol where order matters (e.g., SMTP, HTTP).
//!
//! **Anti-patterns:**
//! - Defining a `Strategy` trait with a single method and implementing it for empty structs (Java-style).
//! - Using `Box<dyn Any>` to simulate subclassing.
//! - Using boolean flags (`is_authenticated`) inside structs to track protocol state.

// ============================================================================
// Category 1: Dissolves Entirely — Strategy vs. Closures
// ============================================================================

// ANTI-PATTERN:
// ```java
// interface DiscountStrategy { float apply(float price); }
// class VIPDiscount implements DiscountStrategy { ... }
// class Context { DiscountStrategy strategy; ... }
// ```
// In Rust, we don't need a whole trait and empty structs just to pass behavior.

pub struct ShoppingCart {
    pub total: f64,
}

impl ShoppingCart {
    // OWNERSHIP INSIGHT: We pass a closure that takes `f64` and returns `f64`.
    // The `Fn` trait is implemented by plain functions and closures natively.
    // COMPILE-TIME WIN: By using generic `F`, the compiler monomorphizes this function,
    // resulting in zero-cost static dispatch (no vtables or heap allocations).
    pub fn calculate_total<F>(&self, discount_strategy: F) -> f64
    where
        F: Fn(f64) -> f64,
    {
        discount_strategy(self.total)
    }
}

// PRODUCTION NOTE: For larger strategies with internal state, you can use `FnMut` or
// define a specific trait. But for 90% of Strategy use-cases, closures are vastly superior.

// ============================================================================
// Category 2: Transforms into Something Better — Template Method vs Traits
// ============================================================================

// ANTI-PATTERN: OOP Template Method requires an Abstract Base Class, forcing inheritance.
// In Rust, we use Traits with default method implementations.

pub trait DataProcessor {
    // These methods must be implemented by the concrete type.
    fn read_data(&self) -> String;
    fn process_data(&self, data: &str) -> String;

    // The "Template Method" provides the skeletal algorithm, relying on the above methods.
    // TRADEOFF: You cannot override the template method itself if it's not part of the
    // trait contract (e.g., if you write it as an extension trait), but here it is overridable
    // by default. Usually, you want it to be final, which Rust doesn't strictly support without
    // splitting into two traits.
    fn execute_pipeline(&self) -> String {
        let raw = self.read_data();
        let processed = self.process_data(&raw);
        format!("Pipeline complete: {processed}")
    }
}

pub struct CsvProcessor;

impl DataProcessor for CsvProcessor {
    fn read_data(&self) -> String {
        "id,name\n1,alice".to_string()
    }

    fn process_data(&self, data: &str) -> String {
        data.replace(',', " | ")
    }
    // `execute_pipeline` is inherited automatically.
}

// ============================================================================
// Category 3: Genuinely New — Session Types (Linear State Machines)
// ============================================================================

// ANTI-PATTERN: The OOP way uses boolean flags:
// ```cpp
// class SmtpClient {
//     bool is_helo_sent = false;
//     void mail_from() { if (!is_helo_sent) throw Error(); ... }
// }
// ```

// META-PATTERN: "Make illegal states unrepresentable." We encode the protocol into the type system.

pub struct Connected;
pub struct HeloSent;
pub struct MailFromSent;

/// A protocol client using typestates to enforce step order.
pub struct SmtpClient<State> {
    server: String,
    _state: std::marker::PhantomData<State>,
}

impl SmtpClient<Connected> {
    pub fn new(server: &str) -> Self {
        Self {
            server: server.to_string(),
            _state: std::marker::PhantomData,
        }
    }

    // OWNERSHIP INSIGHT: Consumes `self` (the Connected state), returning the next state.
    // The previous state is destroyed.
    pub fn send_helo(self) -> SmtpClient<HeloSent> {
        // ... network I/O ...
        SmtpClient {
            server: self.server,
            _state: std::marker::PhantomData,
        }
    }
}

impl SmtpClient<HeloSent> {
    // GOTCHA: If you take `&mut self` instead of `self`, the user could call this twice!
    // By taking `self`, we strictly enforce that `mail_from` can only happen ONCE per session state.
    pub fn mail_from(self, sender: &str) -> SmtpClient<MailFromSent> {
        // ... network I/O ...
        SmtpClient {
            server: self.server,
            _state: std::marker::PhantomData,
        }
    }
}

impl SmtpClient<MailFromSent> {
    pub fn quit(self) {
        // Session ends naturally. `self` is dropped.
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strategy_dissolved() {
        let cart = ShoppingCart { total: 100.0 };

        // We just pass closures instead of entire class hierarchies.
        let no_discount = |price: f64| price;
        let vip_discount = |price: f64| price * 0.8;

        assert_eq!(cart.calculate_total(no_discount), 100.0);
        assert_eq!(cart.calculate_total(vip_discount), 80.0);
    }

    #[test]
    fn test_template_method_transformed() {
        let processor = CsvProcessor;

        assert_eq!(
            processor.execute_pipeline(),
            "Pipeline complete: id | name\n1 | alice"
        );
    }

    #[test]
    fn test_session_types_new() {
        let client = SmtpClient::<Connected>::new("smtp.example.com");

        // COMPILE ERROR if uncommented:
        // client.mail_from("alice@example.com"); // Method not found in `SmtpClient<Connected>`

        // Must follow exact protocol: Connected -> HeloSent -> MailFromSent -> Quit
        let helo = client.send_helo();
        let mail = helo.mail_from("alice@example.com");
        mail.quit();
    }
}

// ============================================================================
// Footer
// ============================================================================
//
// How this pattern appears in std/major crates:
// - **Strategy -> Closures:** `Iterator::map`, `Option::and_then`, `thread::spawn`.
// - **Template Method -> Trait Defaults:** `std::io::Read` (implement `read`, get `read_to_end` for free), `Iterator` (implement `next`, get 70+ methods for free).
// - **Session Types:** The `hyper` crate internally manages HTTP state machines via affine types, preventing headers from being sent after body chunks.
//
// What the GoF/OOP equivalent is and why it doesn't translate directly:
// - **Strategy:** OOP requires defining an Interface and multiple Classes just to pass a single method. Rust passes closures directly.
// - **Template Method:** OOP requires inheritance (tight coupling). Rust uses Traits, preserving composition and flat hierarchies.
//
// When to reach for this vs simpler alternatives:
// - Use closures over traits when behavior is isolated and doesn't require sharing state.
// - Use traits with default methods heavily—it is the foundation of Rust's standard library.
// - Use Session Types when modeling rigid state machines (protocols, multi-step wizards) where sequence errors must be caught at compile time.
