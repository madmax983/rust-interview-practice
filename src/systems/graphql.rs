//! # GraphQL Execution Engine
//!
//! A minimal from-scratch GraphQL lexer, parser, and recursive execution engine.
//!
//! ## What This Implements
//! This module implements a core subset of a GraphQL execution engine, taking a query
//! string and a root resolution object, then parsing and executing the query to produce
//! a JSON-like map of results.
//!
//! ## Crates Replaced
//! - `async-graphql`
//! - `juniper`
//!
//! ## Real-World Systems
//! - Apollo GraphQL Server
//! - Hasura
//! - Absinthe (Elixir)
//!
//! ## Why Build It Yourself?
//! Understanding the parsing phases (Lexing -> Parsing -> Execution) of a query language
//! reveals how to map abstract syntax trees to dynamic data resolution. It also highlights
//! how Rust's trait system can be used to build recursive data resolvers safely.
//!
//! ## Architecture
//!
//! ### Lexer
//! Converts a raw string into a stream of tokens (`Ident`, `String`, `Punctuation`).
//! It uses iterative loops (not recursion) to avoid stack overflows on invalid/deep input.
//!
//! ### Parser
//! Takes tokens and builds an Abstract Syntax Tree (AST), representing Operations and
//! Selections (fields, sub-selections).
//!
//! ### Executor
//! Walks the AST and resolves fields using a `Resolver` trait. Evaluated values use owned
//! data types (e.g., `Value::String(String)`) to avoid complex lifetime issues with
//! dynamically generated trait objects during recursive resolution.
//!
//! ### Complexity
//! - **Lexing:** O(N) where N is the length of the query string.
//! - **Parsing:** O(T) where T is the number of tokens.
//! - **Execution:** O(S * R) where S is the size of the selection set and R is the time
//!   taken by resolvers.
//!
//! ## Benchmarking Note
//! Benchmarking a GraphQL engine involves testing lexing/parsing throughput (requests/sec)
//! and resolution latency. Use `criterion` to measure parse times of large queries.
//!
//! ## Footer
//!
//! ### Comparison to Canonical Crates
//! Crates like `async-graphql` provide macros to automatically derive resolvers for Rust
//! types, validate queries against a schema, handle variables, directives, and asynchronous
//! resolution.
//!
//! ### What's Missing
//! - Asynchronous resolution (this is purely synchronous).
//! - Schema validation (this blindly executes queries against the provided root).
//! - Variables, Aliases, Fragments, and Directives.
//! - Mutations and Subscriptions.
//!
//! ### Next Steps
//! - Add asynchronous resolvers using `BoxFuture`.
//! - Implement a Schema definition language (SDL) parser and validation phase.

use std::collections::HashMap;

/// Represents the types of tokens emitted by the lexer.
#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    Ident(String),
    String(String),
    Int(i64),
    Punct(char),
    Eof,
}

/// A minimal Lexer for GraphQL queries.
pub struct Lexer<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    fn peek_char(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn consume_char(&mut self) -> Option<char> {
        let c = self.peek_char()?;
        self.pos += c.len_utf8();
        Some(c)
    }

    /// Fetches the next token from the input.
    pub fn next_token(&mut self) -> Token {
        // PRODUCTION NOTE: We use an iterative `loop` rather than tail recursion to skip whitespace
        // and invalid characters. Tail recursion in lexers can cause stack overflows if the input
        // contains long sequences of invalid characters.
        loop {
            match self.peek_char() {
                Some(c) if c.is_whitespace() || c == ',' => {
                    self.consume_char();
                }
                Some('#') => {
                    // Skip comment
                    while let Some(c) = self.peek_char() {
                        if c == '\n' {
                            break;
                        }
                        self.consume_char();
                    }
                }
                Some('{') | Some('}') | Some('(') | Some(')') | Some(':') | Some('@') => {
                    return Token::Punct(self.consume_char().unwrap());
                }
                Some('"') => {
                    self.consume_char(); // skip opening quote
                    let mut string_val = String::new();
                    while let Some(c) = self.consume_char() {
                        if c == '"' {
                            break;
                        }
                        string_val.push(c);
                    }
                    return Token::String(string_val);
                }
                Some(c) if c.is_ascii_alphabetic() || c == '_' => {
                    let mut ident = String::new();
                    while let Some(c) = self.peek_char() {
                        if c.is_ascii_alphanumeric() || c == '_' {
                            ident.push(self.consume_char().unwrap());
                        } else {
                            break;
                        }
                    }
                    return Token::Ident(ident);
                }
                Some(c) if c.is_ascii_digit() || c == '-' => {
                    let mut num_str = String::new();
                    while let Some(c) = self.peek_char() {
                        if c.is_ascii_digit() || c == '-' {
                            num_str.push(self.consume_char().unwrap());
                        } else {
                            break;
                        }
                    }
                    if let Ok(val) = num_str.parse::<i64>() {
                        return Token::Int(val);
                    } else {
                        // GOTCHA: Silently ignoring invalid numbers for simplicity, but a real parser
                        // would return an error token.
                        return Token::Eof;
                    }
                }
                Some(_) => {
                    // Skip unrecognized char safely without recursion
                    self.consume_char();
                }
                None => return Token::Eof,
            }
        }
    }
}

/// Abstract Syntax Tree node for a Field in a GraphQL query.
#[derive(Debug, PartialEq, Clone)]
pub struct Field {
    pub name: String,
    // Arguments could be added here
    pub selection_set: Vec<Field>,
}

/// A minimal Parser for GraphQL queries.
pub struct Parser<'a> {
    lexer: Lexer<'a>,
    current_token: Token,
}

impl<'a> Parser<'a> {
    pub fn new(mut lexer: Lexer<'a>) -> Self {
        let current_token = lexer.next_token();
        Self { lexer, current_token }
    }

    fn advance(&mut self) {
        self.current_token = self.lexer.next_token();
    }

    /// Parses a selection set: `{ field1 field2 { subfield } }`
    pub fn parse_selection_set(&mut self) -> Result<Vec<Field>, String> {
        if self.current_token != Token::Punct('{') {
            return Err("Expected '{'".into());
        }
        self.advance(); // consume '{'

        let mut fields = Vec::new();
        while self.current_token != Token::Punct('}') && self.current_token != Token::Eof {
            fields.push(self.parse_field()?);
        }

        if self.current_token == Token::Punct('}') {
            self.advance(); // consume '}'
        } else {
            return Err("Expected '}'".into());
        }

        Ok(fields)
    }

    fn parse_field(&mut self) -> Result<Field, String> {
        let name = match &self.current_token {
            Token::Ident(n) => n.clone(),
            _ => return Err(format!("Expected identifier, got {:?}", self.current_token)),
        };
        self.advance(); // consume field name

        let mut selection_set = Vec::new();
        if self.current_token == Token::Punct('{') {
            selection_set = self.parse_selection_set()?;
        }

        Ok(Field { name, selection_set })
    }

    pub fn parse(&mut self) -> Result<Vec<Field>, String> {
        // For simplicity, we assume the root is a single query selection set.
        // A full parser would handle `query { ... }` or `mutation { ... }`.

        // Optional "query" keyword
        if let Token::Ident(name) = &self.current_token {
            if name == "query" {
                self.advance();
                // skip optional query name
                if let Token::Ident(_) = &self.current_token {
                    self.advance();
                }
            }
        }

        self.parse_selection_set()
    }
}

// RUST INSIGHT:
// Using `Value` to represent evaluation output. We use owned structures like `String` and
// `HashMap` to avoid tying the output lifetime to the input query string or resolvers.
// This greatly simplifies recursive dynamic dispatch where trait objects may create new
// values on the fly.
#[derive(Debug, PartialEq, Clone)]
pub enum Value {
    Null,
    String(String),
    Int(i64),
    Float(f64),
    Boolean(bool),
    List(Vec<Value>),
    Object(HashMap<String, Value>),
}

/// The Resolver trait defines how a GraphQL type resolves its fields.
pub trait Resolver {
    /// Resolves a single field by name. Returns `None` if the field is not supported.
    fn resolve_field(&self, name: &str) -> Option<Value>;

    /// Resolves a sub-selection, returning a new `Resolver` dynamically if this field
    /// returns an object.
    fn resolve_object(&self, name: &str) -> Option<Box<dyn Resolver>>;
}

/// Executes an AST against a root resolver.
pub fn execute(ast: &[Field], root: &dyn Resolver) -> Value {
    let mut result_map = HashMap::new();

    for field in ast {
        if field.selection_set.is_empty() {
            // Leaf node, resolve as value
            if let Some(val) = root.resolve_field(&field.name) {
                result_map.insert(field.name.clone(), val);
            } else {
                result_map.insert(field.name.clone(), Value::Null);
            }
        } else {
            // Object node, get the sub-resolver and execute recursively
            if let Some(sub_resolver) = root.resolve_object(&field.name) {
                let sub_val = execute(&field.selection_set, sub_resolver.as_ref());
                result_map.insert(field.name.clone(), sub_val);
            } else {
                result_map.insert(field.name.clone(), Value::Null);
            }
        }
    }

    Value::Object(result_map)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct UserResolver {
        id: i64,
        name: String,
    }

    impl Resolver for UserResolver {
        fn resolve_field(&self, name: &str) -> Option<Value> {
            match name {
                "id" => Some(Value::Int(self.id)),
                "name" => Some(Value::String(self.name.clone())),
                _ => None,
            }
        }

        fn resolve_object(&self, _name: &str) -> Option<Box<dyn Resolver>> {
            None
        }
    }

    struct RootResolver;

    impl Resolver for RootResolver {
        fn resolve_field(&self, name: &str) -> Option<Value> {
            match name {
                "version" => Some(Value::String("1.0".to_string())),
                _ => None,
            }
        }

        fn resolve_object(&self, name: &str) -> Option<Box<dyn Resolver>> {
            match name {
                "me" => Some(Box::new(UserResolver {
                    id: 42,
                    name: "Alice".to_string(),
                })),
                _ => None,
            }
        }
    }

    #[test]
    fn test_lexer() {
        let input = "{ me { id name } }";
        let mut lexer = Lexer::new(input);

        assert_eq!(lexer.next_token(), Token::Punct('{'));
        assert_eq!(lexer.next_token(), Token::Ident("me".to_string()));
        assert_eq!(lexer.next_token(), Token::Punct('{'));
        assert_eq!(lexer.next_token(), Token::Ident("id".to_string()));
        assert_eq!(lexer.next_token(), Token::Ident("name".to_string()));
        assert_eq!(lexer.next_token(), Token::Punct('}'));
        assert_eq!(lexer.next_token(), Token::Punct('}'));
        assert_eq!(lexer.next_token(), Token::Eof);
    }

    #[test]
    fn test_lexer_skip_invalid_iteratively() {
        let input = "id ^&*% name";
        let mut lexer = Lexer::new(input);

        assert_eq!(lexer.next_token(), Token::Ident("id".to_string()));
        assert_eq!(lexer.next_token(), Token::Ident("name".to_string()));
        assert_eq!(lexer.next_token(), Token::Eof);
    }

    #[test]
    fn test_parser() {
        let input = "query { me { id name } version }";
        let lexer = Lexer::new(input);
        let mut parser = Parser::new(lexer);

        let ast = parser.parse().unwrap();
        assert_eq!(ast.len(), 2);
        assert_eq!(ast[0].name, "me");
        assert_eq!(ast[0].selection_set.len(), 2);
        assert_eq!(ast[0].selection_set[0].name, "id");
        assert_eq!(ast[0].selection_set[1].name, "name");
        assert_eq!(ast[1].name, "version");
    }

    #[test]
    fn test_executor() {
        let input = "{ me { id name } version }";
        let lexer = Lexer::new(input);
        let mut parser = Parser::new(lexer);
        let ast = parser.parse().unwrap();

        let root = RootResolver;
        let result = execute(&ast, &root);

        if let Value::Object(map) = result {
            assert_eq!(map.get("version"), Some(&Value::String("1.0".to_string())));

            if let Some(Value::Object(me_map)) = map.get("me") {
                assert_eq!(me_map.get("id"), Some(&Value::Int(42)));
                assert_eq!(me_map.get("name"), Some(&Value::String("Alice".to_string())));
            } else {
                panic!("'me' should be an object");
            }
        } else {
            panic!("Root result should be an object");
        }
    }
}
