//! # GraphQL Execution Engine
//!
//! Implements a minimal GraphQL lexer, parser, and trait-based recursive execution engine.
//!
//! **Replaces Crates:** `async-graphql`, `juniper`
//!
//! **Real-world Usage:**
//! - Core engines in API gateways (Apollo Router, Hasura)
//! - Server-side typed resolvers in Rust web backends
//! - Custom query languages that need typed responses and selective fetching
//!
//! **Why build it yourself?**
//! Building a GraphQL engine teaches you how to construct a lexer and recursive descent parser.
//! More importantly in Rust, it teaches you how to build a dynamic, trait-based execution engine
//! (the `Resolver` trait) that can evaluate abstract syntax trees while managing lifetimes,
//! allocations, and recursion without blowing the stack.
//!
//! # Architecture
//!
//! **Data Structure & Flow:**
//!
//!      Input Query String
//!            │
//!            ▼
//!      Lexer (Tokens: Ident, Punct, String, Number)
//!            │
//!            ▼
//!      Parser (AST: Document -> Operation -> SelectionSet -> Field)
//!            │
//!            ▼
//!      Executor + Trait Objects (`Box<dyn Resolver>`)
//!            │
//!            ▼
//!      JSON Output (`Value`)
//!
//! **Invariants:**
//! 1. Lexer must iteratively consume input to avoid stack overflows on invalid strings.
//! 2. Resolvers return owned `Value` types (e.g., `Value::String(String)`) to avoid tying output to the query's lifetime.
//! 3. The parser handles basic field selection, ignoring aliases and fragments for simplicity.
//!
//! **Complexity:**
//! - Lexing: O(N) where N is the length of the query string.
//! - Parsing: O(N) time and space.
//! - Execution: O(M) where M is the number of resolved fields (assuming O(1) resolver logic).
//!
//! **Design Decisions:**
//! - **Iterative Lexer:** Uses an explicit `while let` loop instead of tail recursion for advancing characters.
//! - **Owned Values:** Resolvers return owned strings (`String`) instead of zero-copy references (`&str`) to circumvent complex lifetime annotations across dynamic trait bounds.

use std::collections::HashMap;

// =========================================================================================
// Types & AST
// =========================================================================================

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

#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    Ident(String),
    String(String),
    Number(f64),
    Punctuation(char),
    Eof,
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub selection_set: Vec<Field>,
}

#[derive(Debug, Clone)]
pub struct Operation {
    pub selection_set: Vec<Field>,
}

#[derive(Debug, Clone)]
pub struct Document {
    pub operation: Operation,
}

// =========================================================================================
// Lexer
// =========================================================================================

pub struct Lexer<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    fn current_char(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn advance(&mut self) {
        if let Some(c) = self.current_char() {
            self.pos += c.len_utf8();
        }
    }

    fn skip_whitespace(&mut self) {
        // RUST INSIGHT: Instead of tail recursion, we use a loop to skip whitespace and commas
        // to avoid potential stack overflows on heavily padded input.
        while let Some(c) = self.current_char() {
            if c.is_whitespace() || c == ',' {
                self.advance();
            } else {
                break;
            }
        }
    }

    pub fn next_token(&mut self) -> Token {
        self.skip_whitespace();

        let Some(c) = self.current_char() else {
            return Token::Eof;
        };

        if c.is_alphabetic() || c == '_' {
            let start = self.pos;
            while let Some(c) = self.current_char() {
                if c.is_alphanumeric() || c == '_' {
                    self.advance();
                } else {
                    break;
                }
            }
            return Token::Ident(self.input[start..self.pos].to_string());
        }

        if c == '"' {
            self.advance();
            let start = self.pos;
            while let Some(c) = self.current_char() {
                if c == '"' {
                    let s = self.input[start..self.pos].to_string();
                    self.advance();
                    return Token::String(s);
                }
                self.advance();
            }
            // In a real lexer, handle unclosed strings.
            return Token::String(self.input[start..self.pos].to_string());
        }

        if c.is_ascii_digit() || c == '-' {
            let start = self.pos;
            self.advance(); // consume first char
            while let Some(c) = self.current_char() {
                if c.is_ascii_digit() || c == '.' {
                    self.advance();
                } else {
                    break;
                }
            }
            let s = &self.input[start..self.pos];
            if let Ok(n) = s.parse::<f64>() {
                return Token::Number(n);
            }
        }

        if "{}().:!".contains(c) {
            self.advance();
            return Token::Punctuation(c);
        }

        // GOTCHA: If we encounter an unrecognized character, we must advance past it to avoid infinite loops.
        self.advance();
        self.next_token() // It's generally better to return an error, but we skip it here.
    }
}

// =========================================================================================
// Parser
// =========================================================================================

pub struct Parser<'a> {
    lexer: Lexer<'a>,
    current_token: Token,
}

impl<'a> Parser<'a> {
    pub fn new(input: &'a str) -> Self {
        let mut lexer = Lexer::new(input);
        let current_token = lexer.next_token();
        Self {
            lexer,
            current_token,
        }
    }

    fn advance(&mut self) {
        self.current_token = self.lexer.next_token();
    }

    fn parse_selection_set(&mut self) -> Result<Vec<Field>, String> {
        if self.current_token != Token::Punctuation('{') {
            return Err("Expected '{' for selection set".to_string());
        }
        self.advance();

        let mut fields = Vec::new();
        while self.current_token != Token::Punctuation('}') && self.current_token != Token::Eof {
            fields.push(self.parse_field()?);
        }

        if self.current_token == Token::Punctuation('}') {
            self.advance();
        }

        Ok(fields)
    }

    fn parse_field(&mut self) -> Result<Field, String> {
        let name = match &self.current_token {
            Token::Ident(s) => s.clone(),
            _ => return Err(format!("Expected field name, got {:?}", self.current_token)),
        };
        self.advance();

        let selection_set = if self.current_token == Token::Punctuation('{') {
            self.parse_selection_set()?
        } else {
            Vec::new()
        };

        Ok(Field {
            name,
            selection_set,
        })
    }

    pub fn parse_document(&mut self) -> Result<Document, String> {
        // Assume an implicit query operation for simplicity
        let selection_set = self.parse_selection_set()?;
        Ok(Document {
            operation: Operation { selection_set },
        })
    }
}

// =========================================================================================
// Execution Engine
// =========================================================================================

/// Trait for executing a field resolution.
pub trait Resolver {
    fn resolve(&self, field: &Field) -> Result<Value, String>;
}

/// Represents an Object that has sub-fields to resolve.
pub trait ObjectResolver {
    fn resolve_field(&self, name: &str, field: &Field) -> Result<Value, String>;
}

// Blanket implementation to easily treat ObjectResolvers as general Resolvers.
impl<T: ObjectResolver> Resolver for T {
    fn resolve(&self, field: &Field) -> Result<Value, String> {
        let mut map = HashMap::new();
        for sub_field in &field.selection_set {
            // RUST INSIGHT: The returned Value (e.g. Value::String) owns its data.
            // This decouples the execution result from the lifetime of the schema/query,
            // avoiding 'a and 'b lifetime hell in highly nested trait objects.
            let val = self.resolve_field(&sub_field.name, sub_field)?;
            map.insert(sub_field.name.clone(), val);
        }
        Ok(Value::Object(map))
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `async-graphql` and `juniper` use procedural macros (`#[derive(Object)]`) to automatically
//   generate resolver implementations. They are heavily async-first.
// - Our implementation requires manual implementation of `ObjectResolver`.
//
// Missing vs. Production:
// - **Arguments & Variables:** No support for query arguments (e.g., `user(id: 1)`).
// - **Async Execution:** Real engines run resolvers concurrently via futures.
// - **Validation:** We don't validate the query against a schema before execution.
// - **Fragments & Interfaces:** GraphQL features like inline fragments are omitted.
//
// Next Steps:
// 1. Add procedural macros to derive `ObjectResolver` for structs.
// 2. Introduce a `Context` object for dependency injection (e.g., database connection).
// 3. Make `Resolver::resolve` async.

#[cfg(test)]
mod tests {
    use super::*;

    struct User {
        id: i64,
        name: String,
    }

    impl ObjectResolver for User {
        fn resolve_field(&self, name: &str, _field: &Field) -> Result<Value, String> {
            match name {
                "id" => Ok(Value::Int(self.id)),
                "name" => Ok(Value::String(self.name.clone())),
                _ => Err(format!("Unknown field on User: {}", name)),
            }
        }
    }

    struct QueryRoot;

    impl ObjectResolver for QueryRoot {
        fn resolve_field(&self, name: &str, field: &Field) -> Result<Value, String> {
            match name {
                "me" => {
                    let user = User {
                        id: 42,
                        name: "Alice".to_string(),
                    };
                    user.resolve(field)
                }
                "version" => Ok(Value::String("1.0.0".to_string())),
                _ => Err(format!("Unknown root field: {}", name)),
            }
        }
    }

    #[test]
    fn test_lexer() {
        let mut lexer = Lexer::new("{ me { id name } }");
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
    fn test_parser_and_execution() {
        let query = "{ me { id name } version }";
        let mut parser = Parser::new(query);
        let doc = parser.parse_document().unwrap();

        let root = QueryRoot;
        let mut result_map = HashMap::new();

        for field in &doc.operation.selection_set {
            let val = root.resolve_field(&field.name, field).unwrap();
            result_map.insert(field.name.clone(), val);
        }

        let me_obj = match result_map.get("me").unwrap() {
            Value::Object(map) => map,
            _ => panic!("Expected object"),
        };

        assert_eq!(me_obj.get("id"), Some(&Value::Int(42)));
        assert_eq!(
            me_obj.get("name"),
            Some(&Value::String("Alice".to_string()))
        );
        assert_eq!(
            result_map.get("version"),
            Some(&Value::String("1.0.0".to_string()))
        );
    }

    #[test]
    fn test_invalid_syntax() {
        let query = "{ me { id "; // Unclosed
        let mut parser = Parser::new(query);
        let _ = parser.parse_document(); // Should handle EOF gracefully without panic
    }
}
