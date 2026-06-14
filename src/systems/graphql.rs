//! # GraphQL Execution Engine
//!
//! Implements a custom GraphQL execution engine from scratch.
//!
//! **Replaces Crates:** `async-graphql`, `juniper`
//!
//! **Real-world Usage:**
//! - API Gateways and BFFs (Backend for Frontend)
//! - Content Management Systems (e.g., Contentful)
//! - Federation layers combining multiple microservices
//!
//! **Why build it yourself?**
//! Building a GraphQL engine teaches you recursive descent parsing, Abstract Syntax Tree (AST) evaluation,
//! and dynamic dispatch in Rust. You learn how to traverse trees and how trait objects (`Box<dyn Trait>`)
//! can be used to model dynamically resolved schema fields without compile-time macros.

use std::collections::HashMap;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Flow:
//
//   [Query String] ──► Lexer ──► [Tokens] ──► Parser ──► [AST Document]
//                                                               │
//                                                               ▼
//                                                            Executor
//                                                               │
//                                                               ▼
//                                                      [Resolved JSON/Value]
//
// Invariants:
// 1. The lexer accurately tokenizes standard GraphQL syntax without panicking on multibyte characters.
// 2. The parser produces a valid AST representing the query structure.
// 3. Execution resolves fields recursively using dynamic `Resolver` trait objects.
//
// Complexity:
// ┌───────────┬──────────────┬──────────────┐
// │ Operation │ Time         │ Space        │
// ├───────────┼──────────────┼──────────────┤
// │ Lexing    │ O(N)         │ O(N)         │
// │ Parsing   │ O(N)         │ O(N)         │
// │ Execution │ O(Nodes * R) │ O(Depth * R) │
// └───────────┴──────────────┴──────────────┘
// N = Length of query, Nodes = Number of AST nodes, R = Resolver complexity.
//
// Design Decisions:
// - **AST Representation**: We use strong typing for AST nodes but output dynamic `GqlValue`
//   structures to represent JSON-like responses.
// - **Execution Engine**: We use dynamic dispatch `Box<dyn Resolver>` for field resolution.
//   - *Tradeoff*: Introduces heap allocation and virtual function call overhead per field.
//   - *Alternative*: Procedural macros to generate statically typed resolvers (like `async-graphql`),
//     which avoids dynamic dispatch but vastly increases compile times and macro complexity.
// PRODUCTION NOTE: A production GraphQL parser like `async-graphql`'s internal parser would use zero-copy string slices (`&'a str`) to avoid allocating thousands of strings per query. We use owned strings here for simplicity.
// - **Memory Management**: The parser outputs owned `String` values for identifiers and the evaluated output representation rather than zero-copy string slices to avoid lifetime and borrowing conflicts with dynamically generated trait objects.

// --- AST & Values ---

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Token {
    Name(String),
    String(String),
    Int(i64),
    Float(String), // Simplification: store as string for Eq, parsed later if needed.
    Punctuation(char),
    Eof,
}

#[derive(Debug, PartialEq, Clone)]
pub enum GqlValue {
    Null,
    Int(i64),
    Float(f64),
    String(String),
    Boolean(bool),
    List(Vec<GqlValue>),
    Object(HashMap<String, GqlValue>),
}

#[derive(Debug, PartialEq, Clone)]
pub struct Argument {
    pub name: String,
    pub value: GqlValue,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Field {
    pub name: String,
    pub alias: Option<String>,
    pub arguments: Vec<Argument>,
    pub selection_set: Vec<Field>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Operation {
    pub op_type: String,
    pub name: Option<String>,
    pub selection_set: Vec<Field>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Document {
    pub operations: Vec<Operation>,
}

// --- Lexer ---

pub struct Lexer<'a> {
    input: &'a str,
    chars: std::str::CharIndices<'a>,
    current_char: Option<(usize, char)>,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        let mut chars = input.char_indices();
        let current_char = chars.next();
        Self {
            input,
            chars,
            current_char,
        }
    }

    fn advance(&mut self) {
        self.current_char = self.chars.next();
    }

    fn skip_whitespace(&mut self) {
        while let Some((_, c)) = self.current_char {
            // GraphQL considers commas as whitespace
            if c.is_whitespace() || c == ',' {
                self.advance();
            } else {
                break;
            }
        }
    }

    pub fn next_token(&mut self) -> Token {
        self.skip_whitespace();

        if let Some((start, c)) = self.current_char {
            match c {
                '{' | '}' | '(' | ')' | ':' | '[' | ']' | '!' | '$' | '=' => {
                    self.advance();
                    Token::Punctuation(c)
                }
                '"' => {
                    self.advance(); // Skip initial quote
                    // GOTCHA: Avoid manually incrementing byte indices (`i += 1`) for string slicing, as this causes panics on multibyte Unicode characters. Instead, safely advance indices using `.char_indices()`.
                    let str_start = self.current_char.map(|(i, _)| i).unwrap_or(self.input.len());
                    while let Some((_, ch)) = self.current_char {
                        if ch == '"' {
                            break;
                        }
                        self.advance();
                    }
                    let str_end = self.current_char.map(|(i, _)| i).unwrap_or(self.input.len());
                    if self.current_char.is_some() {
                        self.advance(); // Skip closing quote
                    }
                    Token::String(self.input[str_start..str_end].to_string())
                }
                _ if c.is_ascii_alphabetic() || c == '_' => {
                    let name_start = start;
                    while let Some((_, ch)) = self.current_char {
                        if ch.is_ascii_alphanumeric() || ch == '_' {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    let name_end = self.current_char.map(|(i, _)| i).unwrap_or(self.input.len());
                    Token::Name(self.input[name_start..name_end].to_string())
                }
                _ if c.is_ascii_digit() || c == '-' => {
                    let num_start = start;
                    let mut is_float = false;
                    while let Some((_, ch)) = self.current_char {
                        if ch.is_ascii_digit() || ch == '-' {
                            self.advance();
                        } else if ch == '.' {
                            is_float = true;
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    let num_end = self.current_char.map(|(i, _)| i).unwrap_or(self.input.len());
                    let num_str = &self.input[num_start..num_end];
                    if is_float {
                        Token::Float(num_str.to_string())
                    } else {
                        Token::Int(num_str.parse().unwrap_or(0))
                    }
                }
                _ => {
                    self.advance();
                    self.next_token() // Skip unsupported characters
                }
            }
        } else {
            Token::Eof
        }
    }
}

// --- Parser ---

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

    fn expect_name(&mut self) -> Result<String, String> {
        if let Token::Name(name) = &self.current_token {
            let n = name.clone();
            self.advance();
            Ok(n)
        } else {
            Err(format!("Expected Name, found {:?}", self.current_token))
        }
    }

    fn expect_punctuation(&mut self, p: char) -> Result<(), String> {
        if let Token::Punctuation(c) = self.current_token {
            if c == p {
                self.advance();
                return Ok(());
            }
        }
        Err(format!(
            "Expected punctuation '{}', found {:?}",
            p, self.current_token
        ))
    }

    pub fn parse_document(&mut self) -> Result<Document, String> {
        let mut operations = Vec::new();
        while self.current_token != Token::Eof {
            if self.current_token == Token::Punctuation('{') {
                let selection_set = self.parse_selection_set()?;
                operations.push(Operation {
                    op_type: "query".to_string(),
                    name: None,
                    selection_set,
                });
            } else if let Token::Name(op) = &self.current_token {
                let op_type = op.clone();
                self.advance();
                let mut name = None;
                if let Token::Name(n) = &self.current_token {
                    name = Some(n.clone());
                    self.advance();
                }
                let selection_set = self.parse_selection_set()?;
                operations.push(Operation {
                    op_type,
                    name,
                    selection_set,
                });
            } else {
                return Err("Expected operation definition or selection set".to_string());
            }
        }
        Ok(Document { operations })
    }

    fn parse_selection_set(&mut self) -> Result<Vec<Field>, String> {
        self.expect_punctuation('{')?;
        let mut fields = Vec::new();
        while self.current_token != Token::Punctuation('}') && self.current_token != Token::Eof {
            fields.push(self.parse_field()?);
        }
        self.expect_punctuation('}')?;
        Ok(fields)
    }

    fn parse_field(&mut self) -> Result<Field, String> {
        let name_or_alias = self.expect_name()?;
        let mut name = name_or_alias.clone();
        let mut alias = None;

        if self.current_token == Token::Punctuation(':') {
            self.advance();
            alias = Some(name_or_alias);
            name = self.expect_name()?;
        }

        let mut arguments = Vec::new();
        if self.current_token == Token::Punctuation('(') {
            self.advance();
            while self.current_token != Token::Punctuation(')') && self.current_token != Token::Eof
            {
                let arg_name = self.expect_name()?;
                self.expect_punctuation(':')?;
                let arg_value = self.parse_value()?;
                arguments.push(Argument {
                    name: arg_name,
                    value: arg_value,
                });
            }
            self.expect_punctuation(')')?;
        }

        let mut selection_set = Vec::new();
        if self.current_token == Token::Punctuation('{') {
            selection_set = self.parse_selection_set()?;
        }

        Ok(Field {
            name,
            alias,
            arguments,
            selection_set,
        })
    }

    fn parse_value(&mut self) -> Result<GqlValue, String> {
        match &self.current_token {
            Token::Int(i) => {
                let v = GqlValue::Int(*i);
                self.advance();
                Ok(v)
            }
            Token::Float(f) => {
                let parsed = f.parse().map_err(|_| "Invalid float".to_string())?;
                let v = GqlValue::Float(parsed);
                self.advance();
                Ok(v)
            }
            Token::String(s) => {
                let v = GqlValue::String(s.clone());
                self.advance();
                Ok(v)
            }
            Token::Name(n) => {
                let v = match n.as_str() {
                    "true" => GqlValue::Boolean(true),
                    "false" => GqlValue::Boolean(false),
                    "null" => GqlValue::Null,
                    _ => GqlValue::String(n.clone()), // Treat enums as strings
                };
                self.advance();
                Ok(v)
            }
            _ => Err(format!(
                "Unexpected token for value: {:?}",
                self.current_token
            )),
        }
    }
}

// --- Execution Engine ---

// RUST INSIGHT:
// By using `Box<dyn Resolver>`, we enable dynamic dispatch.
// This allows us to map arbitrary Rust types to GraphQL types at runtime
// without needing complex compile-time procedural macros.
// We return an owned representation (`ResolvedValue` over `String` instead of zero-copy references)
// to avoid lifetime and borrowing conflicts with the dynamically generated trait objects.
pub enum ResolvedValue {
    Value(GqlValue),
    Object(Box<dyn Resolver>),
    List(Vec<ResolvedValue>),
}

pub trait Resolver: Send + Sync {
    fn resolve_field(&self, name: &str, args: &[Argument]) -> Result<ResolvedValue, String>;
}

pub struct Executor {
    root_query: Box<dyn Resolver>,
}

impl Executor {
    pub fn new(root_query: Box<dyn Resolver>) -> Self {
        Self { root_query }
    }

    pub fn execute(&self, doc: &Document) -> Result<GqlValue, String> {
        // Find the first query operation
        let op = doc
            .operations
            .iter()
            .find(|o| o.op_type == "query" || o.op_type.is_empty())
            .ok_or_else(|| "No query operation found".to_string())?;

        self.execute_selection_set(&op.selection_set, self.root_query.as_ref())
    }

    fn execute_selection_set(
        &self,
        selection_set: &[Field],
        resolver: &dyn Resolver,
    ) -> Result<GqlValue, String> {
        let mut map = HashMap::new();
        for field in selection_set {
            let resolved = resolver.resolve_field(&field.name, &field.arguments)?;
            let value = self.execute_resolved(resolved, &field.selection_set)?;
            let key = field.alias.as_ref().unwrap_or(&field.name).clone();
            map.insert(key, value);
        }
        Ok(GqlValue::Object(map))
    }

    fn execute_resolved(
        &self,
        resolved: ResolvedValue,
        selection_set: &[Field],
    ) -> Result<GqlValue, String> {
        match resolved {
            ResolvedValue::Value(val) => {
                if !selection_set.is_empty() {
                    return Err("Scalars cannot have a selection set".to_string());
                }
                Ok(val)
            }
            ResolvedValue::Object(obj_resolver) => {
                self.execute_selection_set(selection_set, obj_resolver.as_ref())
            }
            ResolvedValue::List(items) => {
                let mut list = Vec::new();
                for item in items {
                    list.push(self.execute_resolved(item, selection_set)?);
                }
                Ok(GqlValue::List(list))
            }
        }
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `async-graphql`: Uses compile-time macros to generate highly optimized, statically typed resolvers. It fully supports Async, Subscriptions, and complex types.
// - `juniper`: Similar to async-graphql but older.
//
// Missing vs. Production:
// - **Async Execution**: Real engines execute resolvers concurrently.
// - **Type Checking against Schema**: Our engine skips the validation phase and relies on resolvers to not panic or fail.
// - **Variables and Fragments**: Missing support for query variables and fragment spreads.
//
// Next Steps:
// 1. Add schema definition language (SDL) parsing.
// 2. Validate incoming queries against the schema before execution.
// 3. Make `Resolver::resolve_field` async using `async_trait` or native async traits.

#[cfg(test)]
mod tests {
    use super::*;

    struct DummyUser {
        id: i64,
        name: String,
    }

    impl Resolver for DummyUser {
        fn resolve_field(&self, name: &str, _args: &[Argument]) -> Result<ResolvedValue, String> {
            match name {
                "id" => Ok(ResolvedValue::Value(GqlValue::Int(self.id))),
                "name" => Ok(ResolvedValue::Value(GqlValue::String(self.name.clone()))),
                _ => Err(format!("Unknown field on User: {}", name)),
            }
        }
    }

    struct RootQuery;

    impl Resolver for RootQuery {
        fn resolve_field(&self, name: &str, args: &[Argument]) -> Result<ResolvedValue, String> {
            match name {
                "user" => {
                    let id_arg = args.iter().find(|a| a.name == "id");
                    let id = if let Some(arg) = id_arg {
                        if let GqlValue::Int(val) = arg.value {
                            val
                        } else {
                            return Err("Expected Int for id".to_string());
                        }
                    } else {
                        1
                    };
                    Ok(ResolvedValue::Object(Box::new(DummyUser {
                        id,
                        name: format!("User {}", id),
                    })))
                }
                "users" => {
                    let users = vec![
                        ResolvedValue::Object(Box::new(DummyUser {
                            id: 1,
                            name: "Alice".to_string(),
                        })),
                        ResolvedValue::Object(Box::new(DummyUser {
                            id: 2,
                            name: "Bob".to_string(),
                        })),
                    ];
                    Ok(ResolvedValue::List(users))
                }
                _ => Err(format!("Unknown query field: {}", name)),
            }
        }
    }

    #[test]
    fn test_lexer() {
        let input = r#"query { user(id: 42) { name } }"#;
        let mut lexer = Lexer::new(input);
        assert_eq!(lexer.next_token(), Token::Name("query".to_string()));
        assert_eq!(lexer.next_token(), Token::Punctuation('{'));
        assert_eq!(lexer.next_token(), Token::Name("user".to_string()));
        assert_eq!(lexer.next_token(), Token::Punctuation('('));
        assert_eq!(lexer.next_token(), Token::Name("id".to_string()));
        assert_eq!(lexer.next_token(), Token::Punctuation(':'));
        assert_eq!(lexer.next_token(), Token::Int(42));
        assert_eq!(lexer.next_token(), Token::Punctuation(')'));
        assert_eq!(lexer.next_token(), Token::Punctuation('{'));
        assert_eq!(lexer.next_token(), Token::Name("name".to_string()));
        assert_eq!(lexer.next_token(), Token::Punctuation('}'));
        assert_eq!(lexer.next_token(), Token::Punctuation('}'));
        assert_eq!(lexer.next_token(), Token::Eof);
    }

    #[test]
    fn test_parser_and_execution() {
        let input = r#"
            {
                u1: user(id: 10) {
                    id
                    name
                }
                users {
                    name
                }
            }
        "#;
        let lexer = Lexer::new(input);
        let mut parser = Parser::new(lexer);
        let doc = parser.parse_document().expect("Failed to parse");

        let executor = Executor::new(Box::new(RootQuery));
        let result = executor.execute(&doc).expect("Execution failed");

        if let GqlValue::Object(map) = result {
            // Check u1
            let u1 = map.get("u1").expect("Missing u1 alias");
            if let GqlValue::Object(u1_map) = u1 {
                assert_eq!(u1_map.get("id"), Some(&GqlValue::Int(10)));
                assert_eq!(
                    u1_map.get("name"),
                    Some(&GqlValue::String("User 10".to_string()))
                );
            } else {
                panic!("Expected Object for u1");
            }

            // Check users
            let users = map.get("users").expect("Missing users field");
            if let GqlValue::List(list) = users {
                assert_eq!(list.len(), 2);
                if let GqlValue::Object(first_user) = &list[0] {
                    assert_eq!(
                        first_user.get("name"),
                        Some(&GqlValue::String("Alice".to_string()))
                    );
                }
            } else {
                panic!("Expected List for users");
            }
        } else {
            panic!("Expected Object as root result");
        }
    }

    #[test]
    fn test_multibyte_characters() {
        let input = r#" { user(name: "アリス") { name } } "#;
        let mut lexer = Lexer::new(input);
        assert_eq!(lexer.next_token(), Token::Punctuation('{'));
        assert_eq!(lexer.next_token(), Token::Name("user".to_string()));
        assert_eq!(lexer.next_token(), Token::Punctuation('('));
        assert_eq!(lexer.next_token(), Token::Name("name".to_string()));
        assert_eq!(lexer.next_token(), Token::Punctuation(':'));
        assert_eq!(lexer.next_token(), Token::String("アリス".to_string()));
    }

    #[test]
    fn test_benchmark_note() {
        // BENCHMARKING NOTE:
        // To benchmark this executor against `async-graphql`:
        // 1. Use `criterion::Criterion`.
        // 2. Parse a deeply nested query (e.g., `query { users { friends { friends { name } } } }`).
        // 3. Compare the time taken by `Executor::execute` vs `async-graphql::Schema::execute`.
        // Expected result: This dynamic dispatch approach will be significantly slower than
        // `async-graphql`'s statically generated resolvers, especially on large lists, due to virtual method call overhead.
    }
}
