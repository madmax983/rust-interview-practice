//! # Decorator Pattern
//!
//! Replaces: **Decorator Pattern** (OOP), **Subclassing for feature addition**
//!
//! Real Rust usage: `std::io::BufReader`, `tower::Service` middleware, `Iterator` combinators (`Map`, `Filter`)
//!
//! ## Why this pattern exists in Rust
//! In OOP, Decorators dynamically attach new behaviors to objects, usually via abstract base classes and inheritance.
//! Rust doesn't have inheritance. Instead, the Decorator pattern emerges naturally as **Generic Wrapper Structs**
//! that implement the same Trait as the object they are wrapping.
//!
//! Because Rust heavily favors composition and traits, the Decorator pattern is ubiquitous but often goes
//! by other names (e.g., "Adapters", "Combinators", "Middleware").
//!
//! ## Architecture
//!
//! ```text
//! [ Client ] --> (Trait: TextProcessor)
//!                        |
//!             [ UppercaseDecorator<T> ] --(delegates)--> [ T: TextProcessor ]
//!                                                                |
//!                                                     [ BaseTextProcessor ]
//! ```
//!
//! **Invariants:**
//! - A Decorator owns (or mutably borrows) the decorated component.
//! - A Decorator implements the same core trait as the component.
//! - State/behavior is added inside the Decorator, before or after delegating to the inner component.
//!
//! ## When to use
//! - When you want to add responsibilities to individual objects statically or dynamically without modifying the original code.
//! - When subclassing is impossible (Rust) or impractical (combinatorial explosion of classes).
//! - Stacking streaming operations (e.g., IO, Iterators).

// ============================================================================
// The Core Trait
// ============================================================================

/// The base trait that both the concrete component and all decorators will implement.
pub trait TextProcessor {
    fn process(&self, text: &str) -> String;
}

// ============================================================================
// Concrete Component
// ============================================================================

/// The base component that does the fundamental work.
pub struct BaseTextProcessor;

impl TextProcessor for BaseTextProcessor {
    fn process(&self, text: &str) -> String {
        // Base behavior: just return the text
        text.to_string()
    }
}

// ============================================================================
// Decorator 1: Uppercase
// ============================================================================

/// A decorator that converts text to uppercase.
///
/// **COMPILE-TIME WIN:** We use a generic type parameter `T: TextProcessor`.
/// This means the decorator is resolved at compile time (static dispatch) via monomorphization,
/// resulting in zero runtime overhead compared to `Box<dyn TextProcessor>`.
pub struct UppercaseDecorator<T> {
    // OWNERSHIP INSIGHT: The decorator takes ownership of the inner component.
    inner: T,
}

impl<T: TextProcessor> UppercaseDecorator<T> {
    pub const fn new(inner: T) -> Self {
        Self { inner }
    }
}

impl<T: TextProcessor> TextProcessor for UppercaseDecorator<T> {
    fn process(&self, text: &str) -> String {
        // 1. Let the inner component do its work
        let inner_result = self.inner.process(text);
        // 2. Add our specific behavior
        inner_result.to_uppercase()
    }
}

// ============================================================================
// Decorator 2: HtmlTag
// ============================================================================

/// A decorator that wraps the text in an HTML tag.
pub struct HtmlTagDecorator<T> {
    inner: T,
    tag: String,
}

impl<T: TextProcessor> HtmlTagDecorator<T> {
    pub fn new(inner: T, tag: impl Into<String>) -> Self {
        Self {
            inner,
            tag: tag.into(),
        }
    }
}

impl<T: TextProcessor> TextProcessor for HtmlTagDecorator<T> {
    fn process(&self, text: &str) -> String {
        let inner_result = self.inner.process(text);
        format!("<{}>{}</{}>", self.tag, inner_result, self.tag)
    }
}

// ============================================================================
// Dynamic Decorator (Trait Objects)
// ============================================================================

// TRADEOFF: Sometimes we need to determine the chain of decorators at runtime.
// In this case, we MUST use dynamic dispatch (`Box<dyn Trait>`), which incurs
// heap allocation and virtual method call overhead.

pub struct DynamicDecorator {
    inner: Box<dyn TextProcessor>,
    prefix: String,
}

impl DynamicDecorator {
    pub fn new(inner: Box<dyn TextProcessor>, prefix: impl Into<String>) -> Self {
        Self {
            inner,
            prefix: prefix.into(),
        }
    }
}

impl TextProcessor for DynamicDecorator {
    fn process(&self, text: &str) -> String {
        let inner_result = self.inner.process(text);
        format!("{}: {}", self.prefix, inner_result)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_static_decorators() {
        // Create the base processor
        let base = BaseTextProcessor;

        // Wrap it in the Uppercase decorator
        let upper = UppercaseDecorator::new(base);

        // Wrap THAT in the HTML tag decorator
        // Notice the type of `html` is `HtmlTagDecorator<UppercaseDecorator<BaseTextProcessor>>`.
        // The entire chain is known at compile time!
        let html = HtmlTagDecorator::new(upper, "b");

        let result = html.process("hello world");
        assert_eq!(result, "<b>HELLO WORLD</b>");
    }

    #[test]
    fn test_dynamic_decorators() {
        // We can build the chain dynamically
        let mut base: Box<dyn TextProcessor> = Box::new(BaseTextProcessor);

        // Condition only known at runtime
        let use_prefix = true;

        if use_prefix {
            base = Box::new(DynamicDecorator::new(base, "DEBUG"));
        }

        let result = base.process("system failure");
        assert_eq!(result, "DEBUG: system failure");
    }
}

// ============================================================================
// Footer
// ============================================================================
//
// How this pattern appears in std/crates:
// - Standard Library I/O: `std::io::BufReader<R>` takes an `R: Read` and adds buffering, while itself implementing `Read`.
// - Iterators: Every call to `.map()`, `.filter()`, etc., returns a new generic struct (`Map<I, F>`, `Filter<I, P>`) that wraps the previous iterator and implements `Iterator`.
//
// What the GoF/OOP equivalent is and why it doesn't translate directly:
// In OOP, Decorator uses inheritance: `class Decorator extends Component`. In Rust, we use generic composition:
// `struct Decorator<T> { inner: T }`. This is strictly more powerful because we can choose between static
// dispatch (zero cost) or dynamic dispatch (`Box<dyn Component>`), whereas OOP forces dynamic dispatch.
//
// When to reach for this vs. simpler alternatives:
// Reach for this when you have a core Trait and you want to layer on orthogonal functionality (like buffering, logging, caching)
// without polluting the core implementation.
//
// Suggested combinations with other patterns in this collection:
// - **Builder Pattern**: Constructing a deeply nested static decorator type by hand is tedious. A Builder can simplify this.
// - **Middleware**: The `middleware.rs` pattern is literally the Decorator pattern applied specifically to async request/response services.
//
// PRODUCTION NOTE:
// When defining decorators that take ownership of `T`, you'll often want to implement helper methods
// to "unwrap" the decorator and get the inner `T` back. For example, `BufReader::into_inner()`.
//
// GOTCHA:
// Deeply nested generic decorators (like Iterator chains) produce massive, unreadable type signatures
// in compiler error messages (e.g., `Map<Filter<Enumerate<Skip<...>>>>`).
//
// ANTI-PATTERN:
// Defaulting to `Box<dyn Trait>` for decorators. Unless you are building the decorator chain conditionally
// at runtime, you should always use generics `Decorator<T>` to allow the compiler to inline the calls.
