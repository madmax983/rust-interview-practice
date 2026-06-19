//! # GraphQL Execution Engine
//!
//! Implements a minimal GraphQL lexer, parser, and trait-based recursive execution engine.
//!
//! **Replaces Crates:** (Real-world equivalents: `async-graphql`, `juniper`)
//!
//! **Real-world Usage:**
//! - API gateways and backend-for-frontend (BFF) layers.
//! - Data aggregation services (fetching from DB, gRPC, and REST simultaneously).
//!
//! **Why build it yourself?**
//! Implementing a GraphQL engine reveals the complexity of trait-based recursive resolution.
//! You'll learn how to build lexers that don't stack overflow on invalid input by using
//! iterative approaches rather than tail recursion. You'll also confront Rust's lifetime
//! challenges when returning dynamically evaluated tree structures, pushing you toward
//! owned data types rather than complex borrowing.

use std::collections::HashMap;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Lexer -> Parser -> Executor
//
// 1. Lexer: Tokenizes the input query string iteratively.
// 2. Parser: Builds an Abstract Syntax Tree (AST) representing the query.
// 3. Executor: Recursively resolves the AST against a provided schema (Root Resolver),
//    returning a structured JSON-like result.
//
// Invariants:
// - Lexer uses an iterative loop to avoid stack overflow on long, invalid inputs.
// - Executor returns owned values (String, JSON objects) to avoid lifetime hell with trait objects.

#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    Ident(String),
    LBrace,
    RBrace,
    Colon,
    StringLit(String),
    IntLit(i64),
    Eof,
}

pub struct Lexer<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    pub fn next_token(&mut self) -> Token {
        // GOTCHA: Use an iterative loop instead of tail recursion (like calling self.next_token()
        // at the end of the whitespace block) to prevent stack overflow on long sequences of spaces.
        loop {
            if self.pos >= self.input.len() {
                return Token::Eof;
            }

            let ch = self.input[self.pos..].chars().next().unwrap();
            let ch_len = ch.len_utf8();

            if ch.is_whitespace() {
                self.pos += ch_len;
                continue;
            }

            match ch {
                '{' => {
                    self.pos += ch_len;
                    return Token::LBrace;
                }
                '}' => {
                    self.pos += ch_len;
                    return Token::RBrace;
                }
                ':' => {
                    self.pos += ch_len;
                    return Token::Colon;
                }
                '"' => {
                    self.pos += ch_len;
                    let start = self.pos;
                    while self.pos < self.input.len() {
                        let c = self.input[self.pos..].chars().next().unwrap();
                        if c == '"' {
                            break;
                        }
                        self.pos += c.len_utf8();
                    }
                    let s = self.input[start..self.pos].to_string();
                    if self.pos < self.input.len() {
                        self.pos += 1; // Consume closing quote
                    }
                    return Token::StringLit(s);
                }
                _ if ch.is_alphabetic() || ch == '_' => {
                    let start = self.pos;
                    while self.pos < self.input.len() {
                        let c = self.input[self.pos..].chars().next().unwrap();
                        if !(c.is_alphanumeric() || c == '_') {
                            break;
                        }
                        self.pos += c.len_utf8();
                    }
                    return Token::Ident(self.input[start..self.pos].to_string());
                }
                _ if ch.is_ascii_digit() => {
                    let start = self.pos;
                    while self.pos < self.input.len() {
                        let c = self.input[self.pos..].chars().next().unwrap();
                        if !c.is_ascii_digit() {
                            break;
                        }
                        self.pos += c.len_utf8();
                    }
                    let val: i64 = self.input[start..self.pos].parse().unwrap();
                    return Token::IntLit(val);
                }
                _ => {
                    // Unknown character, skip it. In a real engine, this would be an error.
                    self.pos += ch_len;
                    continue;
                }
            }
        }
    }
}

// AST Nodes
#[derive(Debug, PartialEq, Clone)]
pub struct FieldNode {
    pub name: String,
    pub alias: Option<String>,
    pub selections: Vec<FieldNode>,
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
                break;
            }
            tokens.push(tok);
        }
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) {
        self.pos += 1;
    }

    pub fn parse_query(&mut self) -> Vec<FieldNode> {
        if let Some(Token::LBrace) = self.peek() {
            self.parse_selection_set()
        } else {
            Vec::new() // simplified, queries often start with '{'
        }
    }

    fn parse_selection_set(&mut self) -> Vec<FieldNode> {
        let mut selections = Vec::new();
        if let Some(Token::LBrace) = self.peek() {
            self.advance(); // consume '{'
            while let Some(tok) = self.peek() {
                if tok == &Token::RBrace {
                    self.advance(); // consume '}'
                    break;
                }
                if let Some(field) = self.parse_field() {
                    selections.push(field);
                } else {
                    break; // Error recovery
                }
            }
        }
        selections
    }

    fn parse_field(&mut self) -> Option<FieldNode> {
        let mut name = String::new();
        let mut alias = None;

        if let Some(Token::Ident(id)) = self.peek() {
            name = id.clone();
            self.advance();
        } else {
            return None;
        }

        // Check for alias
        if let Some(Token::Colon) = self.peek() {
            self.advance(); // consume ':'
            alias = Some(name); // The previous ident was the alias
            if let Some(Token::Ident(id)) = self.peek() {
                name = id.clone(); // The new ident is the actual name
                self.advance();
            } else {
                return None;
            }
        }

        let mut selections = Vec::new();
        if let Some(Token::LBrace) = self.peek() {
            selections = self.parse_selection_set();
        }

        Some(FieldNode {
            name,
            alias,
            selections,
        })
    }
}

// Execution Value
#[derive(Debug, PartialEq, Clone)]
pub enum GqlValue {
    Null,
    Int(i64),
    String(String),
    Object(HashMap<String, GqlValue>),
    List(Vec<GqlValue>),
}

// Trait for Resolvers
// RUST INSIGHT: Returning an owned GqlValue instead of a borrowed reference ensures
// the dynamically generated trait objects don't run into lifetime issues when constructing the response tree.
pub trait Resolver {
    fn resolve(&self, field: &str) -> Option<Box<dyn Resolver>>;
    fn value(&self) -> GqlValue {
        GqlValue::Null
    }
}

// Execution Engine
pub struct Executor;

impl Executor {
    pub fn execute(root: &dyn Resolver, query: &[FieldNode]) -> GqlValue {
        let mut result_map = HashMap::new();

        for field in query {
            let key = field.alias.as_ref().unwrap_or(&field.name).clone();

            if let Some(resolved) = root.resolve(&field.name) {
                if field.selections.is_empty() {
                    result_map.insert(key, resolved.value());
                } else {
                    result_map.insert(key, Self::execute(resolved.as_ref(), &field.selections));
                }
            } else {
                result_map.insert(key, GqlValue::Null);
            }
        }

        GqlValue::Object(result_map)
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `async-graphql` or `juniper`: These provide full macro-based schema generation, async resolvers,
//   variables, fragments, mutations, and directives.
//
// Missing vs. Production:
// - Type checking against a schema during parsing.
// - Asynchronous resolver support (`async fn`).
// - Variables and Fragments.
// - Arguments on fields (e.g., `user(id: 1) { name }`).
//
// Next Steps:
// 1. Add field arguments support in Lexer, Parser, and Resolver traits.
// 2. Add Async traits using `async-trait` or native async traits.
// 3. Implement schema validation.
//
// Benchmarking Note:
// Use `std::time::Instant::now()` to measure the end-to-end latency of `Lexer -> Parser -> Executor`
// on complex, deeply nested queries. Alternatively, use `criterion` to benchmark each phase separately
// (e.g., measuring lexer throughput in tokens/sec). Use `std::hint::black_box` around the AST
// to prevent the compiler from optimizing away the parsing phase during benchmarks.

#[cfg(test)]
mod tests {
    use super::*;

    struct UserResolver {
        id: i64,
        name: String,
    }

    impl Resolver for UserResolver {
        fn resolve(&self, field: &str) -> Option<Box<dyn Resolver>> {
            match field {
                "id" => Some(Box::new(ScalarResolver(GqlValue::Int(self.id)))),
                "name" => Some(Box::new(ScalarResolver(GqlValue::String(
                    self.name.clone(),
                )))),
                _ => None,
            }
        }
    }

    struct RootResolver;

    impl Resolver for RootResolver {
        fn resolve(&self, field: &str) -> Option<Box<dyn Resolver>> {
            if field == "me" {
                Some(Box::new(UserResolver {
                    id: 1,
                    name: "Alice".to_string(),
                }))
            } else {
                None
            }
        }
    }

    struct ScalarResolver(GqlValue);

    impl Resolver for ScalarResolver {
        fn resolve(&self, _field: &str) -> Option<Box<dyn Resolver>> {
            None
        }
        fn value(&self) -> GqlValue {
            self.0.clone()
        }
    }

    #[test]
    fn test_lexer() {
        let query = "{ me { name } }";
        let mut lexer = Lexer::new(query);
        assert_eq!(lexer.next_token(), Token::LBrace);
        assert_eq!(lexer.next_token(), Token::Ident("me".to_string()));
        assert_eq!(lexer.next_token(), Token::LBrace);
        assert_eq!(lexer.next_token(), Token::Ident("name".to_string()));
        assert_eq!(lexer.next_token(), Token::RBrace);
        assert_eq!(lexer.next_token(), Token::RBrace);
        assert_eq!(lexer.next_token(), Token::Eof);
    }

    #[test]
    fn test_parser() {
        let query = "{ userAlias: me { name } }";
        let lexer = Lexer::new(query);
        let mut parser = Parser::new(lexer);
        let ast = parser.parse_query();

        assert_eq!(ast.len(), 1);
        assert_eq!(ast[0].name, "me");
        assert_eq!(ast[0].alias, Some("userAlias".to_string()));
        assert_eq!(ast[0].selections.len(), 1);
        assert_eq!(ast[0].selections[0].name, "name");
    }

    #[test]
    fn test_execution() {
        let query = "{ me { id name } }";
        let lexer = Lexer::new(query);
        let mut parser = Parser::new(lexer);
        let ast = parser.parse_query();

        let root = RootResolver;
        let result = Executor::execute(&root, &ast);

        if let GqlValue::Object(map) = result {
            if let Some(GqlValue::Object(me_map)) = map.get("me") {
                assert_eq!(me_map.get("id"), Some(&GqlValue::Int(1)));
                assert_eq!(
                    me_map.get("name"),
                    Some(&GqlValue::String("Alice".to_string()))
                );
            } else {
                panic!("Expected 'me' to be an object");
            }
        } else {
            panic!("Expected root result to be an object");
        }
    }
}
