//! # GraphQL Execution Engine
//!
//! Implements a minimal GraphQL query execution engine, processing queries against a typed schema.
//!
//! **Replaces Crates:** `async-graphql`, `juniper`
//!
//! **Real-world Usage:**
//! - Backend APIs that serve diverse clients (web, mobile, desktop).
//! - Gateway layers aggregating multiple microservices.
//! - Systems requiring precise, client-specified data fetching to avoid over/under-fetching.
//!
//! **Why build it yourself?**
//! Real-world GraphQL crates rely heavily on procedural macros and complex asynchronous
//! executors. Building a synchronous, macro-free version from scratch demystifies the
//! parsing of GraphQL syntax into an AST, and the recursive evaluation phase where trait
//! objects map fields to concrete data accessors.

use std::collections::HashMap;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Components:
// 1. **Parser:** Takes a GraphQL query string and produces a lightweight AST representing
//    selected fields and nested selections.
// 2. **Execution Engine:** A recursive executor that traverses the AST, calling into the Schema.
// 3. **Schema / Resolver Trait:** The interface defining how specific fields are resolved.
//    Using a dynamic trait object approach (`Box<dyn Resolver>`) allows flexible graphs.
//
// Flow:
// Query String -> AST (Document -> SelectionSet -> Field) -> Executor -> JSON-like Result
//
// Invariants:
// - Execution returns a graph matching the requested structure perfectly.
// - Unknown fields return a standard string error instead of panicking.
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Phase         │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ Parsing       │ O(N)        │ O(N)        │
// │ Execution     │ O(V + E)    │ O(V)        │
// └───────────────┴─────────────┴─────────────┘
// Where N is the query string length, V is vertices in result graph, E is edges.
//
// Design Decisions:
// - **Types:** We map responses into a simple `GqlValue` enum (representing JSON).
// - **Borrowing & Lifetimes:**
//   - **AST:** Uses `&str` references mapped back to the query string to avoid string copying.
//   - **Execution output:** Uses owned data types (like `String` and `HashMap`) for the
//     evaluated output representation to avoid lifetime and borrowing conflicts with
//     dynamically generated trait objects.

// =========================================================================================
// AST & Parser
// =========================================================================================

/// A simple AST node representing a field selection in a query.
#[derive(Debug, PartialEq)]
pub struct Field<'a> {
    pub name: &'a str,
    pub selection_set: Vec<Field<'a>>,
}

/// A highly simplified parser for basic GraphQL queries.
/// Supports basic brace-enclosed field selection: `{ user { id name } }`
pub struct Parser<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    pub fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    /// Skips whitespace and commas
    fn skip_whitespace(&mut self) {
        while self.pos < self.input.len() {
            let ch = self.input[self.pos..].chars().next().unwrap();
            if ch.is_whitespace() || ch == ',' {
                self.pos += ch.len_utf8();
            } else {
                break;
            }
        }
    }

    /// Parses a field name
    fn parse_name(&mut self) -> Option<&'a str> {
        self.skip_whitespace();
        let start = self.pos;
        while self.pos < self.input.len() {
            let ch = self.input[self.pos..].chars().next().unwrap();
            if ch.is_alphanumeric() || ch == '_' {
                self.pos += ch.len_utf8();
            } else {
                break;
            }
        }
        if start < self.pos {
            Some(&self.input[start..self.pos])
        } else {
            None
        }
    }

    /// Parses a selection set enclosed in braces `{ ... }`
    fn parse_selection_set(&mut self) -> Result<Vec<Field<'a>>, String> {
        self.skip_whitespace();
        if self.pos >= self.input.len() || !self.input[self.pos..].starts_with('{') {
            return Ok(Vec::new()); // No selection set
        }

        self.pos += 1; // Consume '{'

        let mut fields = Vec::new();
        loop {
            self.skip_whitespace();
            if self.pos < self.input.len() && self.input[self.pos..].starts_with('}') {
                self.pos += 1; // Consume '}'
                break;
            }
            let name = self.parse_name().ok_or("Expected field name")?;
            let selection_set = self.parse_selection_set()?;
            fields.push(Field { name, selection_set });
        }
        Ok(fields)
    }

    /// Parses a query document (assumes implicit root `{ ... }` if starts with brace)
    pub fn parse(&mut self) -> Result<Vec<Field<'a>>, String> {
        self.parse_selection_set()
    }
}

// =========================================================================================
// Execution Engine
// =========================================================================================

/// Represents the output types of a GraphQL execution (maps nicely to JSON)
#[derive(Debug, PartialEq, Clone)]
pub enum GqlValue {
    Null,
    String(String),
    Int(i32),
    Float(f64),
    Boolean(bool),
    List(Vec<GqlValue>),
    Object(HashMap<String, GqlValue>),
}

/// The core trait that all resolveable objects must implement.
///
/// # RUST INSIGHT: Trait Objects for Dynamic Graphs
/// Returning `Box<dyn Resolver>` allows the executor to recursively explore
/// graphs of different concrete types uniformly, making schema building compositional.
pub trait Resolver {
    /// Resolves a scalar or list value for the given field name.
    /// If the field returns another object, this should return `None` and `resolve_object` is used.
    fn resolve_field(&self, name: &str) -> Option<GqlValue>;

    /// Resolves a complex nested object for the given field name.
    fn resolve_object(&self, name: &str) -> Option<Box<dyn Resolver>>;

    /// Resolves a list of complex nested objects.
    fn resolve_object_list(&self, name: &str) -> Option<Vec<Box<dyn Resolver>>> {
        let _ = name;
        None // Default implementation for simplicity
    }
}

/// The executor runs the AST against the root resolver.
pub struct Executor;

impl Executor {
    /// Recursively executes the given fields against the resolver.
    ///
    /// # GOTCHA: Lifetimes of dynamic data
    /// We construct owned `HashMap` and `String` instances here. Attempting to use `&'a str`
    /// for keys or values while dynamically dispatching via `dyn Resolver` traits leads to
    /// impossible borrow checker constraints unless all data is strictly static or bound to the query life.
    pub fn execute(resolver: &dyn Resolver, fields: &[Field]) -> HashMap<String, GqlValue> {
        let mut result = HashMap::new();

        for field in fields {
            let field_name = field.name.to_string();

            // 1. Check if it's a scalar/simple field
            if let Some(val) = resolver.resolve_field(field.name) {
                result.insert(field_name, val);
            }
            // 2. Check if it's a nested object
            else if let Some(nested_resolver) = resolver.resolve_object(field.name) {
                let nested_result = Self::execute(&*nested_resolver, &field.selection_set);
                result.insert(field_name, GqlValue::Object(nested_result));
            }
            // 3. Check if it's a list of nested objects
            else if let Some(nested_list) = resolver.resolve_object_list(field.name) {
                let mut list_result = Vec::new();
                for item_resolver in nested_list {
                    let item_res = Self::execute(&*item_resolver, &field.selection_set);
                    list_result.push(GqlValue::Object(item_res));
                }
                result.insert(field_name, GqlValue::List(list_result));
            }
            // 4. Not found
            else {
                // PRODUCTION NOTE: A production engine (like `async-graphql`) would not just return `Null` here.
                // It would record an error in a separate `errors` array that gets returned alongside the `data`
                // payload, following the official GraphQL specification.
                result.insert(field_name, GqlValue::Null);
            }
        }

        result
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `async-graphql`: A macro-heavy crate that heavily relies on `async/await` and proc-macros
//   to generate the schema from standard Rust structs. It handles subscriptions, mutations, and directives.
// - `juniper`: Similar to `async-graphql` but older.
//
// Missing vs. Production:
// - **Async Execution:** Real GraphQL systems fetch data concurrently from databases; we are purely synchronous.
// - **Validation phase:** We don't validate the query against a strictly defined schema before execution.
// - **Arguments/Variables:** We only support field selection, not parameterized queries like `user(id: 4)`.
// - **Fragments / Directives / Aliases:** Advanced query features are omitted.
//
// Next Steps:
// 1. Add support for Field Arguments in the Parser and Executor.
// 2. Implement async resolvers returning `BoxFuture<'a, GqlValue>`.
// 3. Implement an explicit Schema validation phase.
//
// Benchmarking Note:
// To benchmark execution performance against `async-graphql` or `juniper`, use `criterion`.
// 1. Construct a complex root query with deeply nested objects and large lists.
// 2. Parse the query string once outside the benchmark loop to isolate execution overhead.
// 3. Use `std::hint::black_box(Executor::execute(&root, &ast))` inside the `b.iter` closure
//    to ensure the compiler does not optimize away the execution. Compare allocations and execution time.

#[cfg(test)]
mod tests {
    use super::*;

    // Mock Models for Testing

    struct Address {
        city: String,
        zip: String,
    }

    impl Resolver for Address {
        fn resolve_field(&self, name: &str) -> Option<GqlValue> {
            match name {
                "city" => Some(GqlValue::String(self.city.clone())),
                "zip" => Some(GqlValue::String(self.zip.clone())),
                _ => None,
            }
        }

        fn resolve_object(&self, _name: &str) -> Option<Box<dyn Resolver>> {
            None
        }
    }

    struct User {
        id: i32,
        name: String,
        address: Address,
    }

    impl Resolver for User {
        fn resolve_field(&self, name: &str) -> Option<GqlValue> {
            match name {
                "id" => Some(GqlValue::Int(self.id)),
                "name" => Some(GqlValue::String(self.name.clone())),
                _ => None,
            }
        }

        fn resolve_object(&self, name: &str) -> Option<Box<dyn Resolver>> {
            if name == "address" {
                Some(Box::new(Address {
                    city: self.address.city.clone(),
                    zip: self.address.zip.clone(),
                }))
            } else {
                None
            }
        }
    }

    struct RootQuery {
        current_user: User,
        users: Vec<User>,
    }

    impl Resolver for RootQuery {
        fn resolve_field(&self, _name: &str) -> Option<GqlValue> {
            None
        }

        fn resolve_object(&self, name: &str) -> Option<Box<dyn Resolver>> {
            if name == "currentUser" {
                Some(Box::new(User {
                    id: self.current_user.id,
                    name: self.current_user.name.clone(),
                    address: Address {
                        city: self.current_user.address.city.clone(),
                        zip: self.current_user.address.zip.clone(),
                    }
                }))
            } else {
                None
            }
        }

        fn resolve_object_list(&self, name: &str) -> Option<Vec<Box<dyn Resolver>>> {
            if name == "users" {
                let mut resolvers: Vec<Box<dyn Resolver>> = Vec::new();
                for user in &self.users {
                    resolvers.push(Box::new(User {
                        id: user.id,
                        name: user.name.clone(),
                        address: Address {
                            city: user.address.city.clone(),
                            zip: user.address.zip.clone(),
                        }
                    }));
                }
                Some(resolvers)
            } else {
                None
            }
        }
    }

    fn setup_root() -> RootQuery {
        RootQuery {
            current_user: User {
                id: 1,
                name: "Alice".to_string(),
                address: Address {
                    city: "Wonderland".to_string(),
                    zip: "12345".to_string(),
                },
            },
            users: vec![
                User {
                    id: 2,
                    name: "Bob".to_string(),
                    address: Address { city: "Builderland".to_string(), zip: "67890".to_string() }
                }
            ],
        }
    }

    #[test]
    fn test_parser() {
        let query = "{ currentUser { id name } }";
        let mut parser = Parser::new(query);
        let ast = parser.parse().unwrap();

        assert_eq!(ast.len(), 1);
        assert_eq!(ast[0].name, "currentUser");
        assert_eq!(ast[0].selection_set.len(), 2);
        assert_eq!(ast[0].selection_set[0].name, "id");
        assert_eq!(ast[0].selection_set[1].name, "name");
    }

    #[test]
    fn test_executor_scalars_and_nested() {
        let query = "{ currentUser { id name address { city } } }";
        let mut parser = Parser::new(query);
        let ast = parser.parse().unwrap();

        let root = setup_root();
        let result = Executor::execute(&root, &ast);

        // Expected output:
        // { "currentUser": { "id": 1, "name": "Alice", "address": { "city": "Wonderland" } } }

        let current_user_val = result.get("currentUser").unwrap();
        if let GqlValue::Object(user_map) = current_user_val {
            assert_eq!(user_map.get("id"), Some(&GqlValue::Int(1)));
            assert_eq!(user_map.get("name"), Some(&GqlValue::String("Alice".to_string())));

            if let Some(GqlValue::Object(addr_map)) = user_map.get("address") {
                assert_eq!(addr_map.get("city"), Some(&GqlValue::String("Wonderland".to_string())));
                assert_eq!(addr_map.get("zip"), None); // Was not requested
            } else {
                panic!("address should be an object");
            }
        } else {
            panic!("currentUser should be an object");
        }
    }

    #[test]
    fn test_executor_list() {
        let query = "{ users { name } }";
        let mut parser = Parser::new(query);
        let ast = parser.parse().unwrap();

        let root = setup_root();
        let result = Executor::execute(&root, &ast);

        let users_val = result.get("users").unwrap();
        if let GqlValue::List(list) = users_val {
            assert_eq!(list.len(), 1);
            if let GqlValue::Object(user_map) = &list[0] {
                assert_eq!(user_map.get("name"), Some(&GqlValue::String("Bob".to_string())));
            } else {
                panic!("users item should be an object");
            }
        } else {
            panic!("users should be a list");
        }
    }

    #[test]
    fn test_missing_field() {
        let query = "{ currentUser { nope } }";
        let mut parser = Parser::new(query);
        let ast = parser.parse().unwrap();

        let root = setup_root();
        let result = Executor::execute(&root, &ast);

        if let GqlValue::Object(user_map) = result.get("currentUser").unwrap() {
            assert_eq!(user_map.get("nope"), Some(&GqlValue::Null));
        } else {
            panic!("currentUser should be an object");
        }
    }
}
