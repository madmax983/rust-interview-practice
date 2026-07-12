//! # GraphQL Execution Engine
//!
//! A minimal GraphQL lexer, parser, and trait-based execution engine.
//!
//! **Replaces Crates:** `async-graphql`, `juniper`
//!
//! **Real-world Usage:**
//! - GitHub API v4 (GraphQL API)
//! - Apollo Federation routers
//! - Hasura / PostGraphile
//!
//! **Why build it yourself?**
//! Understanding how GraphQL translates from a string query into a resolved JSON tree demystifies
//! the magic of schema reflection and field resolution. By building a trait-based engine, you learn
//! how to map dynamic query ASTs to static Rust types.
//!
//! # Architecture
//!
//! ```text
//! Query String -> Lexer -> Tokens -> Parser -> Document (AST) -> Executor -> JSON Result
//!                                                            (using Resolver traits)
//! ```
//!
//! - **Lexer**: Iterative tokenizer to avoid stack overflows on invalid input.
//! - **Parser**: Parses operations and selections (fields, arguments).
//! - **Executor**: Trait `ObjectResolver` defines how a domain object responds to field requests.
//!
//! **Invariants:**
//! - Lexer must not panic on invalid UTF-8 (handled by safe Rust string slicing).
//! - Parser must handle deeply nested queries without unbounded recursion (though here we use controlled recursion, production limits depth).
//! - Execution must match exactly the fields requested in the AST.
//!
//! **Complexity:**
//! - Lexing: O(N) where N is query length.
//! - Parsing: O(N) tokens.
//! - Execution: O(M) where M is the number of nodes in the result tree.
//!
//! **Design Decisions:**
//! - Use an iterative approach in the lexer (not tail recursion) to skip invalid grammar safely.
//! - The `ObjectResolver` trait allows user types to dynamically resolve fields based on AST nodes.
//! - Return generic `Value` enum for results to represent standard JSON-like GraphQL outputs.
//!
//! # Footer
//!
//! **Comparison to `async-graphql`:**
//! `async-graphql` uses heavy procedural macros to generate resolver traits automatically, supports async execution, subscriptions, and directives. This implementation is synchronous and manual, focusing on the core resolution logic.
//!
//! **Missing Features:**
//! - Input types, variables, directives, mutations, subscriptions.
//! - Async execution and parallel field resolution.
//! - Proper error accumulation (GraphQL returns `{ data, errors }` instead of failing the whole query).
//!
//! **Benchmarking Note:**
//! To benchmark, measure the time from lexing a complex query string to final value resolution using `criterion`. Focus on allocations during AST generation and value creation.

use std::collections::HashMap;

// --- AST & Values ---

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Int(i64),
    Float(f64),
    String(String),
    Boolean(bool),
    List(Vec<Value>),
    Object(HashMap<String, Value>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub name: String,
    pub alias: Option<String>,
    pub selections: Vec<Selection>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Selection {
    Field(Field),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    pub selections: Vec<Selection>,
}

// --- Lexer ---

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Name(String),
    LBrace,
    RBrace,
    Colon,
    Eof,
}

pub struct Lexer<'a> {
    _input: &'a str,
    chars: std::str::Chars<'a>,
    current: Option<char>,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        let mut chars = input.chars();
        let current = chars.next();
        Self {
            _input: input,
            chars,
            current,
        }
    }

    fn advance(&mut self) {
        self.current = self.chars.next();
    }

    fn skip_whitespace(&mut self) {
        // RUST INSIGHT: Using `while let` instead of tail recursion prevents stack overflows
        // when skipping large amounts of whitespace or invalid characters.
        while let Some(c) = self.current {
            if c.is_whitespace() || c == ',' {
                self.advance();
            } else {
                break;
            }
        }
    }

    pub fn next_token(&mut self) -> Token {
        // GOTCHA: We use an iterative loop to skip unrecognized characters rather than
        // recursing (which could blow the stack on large invalid input).
        loop {
            self.skip_whitespace();

            if let Some(c) = self.current {
                match c {
                    '{' => {
                        self.advance();
                        return Token::LBrace;
                    }
                    '}' => {
                        self.advance();
                        return Token::RBrace;
                    }
                    ':' => {
                        self.advance();
                        return Token::Colon;
                    }
                    c if c.is_alphabetic() || c == '_' => {
                        let mut name = String::new();
                        while let Some(ch) = self.current {
                            if ch.is_alphanumeric() || ch == '_' {
                                name.push(ch);
                                self.advance();
                            } else {
                                break;
                            }
                        }
                        return Token::Name(name);
                    }
                    _ => {
                        // Unrecognized character, skip it.
                        self.advance();
                        continue;
                    }
                }
            } else {
                return Token::Eof;
            }
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

    pub fn parse(&mut self) -> Result<Document, String> {
        let mut selections = Vec::new();

        // A document usually starts with { for an anonymous query
        if self.current_token == Token::LBrace {
            self.advance();
            selections = self.parse_selections()?;
            if self.current_token != Token::RBrace {
                return Err("Expected }".to_string());
            }
        } else {
            // Simplified: just parse selections directly
            while self.current_token != Token::Eof {
                selections.push(self.parse_selection()?);
            }
        }

        Ok(Document { selections })
    }

    fn parse_selections(&mut self) -> Result<Vec<Selection>, String> {
        let mut selections = Vec::new();
        while self.current_token != Token::RBrace && self.current_token != Token::Eof {
            selections.push(self.parse_selection()?);
        }
        Ok(selections)
    }

    fn parse_selection(&mut self) -> Result<Selection, String> {
        if let Token::Name(name) = &self.current_token {
            let field_name = name.clone();
            self.advance();

            let mut alias = None;
            let mut actual_name = field_name.clone();

            if self.current_token == Token::Colon {
                self.advance();
                if let Token::Name(real_name) = &self.current_token {
                    alias = Some(field_name);
                    actual_name = real_name.clone();
                    self.advance();
                } else {
                    return Err("Expected field name after colon".to_string());
                }
            }

            let mut selections = Vec::new();
            if self.current_token == Token::LBrace {
                self.advance();
                selections = self.parse_selections()?;
                if self.current_token == Token::RBrace {
                    self.advance();
                } else {
                    return Err("Expected }".to_string());
                }
            }

            Ok(Selection::Field(Field {
                name: actual_name,
                alias,
                selections,
            }))
        } else {
            Err("Expected field name".to_string())
        }
    }
}

// --- Execution Engine ---

pub trait ObjectResolver {
    /// Resolves a field by AST Field node, allowing nested resolution if the field has sub-selections.
    fn resolve_field(&self, field: &Field) -> Option<Value>;
}

/// Executes a GraphQL AST against a root resolver.
pub fn execute<R: ObjectResolver>(document: &Document, root: &R) -> Value {
    let mut result_map = HashMap::new();

    for selection in &document.selections {
        let Selection::Field(field) = selection;
        let key = field.alias.as_ref().unwrap_or(&field.name).clone();
        let resolved_value = root.resolve_field(field);

        // PRODUCTION NOTE: Real implementations use `async` and futures for parallel execution.
        result_map.insert(key, resolved_value.unwrap_or(Value::Null));
    }

    Value::Object(result_map)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct QueryRoot;

    impl ObjectResolver for QueryRoot {
        fn resolve_field(&self, field: &Field) -> Option<Value> {
            match field.name.as_str() {
                "version" => Some(Value::String("1.0.0".to_string())),
                "status" => Some(Value::String("ok".to_string())),
                "uptime" => Some(Value::Int(42)),
                "nested" => {
                    // Demonstrate nested resolution
                    let sub_doc = Document { selections: field.selections.clone() };
                    let sub_root = QueryRoot;
                    Some(execute(&sub_doc, &sub_root))
                }
                _ => None,
            }
        }
    }

    #[test]
    fn test_lexer() {
        let mut lexer = Lexer::new("{ myAlias: fieldName }");
        assert_eq!(lexer.next_token(), Token::LBrace);
        assert_eq!(lexer.next_token(), Token::Name("myAlias".to_string()));
        assert_eq!(lexer.next_token(), Token::Colon);
        assert_eq!(lexer.next_token(), Token::Name("fieldName".to_string()));
        assert_eq!(lexer.next_token(), Token::RBrace);
        assert_eq!(lexer.next_token(), Token::Eof);
    }

    #[test]
    fn test_parser() {
        let lexer = Lexer::new("{ user { id name } }");
        let mut parser = Parser::new(lexer);
        let doc = parser.parse().unwrap();

        assert_eq!(doc.selections.len(), 1);
        let Selection::Field(f) = &doc.selections[0];
        assert_eq!(f.name, "user");
        assert_eq!(f.selections.len(), 2);
    }

    #[test]
    fn test_execution() {
        let query = "{ v: version status nested { nested_v: version } }";
        let lexer = Lexer::new(query);
        let mut parser = Parser::new(lexer);
        let doc = parser.parse().unwrap();

        let root = QueryRoot;
        let result = execute(&doc, &root);

        if let Value::Object(map) = result {
            assert_eq!(map.get("v"), Some(&Value::String("1.0.0".to_string())));
            assert_eq!(map.get("status"), Some(&Value::String("ok".to_string())));

            if let Some(Value::Object(nested_map)) = map.get("nested") {
                assert_eq!(nested_map.get("nested_v"), Some(&Value::String("1.0.0".to_string())));
            } else {
                panic!("Expected nested object");
            }
        } else {
            panic!("Expected object");
        }
    }
}
