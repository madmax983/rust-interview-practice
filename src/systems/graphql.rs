//! GraphQL Execution Engine
//!
//! # What this implements and what crate(s) it replaces
//! This module provides a minimal lexer, parser, and trait-based execution engine for GraphQL.
//! It replaces the fundamental parts of crates like `async-graphql` and `juniper`.
//!
//! # Real-world systems that use this
//! Real-world API gateways, federated graphs (like Apollo Federation), and standalone GraphQL endpoints
//! use execution engines like this to parse raw queries and resolve data across multiple microservices or databases.
//!
//! # Why build it yourself?
//! By implementing a GraphQL engine from scratch, you gain deep mastery over AST parsing,
//! trait-based recursive resolution, and iterative lexical analysis. It exposes how complex graphs
//! are safely evaluated without stack overflows.
//!
//! # Architecture
//!
//! ```text
//! [Query String] -> [Lexer] -> [Tokens] -> [Parser] -> [Document AST] -> [Executor] -> [JSON String]
//! ```
//!
//! **Invariants:**
//! 1. Lexer does not use tail recursion; it iteratively consumes the stream to prevent stack overflow.
//! 2. Executor traverses the AST and relies on dynamic trait objects (`Box<dyn Resolver>`) for flexible data bindings.
//! 3. Evaluated output uses owned `String` instead of zero-copy slices to avoid lifetime complexity during dynamic resolution.
//!
//! **Complexity:**
//! - **Lexing:** O(N) where N is the length of the query.
//! - **Parsing:** O(T) where T is the number of tokens.
//! - **Execution:** O(V + E) where V is vertices (fields resolved) and E is edges, scaled by data fetched.
//!
//! **Design Decisions and Tradeoffs:**
//! We use an iterative lexer to strictly avoid deep stack traces from tail recursion on malicious inputs.
//! For execution, we use `String` values for outputs, which adds a slight allocation overhead compared to
//! zero-copy slices but greatly simplifies the borrowing rules for a dynamically sized trait object graph.
//!
//! # Footer
//!
//! **Comparison to Canonical Crates:**
//! Production engines like `async-graphql` include sophisticated schema validation, introspection, async/await resolution,
//! directives, and union types. This engine focuses solely on the core synchronous execution and resolution path.
//!
//! **What's Missing:**
//! Validation against a formal Schema (we skip the Validation phase and execute whatever is requested if resolvers permit),
//! Input variables, Fragments, Async resolvers, and robust Error handling paths (errors just panic or return "null").
//!
//! **Next Steps:**
//! Implement an `async` version of the `Resolver` trait using `BoxFuture` to allow fetching data from databases non-blockingly.
//!
//! # Benchmarking Note
//! You could benchmark the lexer and parser stages using `criterion` by feeding them deeply nested queries
//! to measure throughput and allocation rates.

use std::collections::HashMap;
use std::fmt::Write;

#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    Name(String),
    StringLiteral(String),
    Int(i64),
    Punctuation(char),
    Eof,
}

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

    fn peek(&self) -> Option<u8> {
        self.input.get(self.pos).copied()
    }

    fn advance(&mut self) {
        self.pos += 1;
    }

    // RUST INSIGHT:
    // We use an iterative `loop` over the input stream to skip whitespace instead of tail recursion.
    // This strictly prevents stack overflows on inputs containing megabytes of spaces.
    pub fn next_token(&mut self) -> Token {
        loop {
            match self.peek() {
                Some(b' ') | Some(b'\n') | Some(b'\r') | Some(b'\t') | Some(b',') => {
                    self.advance();
                }
                Some(b'#') => {
                    self.advance();
                    while let Some(c) = self.peek() {
                        if c == b'\n' || c == b'\r' {
                            break;
                        }
                        self.advance();
                    }
                }
                Some(b'{') | Some(b'}') | Some(b'(') | Some(b')') | Some(b':') | Some(b'@') => {
                    let c = self.peek().unwrap() as char;
                    self.advance();
                    return Token::Punctuation(c);
                }
                Some(c) if c.is_ascii_alphabetic() || c == b'_' => {
                    let mut name = String::new();
                    while let Some(c) = self.peek() {
                        if c.is_ascii_alphanumeric() || c == b'_' {
                            name.push(c as char);
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    return Token::Name(name);
                }
                Some(b'"') => {
                    self.advance();
                    let mut s = String::new();
                    while let Some(c) = self.peek() {
                        if c == b'"' {
                            self.advance();
                            break;
                        }
                        s.push(c as char);
                        self.advance();
                    }
                    return Token::StringLiteral(s);
                }
                Some(c) if c.is_ascii_digit() || c == b'-' => {
                    let mut num = String::new();
                    if c == b'-' {
                        num.push('-');
                        self.advance();
                    }
                    while let Some(c) = self.peek() {
                        if c.is_ascii_digit() {
                            num.push(c as char);
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    return Token::Int(num.parse().unwrap_or(0));
                }
                Some(_) => {
                    // Skip unrecognized characters iteratively to avoid stack overflows.
                    self.advance();
                }
                None => return Token::Eof,
            }
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Field {
    pub name: String,
    pub alias: Option<String>,
    pub arguments: HashMap<String, Value>,
    pub selection_set: Vec<Selection>,
}

#[derive(Debug, PartialEq, Clone)]
pub enum Selection {
    Field(Field),
}

#[derive(Debug, PartialEq, Clone)]
pub struct Operation {
    pub operation_type: String, // "query", "mutation"
    pub name: Option<String>,
    pub selection_set: Vec<Selection>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Document {
    pub operations: Vec<Operation>,
}

#[derive(Debug, PartialEq, Clone)]
pub enum Value {
    Int(i64),
    String(String),
}

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

    fn expect_punctuation(&mut self, p: char) -> bool {
        if let Token::Punctuation(c) = self.current_token {
            if c == p {
                self.advance();
                return true;
            }
        }
        false
    }

    pub fn parse_document(&mut self) -> Document {
        let mut operations = Vec::new();
        while self.current_token != Token::Eof {
            if let Some(op) = self.parse_operation() {
                operations.push(op);
            } else {
                self.advance(); // Skip invalid tokens at top level
            }
        }
        Document { operations }
    }

    fn parse_operation(&mut self) -> Option<Operation> {
        let mut operation_type = "query".to_string();
        let mut name = None;

        if let Token::Name(ref op) = self.current_token {
            if op == "query" || op == "mutation" {
                operation_type = op.clone();
                self.advance();
                if let Token::Name(ref op_name) = self.current_token {
                    name = Some(op_name.clone());
                    self.advance();
                }
            }
        }

        if let Token::Punctuation('{') = self.current_token {
            let selection_set = self.parse_selection_set();
            Some(Operation {
                operation_type,
                name,
                selection_set,
            })
        } else {
            None
        }
    }

    fn parse_selection_set(&mut self) -> Vec<Selection> {
        let mut selections = Vec::new();
        if !self.expect_punctuation('{') {
            return selections;
        }

        while self.current_token != Token::Punctuation('}') && self.current_token != Token::Eof {
            if let Some(field) = self.parse_field() {
                selections.push(Selection::Field(field));
            } else {
                self.advance();
            }
        }
        self.expect_punctuation('}');
        selections
    }

    fn parse_field(&mut self) -> Option<Field> {
        let mut alias = None;
        let mut name = String::new();

        if let Token::Name(ref n) = self.current_token {
            name = n.clone();
            self.advance();
        } else {
            return None;
        }

        if self.expect_punctuation(':') {
            alias = Some(name);
            if let Token::Name(ref n) = self.current_token {
                name = n.clone();
                self.advance();
            } else {
                return None;
            }
        }

        let mut arguments = HashMap::new();
        if self.expect_punctuation('(') {
            while self.current_token != Token::Punctuation(')') && self.current_token != Token::Eof {
                if let Token::Name(ref arg_name) = self.current_token {
                    let arg_key = arg_name.clone();
                    self.advance();
                    if self.expect_punctuation(':') {
                        let value = self.parse_value();
                        if let Some(v) = value {
                            arguments.insert(arg_key, v);
                        }
                    }
                } else {
                    self.advance();
                }
            }
            self.expect_punctuation(')');
        }

        let mut selection_set = Vec::new();
        if let Token::Punctuation('{') = self.current_token {
            selection_set = self.parse_selection_set();
        }

        Some(Field {
            name,
            alias,
            arguments,
            selection_set,
        })
    }

    fn parse_value(&mut self) -> Option<Value> {
        match &self.current_token {
            Token::Int(i) => {
                let val = Value::Int(*i);
                self.advance();
                Some(val)
            }
            Token::StringLiteral(s) => {
                let val = Value::String(s.clone());
                self.advance();
                Some(val)
            }
            _ => {
                self.advance();
                None
            }
        }
    }
}

pub trait Resolver {
    // PRODUCTION NOTE:
    // A real production crate (e.g., `async-graphql`) would use a generic async `resolve` method,
    // usually passing down a context object containing loaders and database connections.
    // It would also return types that can be serialized directly instead of manually emitting JSON.
    fn resolve_field(&self, name: &str, args: &HashMap<String, Value>) -> Option<ResolverOutput>;
}

pub enum ResolverOutput {
    Object(Box<dyn Resolver>),
    List(Vec<Box<dyn Resolver>>),
    String(String),
    Int(i64),
    Null,
}

pub struct Executor;

impl Executor {
    pub fn execute(document: &Document, root_resolver: &dyn Resolver) -> String {
        let mut out = String::new();
        out.push_str("{\"data\":{");

        if let Some(op) = document.operations.first() {
            let mut first = true;
            for selection in &op.selection_set {
                if !first {
                    out.push(',');
                }
                first = false;
                match selection {
                    Selection::Field(field) => {
                        Self::execute_field(field, root_resolver, &mut out);
                    }
                }
            }
        }

        out.push_str("}}");
        out
    }

    fn execute_field(field: &Field, resolver: &dyn Resolver, out: &mut String) {
        let response_name = field.alias.as_ref().unwrap_or(&field.name);

        // RUST INSIGHT:
        // We use the `write!` macro directly onto the mutable output `String` buffer.
        // This avoids creating intermediate strings and saves allocation overhead during JSON emission.
        write!(out, "\"{}\":", response_name).unwrap();

        match resolver.resolve_field(&field.name, &field.arguments) {
            Some(ResolverOutput::Object(obj_resolver)) => {
                out.push('{');
                let mut first = true;
                for selection in &field.selection_set {
                    if !first {
                        out.push(',');
                    }
                    first = false;
                    match selection {
                        Selection::Field(sub_field) => {
                            Self::execute_field(sub_field, obj_resolver.as_ref(), out);
                        }
                    }
                }
                out.push('}');
            }
            Some(ResolverOutput::List(list)) => {
                out.push('[');
                let mut first_list = true;
                for item in list {
                    if !first_list {
                        out.push(',');
                    }
                    first_list = false;
                    out.push('{');
                    let mut first_field = true;
                    for selection in &field.selection_set {
                        if !first_field {
                            out.push(',');
                        }
                        first_field = false;
                        match selection {
                            Selection::Field(sub_field) => {
                                Self::execute_field(sub_field, item.as_ref(), out);
                            }
                        }
                    }
                    out.push('}');
                }
                out.push(']');
            }
            Some(ResolverOutput::String(s)) => {
                write!(out, "\"{}\"", s).unwrap();
            }
            Some(ResolverOutput::Int(i)) => {
                write!(out, "{}", i).unwrap();
            }
            Some(ResolverOutput::Null) | None => {
                out.push_str("null");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct UserResolver {
        id: i64,
        name: String,
    }

    impl Resolver for UserResolver {
        fn resolve_field(&self, name: &str, _args: &HashMap<String, Value>) -> Option<ResolverOutput> {
            match name {
                "id" => Some(ResolverOutput::Int(self.id)),
                "name" => Some(ResolverOutput::String(self.name.clone())),
                _ => None,
            }
        }
    }

    struct RootResolver;

    impl Resolver for RootResolver {
        fn resolve_field(&self, name: &str, args: &HashMap<String, Value>) -> Option<ResolverOutput> {
            match name {
                "user" => {
                    let id = match args.get("id") {
                        Some(Value::Int(i)) => *i,
                        _ => 1,
                    };
                    Some(ResolverOutput::Object(Box::new(UserResolver {
                        id,
                        name: format!("User{}", id),
                    })))
                }
                "users" => {
                    let list: Vec<Box<dyn Resolver>> = vec![
                        Box::new(UserResolver { id: 1, name: "Alice".to_string() }),
                        Box::new(UserResolver { id: 2, name: "Bob".to_string() }),
                    ];
                    Some(ResolverOutput::List(list))
                }
                _ => None,
            }
        }
    }

    #[test]
    fn test_lexer() {
        let input = "query { user(id: 42) { name } }";
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
                me: user(id: 99) {
                    id
                    name
                }
            }
        "#;
        let lexer = Lexer::new(input);
        let mut parser = Parser::new(lexer);
        let doc = parser.parse_document();

        let root = RootResolver;
        let result = Executor::execute(&doc, &root);

        assert_eq!(result, r#"{"data":{"me":{"id":99,"name":"User99"}}}"#);
    }

    #[test]
    fn test_list_execution() {
        let input = "{ users { name } }";
        let lexer = Lexer::new(input);
        let mut parser = Parser::new(lexer);
        let doc = parser.parse_document();

        let root = RootResolver;
        let result = Executor::execute(&doc, &root);

        assert_eq!(result, r#"{"data":{"users":[{"name":"Alice"},{"name":"Bob"}]}}"#);
    }
}
