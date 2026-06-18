//! # GraphQL Execution Engine
//!
//! Implements a minimal GraphQL Execution Engine from scratch.
//! It parses GraphQL queries into an Abstract Syntax Tree (AST) and evaluates
//! them dynamically against a typed schema using a trait-based `Resolver` interface.
//!
//! **Replaces Crates:** `async-graphql`, `juniper`
//!
//! **Real-world Usage:**
//! - API Gateways (e.g., Apollo Router, Hasura) dynamically resolving queries.
//! - Backend microservices exposing structured, typed graphs of data.
//! - Headless CMS platforms evaluating custom frontend queries.
//!
//! **Why build it yourself?**
//! Implementing a GraphQL engine reveals the complexity behind dynamic query resolution.
//! You learn how to write a lexer and recursive descent parser, and more importantly,
//! how to model a trait-based evaluation engine where field resolution is deferred to
//! runtime structures rather than compile-time macros.
//!
//! # Architecture
//!
//! ```text
//! Query String -> Lexer -> [Tokens] -> Parser -> AST -> Executor
//!                                                          |
//! Schema (Resolvers) <-------------------------------------+
//! ```
//!
//! - **Lexer:** Tokenizes the query string. Uses an iterative approach rather than tail recursion to prevent stack overflows on long malformed inputs.
//! - **Parser:** Converts tokens into a query AST (Operations, Selections, Fields).
//! - **Executor:** Recursively traverses the AST, invoking the matching `Resolver` trait methods on the schema.
//!
//! ## Invariants
//! - The lexer must never panic on multibyte characters; it advances using UTF-8 char boundaries.
//! - The executor must return matching JSON-like output structures corresponding exactly to the requested fields.
//! - Resolvers must be able to return scalar values or further nested objects (Trait Objects).
//!
//! ## Complexity
//! - **Lexing/Parsing:** O(N) where N is the length of the query string.
//! - **Execution:** O(V * F) where V is the number of visited nodes in the graph, and F is the cost of the resolver function.

use std::collections::HashMap;

// =========================================================================================
// Tokenizer (Lexer)
// =========================================================================================

#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    Name(String),
    StringLiteral(String),
    IntLiteral(i64),
    LeftBrace,
    RightBrace,
    LeftParen,
    RightParen,
    Colon,
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

    fn peek_char(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.peek_char()?;
        // GOTCHA: Using c.len_utf8() ensures we safely step over multibyte characters
        // instead of blindly pos += 1 which could panic.
        self.pos += c.len_utf8();
        Some(c)
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek_char() {
            if c.is_whitespace() || c == ',' {
                self.advance();
            } else {
                break;
            }
        }
    }

    // RUST INSIGHT: We use a `loop` iterative approach for tokenization instead of
    // tail-recursive `next_token()` calls. This prevents stack overflows on deeply
    // nested or long strings of invalid tokens.
    pub fn next_token(&mut self) -> Token {
        loop {
            self.skip_whitespace();

            let c = match self.peek_char() {
                Some(ch) => ch,
                None => return Token::Eof,
            };

            match c {
                '{' => {
                    self.advance();
                    return Token::LeftBrace;
                }
                '}' => {
                    self.advance();
                    return Token::RightBrace;
                }
                '(' => {
                    self.advance();
                    return Token::LeftParen;
                }
                ')' => {
                    self.advance();
                    return Token::RightParen;
                }
                ':' => {
                    self.advance();
                    return Token::Colon;
                }
                '"' => return self.read_string(),
                _ if c.is_alphabetic() || c == '_' => return self.read_name(),
                _ if c.is_ascii_digit() || c == '-' => return self.read_number(),
                _ => {
                    // Skip invalid char and loop again
                    self.advance();
                }
            }
        }
    }

    fn read_name(&mut self) -> Token {
        let start = self.pos;
        while let Some(c) = self.peek_char() {
            if c.is_alphanumeric() || c == '_' {
                self.advance();
            } else {
                break;
            }
        }
        Token::Name(self.input[start..self.pos].to_string())
    }

    fn read_string(&mut self) -> Token {
        self.advance(); // skip opening quote
        let start = self.pos;
        while let Some(c) = self.peek_char() {
            if c == '"' {
                break;
            }
            self.advance();
        }
        let val = self.input[start..self.pos].to_string();
        self.advance(); // skip closing quote
        Token::StringLiteral(val)
    }

    fn read_number(&mut self) -> Token {
        let start = self.pos;
        if self.peek_char() == Some('-') {
            self.advance();
        }
        while let Some(c) = self.peek_char() {
            if c.is_ascii_digit() {
                self.advance();
            } else {
                break;
            }
        }
        let num_str = &self.input[start..self.pos];
        let val = num_str.parse::<i64>().unwrap_or(0);
        Token::IntLiteral(val)
    }
}

// =========================================================================================
// AST
// =========================================================================================

#[derive(Debug, PartialEq, Clone)]
pub struct Query {
    pub selections: Vec<Selection>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Selection {
    pub name: String,
    pub arguments: HashMap<String, Value>,
    pub sub_selections: Vec<Selection>,
}

// RUST INSIGHT: We use owned types (String) instead of string slices (`&str`) to represent
// values. This avoids lifetime constraints leaking into the dynamic evaluation engine,
// allowing trait objects (`Box<dyn Resolver>`) to return independently owned data.
#[derive(Debug, PartialEq, Clone)]
pub enum Value {
    String(String),
    Int(i64),
    Object(HashMap<String, Value>),
    List(Vec<Value>),
    Null,
}

// =========================================================================================
// Parser
// =========================================================================================

pub struct Parser<'a> {
    lexer: Lexer<'a>,
    current_token: Token,
}

impl<'a> Parser<'a> {
    pub fn new(mut lexer: Lexer<'a>) -> Self {
        let current_token = lexer.next_token();
        Parser { lexer, current_token }
    }

    fn advance(&mut self) {
        self.current_token = self.lexer.next_token();
    }

    fn expect(&mut self, expected: Token) -> bool {
        if self.current_token == expected {
            self.advance();
            true
        } else {
            false
        }
    }

    pub fn parse_query(&mut self) -> Result<Query, String> {
        // Optional "query" keyword
        if let Token::Name(n) = &self.current_token {
            if n == "query" {
                self.advance();
                // Optional query name
                if let Token::Name(_) = self.current_token {
                    self.advance();
                }
            }
        }

        let selections = self.parse_selection_set()?;
        Ok(Query { selections })
    }

    fn parse_selection_set(&mut self) -> Result<Vec<Selection>, String> {
        let mut selections = Vec::new();
        if !self.expect(Token::LeftBrace) {
            return Err("Expected '{'".to_string());
        }

        while self.current_token != Token::RightBrace && self.current_token != Token::Eof {
            selections.push(self.parse_selection()?);
        }

        if !self.expect(Token::RightBrace) {
            return Err("Expected '}'".to_string());
        }

        Ok(selections)
    }

    fn parse_selection(&mut self) -> Result<Selection, String> {
        let name = match &self.current_token {
            Token::Name(n) => n.clone(),
            _ => return Err("Expected field name".to_string()),
        };
        self.advance();

        let mut arguments = HashMap::new();
        if self.current_token == Token::LeftParen {
            self.advance();
            while self.current_token != Token::RightParen && self.current_token != Token::Eof {
                let arg_name = match &self.current_token {
                    Token::Name(n) => n.clone(),
                    _ => return Err("Expected argument name".to_string()),
                };
                self.advance();

                if !self.expect(Token::Colon) {
                    return Err("Expected ':' after argument name".to_string());
                }

                let arg_val = self.parse_value()?;
                arguments.insert(arg_name, arg_val);
            }
            self.expect(Token::RightParen);
        }

        let mut sub_selections = Vec::new();
        if self.current_token == Token::LeftBrace {
            sub_selections = self.parse_selection_set()?;
        }

        Ok(Selection {
            name,
            arguments,
            sub_selections,
        })
    }

    fn parse_value(&mut self) -> Result<Value, String> {
        match &self.current_token {
            Token::StringLiteral(s) => {
                let v = Value::String(s.clone());
                self.advance();
                Ok(v)
            }
            Token::IntLiteral(i) => {
                let v = Value::Int(*i);
                self.advance();
                Ok(v)
            }
            _ => Err("Expected value".to_string()),
        }
    }
}

// =========================================================================================
// Executor
// =========================================================================================

/// Trait defining how a GraphQL field is resolved.
///
/// **PRODUCTION NOTE:** A production implementation would make this async and allow
/// returning an error to handle database or network failures. For our pedagogical
/// implementation, we use synchronous resolution and Options.
pub trait Resolver {
    fn resolve_field(&self, name: &str, args: &HashMap<String, Value>) -> Option<FieldValue>;
}

pub enum FieldValue {
    Scalar(Value),
    Object(Box<dyn Resolver>),
    List(Vec<Box<dyn Resolver>>),
}

pub struct Executor;

impl Executor {
    pub fn execute(query: &Query, root: &dyn Resolver) -> Value {
        let mut result = HashMap::new();
        for selection in &query.selections {
            if let Some(val) = Self::execute_selection(selection, root) {
                result.insert(selection.name.clone(), val);
            }
        }
        Value::Object(result)
    }

    fn execute_selection(selection: &Selection, resolver: &dyn Resolver) -> Option<Value> {
        let field_val = resolver.resolve_field(&selection.name, &selection.arguments)?;

        match field_val {
            FieldValue::Scalar(v) => Some(v),
            FieldValue::Object(obj_resolver) => {
                let mut result = HashMap::new();
                for sub_sel in &selection.sub_selections {
                    if let Some(val) = Self::execute_selection(sub_sel, obj_resolver.as_ref()) {
                        result.insert(sub_sel.name.clone(), val);
                    }
                }
                Some(Value::Object(result))
            }
            FieldValue::List(resolvers) => {
                let mut list_result = Vec::new();
                for obj_resolver in resolvers {
                    let mut result = HashMap::new();
                    for sub_sel in &selection.sub_selections {
                        if let Some(val) = Self::execute_selection(sub_sel, obj_resolver.as_ref()) {
                            result.insert(sub_sel.name.clone(), val);
                        }
                    }
                    list_result.push(Value::Object(result));
                }
                Some(Value::List(list_result))
            }
        }
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// **Comparison to Canonical Crates (async-graphql, juniper):**
// - `async-graphql` uses procedural macros to derive schemas at compile time, guaranteeing
//   type safety between the Rust code and the GraphQL schema.
// - Production crates support async resolution natively, preventing thread blocking during I/O.
// - We implemented a minimal subset. We do not handle mutations, interfaces, unions, fragments,
//   variables, or introspection (a crucial feature of real GraphQL servers).
//
// **Missing Features:**
// - Schema validation (we blindly evaluate whatever AST the parser produces).
// - Async execution / concurrent field resolution.
// - Advanced scalar types (Floats, Booleans, Enums) and input object types.
// - Proper error propagation (returning an `errors` array alongside `data`).
//
// **Suggested Next Steps:**
// - Implement a `Schema` type that validates incoming queries against a known set of types before execution.
// - Benchmark the lexer and executor with Criterion (`cargo bench`) on deeply nested queries to find allocation bottlenecks.

#[cfg(test)]
mod tests {
    use super::*;

    // Mock Schema objects
    struct User {
        id: i64,
        name: String,
    }

    impl Resolver for User {
        fn resolve_field(&self, name: &str, _args: &HashMap<String, Value>) -> Option<FieldValue> {
            match name {
                "id" => Some(FieldValue::Scalar(Value::Int(self.id))),
                "name" => Some(FieldValue::Scalar(Value::String(self.name.clone()))),
                _ => None,
            }
        }
    }

    struct Root;

    impl Resolver for Root {
        fn resolve_field(&self, name: &str, args: &HashMap<String, Value>) -> Option<FieldValue> {
            match name {
                "me" => Some(FieldValue::Object(Box::new(User {
                    id: 1,
                    name: "Alice".to_string(),
                }))),
                "user" => {
                    let id = match args.get("id") {
                        Some(Value::Int(i)) => *i,
                        _ => 0,
                    };
                    Some(FieldValue::Object(Box::new(User {
                        id,
                        name: format!("User {}", id),
                    })))
                }
                "users" => {
                    let list: Vec<Box<dyn Resolver>> = vec![
                        Box::new(User {
                            id: 1,
                            name: "Alice".to_string(),
                        }),
                        Box::new(User {
                            id: 2,
                            name: "Bob".to_string(),
                        }),
                    ];
                    Some(FieldValue::List(list))
                }
                _ => None,
            }
        }
    }

    #[test]
    fn test_lexer() {
        let input = r#"
            {
                user(id: 123) {
                    name
                }
            }
        "#;
        let mut lexer = Lexer::new(input);
        assert_eq!(lexer.next_token(), Token::LeftBrace);
        assert_eq!(lexer.next_token(), Token::Name("user".to_string()));
        assert_eq!(lexer.next_token(), Token::LeftParen);
        assert_eq!(lexer.next_token(), Token::Name("id".to_string()));
        assert_eq!(lexer.next_token(), Token::Colon);
        assert_eq!(lexer.next_token(), Token::IntLiteral(123));
        assert_eq!(lexer.next_token(), Token::RightParen);
        assert_eq!(lexer.next_token(), Token::LeftBrace);
        assert_eq!(lexer.next_token(), Token::Name("name".to_string()));
        assert_eq!(lexer.next_token(), Token::RightBrace);
        assert_eq!(lexer.next_token(), Token::RightBrace);
        assert_eq!(lexer.next_token(), Token::Eof);
    }

    #[test]
    fn test_parser_and_executor() {
        let query_str = r#"
            {
                me {
                    id
                    name
                }
                user(id: 42) {
                    name
                }
                users {
                    id
                }
            }
        "#;

        let lexer = Lexer::new(query_str);
        let mut parser = Parser::new(lexer);
        let query = parser.parse_query().unwrap();

        let root = Root;
        let result = Executor::execute(&query, &root);

        if let Value::Object(map) = result {
            // Check me
            if let Some(Value::Object(me_obj)) = map.get("me") {
                assert_eq!(me_obj.get("id"), Some(&Value::Int(1)));
                assert_eq!(
                    me_obj.get("name"),
                    Some(&Value::String("Alice".to_string()))
                );
            } else {
                panic!("me not found");
            }

            // Check user
            if let Some(Value::Object(user_obj)) = map.get("user") {
                assert_eq!(
                    user_obj.get("name"),
                    Some(&Value::String("User 42".to_string()))
                );
            } else {
                panic!("user not found");
            }

            // Check users
            if let Some(Value::List(users_list)) = map.get("users") {
                assert_eq!(users_list.len(), 2);
                if let Value::Object(u1) = &users_list[0] {
                    assert_eq!(u1.get("id"), Some(&Value::Int(1)));
                } else {
                    panic!("u1 is not an object");
                }
                if let Value::Object(u2) = &users_list[1] {
                    assert_eq!(u2.get("id"), Some(&Value::Int(2)));
                } else {
                    panic!("u2 is not an object");
                }
            } else {
                panic!("users not found");
            }
        } else {
            panic!("Result is not an object");
        }
    }
}
