//! # GraphQL Execution Engine
//!
//! Implements a dynamic GraphQL execution engine from scratch.
//! This parses queries into an Abstract Syntax Tree (AST) and executes them against
//! a typed schema using a trait-based `Resolver` interface.
//!
//! **Replaces Crates:** `async-graphql`, `juniper`
//!
//! **Real-world Usage:**
//! - Core backend services exposing unified data graphs.
//! - API Gateways and BFF (Backend-For-Frontend) layers.
//!
//! **Why build it yourself?**
//! Building a GraphQL engine demystifies the magic of compile-time macros and dynamic dispatch.
//! You learn how to write lexers, parsers for nested selection sets, and how to execute a tree
//! of asynchronous or synchronous resolvers recursively while maintaining type safety.

use std::collections::HashMap;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Flow:
//
//   [Query String]
//         │
//         ▼ (Lexer)
//     [Tokens]
//         │
//         ▼ (Parser)
//   [Query AST (Document -> Operation -> SelectionSet -> Field)]
//         │
//         ▼ (Executor + Root Resolver)
//   [Evaluated Output Representation (Value)]
//
// Invariants:
// 1. Every requested field in the AST must be resolved by the corresponding type's Resolver.
// 2. The output structure exactly matches the shape of the SelectionSet.
// 3. Output representations use owned types (`String`) rather than zero-copy string slices
//    to avoid lifetime constraints during dynamic dispatch with trait objects.
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ Lexing        │ O(N)        │ O(N)        │
// │ Parsing       │ O(N)        │ O(N)        │
// │ Execution     │ O(N * R)    │ O(D)        │
// └───────────────┴─────────────┴─────────────┘
// N = Query size, R = Avg resolve time, D = Query Depth
//
// Design Decisions:
// - **Dynamic Resolution**: We use `Box<dyn Resolver>` for object resolution to allow
//   recursive evaluation without complex generic constraints or compile-time macros.
// - **Owned Data**: Returning `String` and `Value` rather than `&str` avoids nightmare
//   lifetime boundaries when resolvers fetch data dynamically.

// =========================================================================================
// Types & AST
// =========================================================================================

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Token {
    Name(String),
    StringLiteral(String),
    IntLiteral(i64),
    Punctuation(char),
}

#[derive(Debug, Clone)]
pub struct FieldNode {
    pub name: String,
    pub selection_set: Vec<FieldNode>,
}

pub struct QueryDocument {
    pub selection_set: Vec<FieldNode>,
}

// RUST INSIGHT: Using owned strings and `i64` instead of referencing the query string.
// This simplifies the execution engine, where data might come from external sources
// and outlive the original query string.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Value {
    Null,
    String(String),
    Int(i64),
    Object(HashMap<String, Value>),
    List(Vec<Value>),
}

// =========================================================================================
// Resolver Interface
// =========================================================================================

pub enum FieldResult {
    Scalar(Value),
    Object(Box<dyn Resolver>),
    List(Vec<Box<dyn Resolver>>),
    Error(String),
}

// PRODUCTION NOTE: Production GraphQL engines compile the schema and optimize resolvers ahead of time, often parallelizing execution. Here we evaluate sequentially.
// PRODUCTION NOTE: Production GraphQL engines compile the schema and optimize resolvers ahead of time, often parallelizing execution. Here we evaluate sequentially.
// RUST INSIGHT: Trait objects (`dyn Resolver`) allow us to return different concrete types
// that implement the trait from our fields, enabling dynamic graphs.
pub trait Resolver {
    fn resolve(&self, field_name: &str) -> FieldResult;
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

    fn peek_char(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.peek_char()?;
        // GOTCHA: Use `len_utf8()` to safely advance over multibyte characters
        self.pos += c.len_utf8();
        Some(c)
    }

    fn consume_whitespace(&mut self) {
        while let Some(c) = self.peek_char() {
            if c.is_whitespace() || c == ',' {
                self.advance();
            } else {
                break;
            }
        }
    }

    pub fn next_token(&mut self) -> Option<Token> {
        loop {
            self.consume_whitespace();
            let c = self.peek_char()?;

            if c.is_alphabetic() || c == '_' {
                let mut name = String::new();
                while let Some(ch) = self.peek_char() {
                    if ch.is_alphanumeric() || ch == '_' {
                        name.push(ch);
                        self.advance();
                    } else {
                        break;
                    }
                }
                return Some(Token::Name(name));
            } else if c == '{' || c == '}' {
                self.advance();
                return Some(Token::Punctuation(c));
            } else {
                // Simplification: Not handling all GraphQL grammar (arguments, aliases, fragments)
                // GOTCHA: Avoid recursion here to prevent stack overflow on long invalid sequences
                self.advance();
            }
        }
    }
    pub fn tokenize(&mut self) -> Vec<Token> {
        let mut tokens = Vec::new();
        while let Some(t) = self.next_token() {
            tokens.push(t);
        }
        tokens
    }
}

// =========================================================================================
// Parser
// =========================================================================================

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
        let token = self.tokens.get(self.pos);
        self.pos += 1;
        token
    }

    pub fn parse(&mut self) -> Result<QueryDocument, String> {
        let selection_set = self.parse_selection_set()?;
        Ok(QueryDocument { selection_set })
    }

    fn parse_selection_set(&mut self) -> Result<Vec<FieldNode>, String> {
        let mut fields = Vec::new();

        if let Some(Token::Punctuation('{')) = self.peek() {
            self.advance();
        } else {
            return Ok(fields); // EOF or empty
        }

        while let Some(token) = self.peek() {
            match token {
                Token::Punctuation('}') => {
                    self.advance();
                    break;
                }
                Token::Name(name) => {
                    let field_name = name.clone();
                    self.advance();

                    let mut selection_set = Vec::new();
                    if let Some(Token::Punctuation('{')) = self.peek() {
                        selection_set = self.parse_selection_set()?;
                    }

                    fields.push(FieldNode {
                        name: field_name,
                        selection_set,
                    });
                }
                _ => return Err("Unexpected token in selection set".to_string()),
            }
        }

        Ok(fields)
    }
}

// =========================================================================================
// Executor
// =========================================================================================

pub struct Executor;

impl Executor {
    pub fn execute(query: &QueryDocument, root: &dyn Resolver) -> Value {
        Self::execute_selection_set(&query.selection_set, root)
    }

    fn execute_selection_set(selection_set: &[FieldNode], resolver: &dyn Resolver) -> Value {
        let mut result_map = HashMap::new();

        for field in selection_set {
            let field_result = resolver.resolve(&field.name);

            let resolved_value = match field_result {
                FieldResult::Scalar(val) => val,
                FieldResult::Object(child_resolver) => {
                    if field.selection_set.is_empty() {
                        Value::Null // Object requested without selection set
                    } else {
                        Self::execute_selection_set(&field.selection_set, child_resolver.as_ref())
                    }
                }
                FieldResult::List(child_resolvers) => {
                    if field.selection_set.is_empty() {
                        Value::Null
                    } else {
                        let mut list_vals = Vec::new();
                        for cr in child_resolvers {
                            list_vals.push(Self::execute_selection_set(&field.selection_set, cr.as_ref()));
                        }
                        Value::List(list_vals)
                    }
                }
                FieldResult::Error(err) => Value::String(format!("ERROR: {}", err)),
            };

            result_map.insert(field.name.clone(), resolved_value);
        }

        Value::Object(result_map)
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `async-graphql` and `juniper` heavily utilize Rust's macro system to generate resolving
//   logic at compile-time. They are async-first and heavily optimized.
// - This implementation uses dynamic dispatch (`dyn Resolver`) which allocates and follows
//   pointers, making it slower but much simpler to understand without macros.
//
// Missing vs. Production:
// - No support for GraphQL Arguments, Variables, Aliases, or Fragments.
// - No asynchronous resolution (`async/await`).
// - No schema validation phase (we just execute the raw AST).
// - No introspection query support.
//
// Benchmark Note: Use `criterion` to benchmark the overhead of `Box<dyn Resolver>` allocation vs a static schema.
// Benchmark Note: Use `criterion` to benchmark the overhead of `Box<dyn Resolver>` allocation vs a static schema.
// Suggested next steps:
// 1. Add support for Arguments to `FieldNode` and `resolve()`.
// 2. Make `resolve` return a `Future` for async execution.

#[cfg(test)]
mod tests {
    use super::*;

    struct UserResolver {
        id: i64,
        name: String,
    }

    impl Resolver for UserResolver {
        fn resolve(&self, field_name: &str) -> FieldResult {
            match field_name {
                "id" => FieldResult::Scalar(Value::Int(self.id)),
                "name" => FieldResult::Scalar(Value::String(self.name.clone())),
                _ => FieldResult::Error("Unknown field".to_string()),
            }
        }
    }

    struct RootResolver;

    impl Resolver for RootResolver {
        fn resolve(&self, field_name: &str) -> FieldResult {
            match field_name {
                "me" => FieldResult::Object(Box::new(UserResolver {
                    id: 42,
                    name: "Alice".to_string(),
                })),
                "users" => {
                    let u1 = Box::new(UserResolver { id: 1, name: "Bob".to_string() }) as Box<dyn Resolver>;
                    let u2 = Box::new(UserResolver { id: 2, name: "Charlie".to_string() }) as Box<dyn Resolver>;
                    FieldResult::List(vec![u1, u2])
                }
                _ => FieldResult::Error("Unknown field".to_string()),
            }
        }
    }

    #[test]
    fn test_graphql_execution() {
        let query_str = "{ me { id name } }";

        let mut lexer = Lexer::new(query_str);
        let tokens = lexer.tokenize();

        let mut parser = Parser::new(tokens);
        let doc = parser.parse().unwrap();

        let root = RootResolver;
        let result = Executor::execute(&doc, &root);

        let me_obj = match result {
            Value::Object(map) => map.get("me").unwrap().clone(),
            _ => panic!("Expected Object"),
        };

        match me_obj {
            Value::Object(map) => {
                assert_eq!(map.get("id"), Some(&Value::Int(42)));
                assert_eq!(map.get("name"), Some(&Value::String("Alice".to_string())));
            }
            _ => panic!("Expected me to be an Object"),
        }
    }

    #[test]
    fn test_graphql_list_execution() {
        let query_str = "{ users { name } }";

        let mut lexer = Lexer::new(query_str);
        let mut parser = Parser::new(lexer.tokenize());
        let doc = parser.parse().unwrap();

        let result = Executor::execute(&doc, &RootResolver);

        let users_list = match result {
            Value::Object(map) => map.get("users").unwrap().clone(),
            _ => panic!("Expected Object"),
        };

        match users_list {
            Value::List(items) => {
                assert_eq!(items.len(), 2);
                if let Value::Object(ref map) = items[0] {
                    assert_eq!(map.get("name"), Some(&Value::String("Bob".to_string())));
                } else {
                    panic!("Expected item to be an Object");
                }
            }
            _ => panic!("Expected users to be a List"),
        }
    }

    #[test]
    fn test_graphql_error_handling() {
        let query_str = "{ nonExistentField }";

        let mut lexer = Lexer::new(query_str);
        let mut parser = Parser::new(lexer.tokenize());
        let doc = parser.parse().unwrap();

        let result = Executor::execute(&doc, &RootResolver);

        match result {
            Value::Object(map) => {
                let err_val = map.get("nonExistentField").unwrap();
                assert_eq!(err_val, &Value::String("ERROR: Unknown field".to_string()));
            }
            _ => panic!("Expected Object"),
        }
    }
}
