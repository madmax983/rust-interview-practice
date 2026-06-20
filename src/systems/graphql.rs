//! # GraphQL Execution Engine
//!
//! Implements a minimal lexer, parser, and trait-based recursive execution engine
//! for a subset of GraphQL.
//!
//! **Replaces Crates:** `async-graphql`, `juniper`
//!
//! **Real-world Usage:**
//! - Core of GraphQL servers, orchestrating the resolution of queries against a schema.
//! - Used in federated gateway services to parse and split queries.
//!
//! **Why build it yourself?**
//! Building a GraphQL engine from scratch teaches you how text-based queries are tokenized,
//! parsed into an Abstract Syntax Tree (AST), and then evaluated using recursive trait resolvers.
//! You learn about the complexities of resolving nested fields and returning structured JSON-like data.
//!
//! ## Architecture
//!
//! The execution engine consists of three main components:
//! 1. **Lexer:** Converts a raw string query into a stream of tokens. It uses an iterative approach
//!    to safely consume invalid characters without risking stack overflows from tail recursion.
//! 2. **Parser:** Converts the stream of tokens into an AST (Document -> OperationDefinition -> SelectionSet).
//! 3. **Executor:** Uses a `Resolver` trait to recursively resolve the AST against the provided root data structure.
//!
//! ### Data Structure Diagram
//! ```text
//! Query String -> Lexer -> [Token, Token, ...]
//!                          |
//!                          v
//!                        Parser -> AST (Document -> SelectionSet -> Field)
//!                                  |
//!                                  v
//!                                Executor (resolves AST via Resolver trait) -> JSON-like Value
//! ```
//!
//! ### Invariants
//! - Lexer must not panic on invalid UTF-8 (handled by safe char boundary advancement).
//! - Executor must safely resolve nested selections without unbounded recursion.
//!
//! ### Complexity
//! - **Lexing:** O(N) where N is the length of the query string.
//! - **Parsing:** O(N) where N is the number of tokens.
//! - **Execution:** O(V + E) where V is the number of fields selected and E is the resolution cost per field.
//!
//! ## Design Decisions
//! - **Iterative Lexer:** Instead of recursive calls to `next_token` (which could stack overflow on long sequences of invalid chars), we use a loop.
//! - **Owned Strings in Output:** We use owned `String` instead of zero-copy `&str` for the evaluated JSON-like output. This avoids lifetime conflicts with dynamically generated trait objects.
//!
//! ## Benchmarking Note
//! You can benchmark this using `criterion`. Due to the nature of parsing, compare execution
//! time and allocations for increasingly deeply nested queries versus flat queries.
//! `std::hint::black_box` should be used around the input query to prevent constant folding.
//!
//! ## Footer
//! - **Comparison to Canonical:** `async-graphql` generates code at compile time via macros to strongly
//!   type inputs and outputs, and supports async resolution. This implementation uses runtime string
//!   comparisons and only supports synchronous resolution.
//! - **What's Missing:** Schema validation, mutations, subscriptions, fragments, variables, directives,
//!   type introspection, and asynchronous resolvers.
//! - **Next Steps:** Implement a validation phase to check AST against a `Schema` definition before execution.

use std::collections::HashMap;

// -----------------------------------------------------------------------------
// AST and Values
// -----------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Name(String),
    BraceL,
    BraceR,
    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub name: String,
    pub selection_set: Option<Vec<Field>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    pub selection_set: Vec<Field>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GqlValue {
    Null,
    String(String),
    Int(i64),
    Float(f64),
    Boolean(bool),
    Object(HashMap<String, GqlValue>),
    List(Vec<GqlValue>),
}

// -----------------------------------------------------------------------------
// Lexer
// -----------------------------------------------------------------------------

pub struct Lexer<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    pub fn next_token(&mut self) -> Token {
        // RUST INSIGHT:
        // We use an iterative `loop` instead of tail recursion to skip whitespace
        // and unrecognized characters. This prevents stack overflows on long
        // sequences of invalid input.
        loop {
            if self.pos >= self.input.len() {
                return Token::Eof;
            }

            let remaining = &self.input[self.pos..];
            let c = remaining.chars().next().unwrap();

            match c {
                ' ' | '\t' | '\n' | '\r' | ',' => {
                    // Safe advancement by char byte length
                    self.pos += c.len_utf8();
                }
                '{' => {
                    self.pos += c.len_utf8();
                    return Token::BraceL;
                }
                '}' => {
                    self.pos += c.len_utf8();
                    return Token::BraceR;
                }
                _ if c.is_alphabetic() || c == '_' => {
                    let start = self.pos;
                    while self.pos < self.input.len() {
                        let inner_c = self.input[self.pos..].chars().next().unwrap();
                        if inner_c.is_alphanumeric() || inner_c == '_' {
                            self.pos += inner_c.len_utf8();
                        } else {
                            break;
                        }
                    }
                    // PRODUCTION NOTE:
                    // A real lexer might return string slices to avoid allocation,
                    // but we allocate owned Strings here for simplicity.
                    return Token::Name(self.input[start..self.pos].to_string());
                }
                _ => {
                    // Skip unrecognized characters to be robust
                    self.pos += c.len_utf8();
                }
            }
        }
    }
}

// -----------------------------------------------------------------------------
// Parser
// -----------------------------------------------------------------------------

pub struct Parser<'a> {
    lexer: Lexer<'a>,
    current_token: Token,
}

impl<'a> Parser<'a> {
    pub fn new(mut lexer: Lexer<'a>) -> Self {
        let current_token = lexer.next_token();
        Self { lexer, current_token }
    }

    fn advance(&mut self) {
        self.current_token = self.lexer.next_token();
    }

    pub fn parse_document(&mut self) -> Result<Document, String> {
        let selection_set = self.parse_selection_set()?;
        Ok(Document { selection_set })
    }

    fn parse_selection_set(&mut self) -> Result<Vec<Field>, String> {
        if self.current_token != Token::BraceL {
            return Err("Expected '{'".to_string());
        }
        self.advance();

        let mut fields = Vec::new();
        while self.current_token != Token::BraceR && self.current_token != Token::Eof {
            fields.push(self.parse_field()?);
        }

        if self.current_token == Token::BraceR {
            self.advance();
            Ok(fields)
        } else {
            Err("Expected '}'".to_string())
        }
    }

    fn parse_field(&mut self) -> Result<Field, String> {
        match &self.current_token {
            Token::Name(name) => {
                let field_name = name.clone();
                self.advance();

                let selection_set = if self.current_token == Token::BraceL {
                    Some(self.parse_selection_set()?)
                } else {
                    None
                };

                Ok(Field {
                    name: field_name,
                    selection_set,
                })
            }
            _ => Err("Expected field name".to_string()),
        }
    }
}

// -----------------------------------------------------------------------------
// Executor
// -----------------------------------------------------------------------------

/// The Resolver trait represents any value that can be queried in our GraphQL engine.
/// We use Box<dyn Resolver> for objects, which allows for dynamic resolution.
pub trait Resolver {
    fn resolve(&self, field_name: &str) -> Option<GqlValue>;
}

pub struct Executor;

impl Executor {
    pub fn execute(document: &Document, root: &dyn Resolver) -> GqlValue {
        let mut result = HashMap::new();
        for field in &document.selection_set {
            if let Some(val) = root.resolve(&field.name) {
                // GOTCHA:
                // We must handle nested selection sets if the resolved value is an object
                // and the query specifies sub-fields.
                let resolved_val = if let Some(ref selection) = field.selection_set {
                    if let GqlValue::Object(obj) = val {
                        Self::execute_selection(selection, &obj)
                    } else if let GqlValue::List(list) = val {
                        let mut list_res = Vec::new();
                        for item in list {
                            if let GqlValue::Object(item_obj) = item {
                                list_res.push(Self::execute_selection(selection, &item_obj));
                            } else {
                                list_res.push(GqlValue::Null); // Cannot sub-select on scalar
                            }
                        }
                        GqlValue::List(list_res)
                    } else {
                        val // Invalid sub-selection on scalar, ignore or return null
                    }
                } else {
                    val
                };
                result.insert(field.name.clone(), resolved_val);
            }
        }
        GqlValue::Object(result)
    }

    fn execute_selection(selection_set: &[Field], obj: &HashMap<String, GqlValue>) -> GqlValue {
        let mut result = HashMap::new();
        for field in selection_set {
            if let Some(val) = obj.get(&field.name) {
                let resolved_val = if let Some(ref selection) = field.selection_set {
                    if let GqlValue::Object(sub_obj) = val {
                        Self::execute_selection(selection, sub_obj)
                    } else if let GqlValue::List(list) = val {
                        let mut list_res = Vec::new();
                        for item in list {
                            if let GqlValue::Object(item_obj) = item {
                                list_res.push(Self::execute_selection(selection, item_obj));
                            } else {
                                list_res.push(GqlValue::Null);
                            }
                        }
                        GqlValue::List(list_res)
                    } else {
                        val.clone()
                    }
                } else {
                    val.clone()
                };
                result.insert(field.name.clone(), resolved_val);
            }
        }
        GqlValue::Object(result)
    }
}

// Implement Resolver for a simple HashMap root
impl Resolver for HashMap<String, GqlValue> {
    fn resolve(&self, field_name: &str) -> Option<GqlValue> {
        self.get(field_name).cloned()
    }
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lexer() {
        let mut lexer = Lexer::new("{ user { id name } }");
        assert_eq!(lexer.next_token(), Token::BraceL);
        assert_eq!(lexer.next_token(), Token::Name("user".to_string()));
        assert_eq!(lexer.next_token(), Token::BraceL);
        assert_eq!(lexer.next_token(), Token::Name("id".to_string()));
        assert_eq!(lexer.next_token(), Token::Name("name".to_string()));
        assert_eq!(lexer.next_token(), Token::BraceR);
        assert_eq!(lexer.next_token(), Token::BraceR);
        assert_eq!(lexer.next_token(), Token::Eof);
    }

    #[test]
    fn test_lexer_invalid_chars() {
        // Should skip invalid characters (!) using the iterative loop
        let mut lexer = Lexer::new("{ user! }");
        assert_eq!(lexer.next_token(), Token::BraceL);
        assert_eq!(lexer.next_token(), Token::Name("user".to_string()));
        assert_eq!(lexer.next_token(), Token::BraceR);
        assert_eq!(lexer.next_token(), Token::Eof);
    }

    #[test]
    fn test_parser() {
        let lexer = Lexer::new("{ user { id name } }");
        let mut parser = Parser::new(lexer);
        let doc = parser.parse_document().unwrap();

        assert_eq!(doc.selection_set.len(), 1);
        assert_eq!(doc.selection_set[0].name, "user");
        let sub_fields = doc.selection_set[0].selection_set.as_ref().unwrap();
        assert_eq!(sub_fields.len(), 2);
        assert_eq!(sub_fields[0].name, "id");
        assert_eq!(sub_fields[1].name, "name");
    }

    #[test]
    fn test_executor() {
        let mut root = HashMap::new();
        let mut user = HashMap::new();
        user.insert("id".to_string(), GqlValue::Int(1));
        user.insert("name".to_string(), GqlValue::String("Alice".to_string()));
        root.insert("user".to_string(), GqlValue::Object(user));

        let lexer = Lexer::new("{ user { name } }");
        let mut parser = Parser::new(lexer);
        let doc = parser.parse_document().unwrap();

        let result = Executor::execute(&doc, &root);

        if let GqlValue::Object(map) = result {
            let user_res = map.get("user").unwrap();
            if let GqlValue::Object(user_map) = user_res {
                assert_eq!(user_map.get("name").unwrap(), &GqlValue::String("Alice".to_string()));
                assert!(user_map.get("id").is_none()); // ID was not requested
            } else {
                panic!("Expected Object");
            }
        } else {
            panic!("Expected Object");
        }
    }
}
