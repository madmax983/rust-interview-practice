//! # GraphQL Execution Engine
//!
//! Implements a simplified from-scratch GraphQL lexer, parser, and recursive execution engine.
//! It transforms a raw string query into an AST and resolves it against typed Rust structs using traits.
//!
//! **Replaces Crates:** `async-graphql`, `juniper`
//!
//! **Real-world Usage:**
//! - Core query processing engine in API gateways.
//! - Subgraph resolution in federated GraphQL architecture.
//! - Custom query languages for specialized data stores.
//!
//! **Why build it yourself?**
//! Building a GraphQL engine demystifies the magic of how a text query is parsed into a tree,
//! and how resolvers dynamically map that tree to static application data. It forces you to grapple
//! with recursive descent parsing and the trade-offs of dynamic vs. static typing in Rust.
//!
//! # Architecture
//!
//! Flow:
//!
//!      [String Query]
//!           │
//!           ▼
//!        Lexer (Iterative Tokenization)
//!           │
//!           ▼
//!      [Stream of Tokens]
//!           │
//!           ▼
//!        Parser (Recursive Descent)
//!           │
//!           ▼
//!      [AST: Vec<Field>]
//!           │
//!           ▼
//!      Execution Engine (execute_selection_set + Resolve Trait)
//!           │
//!           ▼
//!      [Value (JSON-like output)]
//!
//! Invariants:
//! 1. Lexing must not stack overflow on malformed/garbage input (use iterative loops).
//! 2. The parser produces a strictly valid AST representing the query selection set.
//! 3. The execution engine must return exactly the shape requested in the query.
//!
//! Complexity:
//! ┌───────────────────┬──────────────┬──────────────┐
//! │ Operation         │ Time         │ Space        │
//! ├───────────────────┼──────────────┼──────────────┤
//! │ Lexing            │ O(N)         │ O(N) tokens  │
//! │ Parsing           │ O(N) tokens  │ O(N) AST depth│
//! │ Execution         │ O(F) fields  │ O(F) result  │
//! └───────────────────┴──────────────┴──────────────┘
//!
//! Design Decisions:
//! - **Iterative Lexer**: Explicitly avoids tail recursion for skipping whitespace/invalid characters to prevent stack overflows.
//! - **Trait-based Resolution**: Uses a `Resolve` trait to allow custom structs to define how their fields are fetched,
//!   enabling deep integration with Rust's type system without needing heavy macros or reflection.
//!
//! # Comparison to Canonical Crates
//! - `async-graphql`: Heavily relies on proc-macros to generate resolver boilerplate and is fully async.
//!   Our implementation uses manual trait implementations to show the underlying mechanics clearly.
//! - `juniper`: Similar trait-heavy approach, but significantly more robust in handling variables, mutations,
//!   fragments, and schema introspection.
//!
//! # Missing Features vs. Production
//! - **Async Support**: Production resolvers are almost always asynchronous to handle DB/network I/O.
//! - **Arguments & Variables**: This minimal engine only parses fields and nested selection sets.
//! - **Fragments, Mutations, Subscriptions**: Not supported.
//! - **Schema Validation**: Execution happens optimistically without validating against a predefined schema.
//!
//! # Next Steps
//! 1. Extend the AST to support Arguments (e.g., `user(id: 1) { ... }`).
//! 2. Make the `Resolve` trait asynchronous using `async trait` or returning a Future.
//!
//! # Benchmarking Note
//! A production implementation would be benchmarked against `async-graphql` or `juniper` using the `criterion` crate.
//! You would benchmark lexing, parsing, and execution separately using `criterion::black_box` to prevent compiler optimizations from eliminating the workload.
//! 3. Implement schema introspection.

use std::collections::HashMap;

// =========================================================================================
// AST and Types
// =========================================================================================

/// Tokens emitted by the Lexer.
#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    Name(String),
    LBrace,
    RBrace,
    Eof,
}

/// Represents a requested field in the GraphQL query AST.
#[derive(Debug, PartialEq, Clone)]
pub struct Field {
    pub name: String,
    pub selection_set: Vec<Field>,
}

/// JSON-like value enum representing the output of a GraphQL query.
#[derive(Debug, PartialEq, Clone)]
pub enum Value {
    Null,
    String(String),
    Int(i64),
    Float(f64),
    Boolean(bool),
    Object(HashMap<String, Value>),
    List(Vec<Value>),
}

// =========================================================================================
// Lexer
// =========================================================================================

/// A minimal lexical analyzer for GraphQL queries.
pub struct Lexer<'a> {
    input: &'a [u8],
    pos: usize,
}

impl<'a> Lexer<'a> {
    /// Creates a new Lexer from a string slice.
    #[must_use]
    pub fn new(input: &'a str) -> Self {
        Self {
            input: input.as_bytes(),
            pos: 0,
        }
    }

    /// Iteratively pulls the next token from the input stream.
    pub fn next_token(&mut self) -> Token {
        // RUST INSIGHT: We use an iterative `loop` here instead of tail recursion
        // (like calling `self.next_token()` to skip unrecognized characters).
        // Rust does not guarantee Tail Call Optimization (TCO), so long sequences
        // of whitespace or invalid grammar could cause a stack overflow.
        loop {
            if self.pos >= self.input.len() {
                return Token::Eof;
            }

            match self.input[self.pos] {
                // Skip whitespace and structural commas
                b' ' | b'\t' | b'\n' | b'\r' | b',' => {
                    self.pos += 1;
                }
                b'{' => {
                    self.pos += 1;
                    return Token::LBrace;
                }
                b'}' => {
                    self.pos += 1;
                    return Token::RBrace;
                }
                c if c.is_ascii_alphabetic() || c == b'_' => {
                    let start = self.pos;
                    while self.pos < self.input.len()
                        && (self.input[self.pos].is_ascii_alphanumeric()
                            || self.input[self.pos] == b'_')
                    {
                        self.pos += 1;
                    }
                    // UNSAFE JUSTIFICATION: We strictly validated that all bytes between `start` and `self.pos`
                    // are ASCII alphanumeric or underscore. Therefore, it is guaranteed to be valid UTF-8.
                    let s = unsafe { std::str::from_utf8_unchecked(&self.input[start..self.pos]) };
                    return Token::Name(s.to_string());
                }
                _ => {
                    // Skip invalid characters safely without recursion
                    self.pos += 1;
                }
            }
        }
    }
}

// =========================================================================================
// Parser
// =========================================================================================

/// A recursive descent parser that builds an AST of Fields.
pub struct Parser<'a> {
    lexer: Lexer<'a>,
    current_token: Token,
}

impl<'a> Parser<'a> {
    /// Creates a new Parser and primes the first token.
    #[must_use]
    pub fn new(mut lexer: Lexer<'a>) -> Self {
        let current_token = lexer.next_token();
        Self { lexer, current_token }
    }

    fn advance(&mut self) {
        self.current_token = self.lexer.next_token();
    }

    /// Parses a selection set (e.g., `{ id name }`).
    ///
    /// # Errors
    /// Returns an error string if the query grammar is invalid.
    pub fn parse_selection_set(&mut self) -> Result<Vec<Field>, String> {
        let mut fields = Vec::new();

        if self.current_token == Token::LBrace {
            self.advance(); // consume '{'

            while self.current_token != Token::RBrace && self.current_token != Token::Eof {
                if let Token::Name(name) = &self.current_token {
                    let field_name = name.clone();
                    self.advance(); // consume name

                    let mut selection_set = Vec::new();
                    // If a nested selection set follows, parse it recursively
                    if self.current_token == Token::LBrace {
                        selection_set = self.parse_selection_set()?;
                    }

                    fields.push(Field {
                        name: field_name,
                        selection_set,
                    });
                } else {
                    return Err(format!("Expected field name, got {:?}", self.current_token));
                }
            }

            if self.current_token == Token::RBrace {
                self.advance(); // consume '}'
            } else {
                return Err("Expected '}' to close selection set".to_string());
            }
        }

        Ok(fields)
    }
}

// =========================================================================================
// Execution Engine
// =========================================================================================

/// Trait implemented by any struct that can resolve GraphQL fields.
pub trait Resolve {
    /// Resolves a single requested field into a `Value`.
    ///
    /// # Errors
    /// Returns an error if the field is unknown or cannot be resolved.
    fn resolve(&self, field: &Field) -> Result<Value, String>;
}

/// Executes an entire selection set against a root `Resolve` object.
///
/// # Errors
/// Returns an error if any field resolution fails.
pub fn execute_selection_set<T: Resolve>(
    object: &T,
    selection_set: &[Field],
) -> Result<Value, String> {
    // PRODUCTION NOTE: A production engine would collect these into a pre-allocated structure
    // and potentially execute sibling fields concurrently (for async resolution).
    let mut result_map = HashMap::with_capacity(selection_set.len());

    for field in selection_set {
        let resolved_value = object.resolve(field)?;
        result_map.insert(field.name.clone(), resolved_value);
    }

    Ok(Value::Object(result_map))
}

// =========================================================================================
// Tests
// =========================================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Mock Data Models
    struct User {
        id: i64,
        name: String,
        email: String,
    }

    impl Resolve for User {
        fn resolve(&self, field: &Field) -> Result<Value, String> {
            match field.name.as_str() {
                "id" => Ok(Value::Int(self.id)),
                "name" => Ok(Value::String(self.name.clone())),
                "email" => Ok(Value::String(self.email.clone())),
                _ => Err(format!("Unknown field on User: {}", field.name)),
            }
        }
    }

    struct Query {
        me: User,
    }

    impl Resolve for Query {
        fn resolve(&self, field: &Field) -> Result<Value, String> {
            match field.name.as_str() {
                "me" => {
                    // GOTCHA: If the query requests a complex object (`User`), it MUST provide
                    // a selection set of fields. If not, it's a semantic error.
                    if field.selection_set.is_empty() {
                        return Err("Field 'me' of type User must have a selection of subfields".to_string());
                    }
                    // Recursively delegate execution of the sub-selection set to the `User` object.
                    execute_selection_set(&self.me, &field.selection_set)
                }
                "ping" => Ok(Value::String("pong".to_string())),
                _ => Err(format!("Unknown field on Query: {}", field.name)),
            }
        }
    }

    #[test]
    fn test_lexer() {
        let query = "{ me { id \n name } }";
        let mut lexer = Lexer::new(query);

        assert_eq!(lexer.next_token(), Token::LBrace);
        assert_eq!(lexer.next_token(), Token::Name("me".to_string()));
        assert_eq!(lexer.next_token(), Token::LBrace);
        assert_eq!(lexer.next_token(), Token::Name("id".to_string()));
        assert_eq!(lexer.next_token(), Token::Name("name".to_string()));
        assert_eq!(lexer.next_token(), Token::RBrace);
        assert_eq!(lexer.next_token(), Token::RBrace);
        assert_eq!(lexer.next_token(), Token::Eof);
    }

    #[test]
    fn test_parser() {
        let query = "{ me { id name } }";
        let lexer = Lexer::new(query);
        let mut parser = Parser::new(lexer);

        let ast = parser.parse_selection_set().unwrap();

        assert_eq!(ast.len(), 1);
        assert_eq!(ast[0].name, "me");
        assert_eq!(ast[0].selection_set.len(), 2);
        assert_eq!(ast[0].selection_set[0].name, "id");
        assert_eq!(ast[0].selection_set[1].name, "name");
    }

    #[test]
    fn test_execution_engine() {
        let root = Query {
            me: User {
                id: 42,
                name: "Alice".to_string(),
                email: "alice@example.com".to_string(),
            },
        };

        let query = "
        {
            ping
            me {
                id
                email
            }
        }
        ";

        let lexer = Lexer::new(query);
        let mut parser = Parser::new(lexer);
        let ast = parser.parse_selection_set().unwrap();

        let result = execute_selection_set(&root, &ast).unwrap();

        match result {
            Value::Object(map) => {
                assert_eq!(map.get("ping"), Some(&Value::String("pong".to_string())));

                if let Some(Value::Object(me_map)) = map.get("me") {
                    assert_eq!(me_map.get("id"), Some(&Value::Int(42)));
                    assert_eq!(me_map.get("email"), Some(&Value::String("alice@example.com".to_string())));
                    assert_eq!(me_map.get("name"), None); // Was not requested
                } else {
                    panic!("'me' field was not an object");
                }
            }
            _ => panic!("Expected Object as root result"),
        }
    }

    #[test]
    fn test_execution_unknown_field() {
        let root = Query {
            me: User {
                id: 42,
                name: "Alice".to_string(),
                email: "".to_string(),
            },
        };

        // 'age' does not exist on User
        let query = "{ me { id age } }";
        let lexer = Lexer::new(query);
        let mut parser = Parser::new(lexer);
        let ast = parser.parse_selection_set().unwrap();

        let result = execute_selection_set(&root, &ast);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Unknown field on User: age");
    }
}
