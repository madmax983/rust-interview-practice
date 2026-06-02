//! # GraphQL Execution Engine
//!
//! What this implements and what crate(s) it replaces:
//! This is a from-scratch implementation of a GraphQL parsing and execution engine.
//! It replaces crates like `async-graphql` and `juniper`.
//!
//! Real-world systems that use this:
//! GraphQL engines power APIs at companies like GitHub, Shopify, and Facebook.
//! Tools like Apollo Server and Hasura provide robust GraphQL execution environments.
//!
//! Why build it yourself?
//! Understanding the pipeline from query string -> tokens -> AST -> dynamic resolution
//! demystifies how GraphQL deeply integrates with type systems and allows swappable
//! execution strategies. You learn recursive descent parsing and dynamic dispatch.
//!
//! ## Architecture
//!
//! ```text
//! Query String
//!      │
//!      ▼
//! [ Lexer ] --> Stream of Tokens (Ident, Punctuation, String, Int)
//!      │
//!      ▼
//! [ Parser ] --> AST (Document -> Operations -> Selection Sets -> Fields)
//!      │
//!      ▼
//! [ Executor ] + [ Root Resolver ] --> JSON-like Value (Response)
//! ```
//!
//! ### Invariants
//! - The AST must represent a valid syntactical GraphQL document (we skip full schema validation for simplicity).
//! - Resolvers must correctly match the requested fields, returning errors for unknown fields.
//!
//! ### Time/Space Complexity
//! - **Lexing**: `O(N)` time, `O(N)` space where `N` is the query length.
//! - **Parsing**: `O(N)` time, `O(N)` space (recursive descent).
//! - **Execution**: `O(F)` time, `O(F)` space where `F` is the number of resolved fields in the selection set.
//!
//! ### Design Decisions
//! - **Pull-based resolution**: The executor traverses the AST and asks the `Resolver` for data,
//!   allowing lazy evaluation of expensive fields.
//! - **Trait-based Interfaces**: We use a `Resolver` trait returning `ResolvedValue`, allowing
//!   dynamic objects (`Box<dyn Resolver>`) to be returned and further queried.

use std::collections::HashMap;

// ============================================================================
// Core Types
// ============================================================================

/// Represents a JSON-like value returned by the execution engine.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Int(i64),
    Float(f64),
    String(String),
    Boolean(bool),
    List(Vec<Value>),
    Object(HashMap<String, Value>),
}

/// The result of resolving a specific field.
///
/// # RUST INSIGHT:
/// Returning `Box<dyn Resolver>` here allows the engine to be fully dynamic,
/// dispatching method calls at runtime for nested objects without needing generic type bounds
/// or static lifetimes tying the resolver to the engine itself.
pub enum ResolvedValue {
    Scalar(Value),
    Object(Box<dyn Resolver>),
    List(Vec<ResolvedValue>),
    Null,
}

/// A trait representing a GraphQL Object Type that can resolve fields.
pub trait Resolver {
    fn resolve(&self, field_name: &str, args: &HashMap<String, Value>) -> Result<ResolvedValue, String>;
}

// ============================================================================
// AST Nodes
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    pub operations: Vec<Operation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Operation {
    pub name: Option<String>,
    pub selection_set: Vec<Selection>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Selection {
    Field(Field),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub alias: Option<String>,
    pub name: String,
    pub arguments: HashMap<String, Value>,
    pub selection_set: Vec<Selection>,
}

// ============================================================================
// Lexer
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Ident(String),
    String(String),
    Int(i64),
    Float(f64),
    Punct(char),
    Eof,
}

pub struct Lexer<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Lexer<'a> {
    #[must_use]
    pub fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    pub fn next_token(&mut self) -> Token {
        self.skip_whitespace();
        if self.pos >= self.input.len() {
            return Token::Eof;
        }

        let slice = &self.input[self.pos..];
        let mut chars = slice.chars();
        let ch = chars.next().unwrap();

        match ch {
            '{' | '}' | '(' | ')' | ':' | '[' | ']' => {
                self.pos += ch.len_utf8();
                Token::Punct(ch)
            }
            '"' => self.lex_string(),
            _ if ch.is_ascii_digit() || ch == '-' => self.lex_number(),
            _ if ch.is_alphabetic() || ch == '_' => self.lex_ident(),
            _ => {
                // Skip unrecognized characters safely
                self.pos += ch.len_utf8();
                self.next_token()
            }
        }
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.input.len() {
            let slice = &self.input[self.pos..];
            let mut chars = slice.char_indices();
            if let Some((_, ch)) = chars.next() {
                if ch.is_whitespace() || ch == ',' {
                    // GOTCHA: Use `len_utf8()` to safely advance indices past multibyte characters.
                    self.pos += ch.len_utf8();
                } else if ch == '#' {
                    self.pos += ch.len_utf8();
                    while self.pos < self.input.len() {
                        let inner_slice = &self.input[self.pos..];
                        let mut inner_chars = inner_slice.chars();
                        let c = inner_chars.next().unwrap();
                        self.pos += c.len_utf8();
                        if c == '\n' {
                            break;
                        }
                    }
                } else {
                    break;
                }
            }
        }
    }

    fn lex_string(&mut self) -> Token {
        self.pos += 1; // skip initial quote
        let start = self.pos;
        while self.pos < self.input.len() {
            let slice = &self.input[self.pos..];
            let mut chars = slice.chars();
            let ch = chars.next().unwrap();
            if ch == '"' {
                let s = &self.input[start..self.pos];
                self.pos += 1; // skip closing quote
                return Token::String(s.to_string());
            }
            self.pos += ch.len_utf8();
        }
        Token::Eof
    }

    fn lex_ident(&mut self) -> Token {
        let start = self.pos;
        while self.pos < self.input.len() {
            let slice = &self.input[self.pos..];
            let mut chars = slice.chars();
            let ch = chars.next().unwrap();
            if ch.is_alphanumeric() || ch == '_' {
                self.pos += ch.len_utf8();
            } else {
                break;
            }
        }
        Token::Ident(self.input[start..self.pos].to_string())
    }

    fn lex_number(&mut self) -> Token {
        let start = self.pos;
        let mut is_float = false;
        while self.pos < self.input.len() {
            let slice = &self.input[self.pos..];
            let mut chars = slice.chars();
            let ch = chars.next().unwrap();
            if ch.is_ascii_digit() || ch == '-' {
                self.pos += 1;
            } else if ch == '.' {
                is_float = true;
                self.pos += 1;
            } else {
                break;
            }
        }
        let s = &self.input[start..self.pos];
        if is_float {
            Token::Float(s.parse().unwrap_or(0.0))
        } else {
            Token::Int(s.parse().unwrap_or(0))
        }
    }
}

// ============================================================================
// Parser
// ============================================================================

pub struct Parser<'a> {
    lexer: Lexer<'a>,
    current: Token,
    peeked: Token,
}

impl<'a> Parser<'a> {
    pub fn new(mut lexer: Lexer<'a>) -> Self {
        let current = lexer.next_token();
        let peeked = lexer.next_token();
        Self {
            lexer,
            current,
            peeked,
        }
    }

    fn advance(&mut self) {
        self.current = std::mem::replace(&mut self.peeked, self.lexer.next_token());
    }

    pub fn parse_document(&mut self) -> Result<Document, String> {
        let mut operations = Vec::new();
        while self.current != Token::Eof {
            operations.push(self.parse_operation()?);
        }
        Ok(Document { operations })
    }

    fn parse_operation(&mut self) -> Result<Operation, String> {
        let mut name = None;
        if let Token::Ident(ref s) = self.current {
            if s == "query" || s == "mutation" {
                self.advance();
                if let Token::Ident(ref n) = self.current {
                    name = Some(n.clone());
                    self.advance();
                }
            }
        }
        let selection_set = self.parse_selection_set()?;
        Ok(Operation {
            name,
            selection_set,
        })
    }

    fn parse_selection_set(&mut self) -> Result<Vec<Selection>, String> {
        if self.current != Token::Punct('{') {
            return Err("Expected '{'".to_string());
        }
        self.advance();
        let mut selections = Vec::new();
        while self.current != Token::Punct('}') && self.current != Token::Eof {
            selections.push(self.parse_selection()?);
        }
        if self.current == Token::Punct('}') {
            self.advance();
        } else {
            return Err("Expected '}'".to_string());
        }
        Ok(selections)
    }

    fn parse_selection(&mut self) -> Result<Selection, String> {
        let name = match &self.current {
            Token::Ident(n) => n.clone(),
            _ => return Err("Expected field name".to_string()),
        };
        self.advance();

        let mut alias = None;
        let mut field_name = name.clone();

        if self.current == Token::Punct(':') {
            self.advance();
            alias = Some(name);
            field_name = match &self.current {
                Token::Ident(n) => n.clone(),
                _ => return Err("Expected field name after alias".to_string()),
            };
            self.advance();
        }

        let arguments = if self.current == Token::Punct('(') {
            self.parse_arguments()?
        } else {
            HashMap::new()
        };

        let selection_set = if self.current == Token::Punct('{') {
            self.parse_selection_set()?
        } else {
            Vec::new()
        };

        Ok(Selection::Field(Field {
            alias,
            name: field_name,
            arguments,
            selection_set,
        }))
    }

    fn parse_arguments(&mut self) -> Result<HashMap<String, Value>, String> {
        self.advance(); // skip '('
        let mut args = HashMap::new();
        while self.current != Token::Punct(')') && self.current != Token::Eof {
            let arg_name = match &self.current {
                Token::Ident(n) => n.clone(),
                _ => return Err("Expected argument name".to_string()),
            };
            self.advance();
            if self.current != Token::Punct(':') {
                return Err("Expected ':' after argument name".to_string());
            }
            self.advance();
            let arg_value = self.parse_value()?;
            args.insert(arg_name, arg_value);
        }
        if self.current == Token::Punct(')') {
            self.advance();
        }
        Ok(args)
    }

    fn parse_value(&mut self) -> Result<Value, String> {
        let val = match &self.current {
            Token::Int(i) => Value::Int(*i),
            Token::Float(f) => Value::Float(*f),
            Token::String(s) => Value::String(s.clone()),
            Token::Ident(s) if s == "true" => Value::Boolean(true),
            Token::Ident(s) if s == "false" => Value::Boolean(false),
            Token::Ident(s) if s == "null" => Value::Null,
            Token::Ident(s) => Value::String(s.clone()), // Enum values
            Token::Punct('[') => {
                self.advance();
                let mut list = Vec::new();
                while self.current != Token::Punct(']') && self.current != Token::Eof {
                    list.push(self.parse_value()?);
                }
                if self.current == Token::Punct(']') {
                    self.advance();
                }
                return Ok(Value::List(list));
            }
            Token::Punct('{') => {
                self.advance();
                let mut obj = HashMap::new();
                while self.current != Token::Punct('}') && self.current != Token::Eof {
                    let key = match &self.current {
                        Token::Ident(k) | Token::String(k) => k.clone(),
                        _ => return Err("Expected key in object".to_string()),
                    };
                    self.advance();
                    if self.current != Token::Punct(':') {
                        return Err("Expected ':' in object".to_string());
                    }
                    self.advance();
                    obj.insert(key, self.parse_value()?);
                }
                if self.current == Token::Punct('}') {
                    self.advance();
                }
                return Ok(Value::Object(obj));
            }
            _ => return Err("Unexpected token in value".to_string()),
        };
        self.advance();
        Ok(val)
    }
}

// ============================================================================
// Executor
// ============================================================================

pub struct Executor;

impl Executor {
    /// Executes a given GraphQL document against a root resolver.
    pub fn execute(document: &Document, root_resolver: &dyn Resolver) -> Result<Value, String> {
        let mut result = HashMap::new();
        // For simplicity, we just execute the first operation
        if let Some(op) = document.operations.first() {
            for selection in &op.selection_set {
                if let Selection::Field(field) = selection {
                    let value = Self::execute_field(field, root_resolver)?;
                    let key = field.alias.clone().unwrap_or_else(|| field.name.clone());
                    result.insert(key, value);
                }
            }
        }
        Ok(Value::Object(result))
    }

    fn execute_field(field: &Field, resolver: &dyn Resolver) -> Result<Value, String> {
        let resolved = resolver.resolve(&field.name, &field.arguments)?;
        Self::resolve_to_value(&field.selection_set, resolved)
    }

    fn resolve_to_value(
        selection_set: &[Selection],
        resolved: ResolvedValue,
    ) -> Result<Value, String> {
        match resolved {
            ResolvedValue::Scalar(v) => Ok(v),
            ResolvedValue::Null => Ok(Value::Null),
            ResolvedValue::List(items) => {
                let mut list = Vec::new();
                // PRODUCTION NOTE: A production implementation would evaluate list items concurrently
                // if they do not have side-effects, using `futures::future::join_all`.
                for item in items {
                    list.push(Self::resolve_to_value(selection_set, item)?);
                }
                Ok(Value::List(list))
            }
            ResolvedValue::Object(obj_resolver) => {
                let mut result = HashMap::new();
                for selection in selection_set {
                    if let Selection::Field(field) = selection {
                        let value = Self::execute_field(field, obj_resolver.as_ref())?;
                        let key = field.alias.clone().unwrap_or_else(|| field.name.clone());
                        result.insert(key, value);
                    }
                }
                Ok(Value::Object(result))
            }
        }
    }
}

// ============================================================================
// Footer
// ============================================================================
//
// Comparison to Canonical Crates:
// - `async-graphql`: Uses macros to generate heavily optimized schema types at compile time and is fully async.
// - `juniper`: An older crate using compile-time macros, similar to `async-graphql` but synchronously focused.
// Our engine is synchronous, dynamic, and does not validate against a strong schema definition language (SDL),
// favoring a fully interpretive approach.
//
// What's missing vs. production:
// - **Schema Validation**: We execute whatever fields are asked; a real engine validates fields against a schema AST.
// - **Async Execution**: Resolvers in a real API do network/DB calls, meaning `resolve` must return `Future`.
// - **Fragments / Variables / Directives**: Full spec compliance requires supporting fragments spread, inputs, etc.
// - **N+1 Problem handling (Dataloader)**: Missing batching support.
//
// Next steps:
// 1. Make `Resolver::resolve` async, returning a `BoxFuture`.
// 2. Add an SDL schema parser.

#[cfg(test)]
mod tests {
    use super::*;

    struct UserResolver {
        id: i64,
        name: String,
    }

    impl Resolver for UserResolver {
        fn resolve(
            &self,
            field_name: &str,
            _args: &HashMap<String, Value>,
        ) -> Result<ResolvedValue, String> {
            match field_name {
                "id" => Ok(ResolvedValue::Scalar(Value::Int(self.id))),
                "name" => Ok(ResolvedValue::Scalar(Value::String(self.name.clone()))),
                _ => Err(format!("Unknown field: {}", field_name)),
            }
        }
    }

    struct QueryResolver;

    impl Resolver for QueryResolver {
        fn resolve(
            &self,
            field_name: &str,
            args: &HashMap<String, Value>,
        ) -> Result<ResolvedValue, String> {
            match field_name {
                "user" => {
                    let id = match args.get("id") {
                        Some(Value::Int(i)) => *i,
                        _ => return Err("Missing or invalid 'id' argument".to_string()),
                    };
                    Ok(ResolvedValue::Object(Box::new(UserResolver {
                        id,
                        name: format!("User {}", id),
                    })))
                }
                "hello" => Ok(ResolvedValue::Scalar(Value::String("world".to_string()))),
                _ => Err(format!("Unknown root field: {}", field_name)),
            }
        }
    }

    #[test]
    fn test_graphql_execution() {
        let query = r#"
            query {
                hello
                # This is a comment
                user(id: 42) {
                    id
                    userName: name
                }
            }
        "#;

        let lexer = Lexer::new(query);
        let mut parser = Parser::new(lexer);
        let doc = parser.parse_document().unwrap();

        let root = QueryResolver;
        let result = Executor::execute(&doc, &root).unwrap();

        if let Value::Object(map) = result {
            assert_eq!(map.get("hello"), Some(&Value::String("world".to_string())));

            if let Some(Value::Object(user_map)) = map.get("user") {
                assert_eq!(user_map.get("id"), Some(&Value::Int(42)));
                assert_eq!(
                    user_map.get("userName"),
                    Some(&Value::String("User 42".to_string()))
                );
            } else {
                panic!("Expected user object");
            }
        } else {
            panic!("Expected object result");
        }
    }

    #[test]
    fn test_lexer_comments_and_strings() {
        let mut lexer = Lexer::new("{ # comment \n field(arg: \"str value\") }");
        assert_eq!(lexer.next_token(), Token::Punct('{'));
        assert_eq!(lexer.next_token(), Token::Ident("field".to_string()));
        assert_eq!(lexer.next_token(), Token::Punct('('));
        assert_eq!(lexer.next_token(), Token::Ident("arg".to_string()));
        assert_eq!(lexer.next_token(), Token::Punct(':'));
        assert_eq!(lexer.next_token(), Token::String("str value".to_string()));
        assert_eq!(lexer.next_token(), Token::Punct(')'));
        assert_eq!(lexer.next_token(), Token::Punct('}'));
        assert_eq!(lexer.next_token(), Token::Eof);
    }
}