//! # GraphQL Execution Engine
//!
//! Implements a minimal GraphQL execution engine.
//! It parses a query into an AST and executes it dynamically against a typed schema using a trait-based `Resolver` interface.
//!
//! **Replaces Crates:** `async-graphql`, `juniper`
//!
//! **Real-world Usage:**
//! - API Gateways (Apollo Server)
//! - Backend-for-Frontend (BFF) layers
//! - Exposing complex relational data
//!
//! **Why build it yourself?**
//! Building a GraphQL engine demystifies how a single query string is resolved into a highly nested JSON object.
//! You learn about parsing, Abstract Syntax Trees (ASTs), and most importantly, recursive resolution strategies
//! that map tree nodes to executable functions (resolvers).

use std::collections::HashMap;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure:
// Query String -> Lexer -> Parser -> AST (Document) -> Executor (with Resolvers) -> JSON-like Result
//
// Invariants:
// 1. The result matches the exact shape of the requested query.
// 2. Unrequested fields are never resolved or returned.
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ Parse Query   │ O(Q)        │ O(Q)        │
// │ Execute       │ O(R * N)    │ O(R)        │
// └───────────────┴─────────────┴─────────────┘
// Q = length of query string
// R = size of result
// N = average time of a resolver
//
// Design Decisions:
// - **AST**: A simplified representation of a GraphQL document.
// - **Resolver Trait**: We use a trait object `Box<dyn Resolver>` to allow nested, heterogeneous object resolution.
// - **Owned Data**: We use owned `String` and `Value` types for the evaluated output rather than zero-copy string slices
//   to avoid complex lifetime bounds when returning dynamically generated trait objects.

// =========================================================================================
// 1. AST Definition
// =========================================================================================

// RUST INSIGHT: AST node definitions. Real crates might use smaller string references or enums
// to save space. We use `String` to avoid complex lifetime management in this educational example.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Document {
    pub definitions: Vec<OperationDefinition>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct OperationDefinition {
    pub operation_type: String, // e.g., "query", "mutation"
    pub selection_set: Vec<Field>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Field {
    pub name: String,
    pub arguments: HashMap<String, String>, // Simplified: only string values
    pub selection_set: Vec<Field>,
}

// =========================================================================================
// 2. Simple Parser (Naive Implementation)
// =========================================================================================

/// A very naive parser for a subset of GraphQL.
/// Supports basic nested queries: `{ user(id: "1") { id name } }`
pub fn parse_query(query: &str) -> Result<Document, &'static str> {
    // GOTCHA: Using `.chars()` works for standard UTF-8 text, but if we need exact byte offsets
    // for error reporting, `.char_indices()` is often better to avoid multi-byte char mismatches.
    let mut chars = query.chars().peekable();

    fn consume_whitespace(chars: &mut std::iter::Peekable<std::str::Chars>) {
        while let Some(&c) = chars.peek() {
            if c.is_whitespace() || c == ',' {
                chars.next();
            } else {
                break;
            }
        }
    }

    fn parse_ident(chars: &mut std::iter::Peekable<std::str::Chars>) -> String {
        let mut ident = String::new();
        while let Some(&c) = chars.peek() {
            if c.is_alphanumeric() || c == '_' {
                ident.push(chars.next().unwrap());
            } else {
                break;
            }
        }
        ident
    }

    fn parse_arguments(
        chars: &mut std::iter::Peekable<std::str::Chars>,
    ) -> Result<HashMap<String, String>, &'static str> {
        let mut args = HashMap::new();
        if let Some(&'(') = chars.peek() {
            chars.next(); // consume '('
            loop {
                consume_whitespace(chars);
                if let Some(&')') = chars.peek() {
                    chars.next(); // consume ')'
                    break;
                }

                let key = parse_ident(chars);
                consume_whitespace(chars);

                if chars.next() != Some(':') {
                    return Err("Expected ':' in arguments");
                }
                consume_whitespace(chars);

                // Parse string literal argument
                if chars.next() != Some('"') {
                    return Err("Expected '\"' for argument value");
                }
                let mut value = String::new();
                while let Some(c) = chars.next() {
                    if c == '"' {
                        break;
                    }
                    value.push(c);
                }

                args.insert(key, value);
            }
        }
        Ok(args)
    }

    fn parse_selection_set(
        chars: &mut std::iter::Peekable<std::str::Chars>,
    ) -> Result<Vec<Field>, &'static str> {
        let mut fields = Vec::new();
        consume_whitespace(chars);

        if let Some(&'{') = chars.peek() {
            chars.next(); // consume '{'
            loop {
                consume_whitespace(chars);
                if let Some(&'}') = chars.peek() {
                    chars.next(); // consume '}'
                    break;
                }

                if chars.peek().is_none() {
                    return Err("Unexpected EOF in selection set");
                }

                let name = parse_ident(chars);
                if name.is_empty() {
                    return Err("Expected field name");
                }

                let arguments = parse_arguments(chars)?;
                consume_whitespace(chars);

                let selection_set = if let Some(&'{') = chars.peek() {
                    parse_selection_set(chars)?
                } else {
                    Vec::new()
                };

                fields.push(Field {
                    name,
                    arguments,
                    selection_set,
                });
            }
        }
        Ok(fields)
    }

    consume_whitespace(&mut chars);

    // Assume default "query" operation if '{' starts the document
    let operation_type = if chars.peek() == Some(&'{') {
        "query".to_string()
    } else {
        let op = parse_ident(&mut chars);
        consume_whitespace(&mut chars);
        op
    };

    let selection_set = parse_selection_set(&mut chars)?;

    Ok(Document {
        definitions: vec![OperationDefinition {
            operation_type,
            selection_set,
        }],
    })
}

// =========================================================================================
// 3. Execution Engine
// =========================================================================================

/// Represents an evaluated JSON-like value.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Value {
    Null,
    String(String),
    Int(i32),
    Boolean(bool),
    Object(HashMap<String, Value>),
    List(Vec<Value>),
}

/// The core trait that all GraphQL objects must implement to be resolvable.
pub trait Resolver {
    /// Resolves a specific field on this object.
    /// Returns either a scalar `Value` or another `Box<dyn Resolver>` for nested objects.
    // PRODUCTION NOTE: Real GraphQL engines often pass down a `Context` containing
    // authentication details, database connections, and DataLoaders here.
    fn resolve_field(&self, name: &str, args: &HashMap<String, String>) -> ResolveResult;
}

pub enum ResolveResult {
    Scalar(Value),
    Object(Box<dyn Resolver>),
    List(Vec<Box<dyn Resolver>>),
    Error(String),
}

/// Executes an AST against a root resolver.
pub fn execute(document: &Document, root: &dyn Resolver) -> Result<Value, String> {
    if document.definitions.is_empty() {
        return Err("No operations found".to_string());
    }

    // Execute the first operation
    let op = &document.definitions[0];

    // Root is treated as an object
    let mut result_map = HashMap::new();

    for field in &op.selection_set {
        let value = execute_field(field, root)?;
        result_map.insert(field.name.clone(), value);
    }

    Ok(Value::Object(result_map))
}

fn execute_field(field: &Field, resolver: &dyn Resolver) -> Result<Value, String> {
    match resolver.resolve_field(&field.name, &field.arguments) {
        ResolveResult::Scalar(val) => Ok(val),
        ResolveResult::Object(child_resolver) => {
            let mut result_map = HashMap::new();
            for sub_field in &field.selection_set {
                let val = execute_field(sub_field, child_resolver.as_ref())?;
                result_map.insert(sub_field.name.clone(), val);
            }
            Ok(Value::Object(result_map))
        }
        ResolveResult::List(child_resolvers) => {
            let mut list_results = Vec::new();
            for child_resolver in child_resolvers {
                let mut result_map = HashMap::new();
                for sub_field in &field.selection_set {
                    let val = execute_field(sub_field, child_resolver.as_ref())?;
                    result_map.insert(sub_field.name.clone(), val);
                }
                list_results.push(Value::Object(result_map));
            }
            Ok(Value::List(list_results))
        }
        ResolveResult::Error(err) => Err(err),
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `async-graphql`: Fully async, heavily macro-driven (`#[derive(Object)]`), implements the full spec
//   including subscriptions, interfaces, unions, input objects, and complex variables.
// - `juniper`: Similar to `async-graphql` but historically synchronous (now supports async).
//
// Missing vs. Production:
// - **Async Execution**: Resolvers in real engines return Futures to allow concurrent fetching (e.g., DataLoaders).
// - **Type System Validation**: We don't validate the query against a Schema before execution.
// - **Aliases and Fragments**: Essential features of the GraphQL spec are omitted.
// - **Variables**: We only support hardcoded string literal arguments.
//
// Next Steps:
// 1. Add asynchronous execution (`fn resolve_field(...) -> BoxFuture<'_, ResolveResult>`).
// 2. Implement the `Dataloader` pattern to solve the N+1 problem.
// 3. Add schema validation prior to execution.
//
// Benchmarking Note:
// Use `criterion` to benchmark the execution step of `execute()` against different sized ASTs and mock resolvers.
// Track allocations during the resolution phase.

#[cfg(test)]
mod tests {
    use super::*;

    // Mock Data Models
    struct User {
        id: String,
        name: String,
    }

    impl Resolver for User {
        fn resolve_field(&self, name: &str, _args: &HashMap<String, String>) -> ResolveResult {
            match name {
                "id" => ResolveResult::Scalar(Value::String(self.id.clone())),
                "name" => ResolveResult::Scalar(Value::String(self.name.clone())),
                _ => ResolveResult::Error(format!("Unknown field: {}", name)),
            }
        }
    }

    struct RootQuery;

    impl Resolver for RootQuery {
        fn resolve_field(&self, name: &str, args: &HashMap<String, String>) -> ResolveResult {
            match name {
                "user" => {
                    let id = args.get("id").map(|s| s.as_str()).unwrap_or("");
                    if id == "1" {
                        ResolveResult::Object(Box::new(User {
                            id: "1".to_string(),
                            name: "Alice".to_string(),
                        }))
                    } else {
                        ResolveResult::Scalar(Value::Null)
                    }
                }
                "users" => {
                    let users: Vec<Box<dyn Resolver>> = vec![
                        Box::new(User {
                            id: "1".to_string(),
                            name: "Alice".to_string(),
                        }),
                        Box::new(User {
                            id: "2".to_string(),
                            name: "Bob".to_string(),
                        }),
                    ];
                    ResolveResult::List(users)
                }
                _ => ResolveResult::Error(format!("Unknown field: {}", name)),
            }
        }
    }

    #[test]
    fn test_parse_simple_query() {
        let query = "{ user { id name } }";
        let doc = parse_query(query).unwrap();

        assert_eq!(doc.definitions.len(), 1);
        let op = &doc.definitions[0];
        assert_eq!(op.operation_type, "query");
        assert_eq!(op.selection_set.len(), 1);

        let user_field = &op.selection_set[0];
        assert_eq!(user_field.name, "user");
        assert_eq!(user_field.selection_set.len(), 2);
        assert_eq!(user_field.selection_set[0].name, "id");
        assert_eq!(user_field.selection_set[1].name, "name");
    }

    #[test]
    fn test_parse_query_with_args() {
        let query = "{ user(id: \"1\") { name } }";
        let doc = parse_query(query).unwrap();

        let user_field = &doc.definitions[0].selection_set[0];
        assert_eq!(user_field.name, "user");
        assert_eq!(user_field.arguments.get("id").unwrap(), "1");
    }

    #[test]
    fn test_execute_simple_query() {
        let query = "{ user(id: \"1\") { id name } }";
        let doc = parse_query(query).unwrap();

        let root = RootQuery;
        let result = execute(&doc, &root).unwrap();

        if let Value::Object(map) = result {
            if let Value::Object(user_map) = map.get("user").unwrap() {
                assert_eq!(user_map.get("id").unwrap(), &Value::String("1".to_string()));
                assert_eq!(
                    user_map.get("name").unwrap(),
                    &Value::String("Alice".to_string())
                );
            } else {
                panic!("Expected Object");
            }
        } else {
            panic!("Expected Object");
        }
    }

    #[test]
    fn test_execute_list_query() {
        let query = "{ users { name } }";
        let doc = parse_query(query).unwrap();

        let root = RootQuery;
        let result = execute(&doc, &root).unwrap();

        if let Value::Object(map) = result {
            if let Value::List(users) = map.get("users").unwrap() {
                assert_eq!(users.len(), 2);
                if let Value::Object(user1) = &users[0] {
                    assert_eq!(
                        user1.get("name").unwrap(),
                        &Value::String("Alice".to_string())
                    );
                }
            } else {
                panic!("Expected List");
            }
        } else {
            panic!("Expected Object");
        }
    }

    #[test]
    fn test_unrequested_fields_ignored() {
        let query = "{ user(id: \"1\") { name } }"; // Did not request 'id'
        let doc = parse_query(query).unwrap();

        let root = RootQuery;
        let result = execute(&doc, &root).unwrap();

        if let Value::Object(map) = result {
            if let Value::Object(user_map) = map.get("user").unwrap() {
                assert!(user_map.contains_key("name"));
                assert!(!user_map.contains_key("id")); // Ensure 'id' is absent
            }
        }
    }
}
