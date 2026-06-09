//! # GraphQL Execution Engine
//!
//! What this implements and what crate(s) it replaces:
//! This implements a minimal GraphQL lexer, parser, and execution engine.
//! It replaces heavy crates like `async-graphql` and `juniper` for simpler embedded environments.
//!
//! Real-world systems that use this:
//! GraphQL powers data fetching at Facebook, GitHub's public API, and countless federation gateways.
//!
//! Why build it yourself?
//! Building a GraphQL engine from scratch teaches you recursive descent parsing, the importance of
//! trait objects for dynamic schema traversal, and how to safely lex multibyte UTF-8 string data in Rust.
//!
//! ## Architecture
//!
//! - **Lexer**: Tokenizes the query string on-the-fly without allocations using `.char_indices()`.
//! - **Parser**: A recursive descent parser that constructs an Abstract Syntax Tree (`Document`, `Operation`, `Selection`, `Field`).
//! - **Executor**: Trait-based `Resolver` interface walks the AST dynamically against a schema.
//!
//! | Component    | Time Complexity       | Space Complexity       |
//! | ------------ | --------------------- | ---------------------- |
//! | Lexer        | O(N)                  | O(1)                   |
//! | Parser       | O(N)                  | O(N) for AST           |
//! | Executor     | O(N) over AST         | O(M) for Response JSON |
//!
//! **Invariants:**
//! - The AST structure (`Document<'a>`) is strictly bound to the lifetime of the input query string, enforcing zero-copy parsing.
//! - Multibyte characters must be lexed without causing out-of-bounds byte slicing panics.
//!
//! **Design Decisions:**
//! - **Zero-copy Lexing:** The `Lexer` emits tokens as slices `&str` tied to the input string's lifetime.
//! - **Dynamic Execution:** We use a dynamic `Resolver` trait instead of compile-time macros (as `async-graphql` does) to allow flexible schemas at runtime.

use std::collections::HashMap;

// =========================================================================================
// Lexer
// =========================================================================================

#[derive(Debug, PartialEq, Clone)]
pub enum Token<'a> {
    Ident(&'a str),
    Punct(char),
    String(&'a str),
    Int(i64),
}

pub struct Lexer<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    pub fn next_token(&mut self) -> Option<Token<'a>> {
        self.skip_whitespace();
        if self.pos >= self.input.len() {
            return None;
        }

        // GOTCHA:
        // When parsing strings or implementing custom lexers in Rust, avoid manually incrementing byte
        // indices (`i += 1`) for string slicing (`&text[start..i]`), as this causes panics on multibyte Unicode characters.
        // Instead, safely advance indices using `char.len_utf8()` or iterate over character boundaries with `.char_indices()`.
        let mut iter = self.input[self.pos..].char_indices();
        let (_, c) = iter.next()?;

        match c {
            '{' | '}' | '(' | ')' | ':' | ',' => {
                // PRODUCTION NOTE:
                // A real-world lexer would likely use a robust token generator and emit full source spans (line, column)
                // for accurate error reporting to the user. We omit span tracking here for simplicity.
                self.pos += c.len_utf8();
                Some(Token::Punct(c))
            }
            '"' => {
                let start = self.pos + c.len_utf8();
                let mut end = start;
                let mut found = false;
                for (i, ch) in iter {
                    if ch == '"' {
                        end = self.pos + i;
                        found = true;
                        self.pos += i + ch.len_utf8();
                        break;
                    }
                }
                if found {
                    Some(Token::String(&self.input[start..end]))
                } else {
                    None // unclosed string
                }
            }
            _ if c.is_ascii_alphabetic() || c == '_' => {
                let start = self.pos;
                let mut len = c.len_utf8();
                for (i, ch) in iter {
                    if ch.is_ascii_alphanumeric() || ch == '_' {
                        len = i + ch.len_utf8();
                    } else {
                        break;
                    }
                }
                self.pos += len;
                Some(Token::Ident(&self.input[start..start + len]))
            }
            _ if c.is_ascii_digit() => {
                let start = self.pos;
                let mut len = c.len_utf8();
                for (i, ch) in iter {
                    if ch.is_ascii_digit() {
                        len = i + ch.len_utf8();
                    } else {
                        break;
                    }
                }
                self.pos += len;
                let val_str = &self.input[start..start + len];
                Some(Token::Int(val_str.parse().unwrap_or(0)))
            }
            _ => {
                // Skip unknown
                self.pos += c.len_utf8();
                self.next_token()
            }
        }
    }

    fn skip_whitespace(&mut self) {
        let mut advance = 0;
        for (_, c) in self.input[self.pos..].char_indices() {
            if c.is_whitespace() || c == ',' {
                advance += c.len_utf8();
            } else {
                break;
            }
        }
        self.pos += advance;
    }
}

// =========================================================================================
// Parser & AST
// =========================================================================================

// RUST INSIGHT:
// By tying our AST `Document<'a>` to the lifetime of the input string `&'a str`,
// we enforce at compile-time that the AST cannot outlive the underlying query string.
// This gives us zero-copy parsing safely.

#[derive(Debug, PartialEq)]
pub struct Document<'a> {
    pub operations: Vec<Operation<'a>>,
}

#[derive(Debug, PartialEq)]
pub struct Operation<'a> {
    pub name: Option<&'a str>,
    pub selections: Vec<Selection<'a>>,
}

#[derive(Debug, PartialEq)]
pub enum Selection<'a> {
    Field(Field<'a>),
}

#[derive(Debug, PartialEq)]
pub struct Field<'a> {
    pub name: &'a str,
    pub arguments: Vec<(&'a str, Value)>,
    pub selections: Vec<Selection<'a>>,
}

#[derive(Debug, PartialEq, Clone)]
pub enum Value {
    Int(i64),
    String(String),
    Object(HashMap<String, Value>),
    List(Vec<Value>),
    Null,
}

pub struct Parser<'a> {
    lexer: Lexer<'a>,
    current: Option<Token<'a>>,
}

impl<'a> Parser<'a> {
    pub fn new(mut lexer: Lexer<'a>) -> Self {
        let current = lexer.next_token();
        Self { lexer, current }
    }

    fn advance(&mut self) {
        self.current = self.lexer.next_token();
    }

    pub fn parse_document(&mut self) -> Result<Document<'a>, String> {
        let mut operations = Vec::new();
        while self.current.is_some() {
            operations.push(self.parse_operation()?);
        }
        Ok(Document { operations })
    }

    fn parse_operation(&mut self) -> Result<Operation<'a>, String> {
        let mut name = None;
        if let Some(Token::Ident("query")) = self.current {
            self.advance();
            if let Some(Token::Ident(n)) = self.current {
                name = Some(n);
                self.advance();
            }
        } else if let Some(Token::Ident("mutation")) = self.current {
            self.advance();
            if let Some(Token::Ident(n)) = self.current {
                name = Some(n);
                self.advance();
            }
        }

        let selections = self.parse_selection_set()?;
        Ok(Operation { name, selections })
    }

    fn parse_selection_set(&mut self) -> Result<Vec<Selection<'a>>, String> {
        if self.current != Some(Token::Punct('{')) {
            return Err("Expected '{'".to_string());
        }
        self.advance();

        let mut selections = Vec::new();
        while self.current.is_some() && self.current != Some(Token::Punct('}')) {
            selections.push(self.parse_selection()?);
        }

        if self.current != Some(Token::Punct('}')) {
            return Err("Expected '}'".to_string());
        }
        self.advance();

        Ok(selections)
    }

    fn parse_selection(&mut self) -> Result<Selection<'a>, String> {
        let name = match self.current {
            Some(Token::Ident(n)) => n,
            _ => return Err("Expected field name".to_string()),
        };
        self.advance();

        let mut arguments = Vec::new();
        if self.current == Some(Token::Punct('(')) {
            self.advance();
            while self.current.is_some() && self.current != Some(Token::Punct(')')) {
                let arg_name = match self.current {
                    Some(Token::Ident(n)) => n,
                    _ => return Err("Expected argument name".to_string()),
                };
                self.advance();

                if self.current != Some(Token::Punct(':')) {
                    return Err("Expected ':' after argument name".to_string());
                }
                self.advance();

                let val = self.parse_value()?;
                arguments.push((arg_name, val));
            }
            if self.current != Some(Token::Punct(')')) {
                return Err("Expected ')'".to_string());
            }
            self.advance();
        }

        let mut selections = Vec::new();
        if self.current == Some(Token::Punct('{')) {
            selections = self.parse_selection_set()?;
        }

        Ok(Selection::Field(Field {
            name,
            arguments,
            selections,
        }))
    }

    fn parse_value(&mut self) -> Result<Value, String> {
        match &self.current {
            Some(Token::Int(i)) => {
                let val = Value::Int(*i);
                self.advance();
                Ok(val)
            }
            Some(Token::String(s)) => {
                let val = Value::String(s.to_string());
                self.advance();
                Ok(val)
            }
            _ => Err("Expected value".to_string()),
        }
    }
}

// =========================================================================================
// Executor
// =========================================================================================

pub enum ResolverResult {
    Scalar(Value),
    Object(Box<dyn Resolver>),
    List(Vec<Box<dyn Resolver>>),
    Null,
}

pub trait Resolver {
    fn resolve_field(&self, field: &str, args: &HashMap<&str, Value>) -> ResolverResult;
}

pub fn execute(doc: &Document<'_>, root_resolver: &dyn Resolver) -> Value {
    if let Some(op) = doc.operations.first() {
        execute_selections(&op.selections, root_resolver)
    } else {
        Value::Null
    }
}

fn execute_selections(selections: &[Selection<'_>], resolver: &dyn Resolver) -> Value {
    let mut result = HashMap::new();
    for selection in selections {
        match selection {
            Selection::Field(f) => {
                let mut args_map = HashMap::new();
                for (k, v) in &f.arguments {
                    args_map.insert(*k, v.clone());
                }

                let resolved = resolver.resolve_field(f.name, &args_map);
                let val = match resolved {
                    ResolverResult::Scalar(v) => v,
                    ResolverResult::Object(sub_resolver) => {
                        execute_selections(&f.selections, sub_resolver.as_ref())
                    }
                    ResolverResult::List(items) => {
                        let mut list_vals = Vec::new();
                        for item in items {
                            list_vals.push(execute_selections(&f.selections, item.as_ref()));
                        }
                        Value::List(list_vals)
                    }
                    ResolverResult::Null => Value::Null,
                };
                result.insert(f.name.to_string(), val);
            }
        }
    }
    Value::Object(result)
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `async-graphql`: A heavy, macro-based schema definition and execution engine that relies
//   heavily on Rust's async/await and compile-time type checking.
// - `juniper`: Similar to async-graphql but older and supports both sync and async.
//
// What's missing vs. production:
// - **Type Checking & Validation**: A production GraphQL engine validates the query AST against
//   the Schema AST before execution.
// - **Variables & Fragments**: We don't support query variables (`$id: ID!`) or `...Fragment`.
// - **Async Execution**: Real resolvers typically fetch from databases asynchronously.
//
// Suggested next steps / extensions:
// 1. Add `Fragment` support to the Parser and Executor.
// 2. Introduce a `Schema` struct to perform query validation before execution.
//
// Benchmarking Note:
// To benchmark this lexer/parser, use `criterion::Criterion` to measure parsing throughput
// on large, nested query strings, comparing it directly against `async-graphql`'s parser.

#[cfg(test)]
mod tests {
    use super::*;

    struct UserResolver;
    impl Resolver for UserResolver {
        fn resolve_field(&self, field: &str, _args: &HashMap<&str, Value>) -> ResolverResult {
            match field {
                "id" => ResolverResult::Scalar(Value::Int(1)),
                "name" => ResolverResult::Scalar(Value::String("Alice".to_string())),
                _ => ResolverResult::Null,
            }
        }
    }

    struct RootResolver;
    impl Resolver for RootResolver {
        fn resolve_field(&self, field: &str, args: &HashMap<&str, Value>) -> ResolverResult {
            match field {
                "user" => {
                    if let Some(Value::Int(1)) = args.get("id") {
                        ResolverResult::Object(Box::new(UserResolver))
                    } else {
                        ResolverResult::Null
                    }
                }
                "users" => {
                    ResolverResult::List(vec![Box::new(UserResolver), Box::new(UserResolver)])
                }
                _ => ResolverResult::Null,
            }
        }
    }

    #[test]
    fn test_lexer() {
        let input = r#"query { user(id: 1) { name } }"#;
        let mut lexer = Lexer::new(input);

        assert_eq!(lexer.next_token(), Some(Token::Ident("query")));
        assert_eq!(lexer.next_token(), Some(Token::Punct('{')));
        assert_eq!(lexer.next_token(), Some(Token::Ident("user")));
        assert_eq!(lexer.next_token(), Some(Token::Punct('(')));
        assert_eq!(lexer.next_token(), Some(Token::Ident("id")));
        assert_eq!(lexer.next_token(), Some(Token::Punct(':')));
        assert_eq!(lexer.next_token(), Some(Token::Int(1)));
        assert_eq!(lexer.next_token(), Some(Token::Punct(')')));
        assert_eq!(lexer.next_token(), Some(Token::Punct('{')));
        assert_eq!(lexer.next_token(), Some(Token::Ident("name")));
        assert_eq!(lexer.next_token(), Some(Token::Punct('}')));
        assert_eq!(lexer.next_token(), Some(Token::Punct('}')));
        assert_eq!(lexer.next_token(), None);
    }

    #[test]
    fn test_parser() {
        let input = r#"{ user(id: 1) { name } }"#;
        let lexer = Lexer::new(input);
        let mut parser = Parser::new(lexer);

        let doc = parser.parse_document().unwrap();
        assert_eq!(doc.operations.len(), 1);

        let op = &doc.operations[0];
        assert_eq!(op.selections.len(), 1);

        if let Selection::Field(f) = &op.selections[0] {
            assert_eq!(f.name, "user");
            assert_eq!(f.arguments, vec![("id", Value::Int(1))]);
            assert_eq!(f.selections.len(), 1);
        } else {
            panic!("Expected Field");
        }
    }

    #[test]
    fn test_execution() {
        let input = r#"{ user(id: 1) { id, name } }"#;
        let lexer = Lexer::new(input);
        let mut parser = Parser::new(lexer);
        let doc = parser.parse_document().unwrap();

        let root = RootResolver;
        let result = execute(&doc, &root);

        if let Value::Object(map) = result {
            if let Value::Object(user_map) = map.get("user").unwrap() {
                assert_eq!(user_map.get("id").unwrap(), &Value::Int(1));
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
    fn test_execution_list() {
        let input = r#"{ users { name } }"#;
        let lexer = Lexer::new(input);
        let mut parser = Parser::new(lexer);
        let doc = parser.parse_document().unwrap();

        let root = RootResolver;
        let result = execute(&doc, &root);

        if let Value::Object(map) = result {
            if let Value::List(users) = map.get("users").unwrap() {
                assert_eq!(users.len(), 2);
                if let Value::Object(u1) = &users[0] {
                    assert_eq!(u1.get("name").unwrap(), &Value::String("Alice".to_string()));
                }
            } else {
                panic!("Expected List");
            }
        } else {
            panic!("Expected Object");
        }
    }
}
