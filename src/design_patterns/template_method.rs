//! # Template Method Pattern
//!
//! Replaces: **Template Method** (OOP / Inheritance)
//!
//! Real Rust usage: `std::io::Read` (read_to_end provides a default impl using required read()), `Iterator` (many provided methods using required next())
//!
//! ## Why this pattern exists in Rust
//! In classical OOP, the Template Method pattern defines the skeleton of an algorithm in a base class,
//! delegating specific steps to subclasses via overridden abstract methods.
//!
//! Because Rust lacks inheritance, this pattern translates directly into **Traits with Default Methods**.
//! A trait can define required methods (the steps to be implemented) and provided methods (the skeleton
//! that calls the required methods).
//!
//! ## Architecture
//!
//! ```text
//! [ trait DataParser ]
//!   |-- required: parse_header(), parse_body()
//!   |-- provided: parse_document() { parse_header(); parse_body(); }
//!
//! [ struct CsvParser ] --(impl DataParser)--> [ overrides required methods ]
//! ```
//!
//! **Invariants:**
//! - The skeleton algorithm (provided method) cannot be modified by the implementor without explicitly overriding it.
//! - Implementors are forced by the compiler to implement all required steps.
//!
//! ## When to use
//! - When multiple types share a common algorithmic structure but differ in specific steps.
//! - To avoid duplicating the "glue" logic that ties the steps together.
//!
//! ## Anti-patterns
//! - Trying to use structs containing function pointers to simulate inheritance. Traits are the idiomatic tool for this.
//! - Overriding the provided (skeleton) method to completely change the algorithmic flow. If the flow needs to change, it might be a different trait or a Strategy pattern.

// ============================================================================
// The Template Trait
// ============================================================================

/// A trait defining the skeleton of a data parsing algorithm.
pub trait DataParser {
    // ------------------------------------------------------------------------
    // Required Methods (The "Holes" to be filled)
    // ------------------------------------------------------------------------

    fn parse_header(&self, data: &str) -> String;
    fn parse_body(&self, data: &str) -> Vec<String>;

    // ------------------------------------------------------------------------
    // Provided Method (The Skeleton / Template Method)
    // ------------------------------------------------------------------------

    /// Parses the entire document. This is the Template Method.
    ///
    /// COMPILE-TIME WIN: By providing a default implementation, we ensure all
    /// implementors share this exact algorithm unless they explicitly override it.
    fn parse_document(&self, data: &str) -> Document {
        // PRODUCTION NOTE: In a real system, you might want this to return a `Result<Document, Error>`
        // so that individual steps can fail gracefully and the template method can short-circuit via `?`.
        let header = self.parse_header(data);
        let body = self.parse_body(data);

        Document { header, body }
    }
}

#[derive(Debug, PartialEq)]
pub struct Document {
    pub header: String,
    pub body: Vec<String>,
}

// ============================================================================
// Concrete Implementations
// ============================================================================

pub struct CsvParser;

impl DataParser for CsvParser {
    fn parse_header(&self, data: &str) -> String {
        // Simple mock implementation
        data.lines()
            .next()
            .unwrap_or("")
            .split(',')
            .collect::<Vec<_>>()
            .join(" | ")
    }

    fn parse_body(&self, data: &str) -> Vec<String> {
        data.lines()
            .skip(1)
            .map(|line| line.replace(',', " -> "))
            .collect()
    }
}

pub struct JsonParser;

impl DataParser for JsonParser {
    fn parse_header(&self, _data: &str) -> String {
        // In a real JSON parser, this might extract a top-level metadata object.
        "JSON_HEADER".to_string()
    }

    fn parse_body(&self, data: &str) -> Vec<String> {
        // Mock parsing logic
        vec![format!("Parsed JSON object with length {}", data.len())]
    }
}

// ============================================================================
// Client Code Usage
// ============================================================================

/// A function that accepts any parser and uses the template method.
///
/// OWNERSHIP INSIGHT: We use `impl DataParser` for static dispatch,
/// creating a specialized, zero-cost version of this function for each parser type.
#[must_use]
pub fn process_data(parser: &impl DataParser, data: &str) -> Document {
    parser.parse_document(data)
}

// ============================================================================
// Footer
// ============================================================================
//
// How this pattern appears in std/crates:
// - `std::io::Read`: Requires `read()`, but provides `read_to_end()`, `read_to_string()`, etc., acting as templates.
// - `std::iter::Iterator`: Requires `next()`, but provides dozens of template methods like `map()`, `filter()`, `collect()`.
//
// What the GoF/OOP equivalent is and why it doesn't translate directly:
// In OOP, you use an abstract base class with abstract methods and a concrete template method.
// Rust doesn't have class inheritance. Traits with default implementations provide the exact
// same functionality in a more flexible, compositional way without deep inheritance trees.
//
// When to reach for this vs. simpler alternatives:
// Use this when you have a complex algorithm with invariant steps, but the implementation of those
// steps varies. It's an excellent way to provide a rich API with minimal required implementation effort.
//
// Suggested combinations with other patterns in this collection:
// - **Strategy Pattern**: While the Template Method locks down the algorithm structure at compile time,
//   you might pass Strategies into the required methods to further customize the steps.
//
// GOTCHA:
// Unlike OOP classes, an implementor of a Rust trait can override the default template method.
// If you must guarantee the algorithm cannot be changed, you should use the Strategy pattern instead:
// make the algorithm a generic function or struct that accepts the "steps" as traits or closures.
//
// META-PATTERN: "Make illegal states unrepresentable"
// By defining the algorithm structure in the trait's provided method, you make it impossible
// for an implementor to forget a step or call them in the wrong order (assuming they don't override the default).

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_csv_parser() {
        let parser = CsvParser;
        let data = "name,age,city\nAlice,30,NY\nBob,25,LA";

        // Calling the template method
        let doc = parser.parse_document(data);

        assert_eq!(doc.header, "name | age | city");
        assert_eq!(doc.body, vec!["Alice -> 30 -> NY", "Bob -> 25 -> LA"]);
    }

    #[test]
    fn test_json_parser() {
        let parser = JsonParser;
        let data = "{\"key\": \"value\"}";

        let doc = process_data(&parser, data);

        assert_eq!(doc.header, "JSON_HEADER");
        assert_eq!(doc.body, vec!["Parsed JSON object with length 16"]);
    }
}
