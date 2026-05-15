//! # Minimal GraphQL Execution Engine
//!
//! Implements a minimal GraphQL execution engine, handling query parsing (AST generation)
//! and field resolution against a typed schema.
//!
//! **Replaces Crates:** `async-graphql`, `juniper`, `graphql-parser`
//!
//! **Real-world Usage:**
//! - API Gateways and BFFs (Backend for Frontend).
//! - Data federation layers.
//! - Typed data fetching in complex client applications.
//!
//! **Why build it yourself?**
//! GraphQL often feels like magic. Building a parser and execution engine from scratch teaches you
//! about abstract syntax trees (ASTs), type systems, and recursive resolution strategies. It demonstrates
//! how to dynamically resolve fields in a strongly typed language like Rust without relying heavily on reflection,
//! often using traits and enums instead.

use std::collections::HashMap;
use std::iter::Peekable;
use std::str::Chars;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Components:
// 1. `AST`: The parsed representation of a GraphQL query (Operations, Selections, Fields).
// 2. `Schema`: The type definitions (Objects, Scalars) representing the data graph.
// 3. `Value`: A JSON-like recursive enum representing the evaluated data response.
// 4. `Resolver`: A trait for objects that can resolve fields dynamically.
// 5. `Executor`: Coordinates traversing the AST and calling resolvers to build the `Value`.
//
// Time Complexity:
// - Parsing: O(N) where N is the length of the query string.
// - Execution: O(M) where M is the number of resolved fields in the response.
// Space Complexity:
// - AST & Response: O(M) for the constructed AST and response tree.

// =========================================================================================
// Core Types (AST & Values)
// =========================================================================================

/// Represents a JSON-like scalar or nested object returned by a resolver.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Boolean(bool),
    Int(i32),
    Float(f64),
    String(String),
    List(Vec<Value>),
    Object(HashMap<String, Value>),
}

/// A parsed GraphQL Document containing operations.
#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    pub operations: Vec<Operation>,
}

/// A GraphQL Operation.
#[derive(Debug, Clone, PartialEq)]
pub struct Operation {
    pub operation_type: OperationType,
    pub name: Option<String>,
    pub selection_set: Vec<Selection>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OperationType {
    Query,
}

/// A selection within a selection set.
#[derive(Debug, Clone, PartialEq)]
pub enum Selection {
    FragmentSpread(String),
    // FUTURE: Add FragmentSpread to parser output and execution logic

    Field(Field),
}

/// A parsed field selection.
#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub name: String,
    pub alias: Option<String>,
    pub selection_set: Vec<Selection>,
}

// =========================================================================================
// Parser
// =========================================================================================

/// A minimal GraphQL parser for turning queries into an AST.
pub struct Parser<'a> {
    chars: Peekable<Chars<'a>>,
}

impl<'a> Parser<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            chars: input.chars().peekable(),
        }
    }

    pub fn parse(&mut self) -> Result<Document, String> {
        self.skip_whitespace();
        let mut operations = Vec::new();

        while self.chars.peek().is_some() {
            operations.push(self.parse_operation()?);
            self.skip_whitespace();
        }

        Ok(Document { operations })
    }

    fn parse_operation(&mut self) -> Result<Operation, String> {
        // For simplicity, we assume an implicit query if we see '{'
        let mut operation_type = OperationType::Query;
        let mut name = None;

        // GOTCHA: A full GraphQL parser needs an explicit robust scanner to avoid arbitrary lookaheads.
        if let Some(&c) = self.chars.peek() {
            if c != '{' {
                let op_type_str = self.parse_name()?;
                if op_type_str != "query" {
                    return Err(format!("Unsupported operation: {}", op_type_str));
                }
                operation_type = OperationType::Query;
                self.skip_whitespace();

                if let Some(&c) = self.chars.peek() {
                    if c != '{' {
                        name = Some(self.parse_name()?);
                    }
                }
            }
        }

        let selection_set = self.parse_selection_set()?;

        Ok(Operation {
            operation_type,
            name,
            selection_set,
        })
    }

    fn parse_selection_set(&mut self) -> Result<Vec<Selection>, String> {
        self.expect_char('{')?;
        self.skip_whitespace();

        let mut selections = Vec::new();
        while let Some(&c) = self.chars.peek() {
            if c == '}' {
                break;
            }
            selections.push(self.parse_selection()?);
            self.skip_whitespace();
        }

        self.expect_char('}')?;
        Ok(selections)
    }

    fn parse_selection(&mut self) -> Result<Selection, String> {
        let name_or_alias = self.parse_name()?;
        self.skip_whitespace();

        let mut name = name_or_alias.clone();
        let mut alias = None;

        // Check if it was an alias
        if let Some(&':') = self.chars.peek() {
            self.expect_char(':')?;
            self.skip_whitespace();
            alias = Some(name_or_alias);
            name = self.parse_name()?;
            self.skip_whitespace();
        }

        // Check for sub-selections
        let mut selection_set = Vec::new();
        if let Some(&'{') = self.chars.peek() {
            selection_set = self.parse_selection_set()?;
        }

        Ok(Selection::Field(Field {
            name,
            alias,
            selection_set,
        }))
    }

    fn parse_name(&mut self) -> Result<String, String> {
        let mut name = String::new();
        while let Some(&c) = self.chars.peek() {
            if c.is_alphanumeric() || c == '_' {
                name.push(c);
                self.chars.next();
            } else {
                break;
            }
        }
        if name.is_empty() {
            return Err("Expected a name".to_string());
        }
        Ok(name)
    }

    fn skip_whitespace(&mut self) {
        while let Some(&c) = self.chars.peek() {
            if c.is_whitespace() || c == ',' {
                self.chars.next();
            } else {
                break;
            }
        }
    }

    fn expect_char(&mut self, expected: char) -> Result<(), String> {
        if let Some(c) = self.chars.next() {
            if c == expected {
                Ok(())
            } else {
                Err(format!("Expected '{}', got '{}'", expected, c))
            }
        } else {
            Err(format!("Expected '{}', got EOF", expected))
        }
    }
}

// =========================================================================================
// Resolver Trait & Execution Engine
// =========================================================================================

/// A trait for objects that can resolve GraphQL fields.
///
/// **RUST INSIGHT:** Instead of relying on runtime reflection (like in Java/Python),
/// Rust approaches this by having types implement a `Resolver` trait. The executor
/// dynamically dispatches to this trait based on the AST.
pub trait Resolver {
    /// Resolves a field by name, optionally returning another Resolver for nested fields.
    fn resolve_field(&self, name: &str) -> Option<ResolvedValue<'_>>;
}

pub enum ResolvedValue<'a> {
    Scalar(Value),
    Object(Box<dyn Resolver + 'a>),
    List(Vec<ResolvedValue<'a>>),
}

/// Executes a parsed GraphQL document against a root resolver.
pub struct Executor;

impl Executor {
    pub fn execute(document: &Document, root_resolver: &dyn Resolver) -> Result<Value, String> {
        if document.operations.is_empty() {
            return Err("No operations found".to_string());
        }

        let op = &document.operations[0];
        Self::execute_selection_set(&op.selection_set, root_resolver)
    }

    fn execute_selection_set(selections: &[Selection], resolver: &dyn Resolver) -> Result<Value, String> {
        let mut result_map = HashMap::new();

        for selection in selections {
            match selection {
                Selection::Field(field) => {
                    let key = field.alias.as_ref().unwrap_or(&field.name).clone();

                    if let Some(resolved) = resolver.resolve_field(&field.name) {
                        let value = Self::execute_resolved_value(resolved, &field.selection_set)?;
                        result_map.insert(key, value);
                    } else {
                        // Field not found on resolver
                        result_map.insert(key, Value::Null);
                    }
                }
                Selection::FragmentSpread(_) => {
                    // FUTURE: Resolve fragment and merge fields
                }
            }
        }

        Ok(Value::Object(result_map))
    }

    fn execute_resolved_value(resolved: ResolvedValue, selection_set: &[Selection]) -> Result<Value, String> {
        match resolved {
            ResolvedValue::Scalar(v) => Ok(v),
            ResolvedValue::Object(obj_resolver) => {
                Self::execute_selection_set(selection_set, obj_resolver.as_ref())
            }
            ResolvedValue::List(list) => {
                let mut values = Vec::new();
                for item in list {
                    values.push(Self::execute_resolved_value(item, selection_set)?);
                }
                Ok(Value::List(values))
            }
        }
    }
}

// =========================================================================================
// Tests
// =========================================================================================

#[cfg(test)]
mod tests {
    use super::*;

    struct User {
        id: i32,
        name: String,
        friends: Vec<User>,
    }

    impl Resolver for User {
        fn resolve_field(&self, name: &str) -> Option<ResolvedValue<'_>> {
            match name {
                "id" => Some(ResolvedValue::Scalar(Value::Int(self.id))),
                "name" => Some(ResolvedValue::Scalar(Value::String(self.name.clone()))),
                "friends" => {
                    let mut resolved_friends = Vec::new();
                    for friend in &self.friends {
                        // RUST INSIGHT: We need to box the reference to the friend
                        // to return it as a dynamic trait object.
                        // However, because trait objects need to be boxed, and we are borrowing `self`,
                        // we must bound the lifetime 'a in ResolvedValue to the lifetime of `self`.
                        // For simplicity in this mock, we clone the user to avoid lifetime issues
                        // in a simple implementation, though a real engine uses Pin or Arena allocation.
                        // Here we just mock it nicely by recreating the User.

                        let cloned_friend = User {
                            id: friend.id,
                            name: friend.name.clone(),
                            friends: vec![],
                        };
                        resolved_friends.push(ResolvedValue::Object(Box::new(cloned_friend)));
                    }
                    Some(ResolvedValue::List(resolved_friends))
                }
                _ => None,
            }
        }
    }

    struct RootQuery;

    impl Resolver for RootQuery {
        fn resolve_field(&self, name: &str) -> Option<ResolvedValue<'_>> {
            match name {
                "me" => {
                    let user = User {
                        id: 1,
                        name: "Alice".to_string(),
                        friends: vec![
                            User { id: 2, name: "Bob".to_string(), friends: vec![] }
                        ],
                    };
                    Some(ResolvedValue::Object(Box::new(user)))
                }
                _ => None,
            }
        }
    }

    #[test]
    fn test_parse_simple_query() {
        let query = "{ me { id name } }";
        let mut parser = Parser::new(query);
        let doc = parser.parse().unwrap();

        assert_eq!(doc.operations.len(), 1);
        let op = &doc.operations[0];
        assert_eq!(op.selection_set.len(), 1);

        if let Selection::Field(f) = &op.selection_set[0] {
            assert_eq!(f.name, "me");
            assert_eq!(f.selection_set.len(), 2);
        } else {
            panic!("Expected field selection");
        }
    }

    #[test]
    fn test_parse_with_alias() {
        let query = "{ author: me { id } }";
        let mut parser = Parser::new(query);
        let doc = parser.parse().unwrap();

        if let Selection::Field(f) = &doc.operations[0].selection_set[0] {
            assert_eq!(f.name, "me");
            assert_eq!(f.alias.as_deref(), Some("author"));
        } else {
            panic!("Expected field selection");
        }
    }

    #[test]
    fn test_execute_query() {
        let query = "{ me { id name friends { name } } }";
        let mut parser = Parser::new(query);
        let doc = parser.parse().unwrap();

        let root = RootQuery;
        let result = Executor::execute(&doc, &root).unwrap();

        if let Value::Object(map) = result {
            if let Value::Object(me_map) = &map["me"] {
                assert_eq!(me_map["id"], Value::Int(1));
                assert_eq!(me_map["name"], Value::String("Alice".to_string()));

                if let Value::List(friends) = &me_map["friends"] {
                    assert_eq!(friends.len(), 1);
                    if let Value::Object(bob_map) = &friends[0] {
                        assert_eq!(bob_map["name"], Value::String("Bob".to_string()));
                    } else {
                        panic!("Expected Bob to be an object");
                    }
                } else {
                    panic!("Expected friends list");
                }
            } else {
                panic!("Expected 'me' to be an object");
            }
        } else {
            panic!("Expected result to be an object");
        }
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// **Canonical Comparisons:**
// - `async-graphql`: A fully-featured, production-ready framework that heavily uses Rust macros
//   and reflection to automatically generate schemas from Rust structs. Our implementation is
//   dynamic and requires explicit schema mapping.
// - `juniper`: Similar to `async-graphql`, highly macro-driven but focuses on synchronous/asynchronous
//   resolution with deep integrations into web frameworks.
//
// **What's missing vs. production:**
// - Asynchronous resolution: Production engines resolve fields concurrently (e.g., using Futures)
//   to avoid blocking on I/O (like DB queries).
// - Complex types: Mutations, Subscriptions, Interfaces, Unions, Fragments, and Input Objects.
// - Validation: Ensuring the query matches the schema *before* execution.
// - Error handling: Implementing proper GraphQL errors with line/column locations.
//
// **Suggested next steps:**
// - Extend `Value` to support GraphQL variables and map them into the `Context`.
// - Make `resolve_field` return a `Result<Option<ResolvedValue>, GraphQLError>`.
// - Convert the `Executor` to `async` using `BoxFuture` for parallel field resolution.
