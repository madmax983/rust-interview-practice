//! # GraphQL Execution Engine
//!
//! Implements a minimalist GraphQL Execution Engine from scratch.
//!
//! **Replaces Crates:** `async-graphql`, `juniper`
//!
//! **Real-world Usage:**
//! - GitHub API v4
//! - Shopify Storefront API
//! - Relay / Apollo Federation gateways
//!
//! **Why build it yourself?**
//! Building a GraphQL engine demystifies how a declarative query string is parsed into an Abstract Syntax Tree (AST),
//! validated against a schema, and executed dynamically. You'll learn how to traverse nested ASTs using Depth-First Search (DFS)
//! and dynamically dispatch field resolution to Rust code without relying on complex compile-time macros, seeing the power
//! of trait objects and dynamic typing in a statically typed language.
//!
//! # Architecture
//!
//! Data Structure:
//!
//! ```text
//! String (Query) -> Lexer -> Tokens -> Parser -> AST (Document -> Operation -> SelectionSet -> Field)
//!                                                  │
//! Schema (Types) ----------------------------------┤ (Execution)
//!                                                  │
//! Resolvers (Trait implementation) ----------------▼
//!                                            Value (JSON-like)
//! ```
//!
//! Time/Space Complexity:
//! - Lexing/Parsing: Time O(N), Space O(N) where N is the length of the query.
//! - Execution: Time O(V + E), Space O(D) where V is the number of resolved fields, E is the number of edges (nested fields), and D is the query depth.
//!
//! Invariants:
//! - The AST structure must reflect proper GraphQL syntax (e.g., no mismatched braces).
//! - Execution must dynamically resolve scalar fields to `Value` and object fields to `Resolver` traits without type confusion.
//!
//! Benchmarking:
//! Use `criterion` to benchmark `Engine::execute` against complex, deeply nested queries to measure allocation
//! overhead during AST construction and trait dynamic dispatch.
//!
//! Design Decisions:
//! - **Trait-based Resolution:** Instead of procedural macros generating boilerplate, we use a `Resolver` trait that takes a field name and returns a dynamically typed `Value`.
//! - **Recursive Execution:** The executor recursively walks the SelectionSet AST, matching fields to the current object's resolver.
//! - **Sync Execution:** For simplicity, we use synchronous execution. A production engine would make resolvers asynchronous.

use std::collections::HashMap;

// =========================================================================================
// Data Types
// =========================================================================================

/// Represents a dynamic GraphQL value, analogous to a JSON value.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Int(i32),
    Float(f64),
    String(String),
    Boolean(bool),
    List(Vec<Value>),
    Object(HashMap<String, Value>),
}

// RUST INSIGHT:
// By defining an explicit `Value` enum, we bridge the gap between Rust's strong static
// typing and GraphQL's dynamic type system. This is identical to how `serde_json::Value` works.

// =========================================================================================
// AST (Abstract Syntax Tree) Nodes
// =========================================================================================

/// A parsed GraphQL query document.
#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    pub operations: Vec<Operation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Operation {
    pub operation_type: OperationType,
    pub name: Option<String>,
    pub selection_set: SelectionSet,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OperationType {
    Query,
    Mutation,
    // Subscription omitted for simplicity
}

#[derive(Debug, Clone, PartialEq)]
pub struct SelectionSet {
    pub selections: Vec<Selection>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Selection {
    Field(Field),
    // FragmentSpread, InlineFragment omitted for simplicity
}

#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub name: String,
    pub alias: Option<String>,
    pub arguments: HashMap<String, Value>,
    pub selection_set: Option<SelectionSet>,
}

// =========================================================================================
// Lexer & Parser
// =========================================================================================

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Name(String),
    Int(i32),
    StringVal(String),
    Punctuation(char),
    Eof,
}

struct Lexer<'a> {
    chars: std::str::Chars<'a>,
    current: Option<char>,
}

impl<'a> Lexer<'a> {
    fn new(input: &'a str) -> Self {
        let mut chars = input.chars();
        let current = chars.next();
        Self { chars, current }
    }

    fn advance(&mut self) {
        self.current = self.chars.next();
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.current {
            if c.is_whitespace() || c == ',' {
                self.advance();
            } else {
                break;
            }
        }
    }

    fn next_token(&mut self) -> Result<Token, String> {
        self.skip_whitespace();

        match self.current {
            Some(c) if c.is_alphabetic() || c == '_' => {
                let mut name = String::new();
                while let Some(ch) = self.current {
                    if ch.is_alphanumeric() || ch == '_' {
                        name.push(ch);
                        self.advance();
                    } else {
                        break;
                    }
                }
                Ok(Token::Name(name))
            }
            Some(c) if c.is_ascii_digit() => {
                let mut num_str = String::new();
                while let Some(ch) = self.current {
                    if ch.is_ascii_digit() {
                        num_str.push(ch);
                        self.advance();
                    } else {
                        break;
                    }
                }
                Ok(Token::Int(num_str.parse().unwrap()))
            }
            Some('"') => {
                self.advance(); // skip quote
                let mut s = String::new();
                while let Some(ch) = self.current {
                    if ch == '"' {
                        self.advance(); // skip quote
                        break;
                    }
                    s.push(ch);
                    self.advance();
                }
                Ok(Token::StringVal(s))
            }
            Some(c) if "{}:()".contains(c) => {
                self.advance();
                Ok(Token::Punctuation(c))
            }
            None => Ok(Token::Eof),
            Some(c) => Err(format!("Unexpected character: {}", c)),
        }
    }
}

pub struct Parser<'a> {
    lexer: Lexer<'a>,
    current_token: Token,
}

impl<'a> Parser<'a> {
    pub fn new(input: &'a str) -> Result<Self, String> {
        let mut lexer = Lexer::new(input);
        let current_token = lexer.next_token()?;
        Ok(Self { lexer, current_token })
    }

    fn advance(&mut self) -> Result<(), String> {
        self.current_token = self.lexer.next_token()?;
        Ok(())
    }

    fn expect_punctuation(&mut self, expected: char) -> Result<(), String> {
        match &self.current_token {
            Token::Punctuation(c) if *c == expected => {
                self.advance()?;
                Ok(())
            }
            _ => Err(format!("Expected '{}', got {:?}", expected, self.current_token)),
        }
    }

    pub fn parse_document(&mut self) -> Result<Document, String> {
        let mut operations = Vec::new();
        while self.current_token != Token::Eof {
            operations.push(self.parse_operation()?);
        }
        Ok(Document { operations })
    }

    fn parse_operation(&mut self) -> Result<Operation, String> {
        // For simplicity, we assume anonymous queries if we start with '{'
        if let Token::Punctuation('{') = self.current_token {
            return Ok(Operation {
                operation_type: OperationType::Query,
                name: None,
                selection_set: self.parse_selection_set()?,
            });
        }

        let op_type = match &self.current_token {
            Token::Name(n) if n == "query" => OperationType::Query,
            Token::Name(n) if n == "mutation" => OperationType::Mutation,
            _ => return Err("Expected 'query' or 'mutation'".to_string()),
        };
        self.advance()?;

        let name = match &self.current_token {
            Token::Name(n) => {
                let name_clone = n.clone();
                self.advance()?;
                Some(name_clone)
            }
            _ => None,
        };

        let selection_set = self.parse_selection_set()?;

        Ok(Operation {
            operation_type: op_type,
            name,
            selection_set,
        })
    }

    fn parse_selection_set(&mut self) -> Result<SelectionSet, String> {
        self.expect_punctuation('{')?;
        let mut selections = Vec::new();

        while self.current_token != Token::Punctuation('}') {
            selections.push(self.parse_selection()?);
        }

        self.expect_punctuation('}')?;
        Ok(SelectionSet { selections })
    }

    fn parse_selection(&mut self) -> Result<Selection, String> {
        let field = self.parse_field()?;
        Ok(Selection::Field(field))
    }

    fn parse_field(&mut self) -> Result<Field, String> {
        let mut name = match &self.current_token {
            Token::Name(n) => n.clone(),
            _ => return Err(format!("Expected field name, got {:?}", self.current_token)),
        };
        self.advance()?;

        let mut alias = None;
        if let Token::Punctuation(':') = self.current_token {
            self.advance()?;
            alias = Some(name);
            name = match &self.current_token {
                Token::Name(n) => n.clone(),
                _ => return Err("Expected field name after alias".to_string()),
            };
            self.advance()?;
        }

        let mut arguments = HashMap::new();
        if let Token::Punctuation('(') = self.current_token {
            self.advance()?;
            while self.current_token != Token::Punctuation(')') {
                let arg_name = match &self.current_token {
                    Token::Name(n) => n.clone(),
                    _ => return Err("Expected argument name".to_string()),
                };
                self.advance()?;
                self.expect_punctuation(':')?;
                let arg_val = self.parse_value()?;
                arguments.insert(arg_name, arg_val);
            }
            self.expect_punctuation(')')?;
        }

        let selection_set = if let Token::Punctuation('{') = self.current_token {
            Some(self.parse_selection_set()?)
        } else {
            None
        };

        Ok(Field {
            name,
            alias,
            arguments,
            selection_set,
        })
    }

    fn parse_value(&mut self) -> Result<Value, String> {
        match &self.current_token {
            Token::Int(i) => {
                let v = Value::Int(*i);
                self.advance()?;
                Ok(v)
            }
            Token::StringVal(s) => {
                let v = Value::String(s.clone());
                self.advance()?;
                Ok(v)
            }
            Token::Name(n) if n == "true" => {
                self.advance()?;
                Ok(Value::Boolean(true))
            }
            Token::Name(n) if n == "false" => {
                self.advance()?;
                Ok(Value::Boolean(false))
            }
            Token::Name(n) if n == "null" => {
                self.advance()?;
                Ok(Value::Null)
            }
            _ => Err("Unsupported value type in arguments".to_string()),
        }
    }
}

// =========================================================================================
// Execution Engine
// =========================================================================================

/// Trait representing a GraphQL object that can resolve its fields.
pub trait Resolver {
    /// Resolves a field dynamically by name.
    /// Returns `None` if the field doesn't exist, or `Some(Result)` with the resolved value.
    fn resolve_field(&self, name: &str, args: &HashMap<String, Value>) -> Option<Result<ResolvedValue, String>>;
}

/// The result of resolving a field can either be a scalar or another Object (Resolver)
pub enum ResolvedValue {
    Scalar(Value),
    Object(Box<dyn Resolver>),
    List(Vec<ResolvedValue>),
}

// GOTCHA:
// We return `Box<dyn Resolver>` for nested objects. This necessitates dynamic dispatch
// but allows immense flexibility without compile-time schema macro generation.

pub struct Engine;

impl Engine {
    pub fn execute(query: &str, root: &dyn Resolver) -> Result<Value, String> {
        let mut parser = Parser::new(query)?;
        let doc = parser.parse_document()?;

        if doc.operations.is_empty() {
            return Err("No operations found in query".to_string());
        }

        // Execute the first operation
        let op = &doc.operations[0];
        Self::execute_selection_set(&op.selection_set, root)
    }

    fn execute_selection_set(selection_set: &SelectionSet, resolver: &dyn Resolver) -> Result<Value, String> {
        let mut result_map = HashMap::new();

        for selection in &selection_set.selections {
            match selection {
                Selection::Field(field) => {
                    let field_name = &field.name;
                    let result_key = field.alias.as_ref().unwrap_or(field_name).clone();

                    // PRODUCTION NOTE:
                    // In a production engine, missing fields are validated against the schema beforehand.
                    // Here we just return `null` or error dynamically.
                    let resolved = match resolver.resolve_field(field_name, &field.arguments) {
                        Some(Ok(val)) => Self::resolve_value(val, field.selection_set.as_ref())?,
                        Some(Err(e)) => return Err(format!("Field resolver error for '{}': {}", field_name, e)),
                        None => Value::Null,
                    };

                    result_map.insert(result_key, resolved);
                }
            }
        }

        Ok(Value::Object(result_map))
    }

    fn resolve_value(value: ResolvedValue, sub_selections: Option<&SelectionSet>) -> Result<Value, String> {
        match (value, sub_selections) {
            (ResolvedValue::Scalar(s), None) => Ok(s),
            (ResolvedValue::Scalar(_), Some(_)) => Err("Cannot have sub-selections on a scalar field".to_string()),
            (ResolvedValue::Object(obj), Some(sel)) => Self::execute_selection_set(sel, obj.as_ref()),
            (ResolvedValue::Object(_), None) => Err("Object field must have sub-selections".to_string()),
            (ResolvedValue::List(items), sel) => {
                let mut list_results = Vec::new();
                for item in items {
                    list_results.push(Self::resolve_value(item, sel)?);
                }
                Ok(Value::List(list_results))
            }
        }
    }
}

// =========================================================================================
// Alternative Approaches Footer
// =========================================================================================
//
// How this compares to `async-graphql` or `juniper`:
// - `async-graphql` uses procedural macros (`#[derive(Object)]`) to inspect Rust structs at compile-time,
//   generating a static, typed schema and fast dispatch code.
// - Our engine uses dynamic dispatch via the `Resolver` trait, making it schema-less at compile-time
//   but more akin to how a dynamically typed language (like JS/Apollo) might implement it.
//
// What's missing vs. production:
// - **Type validation / Schema introspection:** We don't validate the query against a formal Schema definition.
// - **Asynchronous resolvers:** We resolve everything synchronously.
// - **Fragments, Directives, and Subscriptions.**
// - **Variable resolution** outside the query string.

// =========================================================================================
// Tests
// =========================================================================================

#[cfg(test)]
mod tests {
    use super::*;

    struct UserResolver {
        id: i32,
        name: String,
    }

    impl Resolver for UserResolver {
        fn resolve_field(&self, name: &str, _args: &HashMap<String, Value>) -> Option<Result<ResolvedValue, String>> {
            match name {
                "id" => Some(Ok(ResolvedValue::Scalar(Value::Int(self.id)))),
                "name" => Some(Ok(ResolvedValue::Scalar(Value::String(self.name.clone())))),
                _ => None,
            }
        }
    }

    struct QueryRoot;

    impl Resolver for QueryRoot {
        fn resolve_field(&self, name: &str, args: &HashMap<String, Value>) -> Option<Result<ResolvedValue, String>> {
            match name {
                "hello" => Some(Ok(ResolvedValue::Scalar(Value::String("world".to_string())))),
                "user" => {
                    let id = match args.get("id") {
                        Some(Value::Int(i)) => *i,
                        _ => 1, // Default
                    };
                    Some(Ok(ResolvedValue::Object(Box::new(UserResolver {
                        id,
                        name: format!("User{}", id),
                    }))))
                }
                "users" => {
                    Some(Ok(ResolvedValue::List(vec![
                        ResolvedValue::Object(Box::new(UserResolver { id: 1, name: "Alice".to_string() })),
                        ResolvedValue::Object(Box::new(UserResolver { id: 2, name: "Bob".to_string() })),
                    ])))
                }
                _ => None,
            }
        }
    }

    #[test]
    fn test_basic_scalar_query() {
        let query = "{ hello }";
        let root = QueryRoot;
        let result = Engine::execute(query, &root).unwrap();

        let mut expected = HashMap::new();
        expected.insert("hello".to_string(), Value::String("world".to_string()));

        assert_eq!(result, Value::Object(expected));
    }

    #[test]
    fn test_nested_object_with_args() {
        let query = r#"{
            user(id: 42) {
                id
                name
            }
        }"#;
        let root = QueryRoot;
        let result = Engine::execute(query, &root).unwrap();

        let mut user_obj = HashMap::new();
        user_obj.insert("id".to_string(), Value::Int(42));
        user_obj.insert("name".to_string(), Value::String("User42".to_string()));

        let mut expected = HashMap::new();
        expected.insert("user".to_string(), Value::Object(user_obj));

        assert_eq!(result, Value::Object(expected));
    }

    #[test]
    fn test_list_and_aliases() {
        let query = r#"{
            friends: users {
                name
            }
        }"#;
        let root = QueryRoot;
        let result = Engine::execute(query, &root).unwrap();

        let mut alice = HashMap::new();
        alice.insert("name".to_string(), Value::String("Alice".to_string()));
        let mut bob = HashMap::new();
        bob.insert("name".to_string(), Value::String("Bob".to_string()));

        let mut expected = HashMap::new();
        expected.insert("friends".to_string(), Value::List(vec![
            Value::Object(alice),
            Value::Object(bob),
        ]));

        assert_eq!(result, Value::Object(expected));
    }

    #[test]
    fn test_syntax_error() {
        let query = "{ hello "; // missing closing brace
        let root = QueryRoot;
        let result = Engine::execute(query, &root);
        assert!(result.is_err());
    }
}
