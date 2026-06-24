//! # GraphQL Execution Engine
//!
//! Difficulty: Hard
//! Link: <https://graphql.org/learn/execution/>
//!
//! This module demonstrates a from-scratch minimal GraphQL execution engine. It replaces crates like `async-graphql`
//! and `juniper` to show how parsing, AST generation, and recursive field resolution work under the hood.
//!
//! Real-world usage: Writing API gateways, federated graph routers, or highly specialized query engines.
//!
//! This implementation highlights defining custom recursive systems in Rust. It utilizes iterative approaches
//! in its lexer instead of tail-recursion to avoid stack overflows on invalid input. Crucially, it demonstrates
//! why returning owned data types (like `String` or `serde_json::Value`) is necessary when returning dynamically
//! dispatched trait objects (`Box<dyn Resolver>`), as zero-copy references would cause severe lifetime conflicts.

use std::collections::HashMap;

/// Represents a token produced by the Lexer.
#[derive(Debug, PartialEq, Eq)]
pub enum Token {
    Query,
    Identifier(String),
    LBrace,
    RBrace,
    EOF,
}

/// A minimal Lexer for our subset of GraphQL.
pub struct Lexer<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    /// Iterative approach to avoid stack overflow on long invalid inputs.
    pub fn next_token(&mut self) -> Token {
        // RUST INSIGHT: Avoid tail recursion (like calling `self.next_token()`) for skipping whitespace.
        // Deeply nested recursion on large inputs can blow up the stack. An iterative `loop` is safer.
        loop {
            if self.pos >= self.input.len() {
                return Token::EOF;
            }

            let c = self.input[self.pos..].chars().next().unwrap();
            let char_len = c.len_utf8();

            match c {
                ' ' | '\n' | '\t' | '\r' => {
                    self.pos += char_len;
                    continue; // Loop instead of recurse
                }
                '{' => {
                    self.pos += char_len;
                    return Token::LBrace;
                }
                '}' => {
                    self.pos += char_len;
                    return Token::RBrace;
                }
                'a'..='z' | 'A'..='Z' | '_' => {
                    let start = self.pos;
                    while self.pos < self.input.len() {
                        let ch = self.input[self.pos..].chars().next().unwrap();
                        if ch.is_ascii_alphanumeric() || ch == '_' {
                            self.pos += ch.len_utf8();
                        } else {
                            break;
                        }
                    }
                    let ident = &self.input[start..self.pos];
                    if ident == "query" {
                        return Token::Query;
                    }
                    return Token::Identifier(ident.to_string());
                }
                // Skip unrecognized characters
                _ => {
                    self.pos += char_len;
                    continue; // Loop instead of recurse
                }
            }
        }
    }
}

/// The AST representation of a parsed GraphQL query.
#[derive(Debug, PartialEq, Eq)]
pub struct Document {
    pub selection_set: Vec<Field>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Field {
    pub name: String,
    pub selection_set: Vec<Field>,
}

/// A minimal Parser to build the AST from Tokens.
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

    pub fn parse(&mut self) -> Result<Document, String> {
        // Optional 'query' keyword
        if self.current_token == Token::Query {
            self.advance();
            // Skip optional operation name
            if let Token::Identifier(_) = self.current_token {
                self.advance();
            }
        }

        let selection_set = self.parse_selection_set()?;
        Ok(Document { selection_set })
    }

    fn parse_selection_set(&mut self) -> Result<Vec<Field>, String> {
        let mut fields = Vec::new();

        if self.current_token != Token::LBrace {
            return Err("Expected '{'".to_string());
        }
        self.advance();

        while self.current_token != Token::RBrace && self.current_token != Token::EOF {
            if let Token::Identifier(ref name) = self.current_token {
                let field_name = name.clone();
                self.advance();

                let nested_selection = if self.current_token == Token::LBrace {
                    self.parse_selection_set()?
                } else {
                    Vec::new()
                };

                fields.push(Field {
                    name: field_name,
                    selection_set: nested_selection,
                });
            } else {
                return Err("Expected Identifier".to_string());
            }
        }

        if self.current_token == Token::RBrace {
            self.advance();
        } else {
            return Err("Expected '}'".to_string());
        }

        Ok(fields)
    }
}

/// The result of resolving a field in our engine.
/// We use an enum to represent JSON-like structures.
#[derive(Debug, PartialEq, Clone)]
pub enum Value {
    Null,
    String(String),
    Int(i32),
    Object(HashMap<String, Value>),
    List(Vec<Value>),
}

/// A dynamic trait object for resolving fields.
///
/// GOTCHA: We must return owned data (`Value`) and take `&self`. If we tried to return `&Value`
/// from a dynamically generated trait object, the borrow checker would fail because the trait object
/// might generate data on the fly (which it would then own and drop upon returning).
pub trait Resolver {
    fn resolve(&self, field_name: &str) -> Option<Value>;
    // In a real engine, this might return another Box<dyn Resolver> for nested objects
    fn resolve_object(&self, field_name: &str) -> Option<Box<dyn Resolver>>;
}

/// The core execution engine.
pub struct Executor;

impl Executor {
    pub fn execute(document: &Document, root_resolver: &dyn Resolver) -> Value {
        let mut result_map = HashMap::new();
        for field in &document.selection_set {
            let val = Self::execute_field(field, root_resolver);
            result_map.insert(field.name.clone(), val);
        }
        Value::Object(result_map)
    }

    fn execute_field(field: &Field, resolver: &dyn Resolver) -> Value {
        if field.selection_set.is_empty() {
            // Leaf node, resolve scalar
            resolver.resolve(&field.name).unwrap_or(Value::Null)
        } else {
            // Nested object, get nested resolver and recurse
            if let Some(nested_resolver) = resolver.resolve_object(&field.name) {
                let mut result_map = HashMap::new();
                for nested_field in &field.selection_set {
                    let val = Self::execute_field(nested_field, nested_resolver.as_ref());
                    result_map.insert(nested_field.name.clone(), val);
                }
                Value::Object(result_map)
            } else {
                Value::Null
            }
        }
    }
}

// Alternative Approaches:
// 1. AST Interpretation vs Compilation: This is an interpreter. High-performance engines compile queries
//    to a specific execution plan.
// 2. Async Resolvers: Production GraphQL in Rust (like async-graphql) uses async traits (`async fn resolve(...)`)
//    and heavily utilizes `Pin<Box<dyn Future>>` to handle concurrent I/O for nested fields.

#[cfg(test)]
mod tests {
    use super::*;

    // Mock Resolvers for testing
    struct UserResolver {
        id: i32,
        name: String,
    }

    impl Resolver for UserResolver {
        fn resolve(&self, field_name: &str) -> Option<Value> {
            match field_name {
                "id" => Some(Value::Int(self.id)),
                "name" => Some(Value::String(self.name.clone())),
                _ => None,
            }
        }
        fn resolve_object(&self, _field_name: &str) -> Option<Box<dyn Resolver>> {
            None
        }
    }

    struct RootResolver;

    impl Resolver for RootResolver {
        fn resolve(&self, field_name: &str) -> Option<Value> {
            match field_name {
                "version" => Some(Value::String("1.0".to_string())),
                _ => None,
            }
        }

        fn resolve_object(&self, field_name: &str) -> Option<Box<dyn Resolver>> {
            match field_name {
                "me" => Some(Box::new(UserResolver {
                    id: 42,
                    name: "Alice".to_string(),
                })),
                _ => None,
            }
        }
    }

    #[test]
    fn test_happy_path_lexer() {
        let mut lexer = Lexer::new("{ me { id name } }");
        assert_eq!(lexer.next_token(), Token::LBrace);
        assert_eq!(lexer.next_token(), Token::Identifier("me".to_string()));
        assert_eq!(lexer.next_token(), Token::LBrace);
        assert_eq!(lexer.next_token(), Token::Identifier("id".to_string()));
        assert_eq!(lexer.next_token(), Token::Identifier("name".to_string()));
        assert_eq!(lexer.next_token(), Token::RBrace);
        assert_eq!(lexer.next_token(), Token::RBrace);
        assert_eq!(lexer.next_token(), Token::EOF);
    }

    #[test]
    fn test_edge_case_parser_nested() {
        let lexer = Lexer::new("query { me { id } version }");
        let mut parser = Parser::new(lexer);
        let doc = parser.parse().unwrap();

        assert_eq!(doc.selection_set.len(), 2);
        assert_eq!(doc.selection_set[0].name, "me");
        assert_eq!(doc.selection_set[0].selection_set.len(), 1);
        assert_eq!(doc.selection_set[0].selection_set[0].name, "id");
        assert_eq!(doc.selection_set[1].name, "version");
        assert_eq!(doc.selection_set[1].selection_set.len(), 0);
    }

    #[test]
    fn test_stress_boundary_executor_missing_fields() {
        let lexer = Lexer::new("{ me { id missing_field } version doesntexist }");
        let mut parser = Parser::new(lexer);
        let doc = parser.parse().unwrap();

        let root = RootResolver;
        let result = Executor::execute(&doc, &root);

        if let Value::Object(map) = result {
            // 'version' resolves correctly
            assert_eq!(map.get("version"), Some(&Value::String("1.0".to_string())));
            // 'doesntexist' resolves to Null
            assert_eq!(map.get("doesntexist"), Some(&Value::Null));

            if let Some(Value::Object(me_map)) = map.get("me") {
                assert_eq!(me_map.get("id"), Some(&Value::Int(42)));
                // Nested missing field resolves to Null
                assert_eq!(me_map.get("missing_field"), Some(&Value::Null));
            } else {
                panic!("'me' should be an object");
            }
        } else {
            panic!("Result should be an object");
        }
    }
}
