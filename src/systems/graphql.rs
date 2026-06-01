//! GraphQL Execution Engine
//!
//! # What this implements and what crate(s) it replaces
//! This is a minimal, dynamic GraphQL execution engine that parses a query string into an AST
//! and executes it against a schema using a trait-based resolver. It replaces crates like
//! `async-graphql` and `juniper` by demonstrating how query traversal and field resolution work
//! under the hood without relying on compile-time macros.
//!
//! Real-world systems that use this pattern include GitHub's API, Hasura, and Apollo Server.
//!
//! # Why build it yourself?
//! Building a GraphQL engine from scratch teaches you about recursive descent parsing,
//! dynamic dispatch in Rust, and traversing trees. It also highlights the performance costs
//! of dynamic schemas and the overhead of validating queries against types.
//!
//! # Architecture
//! The architecture consists of three main components:
//! 1.  **Lexer**: Converts a raw string into a stream of tokens.
//! 2.  **Parser**: Converts tokens into an AST (`Document`, `Operation`, `Selection`, `Field`).
//! 3.  **Executor**: Takes a root `Resolver` and walks the AST, calling the resolver
//!     dynamically to build a JSON-like `Value` tree.
//!
//! ```text
//! +--------------+     +-------+     +--------+     +-------+     +----------+
//! | Query String | --> | Lexer | --> | Tokens | --> | Parser| --> |    AST   |
//! +--------------+     +-------+     +--------+     +-------+     +----------+
//!                                                                      |
//!                                                                      v
//! +-------+            +---------------+                        +----------+
//! | Value | <--------- |    Executor   | <--------------------- | Resolver |
//! +-------+            +---------------+                        +----------+
//! ```
//!
//! **Invariants:**
//! - Lexer must never panic on malformed UTF-8; it must safely traverse char boundaries.
//! - The AST must correctly nest field selections, treating leaf nodes as empty vectors of selections.
//! - Execution must never return data that was not explicitly requested by the selection set (no over-fetching in the final `Value`).
//!
//! | Phase     | Time Complexity | Space Complexity |
//! |-----------|-----------------|------------------|
//! | Lexing    | O(N)            | O(N)             |
//! | Parsing   | O(T)            | O(T)             |
//! | Execution | O(F)            | O(F)             |
//! * N = query length, T = tokens count, F = fields requested.
//!
//! # Design decisions and tradeoffs vs. alternatives
//! - **Dynamic typing over static compilation**: We use a `Value` enum to represent the result
//!   and a dynamic `Resolver` trait. Production Rust GraphQL crates heavily use procedural macros
//!   to map Rust types directly to GraphQL schema types at compile time, providing better safety
//!   and speed but longer compile times.
//! - **Synchronous execution**: For simplicity, this executor is synchronous. Production
//!   implementations are fully asynchronous to handle concurrent database or network calls for
//!   different fields.
//! - **Minimal AST**: We only support a subset of GraphQL (queries, fields, arguments, aliases,
//!   fragments). Mutations and subscriptions are omitted.
//!
//! # Footer
//! - **Canonical crate**: `async-graphql` (async, macro-heavy, very fast), `juniper` (sync/async, slightly older).
//! - **Missing vs Production**: This implementation lacks validation against a schema (checking types before execution),
//!   error accumulation (returning all errors instead of short-circuiting), variables, directives,
//!   and async execution.
//! - **Next Steps**: Add a full schema definition language (SDL) parser and validation phase.

use std::collections::HashMap;

/// Represents the possible values returned by a GraphQL field.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Boolean(bool),
    Int(i64),
    Float(f64),
    String(String),
    List(Vec<Value>),
    Object(HashMap<String, Value>),
}

/// The core trait for any object that can resolve GraphQL fields.
///
/// // RUST INSIGHT:
/// // By using a trait object (`&dyn Resolver`) or generic bounds, we can build a dynamic
/// // tree of resolvers without needing a pre-compiled schema.
pub trait Resolver {
    fn resolve(&self, field_name: &str, args: &HashMap<String, Value>) -> Result<Value, String>;
}

// --- Lexer ---

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Name(String),
    Int(i64),
    Float(f64),
    String(String),
    Punctuator(char),
    Eof,
}

pub struct Lexer<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Lexer { input, pos: 0 }
    }

    fn skip_whitespace_and_comments(&mut self) {
        while self.pos < self.input.len() {
            let c = self.input[self.pos..].chars().next().unwrap();
            if c.is_whitespace() || c == ',' {
                self.pos += c.len_utf8();
            } else if c == '#' {
                self.pos += c.len_utf8();
                while self.pos < self.input.len() {
                    let next_c = self.input[self.pos..].chars().next().unwrap();
                    self.pos += next_c.len_utf8();
                    if next_c == '\n' {
                        break;
                    }
                }
            } else {
                break;
            }
        }
    }

    fn read_name(&mut self) -> String {
        let start = self.pos;
        while self.pos < self.input.len() {
            let c = self.input[self.pos..].chars().next().unwrap();
            if c.is_alphanumeric() || c == '_' {
                self.pos += c.len_utf8();
            } else {
                break;
            }
        }
        self.input[start..self.pos].to_string()
    }

    fn read_number(&mut self) -> Token {
        let start = self.pos;
        let mut is_float = false;

        if self.pos < self.input.len() && self.input[self.pos..].starts_with('-') {
            self.pos += 1;
        }

        while self.pos < self.input.len() {
            let c = self.input[self.pos..].chars().next().unwrap();
            if c.is_ascii_digit() {
                self.pos += c.len_utf8();
            } else if c == '.' || c == 'e' || c == 'E' {
                is_float = true;
                self.pos += c.len_utf8();
            } else if is_float && (c == '+' || c == '-') {
                self.pos += c.len_utf8();
            } else {
                break;
            }
        }

        let num_str = &self.input[start..self.pos];
        if is_float {
            Token::Float(num_str.parse().unwrap_or(0.0))
        } else {
            Token::Int(num_str.parse().unwrap_or(0))
        }
    }

    fn read_string(&mut self) -> String {
        self.pos += 1; // Skip quote
        let start = self.pos;
        while self.pos < self.input.len() {
            let c = self.input[self.pos..].chars().next().unwrap();
            if c == '"' {
                break;
            }
            self.pos += c.len_utf8();
        }
        let res = self.input[start..self.pos].to_string();
        if self.pos < self.input.len() {
            self.pos += 1; // Skip closing quote
        }
        res
    }

    pub fn next_token(&mut self) -> Token {
        self.skip_whitespace_and_comments();

        if self.pos >= self.input.len() {
            return Token::Eof;
        }

        let c = self.input[self.pos..].chars().next().unwrap();

        if c.is_alphabetic() || c == '_' {
            Token::Name(self.read_name())
        } else if c.is_ascii_digit() || c == '-' {
            self.read_number()
        } else if c == '"' {
            Token::String(self.read_string())
        } else {
            // Punctuators
            match c {
                '{' | '}' | '(' | ')' | '[' | ']' | ':' | '=' | '@' | '$' | '!' | '&' | '|' => {
                    self.pos += 1;
                    Token::Punctuator(c)
                }
                '.' => {
                    if self.pos + 2 < self.input.len()
                        && &self.input[self.pos..self.pos + 3] == "..."
                    {
                        self.pos += 3;
                        Token::Name("...".to_string())
                    } else {
                        self.pos += 1;
                        Token::Punctuator(c)
                    }
                }
                _ => {
                    self.pos += c.len_utf8();
                    Token::Punctuator(c) // Unrecognized
                }
            }
        }
    }
}

// --- Parser & AST ---

#[derive(Debug, Clone)]
pub struct Document {
    pub operations: Vec<Operation>,
    pub fragments: HashMap<String, Fragment>,
}

#[derive(Debug, Clone)]
pub struct Operation {
    pub op_type: String, // query, mutation, etc. (default: query)
    pub name: Option<String>,
    pub selection_set: Vec<Selection>,
}

#[derive(Debug, Clone)]
pub struct Fragment {
    pub name: String,
    pub type_condition: String,
    pub selection_set: Vec<Selection>,
}

#[derive(Debug, Clone)]
pub enum Selection {
    Field(Field),
    FragmentSpread(String),
    InlineFragment(InlineFragment),
}

#[derive(Debug, Clone)]
pub struct Field {
    pub alias: Option<String>,
    pub name: String,
    pub arguments: HashMap<String, Value>,
    pub selection_set: Vec<Selection>,
}

#[derive(Debug, Clone)]
pub struct InlineFragment {
    pub type_condition: Option<String>,
    pub selection_set: Vec<Selection>,
}

pub struct Parser<'a> {
    lexer: Lexer<'a>,
    current_token: Token,
}

impl<'a> Parser<'a> {
    pub fn new(input: &'a str) -> Self {
        let mut lexer = Lexer::new(input);
        let current_token = lexer.next_token();
        Parser {
            lexer,
            current_token,
        }
    }

    fn advance(&mut self) {
        self.current_token = self.lexer.next_token();
    }

    fn expect_punctuator(&mut self, expected: char) -> Result<(), String> {
        match self.current_token {
            Token::Punctuator(c) if c == expected => {
                self.advance();
                Ok(())
            }
            _ => Err(format!("Expected '{}', got {:?}", expected, self.current_token)),
        }
    }

    fn expect_name(&mut self) -> Result<String, String> {
        match &self.current_token {
            Token::Name(name) => {
                let res = name.clone();
                self.advance();
                Ok(res)
            }
            _ => Err(format!("Expected name, got {:?}", self.current_token)),
        }
    }

    pub fn parse(&mut self) -> Result<Document, String> {
        let mut operations = Vec::new();
        let mut fragments = HashMap::new();

        while self.current_token != Token::Eof {
            if let Token::Name(ref name) = self.current_token {
                if name == "fragment" {
                    let frag = self.parse_fragment()?;
                    fragments.insert(frag.name.clone(), frag);
                } else if name == "query" || name == "mutation" || name == "subscription" {
                    operations.push(self.parse_operation()?);
                } else {
                    // Shorthand query
                    operations.push(Operation {
                        op_type: "query".to_string(),
                        name: None,
                        selection_set: self.parse_selection_set()?,
                    });
                }
            } else if let Token::Punctuator('{') = self.current_token {
                 // Shorthand query
                 operations.push(Operation {
                    op_type: "query".to_string(),
                    name: None,
                    selection_set: self.parse_selection_set()?,
                });
            } else {
                return Err(format!("Unexpected token at document level: {:?}", self.current_token));
            }
        }

        Ok(Document { operations, fragments })
    }

    fn parse_operation(&mut self) -> Result<Operation, String> {
        let op_type = self.expect_name()?;
        let mut name = None;
        if let Token::Name(_) = self.current_token {
            name = Some(self.expect_name()?);
        }

        // Skip variables if any (...)
        if let Token::Punctuator('(') = self.current_token {
            self.advance();
            while self.current_token != Token::Punctuator(')') && self.current_token != Token::Eof {
                self.advance(); // naive skip
            }
            self.expect_punctuator(')')?;
        }

        let selection_set = self.parse_selection_set()?;

        Ok(Operation {
            op_type,
            name,
            selection_set,
        })
    }

    fn parse_fragment(&mut self) -> Result<Fragment, String> {
        self.expect_name()?; // "fragment"
        let name = self.expect_name()?;

        let on = self.expect_name()?;
        if on != "on" {
            return Err("Expected 'on' in fragment".to_string());
        }

        let type_condition = self.expect_name()?;
        let selection_set = self.parse_selection_set()?;

        Ok(Fragment {
            name,
            type_condition,
            selection_set,
        })
    }

    fn parse_selection_set(&mut self) -> Result<Vec<Selection>, String> {
        let mut selections = Vec::new();
        self.expect_punctuator('{')?;

        while self.current_token != Token::Punctuator('}') && self.current_token != Token::Eof {
            selections.push(self.parse_selection()?);
        }

        self.expect_punctuator('}')?;
        Ok(selections)
    }

    fn parse_selection(&mut self) -> Result<Selection, String> {
        if let Token::Name(name) = &self.current_token {
            if name == "..." {
                self.advance();
                // Check if inline fragment or fragment spread
                if let Token::Name(n) = &self.current_token {
                    if n == "on" {
                        self.advance();
                        let type_condition = Some(self.expect_name()?);
                        let selection_set = self.parse_selection_set()?;
                        return Ok(Selection::InlineFragment(InlineFragment {
                            type_condition,
                            selection_set,
                        }));
                    }
                }

                if let Token::Punctuator('{') = self.current_token {
                     let selection_set = self.parse_selection_set()?;
                     return Ok(Selection::InlineFragment(InlineFragment {
                         type_condition: None,
                         selection_set,
                     }));
                } else {
                     let frag_name = self.expect_name()?;
                     return Ok(Selection::FragmentSpread(frag_name));
                }
            }
        }

        self.parse_field().map(Selection::Field)
    }

    fn parse_field(&mut self) -> Result<Field, String> {
        let name_or_alias = self.expect_name()?;
        let mut alias = None;
        let mut name = name_or_alias.clone();

        if let Token::Punctuator(':') = self.current_token {
            self.advance();
            alias = Some(name_or_alias);
            name = self.expect_name()?;
        }

        let mut arguments = HashMap::new();
        if let Token::Punctuator('(') = self.current_token {
            self.advance();
            while self.current_token != Token::Punctuator(')') && self.current_token != Token::Eof {
                let arg_name = self.expect_name()?;
                self.expect_punctuator(':')?;
                let arg_val = self.parse_value()?;
                arguments.insert(arg_name, arg_val);
            }
            self.expect_punctuator(')')?;
        }

        let mut selection_set = Vec::new();
        if let Token::Punctuator('{') = self.current_token {
            selection_set = self.parse_selection_set()?;
        }

        Ok(Field {
            alias,
            name,
            arguments,
            selection_set,
        })
    }

    fn parse_value(&mut self) -> Result<Value, String> {
        match self.current_token.clone() {
            Token::Int(i) => {
                self.advance();
                Ok(Value::Int(i))
            }
            Token::Float(f) => {
                self.advance();
                Ok(Value::Float(f))
            }
            Token::String(s) => {
                self.advance();
                Ok(Value::String(s))
            }
            Token::Name(s) => {
                self.advance();
                if s == "true" {
                    Ok(Value::Boolean(true))
                } else if s == "false" {
                    Ok(Value::Boolean(false))
                } else if s == "null" {
                    Ok(Value::Null)
                } else {
                    // Treat enum values as strings for simplicity
                    Ok(Value::String(s))
                }
            }
            Token::Punctuator('[') => {
                self.advance();
                let mut list = Vec::new();
                while self.current_token != Token::Punctuator(']') && self.current_token != Token::Eof {
                    list.push(self.parse_value()?);
                }
                self.expect_punctuator(']')?;
                Ok(Value::List(list))
            }
            Token::Punctuator('{') => {
                self.advance();
                let mut obj = HashMap::new();
                while self.current_token != Token::Punctuator('}') && self.current_token != Token::Eof {
                    let k = self.expect_name()?;
                    self.expect_punctuator(':')?;
                    let v = self.parse_value()?;
                    obj.insert(k, v);
                }
                self.expect_punctuator('}')?;
                Ok(Value::Object(obj))
            }
            _ => Err(format!("Unexpected token in value: {:?}", self.current_token)),
        }
    }
}

// --- Executor ---

/// Executes a GraphQL query against a root resolver.
pub fn execute(query: &str, root: &dyn Resolver) -> Result<Value, String> {
    let mut parser = Parser::new(query);
    let doc = parser.parse()?;

    if doc.operations.is_empty() {
        return Err("No operations found".to_string());
    }

    // GOTCHA: We only execute the first operation for simplicity.
    // A production engine would check the `operationName` argument.
    let op = &doc.operations[0];

    execute_selection_set(&op.selection_set, root, &doc.fragments)
}

fn execute_selection_set(
    selections: &[Selection],
    resolver: &dyn Resolver,
    fragments: &HashMap<String, Fragment>,
) -> Result<Value, String> {
    let mut result_map = HashMap::new();

    for selection in selections {
        match selection {
            Selection::Field(field) => {
                let response_key = field.alias.as_ref().unwrap_or(&field.name).clone();
                let resolved_value = resolver.resolve(&field.name, &field.arguments)?;

                // If there's a sub-selection, the resolved value MUST be an object or a list of objects
                // In this simplified engine, we assume the user implements custom resolvers that handle
                // returning `Object` (or we could pass the sub-selection to the resolver if it returns another Resolver).
                //
                // RUST INSIGHT:
                // We're mixing data-loading and graph-traversal here. If `resolved_value` is an Object,
                // and we have a selection set, we filter the object.
                // If it's a List of Objects, we filter each object in the list.

                let final_value = if !field.selection_set.is_empty() {
                    apply_selection_to_value(resolved_value, &field.selection_set, fragments)?
                } else {
                    resolved_value
                };

                result_map.insert(response_key, final_value);
            }
            Selection::FragmentSpread(frag_name) => {
                if let Some(frag) = fragments.get(frag_name) {
                    // PRODUCTION NOTE: We should check type condition here.
                    let frag_result = execute_selection_set(&frag.selection_set, resolver, fragments)?;
                    if let Value::Object(obj) = frag_result {
                        for (k, v) in obj {
                            result_map.insert(k, v);
                        }
                    }
                }
            }
            Selection::InlineFragment(inline_frag) => {
                // PRODUCTION NOTE: We should check type condition here.
                let frag_result = execute_selection_set(&inline_frag.selection_set, resolver, fragments)?;
                 if let Value::Object(obj) = frag_result {
                    for (k, v) in obj {
                        result_map.insert(k, v);
                    }
                }
            }
        }
    }

    Ok(Value::Object(result_map))
}

fn apply_selection_to_value(
    value: Value,
    selections: &[Selection],
    fragments: &HashMap<String, Fragment>,
) -> Result<Value, String> {
    match value {
        Value::Object(obj) => {
            // Create a dummy resolver that just looks up keys in the object
            struct ObjectResolver {
                data: HashMap<String, Value>,
            }
            impl Resolver for ObjectResolver {
                fn resolve(&self, field_name: &str, _args: &HashMap<String, Value>) -> Result<Value, String> {
                    Ok(self.data.get(field_name).cloned().unwrap_or(Value::Null))
                }
            }
            let resolver = ObjectResolver { data: obj };
            execute_selection_set(selections, &resolver, fragments)
        }
        Value::List(list) => {
            let mut result_list = Vec::new();
            for item in list {
                result_list.push(apply_selection_to_value(item, selections, fragments)?);
            }
            Ok(Value::List(result_list))
        }
        _ => Err("Cannot apply selection set to scalar value".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct QueryRoot;

    impl Resolver for QueryRoot {
        fn resolve(&self, field_name: &str, args: &HashMap<String, Value>) -> Result<Value, String> {
            match field_name {
                "me" => {
                    let mut user = HashMap::new();
                    user.insert("id".to_string(), Value::String("1".to_string()));
                    user.insert("name".to_string(), Value::String("Alice".to_string()));
                    Ok(Value::Object(user))
                }
                "user" => {
                    let id = args.get("id").and_then(|v| {
                        if let Value::String(s) = v { Some(s.clone()) } else { None }
                    }).unwrap_or_default();

                    let mut user = HashMap::new();
                    user.insert("id".to_string(), Value::String(id.clone()));
                    user.insert("name".to_string(), Value::String(format!("User {}", id)));
                    Ok(Value::Object(user))
                }
                "posts" => {
                    let mut p1 = HashMap::new();
                    p1.insert("title".to_string(), Value::String("Hello".to_string()));
                    let mut p2 = HashMap::new();
                    p2.insert("title".to_string(), Value::String("World".to_string()));

                    Ok(Value::List(vec![Value::Object(p1), Value::Object(p2)]))
                }
                _ => Ok(Value::Null),
            }
        }
    }

    #[test]
    fn test_basic_query() {
        let query = "{ me { name } }";
        let root = QueryRoot;
        let res = execute(query, &root).unwrap();

        if let Value::Object(map) = res {
            assert!(map.contains_key("me"));
            if let Value::Object(me_map) = &map["me"] {
                assert_eq!(me_map["name"], Value::String("Alice".to_string()));
                assert!(!me_map.contains_key("id")); // id was not requested
            } else {
                panic!("me is not an object");
            }
        } else {
            panic!("root is not an object");
        }
    }

    #[test]
    fn test_arguments_and_aliases() {
        let query = "{ user1: user(id: \"10\") { id name } }";
        let root = QueryRoot;
        let res = execute(query, &root).unwrap();

        if let Value::Object(map) = res {
            assert!(map.contains_key("user1"));
            if let Value::Object(user_map) = &map["user1"] {
                assert_eq!(user_map["id"], Value::String("10".to_string()));
                assert_eq!(user_map["name"], Value::String("User 10".to_string()));
            } else {
                panic!("user1 is not an object");
            }
        } else {
            panic!("root is not an object");
        }
    }

    #[test]
    fn test_list_resolution() {
        let query = "{ posts { title } }";
        let root = QueryRoot;
        let res = execute(query, &root).unwrap();

        if let Value::Object(map) = res {
            assert!(map.contains_key("posts"));
            if let Value::List(list) = &map["posts"] {
                assert_eq!(list.len(), 2);
                if let Value::Object(p1) = &list[0] {
                    assert_eq!(p1["title"], Value::String("Hello".to_string()));
                }
            } else {
                panic!("posts is not a list");
            }
        } else {
            panic!("root is not an object");
        }
    }

    #[test]
    fn test_benchmark_note() {
        // PRODUCTION NOTE:
        // To properly benchmark this parsing and execution engine against `async-graphql`,
        // one should use the `criterion` crate in a `benches/` directory.
        // A simple inline micro-benchmark could look like this:
        let query = "{ posts { title } }";
        let root = QueryRoot;

        let start = std::time::Instant::now();
        for _ in 0..10_000 {
            std::hint::black_box(super::execute(query, &root).unwrap());
        }
        let duration = start.elapsed();
        // println!("10,000 queries took: {:?}", duration);
        assert!(duration.as_millis() < 1000, "Should be relatively fast");
    }

    #[test]
    fn test_fragments() {
        let query = "
            query {
                me {
                    ...UserFields
                }
            }
            fragment UserFields on User {
                id
                name
            }
        ";
        let root = QueryRoot;
        let res = execute(query, &root).unwrap();

        if let Value::Object(map) = res {
            if let Value::Object(me_map) = &map["me"] {
                assert_eq!(me_map["name"], Value::String("Alice".to_string()));
                assert_eq!(me_map["id"], Value::String("1".to_string()));
            } else {
                panic!("me is not an object");
            }
        } else {
            panic!("root is not an object");
        }
    }
}
