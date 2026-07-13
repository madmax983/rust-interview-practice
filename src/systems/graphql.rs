//! # GraphQL Execution Engine
//!
//! What this implements and what crate(s) it replaces:
//! This implements a minimal, trait-based GraphQL execution engine. It replaces crates like
//! `async-graphql` and `juniper` by providing a minimal lexer, parser, and recursive resolution
//! engine to understand how queries map to typed object fields.
//!
//! Real-world Usage:
//! - API Gateways (Apollo Server) parsing queries and aggregating microservices.
//! - Database wrappers (Hasura) generating SQL from GraphQL ASTs.
//! - Client applications wrapping complex data-fetching into single cohesive requests.
//!
//! Why build it yourself?
//! GraphQL feels magical until you write the engine. You learn how a plain string is
//! tokenized, parsed into an Abstract Syntax Tree (AST), and evaluated against a
//! tree of generic trait objects. This exposes the hidden cost of nested queries
//! (N+1 problem) and demonstrates Rust's pattern matching for compiler-guided parsing.
//!
//! # Architecture
//!
//! Data Structure / Process:
//!
//! ```text
//! String Query --> [ Lexer ] --> [Tokens] --> [ Parser ] --> [ AST ]
//!                                                               |
//!                                                               V
//! [ Executor ] <--- (Resolves against Trait Objects) <---------/
//! ```
//!
//! Invariants:
//! 1. The lexer never recurses deeply (avoids stack overflows on pathological input).
//! 2. The parser produces a valid Abstract Syntax Tree matching the structure of the request.
//! 3. The executor matches requested AST fields against statically typed traits.
//!
//! Complexity:
//! | Operation     | Time Complexity | Space Complexity |
//! |---------------|-----------------|------------------|
//! | Lexing        | O(N)            | O(N)             |
//! | Parsing       | O(T)            | O(T)             |
//! | Execution     | O(F * D)        | O(D)             |
//! (N = input chars, T = tokens, F = fields, D = max depth)
//!
//! Design Decisions & Tradeoffs:
//! - Iterative Lexer: avoids tail-call recursion which Rust does not optimize, preventing stack overflows.
//! - Pull-based Lexing vs Eager Tokenization: We eagerly tokenize for simplicity here, but a true
//!   production lexer might stream tokens to save memory.
//! - Abstract Syntax Tree: Simplified to support just Objects, Fields, and basic Arguments.
//!
//! # Benchmarking Note
//! A production engine would benchmark the execution phase independent of parsing.
//! You can use `std::hint::black_box` to wrap a large AST and measure `Executor::execute`
//! timing without optimizer interference.
//!
//! # Footer
//!
//! Canonical crates like `async-graphql` feature full schema validation, introspection,
//! asynchronous resolution, subscriptions, and custom scalars.
//! Our implementation focuses purely on the Lex -> Parse -> Execute pipeline.
//! Next steps: adding Variables, Fragments, and parallel field execution.

use std::collections::HashMap;

/// A Token emitted by the GraphQL Lexer.
#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    Name(String),
    BraceOpen,
    BraceClose,
    ParenOpen,
    ParenClose,
    Colon,
    String(String),
    Int(i64),
}

/// Iterative Lexer for a subset of GraphQL.
pub struct Lexer<'a> {
    input: &'a [u8],
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            input: input.as_bytes(),
            pos: 0,
        }
    }

    /// Emits the next token. Uses iteration instead of tail recursion to prevent stack overflows
    /// on malformed input with long sequences of invalid characters.
    pub fn next_token(&mut self) -> Option<Token> {
        loop {
            if self.pos >= self.input.len() {
                return None;
            }

            let c = self.input[self.pos];
            match c {
                // RUST INSIGHT: Match on bytes is exceptionally fast and allows the compiler
                // to generate efficient jump tables.
                b' ' | b'\n' | b'\r' | b'\t' | b',' => {
                    self.pos += 1;
                }
                b'{' => {
                    self.pos += 1;
                    return Some(Token::BraceOpen);
                }
                b'}' => {
                    self.pos += 1;
                    return Some(Token::BraceClose);
                }
                b'(' => {
                    self.pos += 1;
                    return Some(Token::ParenOpen);
                }
                b')' => {
                    self.pos += 1;
                    return Some(Token::ParenClose);
                }
                b':' => {
                    self.pos += 1;
                    return Some(Token::Colon);
                }
                b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
                    let start = self.pos;
                    while self.pos < self.input.len() {
                        let ch = self.input[self.pos];
                        if ch.is_ascii_alphanumeric() || ch == b'_' {
                            self.pos += 1;
                        } else {
                            break;
                        }
                    }
                    // PRODUCTION NOTE: Zero-copy strings using `&'a str` instead of `String`
                    // avoid allocation, but we use `String` here for straightforward ownership.
                    let name = String::from_utf8_lossy(&self.input[start..self.pos]).to_string();
                    return Some(Token::Name(name));
                }
                b'"' => {
                    self.pos += 1;
                    let start = self.pos;
                    while self.pos < self.input.len() && self.input[self.pos] != b'"' {
                        self.pos += 1;
                    }
                    let s = String::from_utf8_lossy(&self.input[start..self.pos]).to_string();
                    if self.pos < self.input.len() {
                        self.pos += 1; // skip closing quote
                    }
                    return Some(Token::String(s));
                }
                b'0'..=b'9' => {
                    let start = self.pos;
                    while self.pos < self.input.len() && self.input[self.pos].is_ascii_digit() {
                        self.pos += 1;
                    }
                    let s = std::str::from_utf8(&self.input[start..self.pos]).unwrap();
                    let val = s.parse::<i64>().unwrap();
                    return Some(Token::Int(val));
                }
                _ => {
                    // GOTCHA: Silently ignoring unrecognized characters can hide bugs,
                    // but standard GraphQL lexers often skip BOM or specific control characters.
                    // A proper engine returns a Result<Token, LexError>.
                    self.pos += 1;
                }
            }
        }
    }

    pub fn tokenize(mut self) -> Vec<Token> {
        let mut tokens = Vec::new();
        while let Some(tok) = self.next_token() {
            tokens.push(tok);
        }
        tokens
    }
}

// AST Nodes

#[derive(Debug, PartialEq)]
pub enum Value {
    String(String),
    Int(i64),
}

#[derive(Debug, PartialEq)]
pub struct Argument {
    pub name: String,
    pub value: Value,
}

#[derive(Debug, PartialEq)]
pub struct Field {
    pub name: String,
    pub arguments: Vec<Argument>,
    pub selections: Vec<Field>,
}

#[derive(Debug, PartialEq)]
pub struct Document {
    pub operations: Vec<Field>, // Simplified: operations are just top-level fields
}

/// Recursive descent parser for GraphQL AST.
pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<&Token> {
        let tok = self.tokens.get(self.pos);
        self.pos += 1;
        tok
    }

    fn expect_name(&mut self) -> Result<String, String> {
        match self.advance() {
            Some(Token::Name(name)) => Ok(name.clone()),
            other => Err(format!("Expected Name, got {:?}", other)),
        }
    }

    pub fn parse_document(&mut self) -> Result<Document, String> {
        let mut operations = Vec::new();

        // Very simplified: assume the whole document is a query containing fields (e.g. `{ user { id } }`)
        if let Some(Token::BraceOpen) = self.peek() {
            self.advance();
            while let Some(tok) = self.peek() {
                if tok == &Token::BraceClose {
                    break;
                }
                operations.push(self.parse_field()?);
            }
            if self.advance() != Some(&Token::BraceClose) {
                return Err("Expected closing brace for document".to_string());
            }
        } else {
            // Implicit query without braces
            while self.peek().is_some() {
                operations.push(self.parse_field()?);
            }
        }

        Ok(Document { operations })
    }

    fn parse_field(&mut self) -> Result<Field, String> {
        let name = self.expect_name()?;
        let mut arguments = Vec::new();

        if let Some(Token::ParenOpen) = self.peek() {
            self.advance(); // skip '('
            while let Some(tok) = self.peek() {
                if tok == &Token::ParenClose {
                    break;
                }
                let arg_name = self.expect_name()?;
                if self.advance() != Some(&Token::Colon) {
                    return Err("Expected colon after argument name".to_string());
                }
                let arg_value = match self.advance() {
                    Some(Token::String(s)) => Value::String(s.clone()),
                    Some(Token::Int(i)) => Value::Int(*i),
                    other => return Err(format!("Expected argument value, got {:?}", other)),
                };
                arguments.push(Argument {
                    name: arg_name,
                    value: arg_value,
                });
            }
            if self.advance() != Some(&Token::ParenClose) {
                return Err("Expected closing parenthesis".to_string());
            }
        }

        let mut selections = Vec::new();
        if let Some(Token::BraceOpen) = self.peek() {
            self.advance(); // skip '{'
            while let Some(tok) = self.peek() {
                if tok == &Token::BraceClose {
                    break;
                }
                selections.push(self.parse_field()?);
            }
            if self.advance() != Some(&Token::BraceClose) {
                return Err("Expected closing brace for field selections".to_string());
            }
        }

        Ok(Field {
            name,
            arguments,
            selections,
        })
    }
}

// Execution

/// Represents a resolved value.
#[derive(Debug, PartialEq)]
pub enum ResolvedValue {
    Null,
    String(String),
    Int(i64),
    Object(HashMap<String, ResolvedValue>),
    List(Vec<ResolvedValue>),
}

/// The core Resolve trait that types must implement to be queried.
pub trait Resolve {
    fn resolve_field(&self, name: &str, args: &[Argument]) -> Result<ResolvedValue, String>;
}

/// Executor that runs a parsed GraphQL Document against a root query object.
pub struct Executor;

impl Executor {
    pub fn execute(doc: &Document, root: &dyn Resolve) -> Result<ResolvedValue, String> {
        let mut root_map = HashMap::new();
        for field in &doc.operations {
            let value = Self::execute_field(field, root)?;
            root_map.insert(field.name.clone(), value);
        }
        Ok(ResolvedValue::Object(root_map))
    }

    fn execute_field(field: &Field, resolver: &dyn Resolve) -> Result<ResolvedValue, String> {
        // RUST INSIGHT: We use a trait object `&dyn Resolve` for dynamic dispatch.
        // This is flexible but carries a small vtable overhead.
        let mut value = resolver.resolve_field(&field.name, &field.arguments)?;

        if !field.selections.is_empty() {
            if let ResolvedValue::Object(map) = &mut value {
                // If it's an object (but we don't have its specific traits here in the general enum),
                // in a real crate `Resolve::resolve_field` would return another `&dyn Resolve` or
                // recursive execute directly. Since our `ResolvedValue` is fully evaluated, this
                // simplified executor just filters the pre-evaluated Object.
                // A true engine recursively resolves nested fields lazily.
                let mut filtered = HashMap::new();
                for sub_field in &field.selections {
                    if let Some(sub_val) = map.remove(&sub_field.name) {
                        // Recursively filter (simplified)
                        // This prevents stack overflow by only recursing bounded by the request AST.
                        filtered.insert(sub_field.name.clone(), sub_val);
                    } else {
                        filtered.insert(sub_field.name.clone(), ResolvedValue::Null);
                    }
                }
                return Ok(ResolvedValue::Object(filtered));
            }
        }

        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct QueryRoot;

    impl Resolve for QueryRoot {
        fn resolve_field(&self, name: &str, args: &[Argument]) -> Result<ResolvedValue, String> {
            match name {
                "user" => {
                    let mut id_val = 1;
                    for arg in args {
                        if arg.name == "id" {
                            if let Value::Int(v) = arg.value {
                                id_val = v;
                            }
                        }
                    }

                    let mut user_map = HashMap::new();
                    user_map.insert("id".to_string(), ResolvedValue::Int(id_val));
                    user_map.insert("name".to_string(), ResolvedValue::String("Alice".to_string()));
                    Ok(ResolvedValue::Object(user_map))
                }
                "version" => Ok(ResolvedValue::String("1.0.0".to_string())),
                _ => Err(format!("Unknown field: {}", name)),
            }
        }
    }

    #[test]
    fn test_lexer() {
        let lexer = Lexer::new("{ user(id: 42) { name } }");
        let tokens = lexer.tokenize();
        assert_eq!(
            tokens,
            vec![
                Token::BraceOpen,
                Token::Name("user".to_string()),
                Token::ParenOpen,
                Token::Name("id".to_string()),
                Token::Colon,
                Token::Int(42),
                Token::ParenClose,
                Token::BraceOpen,
                Token::Name("name".to_string()),
                Token::BraceClose,
                Token::BraceClose,
            ]
        );
    }

    #[test]
    fn test_parser() {
        let tokens = vec![
            Token::BraceOpen,
            Token::Name("user".to_string()),
            Token::BraceOpen,
            Token::Name("name".to_string()),
            Token::BraceClose,
            Token::BraceClose,
        ];
        let mut parser = Parser::new(tokens);
        let doc = parser.parse_document().unwrap();
        assert_eq!(doc.operations.len(), 1);
        assert_eq!(doc.operations[0].name, "user");
        assert_eq!(doc.operations[0].selections.len(), 1);
        assert_eq!(doc.operations[0].selections[0].name, "name");
    }

    #[test]
    fn test_execution() {
        let query = "{ user(id: 99) { name } }";
        let tokens = Lexer::new(query).tokenize();
        let doc = Parser::new(tokens).parse_document().unwrap();

        let root = QueryRoot;
        let result = Executor::execute(&doc, &root).unwrap();

        if let ResolvedValue::Object(map) = result {
            if let ResolvedValue::Object(user_map) = map.get("user").unwrap() {
                assert_eq!(user_map.get("name").unwrap(), &ResolvedValue::String("Alice".to_string()));
            } else {
                panic!("Expected user object");
            }
        } else {
            panic!("Expected root object");
        }
    }
}
