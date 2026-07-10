//! GraphQL Execution Engine
//!
//! What this implements and what crate(s) it replaces:
//! This implements a minimal, no-dependency GraphQL execution engine. It includes a custom lexer,
//! a recursive descent parser to construct a syntax tree, and a trait-based execution system.
//! It replaces the core functionality of crates like `async-graphql` and `juniper` by demonstrating
//! how to parse and resolve queries against a defined schema without relying on complex macros or external libraries.
//!
//! Real-world systems that use this:
//! GraphQL engines are widely used in modern APIs to allow clients to specify exactly the data they need.
//! Systems like GitHub's API, Facebook's mobile data endpoints, and Apollo Server implement similar architectures.
//!
//! Why build it yourself?
//! Building a GraphQL engine from scratch teaches you about compiler theory (lexing and parsing),
//! recursive data structures, and how to use traits to build dynamic, resolvable graphs in Rust.
//!
//! # Architecture
//!
//! The engine consists of three main stages:
//! 1. Lexer: Tokenizes the input GraphQL query string into discrete tokens (identifiers, punctuation, strings, etc.).
//! 2. Parser: Uses recursive descent to build an Abstract Syntax Tree (AST) representing the query.
//! 3. Execution: Resolves the AST against a provided schema using trait-based resolution.
//!
//! ```text
//! [ Query String ] -> (Lexer) -> [ Tokens ] -> (Parser) -> [ AST (Query/Fields) ] -> (Executor) -> [ JSON Result ]
//!                                                                                        ^
//!                                                                                        |
//!                                                                                   [ Schema ]
//! ```
//!
//! ## Invariants
//! - Lexer must not panic on invalid characters but gracefully return an error or skip them.
//! - Parser must handle nested fields without infinite recursion.
//! - Executor must properly resolve nested objects by deferring to their trait implementations.
//!
//! ## Complexity
//! - Lexing: O(N) where N is the length of the query string.
//! - Parsing: O(T) where T is the number of tokens.
//! - Execution: O(F) where F is the number of fields requested, assuming resolvers are O(1).
//!
//! # Benchmarking Note
//! To benchmark, use `criterion` to measure the latency of lexing, parsing, and execution separately.
//! The lexer and parser can be optimized to minimize allocations using `&str` references (zero-copy parsing),
//! though this implementation uses `String` for simplicity.
//!
//! # Alternative Approaches
//! A production GraphQL engine would typically use:
//! - A formal grammar and parser generator (e.g., LALRPOP or nom).
//! - Extensive validation against the schema before execution.
//! - Asynchronous resolvers for fetching data from databases or APIs.
//! - A macro-based approach (like `async-graphql`) to derive schema definitions from Rust types.
//!
//! # Missing vs Production
//! - No support for Mutations or Subscriptions.
//! - No support for Variables, Fragments, or Directives.
//! - No schema introspection.
//! - Synchronous execution only.
//! - Allocates strings instead of using zero-copy references for the AST.

use std::collections::HashMap;

// --- LEXER ---

#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    Ident(String),
    String(String),
    Number(f64),
    LBrace,
    RBrace,
    LParen,
    RParen,
    Colon,
    Comma,
    EOF,
}

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

    fn consume_char(&mut self) -> Option<char> {
        let c = self.peek_char()?;
        self.pos += c.len_utf8();
        Some(c)
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            if let Some(c) = self.peek_char() {
                if c.is_whitespace() || c == ',' {
                    self.consume_char();
                } else if c == '#' {
                    while let Some(ch) = self.consume_char() {
                        if ch == '\n' {
                            break;
                        }
                    }
                } else {
                    break;
                }
            } else {
                break;
            }
        }
    }

    pub fn next_token(&mut self) -> Token {
        self.skip_whitespace_and_comments();

        if let Some(c) = self.peek_char() {
            match c {
                '{' => {
                    self.consume_char();
                    Token::LBrace
                }
                '}' => {
                    self.consume_char();
                    Token::RBrace
                }
                '(' => {
                    self.consume_char();
                    Token::LParen
                }
                ')' => {
                    self.consume_char();
                    Token::RParen
                }
                ':' => {
                    self.consume_char();
                    Token::Colon
                }
                '"' => self.read_string(),
                _ if c.is_alphabetic() || c == '_' => self.read_ident(),
                _ if c.is_ascii_digit() || c == '-' => self.read_number(),
                // GOTCHA: Unrecognized characters shouldn't trigger tail recursion,
                // which can cause stack overflows on malformed input.
                // We consume it and loop back in `next_token` implicitly by callers or iterative loops if needed.
                // For this minimal implementation, we'll return EOF on invalid char to stop.
                _ => {
                    self.consume_char(); // Skip invalid
                    Token::EOF // Or Error token
                }
            }
        } else {
            Token::EOF
        }
    }

    fn read_ident(&mut self) -> Token {
        let mut ident = String::new();
        while let Some(c) = self.peek_char() {
            if c.is_alphanumeric() || c == '_' {
                ident.push(c);
                self.consume_char();
            } else {
                break;
            }
        }
        Token::Ident(ident)
    }

    fn read_string(&mut self) -> Token {
        self.consume_char(); // Consume opening quote
        let mut string = String::new();
        while let Some(c) = self.peek_char() {
            if c == '"' {
                self.consume_char(); // Consume closing quote
                break;
            } else {
                string.push(c);
                self.consume_char();
            }
        }
        Token::String(string)
    }

    fn read_number(&mut self) -> Token {
        let mut num_str = String::new();
        while let Some(c) = self.peek_char() {
            if c.is_ascii_digit() || c == '.' || c == '-' {
                num_str.push(c);
                self.consume_char();
            } else {
                break;
            }
        }
        Token::Number(num_str.parse().unwrap_or(0.0))
    }
}

// --- AST ---

#[derive(Debug, PartialEq, Clone)]
pub enum ArgumentValue {
    String(String),
    Int(i64),
    Float(f64),
    Boolean(bool),
}

#[derive(Debug, PartialEq, Clone)]
pub struct Field {
    pub name: String,
    pub alias: Option<String>,
    pub arguments: HashMap<String, ArgumentValue>,
    pub selection_set: Vec<Field>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Query {
    pub operation_name: Option<String>,
    pub selection_set: Vec<Field>,
}

// --- PARSER ---

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

    fn expect(&mut self, expected: Token) -> Result<(), String> {
        if self.current_token == expected {
            self.advance();
            Ok(())
        } else {
            Err(format!(
                "Expected {:?}, got {:?}",
                expected, self.current_token
            ))
        }
    }

    pub fn parse_query(&mut self) -> Result<Query, String> {
        let mut operation_name = None;

        if let Token::Ident(ref name) = self.current_token {
            if name == "query" {
                self.advance();
                if let Token::Ident(ref op_name) = self.current_token {
                    operation_name = Some(op_name.clone());
                    self.advance();
                }
            }
        }

        let selection_set = self.parse_selection_set()?;
        Ok(Query {
            operation_name,
            selection_set,
        })
    }

    fn parse_selection_set(&mut self) -> Result<Vec<Field>, String> {
        self.expect(Token::LBrace)?;
        let mut fields = Vec::new();

        while self.current_token != Token::RBrace && self.current_token != Token::EOF {
            fields.push(self.parse_field()?);
        }

        self.expect(Token::RBrace)?;
        Ok(fields)
    }

    fn parse_field(&mut self) -> Result<Field, String> {
        let mut alias = None;
        let mut name = String::new();

        if let Token::Ident(ref ident) = self.current_token {
            let first_ident = ident.clone();
            self.advance();

            if self.current_token == Token::Colon {
                self.advance(); // consume colon
                alias = Some(first_ident);
                if let Token::Ident(ref actual_name) = self.current_token {
                    name = actual_name.clone();
                    self.advance();
                } else {
                    return Err("Expected field name after alias".to_string());
                }
            } else {
                name = first_ident;
            }
        } else {
            return Err("Expected field name".to_string());
        }

        let arguments = if self.current_token == Token::LParen {
            self.parse_arguments()?
        } else {
            HashMap::new()
        };

        let selection_set = if self.current_token == Token::LBrace {
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

    fn parse_arguments(&mut self) -> Result<HashMap<String, ArgumentValue>, String> {
        self.expect(Token::LParen)?;
        let mut arguments = HashMap::new();

        while self.current_token != Token::RParen && self.current_token != Token::EOF {
            if let Token::Ident(ref name) = self.current_token {
                let arg_name = name.clone();
                self.advance();
                self.expect(Token::Colon)?;

                let value = match self.current_token.clone() {
                    Token::String(s) => {
                        self.advance();
                        ArgumentValue::String(s)
                    }
                    Token::Number(n) => {
                        self.advance();
                        if n.fract() == 0.0 {
                            ArgumentValue::Int(n as i64)
                        } else {
                            ArgumentValue::Float(n)
                        }
                    }
                    Token::Ident(ref s) if s == "true" => {
                        self.advance();
                        ArgumentValue::Boolean(true)
                    }
                    Token::Ident(ref s) if s == "false" => {
                        self.advance();
                        ArgumentValue::Boolean(false)
                    }
                    _ => return Err("Invalid argument value".to_string()),
                };

                arguments.insert(arg_name, value);
            } else {
                return Err("Expected argument name".to_string());
            }
        }

        self.expect(Token::RParen)?;
        Ok(arguments)
    }
}

// --- EXECUTION ENGINE ---

// RUST INSIGHT: We use an enum to represent JSON-like dynamic responses.
// In Rust, we need a unified type to represent diverse schema responses dynamically.
#[derive(Debug, PartialEq, Clone)]
pub enum GqlValue {
    Null,
    String(String),
    Int(i64),
    Float(f64),
    Boolean(bool),
    List(Vec<GqlValue>),
    Object(HashMap<String, GqlValue>),
}

pub trait Resolver {
    fn resolve_field(&self, name: &str, args: &HashMap<String, ArgumentValue>) -> GqlValue;
}

pub fn execute_query(query: &Query, root_resolver: &dyn Resolver) -> GqlValue {
    execute_selection_set(&query.selection_set, root_resolver)
}

fn execute_selection_set(selection_set: &[Field], resolver: &dyn Resolver) -> GqlValue {
    let mut result_map = HashMap::new();

    for field in selection_set {
        let field_name = &field.name;
        let response_key = field.alias.as_ref().unwrap_or(field_name).clone();

        let resolved_val = resolver.resolve_field(field_name, &field.arguments);

        // If it's an object and we have sub-selections, we should theoretically recurse.
        // For simplicity in this minimal engine, we expect the resolver to handle sub-selections
        // or return primitive/flat object values.
        // A production engine would pass the selection_set to the resolver, or have the resolver return
        // another `&dyn Resolver` trait object for deep resolution.

        // Let's implement a rudimentary sub-selection evaluation if the returned value is an object
        // and we have sub-fields.
        let final_val = match resolved_val {
            GqlValue::Object(mut map) if !field.selection_set.is_empty() => {
                // Filter the map down to only requested fields
                let mut filtered_map = HashMap::new();
                for sub_field in &field.selection_set {
                    let sub_name = &sub_field.name;
                    let sub_key = sub_field.alias.as_ref().unwrap_or(sub_name).clone();
                    if let Some(val) = map.remove(sub_name) {
                         filtered_map.insert(sub_key, val);
                    } else {
                        filtered_map.insert(sub_key, GqlValue::Null);
                    }
                }
                GqlValue::Object(filtered_map)
            }
             _ => resolved_val
        };

        result_map.insert(response_key, final_val);
    }

    GqlValue::Object(result_map)
}

// --- TESTS ---

#[cfg(test)]
mod tests {
    use super::*;

    struct MockRootResolver;

    impl Resolver for MockRootResolver {
        fn resolve_field(&self, name: &str, args: &HashMap<String, ArgumentValue>) -> GqlValue {
            match name {
                "hello" => GqlValue::String("world".to_string()),
                "user" => {
                    let mut user = HashMap::new();
                    let id = args.get("id").and_then(|v| {
                        if let ArgumentValue::Int(i) = v { Some(*i) } else { None }
                    }).unwrap_or(1);

                    user.insert("id".to_string(), GqlValue::Int(id));
                    user.insert("name".to_string(), GqlValue::String("Alice".to_string()));
                    GqlValue::Object(user)
                }
                _ => GqlValue::Null,
            }
        }
    }

    #[test]
    fn test_lexer() {
        let query = "{ user(id: 42) { name } }";
        let mut lexer = Lexer::new(query);

        assert_eq!(lexer.next_token(), Token::LBrace);
        assert_eq!(lexer.next_token(), Token::Ident("user".to_string()));
        assert_eq!(lexer.next_token(), Token::LParen);
        assert_eq!(lexer.next_token(), Token::Ident("id".to_string()));
        assert_eq!(lexer.next_token(), Token::Colon);
        assert_eq!(lexer.next_token(), Token::Number(42.0));
        assert_eq!(lexer.next_token(), Token::RParen);
        assert_eq!(lexer.next_token(), Token::LBrace);
        assert_eq!(lexer.next_token(), Token::Ident("name".to_string()));
        assert_eq!(lexer.next_token(), Token::RBrace);
        assert_eq!(lexer.next_token(), Token::RBrace);
        assert_eq!(lexer.next_token(), Token::EOF);
    }

    #[test]
    fn test_parser() {
        let query_str = "query getUser { my_user: user(id: 42) { name } }";
        let lexer = Lexer::new(query_str);
        let mut parser = Parser::new(lexer);

        let query = parser.parse_query().unwrap();
        assert_eq!(query.operation_name.unwrap(), "getUser");
        assert_eq!(query.selection_set.len(), 1);

        let field = &query.selection_set[0];
        assert_eq!(field.name, "user");
        assert_eq!(field.alias.as_ref().unwrap(), "my_user");

        if let ArgumentValue::Int(val) = field.arguments.get("id").unwrap() {
            assert_eq!(*val, 42);
        } else {
            panic!("Expected Int argument");
        }

        assert_eq!(field.selection_set.len(), 1);
        assert_eq!(field.selection_set[0].name, "name");
    }

    #[test]
    fn test_execution() {
        let query_str = "{ hello my_user: user(id: 99) { name } }";
        let lexer = Lexer::new(query_str);
        let mut parser = Parser::new(lexer);
        let query = parser.parse_query().unwrap();

        let root = MockRootResolver;
        let result = execute_query(&query, &root);

        if let GqlValue::Object(map) = result {
            assert_eq!(map.get("hello").unwrap(), &GqlValue::String("world".to_string()));

            if let GqlValue::Object(user_map) = map.get("my_user").unwrap() {
                // Notice that 'id' is NOT in the filtered user map because it wasn't selected!
                assert!(user_map.get("id").is_none());
                assert_eq!(user_map.get("name").unwrap(), &GqlValue::String("Alice".to_string()));
            } else {
                panic!("Expected Object for my_user");
            }
        } else {
            panic!("Expected root Object");
        }
    }
}
