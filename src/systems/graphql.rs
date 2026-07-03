//! # GraphQL Execution Engine
//!
//! Implements a minimal GraphQL lexer, parser, and trait-based recursive execution engine.
//!
//! **Replaces Crates:** `async-graphql`, `juniper`
//!
//! **Real-world Usage:**
//! - API gateways (Apollo Router, Netflix DGS)
//! - Data aggregation layers
//! - Client-driven API endpoints
//!
//! **Why build it yourself?**
//! Building a GraphQL engine demystifies how query strings are transformed into executable ASTs and resolved against a schema.
//! It teaches you about recursive descent parsing, iterative lexing, and how to build a dynamic trait-based resolver engine in Rust.
//! You'll also learn how to manage memory safely (using owned data types rather than zero-copy string slices) when dealing with dynamically generated trait objects.
//!
//! **Benchmarking Note:**
//! To benchmark the GraphQL engine, use the `criterion` crate to measure parsing and execution times for large nested queries.
//! ```rust,ignore
//! b.iter(|| Executor::execute(black_box(&doc), black_box(&root)));
//! ```
//!
//! ## Architecture
//!
//! The execution engine consists of three main components:
//! 1.  **Lexer:** Converts a GraphQL query string into a stream of tokens. It uses an iterative `while let` loop instead of tail recursion to prevent stack overflows on invalid input.
//! 2.  **Parser:** Consumes tokens to build an Abstract Syntax Tree (AST), representing queries and their selected fields.
//! 3.  **Executor:** A trait-based recursive resolver that takes the AST and a root query object, executing the required fields.
//!
//! ### Data Structures
//! - `Token`: Enum representing lexical tokens (Ident, Brace, String, etc.).
//! - `Document`: The top-level AST node containing definitions (queries).
//! - `Field`: Represents a requested field and its sub-selections.
//! - `Value`: Represents the evaluated output data (String, Int, Boolean, List, Object).
//!
//! ### Invariants
//! - **Iterative Lexing:** The lexer must never use tail recursion. Invalid characters must be skipped safely in an iterative loop to avoid stack overflows on malformed input.
//! - **Owned Output:** The evaluated output (`Value`) must use owned data types (e.g., `String`) rather than borrowed string slices to avoid lifetime constraints with dynamic resolvers.
//! - **Safe Execution:** The executor must gracefully handle missing fields by returning `Value::Null` instead of panicking.
//!
//! ### Complexity
//! - **Lexing:** O(N) time, where N is the length of the query string.
//! - **Parsing:** O(N) time to build the AST.
//! - **Execution:** O(N * M) time, where N is the number of requested fields and M is the average time to resolve a field.
//!
//! ## Implementation
//!
//! **RUST INSIGHT:**
//! We use a dynamic trait-based approach (`Box<dyn Resolver>`) to allow flexible schema definitions. Rust's trait objects require static dispatch by default, but using `dyn Resolver` allows us to store heterogeneous types in the schema as long as they implement the `Resolver` trait.
//!
//! **GOTCHA:**
//! The output representation must use owned strings (`String`) instead of zero-copy references (`&str`). Since resolvers can be dynamically generated or loaded from a database, their lifetimes cannot be tied to the input query string or the executor itself. Using `&str` in `Value` would lead to borrowing conflicts.

use std::collections::HashMap;

/// Represents lexical tokens in a GraphQL query.
#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    Ident(String),
    String(String),
    Int(i64),
    Punctuation(char),
    Eof,
}

/// A minimal lexer for GraphQL queries.
pub struct Lexer<'a> {
    input: std::str::Chars<'a>,
    peeked: Option<char>,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        let mut chars = input.chars();
        let peeked = chars.next();
        Self {
            input: chars,
            peeked,
        }
    }

    fn advance(&mut self) -> Option<char> {
        let current = self.peeked;
        self.peeked = self.input.next();
        current
    }

    fn peek(&self) -> Option<char> {
        self.peeked
    }

    /// Skips whitespace and comments.
    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_whitespace() || c == ',' {
                self.advance();
            } else if c == '#' {
                // Skip comment until newline
                while let Some(comment_char) = self.advance() {
                    if comment_char == '\n' {
                        break;
                    }
                }
            } else {
                break;
            }
        }
    }

    /// Consumes the next token from the input.
    ///
    /// **GOTCHA:**
    /// We use an iterative `loop` instead of tail recursion to skip unrecognized characters.
    /// If we used recursion (e.g., `self.next_token()`), a long string of invalid characters could cause a stack overflow.
    pub fn next_token(&mut self) -> Token {
        loop {
            self.skip_whitespace();

            let Some(c) = self.advance() else {
                return Token::Eof;
            };

            match c {
                '{' | '}' | '(' | ')' | ':' | '[' | ']' | '!' => {
                    return Token::Punctuation(c);
                }
                '"' => {
                    let mut s = String::new();
                    while let Some(nc) = self.advance() {
                        if nc == '"' {
                            break;
                        }
                        s.push(nc);
                    }
                    return Token::String(s);
                }
                c if c.is_ascii_alphabetic() || c == '_' => {
                    let mut ident = String::new();
                    ident.push(c);
                    while let Some(nc) = self.peek() {
                        if nc.is_ascii_alphanumeric() || nc == '_' {
                            ident.push(self.advance().unwrap());
                        } else {
                            break;
                        }
                    }
                    return Token::Ident(ident);
                }
                c if c.is_ascii_digit() || c == '-' => {
                    let mut num_str = String::new();
                    num_str.push(c);
                    while let Some(nc) = self.peek() {
                        if nc.is_ascii_digit() {
                            num_str.push(self.advance().unwrap());
                        } else {
                            break;
                        }
                    }
                    if let Ok(num) = num_str.parse::<i64>() {
                        return Token::Int(num);
                    }
                    // If parsing fails, just continue the loop to skip invalid characters.
                }
                _ => {
                    // Invalid character, skip and loop
                }
            }
        }
    }
}

/// Abstract Syntax Tree node for a requested field.
#[derive(Debug, PartialEq, Clone)]
pub struct Field {
    pub name: String,
    pub alias: Option<String>,
    pub selections: Vec<Field>,
}

/// Abstract Syntax Tree top-level document.
#[derive(Debug, PartialEq)]
pub struct Document {
    pub operations: Vec<Field>,
}

/// A minimal parser for GraphQL queries.
pub struct Parser<'a> {
    lexer: Lexer<'a>,
    current_token: Token,
}

impl<'a> Parser<'a> {
    pub fn new(mut lexer: Lexer<'a>) -> Self {
        let current_token = lexer.next_token();
        Self {
            lexer,
            current_token,
        }
    }

    fn advance(&mut self) {
        self.current_token = self.lexer.next_token();
    }

    /// Parses a field selection set `{ field1 field2 { subfield } }`.
    fn parse_selection_set(&mut self) -> Result<Vec<Field>, String> {
        if self.current_token != Token::Punctuation('{') {
            return Ok(vec![]); // No selection set
        }
        self.advance(); // consume '{'

        let mut selections = Vec::new();
        while self.current_token != Token::Punctuation('}') && self.current_token != Token::Eof {
            selections.push(self.parse_field()?);
        }

        if self.current_token == Token::Punctuation('}') {
            self.advance(); // consume '}'
            Ok(selections)
        } else {
            Err("Expected '}'".to_string())
        }
    }

    /// Parses a single field, handling optional aliases.
    fn parse_field(&mut self) -> Result<Field, String> {
        let mut name_or_alias = match &self.current_token {
            Token::Ident(name) => name.clone(),
            _ => return Err(format!("Expected identifier, got {:?}", self.current_token)),
        };
        self.advance();

        let mut alias = None;
        let name = if self.current_token == Token::Punctuation(':') {
            self.advance(); // consume ':'
            alias = Some(name_or_alias);
            match &self.current_token {
                Token::Ident(n) => {
                    let n = n.clone();
                    self.advance();
                    n
                }
                _ => return Err("Expected identifier after ':'".to_string()),
            }
        } else {
            name_or_alias
        };

        // Skip arguments for simplicity
        if self.current_token == Token::Punctuation('(') {
            self.advance();
            while self.current_token != Token::Punctuation(')') && self.current_token != Token::Eof {
                self.advance();
            }
            if self.current_token == Token::Punctuation(')') {
                self.advance();
            }
        }

        let selections = self.parse_selection_set()?;

        Ok(Field {
            name,
            alias,
            selections,
        })
    }

    /// Parses the top-level document, assuming a single implicit query operation for simplicity.
    pub fn parse(&mut self) -> Result<Document, String> {
        let mut operations = Vec::new();

        if let Token::Ident(op_type) = &self.current_token {
            if op_type == "query" || op_type == "mutation" {
                self.advance();
                // skip optional name
                if let Token::Ident(_) = &self.current_token {
                    self.advance();
                }
            }
        }

        let selections = self.parse_selection_set()?;

        // Wrap implicit selections in a root operation field
        operations.push(Field {
            name: "query".to_string(),
            alias: None,
            selections,
        });

        Ok(Document { operations })
    }
}

/// Represents evaluated output data.
///
/// **RUST INSIGHT:**
/// We use owned `String` types here instead of `&str` lifetimes. This is crucial because dynamic resolvers
/// might fetch data from a database or generate it on the fly, meaning the output data's lifetime cannot be tied
/// to the query string or the executor.
#[derive(Debug, PartialEq, Clone)]
pub enum Value {
    Null,
    Int(i64),
    Float(f64),
    String(String),
    Boolean(bool),
    List(Vec<Value>),
    Object(HashMap<String, Value>),
}

/// The core trait for objects that can resolve GraphQL fields.
pub trait Resolver {
    fn resolve_field(&self, name: &str) -> Option<Value>;
}

/// The Execution Engine.
pub struct Executor;

impl Executor {
    /// Recursively executes a requested field against a resolver.
    fn execute_field(field: &Field, resolver: &dyn Resolver) -> Value {
        match resolver.resolve_field(&field.name) {
            Some(Value::Object(map)) => {
                let mut result_map = HashMap::new();
                for sub_field in &field.selections {
                    let key = sub_field.alias.as_ref().unwrap_or(&sub_field.name);

                    // To support recursive execution, we wrap the HashMap in a generic resolver
                    struct MapResolver<'a>(&'a HashMap<String, Value>);
                    impl<'a> Resolver for MapResolver<'a> {
                        fn resolve_field(&self, name: &str) -> Option<Value> {
                            self.0.get(name).cloned()
                        }
                    }

                    let child_resolver = MapResolver(&map);
                    result_map.insert(key.clone(), Self::execute_field(sub_field, &child_resolver));
                }
                Value::Object(result_map)
            }
            Some(Value::List(items)) => {
                // A full implementation would apply sub-selections to each item in the list.
                // Simplified here.
                Value::List(items)
            }
            Some(val) => val,
            None => Value::Null,
        }
    }

    /// Executes an entire document against a root schema resolver.
    pub fn execute(document: &Document, root_resolver: &dyn Resolver) -> Value {
        let mut result = HashMap::new();

        if let Some(op) = document.operations.first() {
             for selection in &op.selections {
                 let key = selection.alias.as_ref().unwrap_or(&selection.name);
                 let value = Self::execute_field(selection, root_resolver);
                 result.insert(key.clone(), value);
             }
        }

        Value::Object(result)
    }
}

/// ## Footer
///
/// **Crate Comparison:**
/// Canonical crates like `async-graphql` and `juniper` use procedural macros to derive resolvers automatically from Rust structs and traits, deeply integrating with `async`/`await` for concurrent field resolution.
///
/// **Missing Features:**
/// - **Validation:** Full GraphQL validation against a schema (types, non-nullability, fragments) is omitted.
/// - **Variables and Arguments:** Argument parsing and execution context threading are simplified.
/// - **Async Resolvers:** Real engines resolve fields concurrently via Futures to avoid blocking on I/O.
/// - **Introspection:** Missing the `__schema` introspection system.
///
/// **Next Steps:**
/// To make this production-ready, implement a formal Schema definition type and validate the AST against it before execution. Convert the `Resolver` trait to return `BoxFuture<'a, Value>` to enable async data fetching.

#[cfg(test)]
mod tests {
    use super::*;

    struct QueryRoot;

    impl Resolver for QueryRoot {
        fn resolve_field(&self, name: &str) -> Option<Value> {
            match name {
                "me" => {
                    let mut user = HashMap::new();
                    user.insert("id".to_string(), Value::Int(1));
                    user.insert("name".to_string(), Value::String("Alice".to_string()));
                    Some(Value::Object(user))
                }
                "status" => Some(Value::String("OK".to_string())),
                _ => None,
            }
        }
    }

    #[test]
    fn test_lexer() {
        let query = "{ me { id name } }";
        let mut lexer = Lexer::new(query);

        assert_eq!(lexer.next_token(), Token::Punctuation('{'));
        assert_eq!(lexer.next_token(), Token::Ident("me".to_string()));
        assert_eq!(lexer.next_token(), Token::Punctuation('{'));
        assert_eq!(lexer.next_token(), Token::Ident("id".to_string()));
        assert_eq!(lexer.next_token(), Token::Ident("name".to_string()));
        assert_eq!(lexer.next_token(), Token::Punctuation('}'));
        assert_eq!(lexer.next_token(), Token::Punctuation('}'));
        assert_eq!(lexer.next_token(), Token::Eof);
    }

    #[test]
    fn test_lexer_invalid_chars() {
        let query = "{ me ^ % $ { id } }"; // contains invalid chars
        let mut lexer = Lexer::new(query);

        assert_eq!(lexer.next_token(), Token::Punctuation('{'));
        assert_eq!(lexer.next_token(), Token::Ident("me".to_string()));
        // Lexer should skip invalid characters iteratively
        assert_eq!(lexer.next_token(), Token::Punctuation('{'));
        assert_eq!(lexer.next_token(), Token::Ident("id".to_string()));
        assert_eq!(lexer.next_token(), Token::Punctuation('}'));
        assert_eq!(lexer.next_token(), Token::Punctuation('}'));
        assert_eq!(lexer.next_token(), Token::Eof);
    }

    #[test]
    fn test_parser() {
        let query = "{ user: me { id } }";
        let lexer = Lexer::new(query);
        let mut parser = Parser::new(lexer);

        let doc = parser.parse().unwrap();
        assert_eq!(doc.operations.len(), 1);
        let root_op = &doc.operations[0];
        assert_eq!(root_op.selections.len(), 1);

        let field = &root_op.selections[0];
        assert_eq!(field.name, "me");
        assert_eq!(field.alias, Some("user".to_string()));
        assert_eq!(field.selections.len(), 1);
        assert_eq!(field.selections[0].name, "id");
    }

    #[test]
    fn test_executor() {
        let query = "{ status, me { id name email } }";
        let lexer = Lexer::new(query);
        let mut parser = Parser::new(lexer);
        let doc = parser.parse().unwrap();

        let root = QueryRoot;
        let result = Executor::execute(&doc, &root);

        if let Value::Object(map) = result {
            assert_eq!(map.get("status"), Some(&Value::String("OK".to_string())));

            if let Some(Value::Object(me_map)) = map.get("me") {
                assert_eq!(me_map.get("id"), Some(&Value::Int(1)));
                assert_eq!(me_map.get("name"), Some(&Value::String("Alice".to_string())));
                assert_eq!(me_map.get("email"), Some(&Value::Null)); // Missing field returns Null
            } else {
                panic!("'me' should be an object");
            }
        } else {
            panic!("Result should be an object");
        }
    }
}
