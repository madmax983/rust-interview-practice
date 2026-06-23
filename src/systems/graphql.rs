//! # GraphQL Execution Engine
//!
//! Implements a minimal GraphQL lexer, parser, and trait-based recursive execution engine.
//!
//! **Replaces Crates:** `async-graphql`, `juniper`
//!
//! **Real-world Usage:**
//! - API gateways (Apollo Router, Hasura)
//! - Backend-for-Frontend (BFF) layers
//! - Unified data graph resolvers
//!
//! **Why build it yourself?**
//! Understanding how a GraphQL string becomes an abstract syntax tree (AST) and how that tree is recursively resolved against a data graph demystifies "magic" crates like `async-graphql`. You'll learn how to write a minimal lexer without relying on regex, build a recursive parser, and use trait objects (`Box<dyn Resolver>`) to execute queries against a dynamic schema safely in Rust.
//!
//! =========================================================================================
//! # Architecture
//! =========================================================================================
//!
//! Data Structure:
//!
//!      Query String
//!         │ Lexer
//!         ▼
//!      Tokens [Ident("query"), Ident("user"), ...]
//!         │ Parser
//!         ▼
//!      AST (Document -> Operation -> SelectionSet -> Field)
//!         │ Executor (with Root Resolver)
//!         ▼
//!      Result JSON Map
//!
//! Invariants:
//! 1. Lexer gracefully handles invalid characters via iterative looping, preventing stack overflows.
//! 2. Execution uses owned data types (`String`, `Value`) for the evaluated output to avoid lifetime conflicts with dynamically generated trait objects.
//! 3. Resolvers are Trait Objects to allow dynamic schemas and custom execution logic.
//!
//! Complexity:
//! ┌───────────┬──────────────┬──────────────┐
//! │ Operation │ Time         │ Space        │
//! ├───────────┼──────────────┼──────────────┤
//! │ Lexing    │ O(N)         │ O(N)         │
//! │ Parsing   │ O(N)         │ O(depth)     │
//! │ Execution │ O(nodes)     │ O(nodes)     │
//! └───────────┴──────────────┴──────────────┘
//!
//! Design Decisions:
//! - **Iterative Lexer**: Replaces tail-recursion with a `loop` over the character stream to prevent stack overflow on extremely large or invalid inputs.
//! - **Owned Output Values**: The execution engine returns `Value` (an enum wrapper over owned types like `String`, `i64`, `HashMap`) rather than string slices. This prevents lifetime issues when resolvers load data dynamically (e.g., from a database or remote API).
//! - **No async/await**: Kept synchronous for simplicity and deep focus on the lexing/parsing/resolving lifecycle. Production engines heavily rely on `async`.

use std::collections::HashMap;
use std::fmt;
use std::iter::Peekable;
use std::str::Chars;

// =========================================================================================
// Lexer
// =========================================================================================

#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    Ident(String),
    Punctuation(char),
    StringLit(String),
    IntLit(i64),
    Eof,
}

pub struct Lexer<'a> {
    input: Peekable<Chars<'a>>,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Lexer {
            input: input.chars().peekable(),
        }
    }

    pub fn next_token(&mut self) -> Token {
        // RUST INSIGHT: We use a loop here instead of tail recursion. While functional languages
        // optimize tail calls, Rust does not guarantee TCO (Tail Call Optimization).
        // Using `loop` prevents stack overflow on malformed or extremely padded inputs.
        loop {
            match self.input.peek() {
                Some(&c) if c.is_whitespace() || c == ',' => {
                    self.input.next();
                    continue; // Skip whitespace and commas
                }
                Some(&c) if c == '#' => {
                    self.input.next();
                    while let Some(&c) = self.input.peek() {
                        if c == '\n' || c == '\r' {
                            break;
                        }
                        self.input.next();
                    }
                    continue; // Skip comments
                }
                Some(&c) if c.is_ascii_alphabetic() || c == '_' => {
                    let mut ident = String::new();
                    while let Some(&c) = self.input.peek() {
                        if c.is_ascii_alphanumeric() || c == '_' {
                            ident.push(c);
                            self.input.next();
                        } else {
                            break;
                        }
                    }
                    return Token::Ident(ident);
                }
                Some(&c) if c.is_ascii_digit() => {
                    let mut num_str = String::new();
                    while let Some(&c) = self.input.peek() {
                        if c.is_ascii_digit() {
                            num_str.push(c);
                            self.input.next();
                        } else {
                            break;
                        }
                    }
                    return Token::IntLit(num_str.parse().unwrap_or(0));
                }
                Some(&c) if c == '"' => {
                    self.input.next(); // Consume opening quote
                    let mut lit = String::new();
                    while let Some(&c) = self.input.peek() {
                        if c == '"' {
                            self.input.next(); // Consume closing quote
                            break;
                        } else {
                            lit.push(c);
                            self.input.next();
                        }
                    }
                    return Token::StringLit(lit);
                }
                Some(&c) if "{}:()[]".contains(c) => {
                    let punc = c;
                    self.input.next();
                    return Token::Punctuation(punc);
                }
                Some(_) => {
                    // PRODUCTION NOTE: Invalid characters are skipped gracefully here for simplicity.
                    // A production lexer (like Apollo) would return an Error token or fail fast.
                    self.input.next();
                    continue;
                }
                None => return Token::Eof,
            }
        }
    }
}

// =========================================================================================
// Parser
// =========================================================================================

#[derive(Debug, PartialEq, Clone)]
pub struct Field {
    pub name: String,
    pub alias: Option<String>,
    pub arguments: HashMap<String, Value>,
    pub selection_set: Vec<Field>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Operation {
    pub operation_type: String, // "query", "mutation"
    pub name: Option<String>,
    pub selection_set: Vec<Field>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Document {
    pub operations: Vec<Operation>,
}

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(mut lexer: Lexer) -> Self {
        let mut tokens = Vec::new();
        loop {
            let tok = lexer.next_token();
            if tok == Token::Eof {
                tokens.push(tok);
                break;
            }
            tokens.push(tok);
        }
        Parser { tokens, pos: 0 }
    }

    fn peek(&self) -> &Token {
        self.tokens.get(self.pos).unwrap_or(&Token::Eof)
    }

    fn consume(&mut self) -> &Token {
        let tok = self.peek();
        if *tok != Token::Eof {
            self.pos += 1;
        }
        self.tokens.get(self.pos - 1).unwrap_or(&Token::Eof)
    }

    fn consume_ident(&mut self) -> Result<String, String> {
        match self.consume() {
            Token::Ident(s) => Ok(s.clone()),
            tok => Err(format!("Expected identifier, got {:?}", tok)),
        }
    }

    fn expect_punctuation(&mut self, expected: char) -> Result<(), String> {
        match self.consume() {
            Token::Punctuation(c) if *c == expected => Ok(()),
            tok => Err(format!("Expected punctuation '{}', got {:?}", expected, tok)),
        }
    }

    pub fn parse(&mut self) -> Result<Document, String> {
        let mut operations = Vec::new();
        while *self.peek() != Token::Eof {
            operations.push(self.parse_operation()?);
        }
        Ok(Document { operations })
    }

    fn parse_operation(&mut self) -> Result<Operation, String> {
        let operation_type = match self.peek() {
            Token::Ident(s) if s == "query" || s == "mutation" => {
                let op = s.clone();
                self.consume();
                op
            }
            Token::Punctuation('{') => "query".to_string(), // Shorthand query
            tok => return Err(format!("Expected 'query', 'mutation' or '{{', got {:?}", tok)),
        };

        let name = if let Token::Ident(s) = self.peek() {
            let n = s.clone();
            self.consume();
            Some(n)
        } else {
            None
        };

        let selection_set = self.parse_selection_set()?;

        Ok(Operation {
            operation_type,
            name,
            selection_set,
        })
    }

    fn parse_selection_set(&mut self) -> Result<Vec<Field>, String> {
        self.expect_punctuation('{')?;
        let mut fields = Vec::new();

        while *self.peek() != Token::Punctuation('}') && *self.peek() != Token::Eof {
            fields.push(self.parse_field()?);
        }

        self.expect_punctuation('}')?;
        Ok(fields)
    }

    fn parse_field(&mut self) -> Result<Field, String> {
        let mut name = self.consume_ident()?;
        let mut alias = None;

        if let Token::Punctuation(':') = self.peek() {
            self.consume();
            alias = Some(name);
            name = self.consume_ident()?;
        }

        let mut arguments = HashMap::new();
        if let Token::Punctuation('(') = self.peek() {
            self.consume();
            while *self.peek() != Token::Punctuation(')') && *self.peek() != Token::Eof {
                let arg_name = self.consume_ident()?;
                self.expect_punctuation(':')?;
                let arg_val = self.parse_value()?;
                arguments.insert(arg_name, arg_val);
            }
            self.expect_punctuation(')')?;
        }

        let selection_set = if let Token::Punctuation('{') = self.peek() {
            self.parse_selection_set()?
        } else {
            Vec::new()
        };

        Ok(Field {
            name,
            alias,
            arguments,
            selection_set,
        })
    }

    fn parse_value(&mut self) -> Result<Value, String> {
        match self.consume() {
            Token::StringLit(s) => Ok(Value::String(s.clone())),
            Token::IntLit(i) => Ok(Value::Int(*i)),
            Token::Ident(s) if s == "true" => Ok(Value::Boolean(true)),
            Token::Ident(s) if s == "false" => Ok(Value::Boolean(false)),
            Token::Ident(s) if s == "null" => Ok(Value::Null),
            tok => Err(format!("Expected value, got {:?}", tok)),
        }
    }
}

// =========================================================================================
// Execution Engine
// =========================================================================================

#[derive(Debug, PartialEq, Clone)]
pub enum Value {
    Null,
    Boolean(bool),
    Int(i64),
    String(String),
    List(Vec<Value>),
    Object(HashMap<String, Value>),
}

// Implement Display for Value to make testing/JSON-like output easier
impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Null => write!(f, "null"),
            Value::Boolean(b) => write!(f, "{}", b),
            Value::Int(i) => write!(f, "{}", i),
            Value::String(s) => write!(f, "\"{}\"", s.replace('"', "\\\"")),
            Value::List(l) => {
                write!(f, "[")?;
                for (i, v) in l.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", v)?;
                }
                write!(f, "]")
            }
            Value::Object(o) => {
                write!(f, "{{")?;
                let mut first = true;
                for (k, v) in o {
                    if !first {
                        write!(f, ", ")?;
                    }
                    write!(f, "\"{}\": {}", k, v)?;
                    first = false;
                }
                write!(f, "}}")
            }
        }
    }
}

/// A trait for resolving fields dynamically.
pub trait Resolver {
    fn resolve_field(&self, name: &str, args: &HashMap<String, Value>) -> Result<Value, String>;
}

/// Evaluates a GraphQL document against a root resolver.
pub fn execute(document: &Document, root_resolver: &dyn Resolver) -> Result<Value, String> {
    if document.operations.is_empty() {
        return Err("No operations found".to_string());
    }

    let op = &document.operations[0]; // Simplification: Execute first operation
    execute_selection_set(&op.selection_set, root_resolver)
}

fn execute_selection_set(
    selection_set: &[Field],
    resolver: &dyn Resolver,
) -> Result<Value, String> {
    let mut result = HashMap::new();

    for field in selection_set {
        let field_key = field.alias.as_ref().unwrap_or(&field.name).clone();
        let resolved_value = resolver.resolve_field(&field.name, &field.arguments)?;

        // If the value is an object and we have sub-selections, we need to recursively resolve.
        // In a full implementation, `resolve_field` would return an object that *implements* Resolver,
        // rather than just a raw `Value`, allowing recursive evaluation.
        // For this minimal engine, if a field has a selection set, we expect the resolver
        // to handle the sub-fields manually, or we map it if it returned a raw object.
        let final_value = if !field.selection_set.is_empty() {
            match resolved_value {
                Value::Object(map) => {
                    // Create a temporary resolver for the map
                    struct MapResolver(HashMap<String, Value>);
                    impl Resolver for MapResolver {
                        fn resolve_field(
                            &self,
                            name: &str,
                            _args: &HashMap<String, Value>,
                        ) -> Result<Value, String> {
                            self.0
                                .get(name)
                                .cloned()
                                .ok_or_else(|| format!("Field {} not found", name))
                        }
                    }
                    let map_resolver = MapResolver(map);
                    execute_selection_set(&field.selection_set, &map_resolver)?
                }
                Value::List(list) => {
                    // Evaluate the sub-selection for every element in the list
                    let mut eval_list = Vec::new();
                    for item in list {
                        if let Value::Object(map) = item {
                            struct MapResolver(HashMap<String, Value>);
                            impl Resolver for MapResolver {
                                fn resolve_field(
                                    &self,
                                    name: &str,
                                    _args: &HashMap<String, Value>,
                                ) -> Result<Value, String> {
                                    self.0
                                        .get(name)
                                        .cloned()
                                        .ok_or_else(|| format!("Field {} not found", name))
                                }
                            }
                            let map_resolver = MapResolver(map);
                            eval_list.push(execute_selection_set(&field.selection_set, &map_resolver)?);
                        } else {
                            return Err(format!("Field '{}' returned a list containing non-objects, which cannot have sub-selections", field.name));
                        }
                    }
                    Value::List(eval_list)
                }
                _ => return Err(format!("Field '{}' has sub-selections but did not resolve to an object or list of objects", field.name)),
            }
        } else {
            resolved_value
        };

        result.insert(field_key, final_value);
    }

    Ok(Value::Object(result))
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `async-graphql`: Highly optimized, fully async, uses macros extensively to map Rust types to GraphQL schemas automatically, supports subscriptions (WebSockets).
// - `juniper`: Also macro-heavy, sync/async support, focused on schema-first or code-first approaches.
//
// Missing vs. Production:
// - **Schema Validation**: We execute queries blindly against resolvers without validating against a type system (Schema).
// - **Async Execution**: Resolvers block the thread. Production systems use `.await` to concurrently fetch disjoint fields.
// - **Variables & Directives**: We don't support `$var` injection or `@skip`/`@include` directives.
// - **Fragments**: No support for `...FragmentName`.
//
// Next Steps:
// 1. Introduce an `async` trait for the `Resolver` and use futures::future::join_all to resolve fields concurrently.
// 2. Add an AST for the Schema definition language (SDL) and implement validation before execution.
//
// Benchmarking Note:
// To benchmark this engine against `async-graphql`, use `criterion` to measure:
// 1. Lexing/Parsing throughput (MB/s) of large nested query strings.
// 2. Execution overhead for deeply nested selections on static data (`std::hint::black_box`).

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lexer() {
        let query = "{ user(id: 1) { name, age } }";
        let mut lexer = Lexer::new(query);

        assert_eq!(lexer.next_token(), Token::Punctuation('{'));
        assert_eq!(lexer.next_token(), Token::Ident("user".to_string()));
        assert_eq!(lexer.next_token(), Token::Punctuation('('));
        assert_eq!(lexer.next_token(), Token::Ident("id".to_string()));
        assert_eq!(lexer.next_token(), Token::Punctuation(':'));
        assert_eq!(lexer.next_token(), Token::IntLit(1));
        assert_eq!(lexer.next_token(), Token::Punctuation(')'));
        assert_eq!(lexer.next_token(), Token::Punctuation('{'));
        assert_eq!(lexer.next_token(), Token::Ident("name".to_string()));
        assert_eq!(lexer.next_token(), Token::Ident("age".to_string()));
        assert_eq!(lexer.next_token(), Token::Punctuation('}'));
        assert_eq!(lexer.next_token(), Token::Punctuation('}'));
        assert_eq!(lexer.next_token(), Token::Eof);
    }

    #[test]
    fn test_parser() {
        let query = "query GetUser { me: user(id: 123) { name } }";
        let lexer = Lexer::new(query);
        let mut parser = Parser::new(lexer);
        let doc = parser.parse().unwrap();

        assert_eq!(doc.operations.len(), 1);
        let op = &doc.operations[0];
        assert_eq!(op.operation_type, "query");
        assert_eq!(op.name, Some("GetUser".to_string()));
        assert_eq!(op.selection_set.len(), 1);

        let field = &op.selection_set[0];
        assert_eq!(field.name, "user");
        assert_eq!(field.alias, Some("me".to_string()));
        assert_eq!(field.arguments.get("id"), Some(&Value::Int(123)));
        assert_eq!(field.selection_set[0].name, "name");
    }

    struct Root;
    impl Resolver for Root {
        fn resolve_field(&self, name: &str, args: &HashMap<String, Value>) -> Result<Value, String> {
            match name {
                "version" => Ok(Value::String("1.0.0".to_string())),
                "user" => {
                    let mut map = HashMap::new();
                    map.insert("name".to_string(), Value::String("Alice".to_string()));
                    let age = match args.get("id") {
                        Some(Value::Int(1)) => 30,
                        _ => 25,
                    };
                    map.insert("age".to_string(), Value::Int(age));
                    Ok(Value::Object(map))
                }
                _ => Err(format!("Unknown field {}", name)),
            }
        }
    }

    #[test]
    fn test_execution() {
        let query = "{ version, user(id: 1) { name age } }";
        let lexer = Lexer::new(query);
        let mut parser = Parser::new(lexer);
        let doc = parser.parse().unwrap();

        let root = Root;
        let result = execute(&doc, &root).unwrap();

        match result {
            Value::Object(map) => {
                assert_eq!(map.get("version"), Some(&Value::String("1.0.0".to_string())));
                match map.get("user") {
                    Some(Value::Object(user_map)) => {
                        assert_eq!(user_map.get("name"), Some(&Value::String("Alice".to_string())));
                        assert_eq!(user_map.get("age"), Some(&Value::Int(30)));
                    }
                    _ => panic!("Expected user object"),
                }
            }
            _ => panic!("Expected object root result"),
        }
    }
}
