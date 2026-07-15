//! # Minimal GraphQL Execution Engine
//!
//! **Replaces Crates:** `async-graphql`, `juniper`
//!
//! **Real-world systems:**
//! - GitHub's v4 API, Facebook's mobile data layer, Apollo GraphQL server
//!
//! **Why build it yourself?**
//! Understanding how a query is lexed, parsed, and recursively executed against
//! a typed schema is crucial for optimizing N+1 query problems and understanding
//! the execution cost of a GraphQL graph. Building it reveals how a string turns
//! into a tree of resolver calls.
//!
//! ## Architecture
//!
//! ```text
//! [Query String]
//!       | (Lexer: iter/while let loop over chars)
//!       v
//! [Tokens] => { '{', 'user', '(', 'id', ':', '1', ')', '{', 'name', '}', '}' }
//!       | (Parser: Recursive Descent)
//!       v
//! [AST (Document -> Operation -> SelectionSet -> Field)]
//!       | (Executor: DFS recursive resolution)
//!       v
//! [Resolver Traits / Schema] -> DB / Cache
//!       |
//!       v
//! [JSON/Value Response]
//! ```
//!
//! ### Invariants
//! - Lexer must not crash on invalid input; iterative parsing to prevent stack overflows.
//! - The executor must map AST fields exactly to Schema trait resolvers.
//! - Valid queries only execute resolvers present in the SelectionSet.
//!
//! ### Complexity
//! - **Lexing:** O(N) where N is the query string length.
//! - **Parsing:** O(N) in the number of tokens.
//! - **Execution:** O(V + E) where V is requested fields and E are relationships, heavily dependent on resolver logic.
//!
//! ### Tradeoffs vs Production
//! - Real engines handle full validation against a strict schema (types, non-null, etc.) before execution. We do schema-less validation where the Schema Traits implicitly define validity.
//! - Real engines use async resolvers to batch data-loaders (solving N+1). We use synchronous resolvers for simplicity.
//!
//! ## Footer
//!
//! ### Crate Comparison
//! - `async-graphql`: Macro-heavy, heavily async, fully spec-compliant.
//! - This implementation: Minimal, explicit trait-based, synchronous, educational subset of the spec.
//!
//! ### Missing Features
//! - Async/await support and DataLoaders.
//! - Full GraphQL type system validation (Introspection).
//! - Variables, Fragments, Directives, Mutations.
//!
//! ### Next Steps
//! - Introduce `async` traits for resolvers to allow parallel fetching.
//! - Add a DataLoader primitive to batch resolver requests.
//!
//! --- Benchmarking Note ---
//! To benchmark, compare the `Executor::execute` time using `std::hint::black_box`
//! on heavily nested selection sets to measure AST traversal overhead versus actual
//! data fetching.

use std::collections::HashMap;

// --- LEXER ---

#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    BraceL,
    BraceR,
    ParenL,
    ParenR,
    Colon,
    Name(String),
    String(String),
    Int(i64),
}

pub struct Lexer<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Lexer<'a> {
    #[must_use]
    pub fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    // RUST INSIGHT:
    // Returning `Option<Token>` provides a safe and idiomatic way to signal EOF.
    pub fn next_token(&mut self) -> Option<Token> {
        let bytes = self.input.as_bytes();
        // GOTCHA:
        // Instead of tail recursion to skip whitespace or invalid characters
        // (which could stack overflow on long invalid inputs), we use an iterative loop.
        loop {
            if self.pos >= bytes.len() {
                return None;
            }

            let c = bytes[self.pos];

            match c {
                b' ' | b'\n' | b'\r' | b'\t' | b',' => {
                    self.pos += 1;
                }
                b'{' => {
                    self.pos += 1;
                    return Some(Token::BraceL);
                }
                b'}' => {
                    self.pos += 1;
                    return Some(Token::BraceR);
                }
                b'(' => {
                    self.pos += 1;
                    return Some(Token::ParenL);
                }
                b')' => {
                    self.pos += 1;
                    return Some(Token::ParenR);
                }
                b':' => {
                    self.pos += 1;
                    return Some(Token::Colon);
                }
                b'"' => {
                    self.pos += 1;
                    let start = self.pos;
                    while self.pos < bytes.len() && bytes[self.pos] != b'"' {
                        self.pos += 1;
                    }
                    let val = &self.input[start..self.pos];
                    if self.pos < bytes.len() {
                        self.pos += 1; // skip closing quote
                    }
                    return Some(Token::String(val.to_string()));
                }
                _ if c.is_ascii_alphabetic() || c == b'_' => {
                    let start = self.pos;
                    while self.pos < bytes.len()
                        && (bytes[self.pos].is_ascii_alphanumeric() || bytes[self.pos] == b'_')
                    {
                        self.pos += 1;
                    }
                    return Some(Token::Name(self.input[start..self.pos].to_string()));
                }
                _ if c.is_ascii_digit() || c == b'-' => {
                    let start = self.pos;
                    if c == b'-' {
                        self.pos += 1;
                    }
                    while self.pos < bytes.len() && bytes[self.pos].is_ascii_digit() {
                        self.pos += 1;
                    }
                    let val: i64 = self.input[start..self.pos].parse().unwrap_or(0);
                    return Some(Token::Int(val));
                }
                _ => {
                    // Skip unrecognized character iteratively
                    self.pos += 1;
                }
            }
        }
    }
}

// --- PARSER ---

#[derive(Debug, PartialEq, Clone)]
pub enum Value {
    String(String),
    Int(i64),
    Object(HashMap<String, Value>),
    List(Vec<Value>),
    Null,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Argument {
    pub name: String,
    pub value: Value,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Field {
    pub name: String,
    pub arguments: Vec<Argument>,
    pub selection_set: Vec<Field>,
}

pub struct Parser<'a> {
    lexer: Lexer<'a>,
    current_token: Option<Token>,
}

impl<'a> Parser<'a> {
    #[must_use]
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

    /// Parses a selection set.
    ///
    /// # Errors
    /// Returns a string error if the parsing fails due to invalid syntax.
    pub fn parse_selection_set(&mut self) -> Result<Vec<Field>, String> {
        let mut fields = Vec::new();

        if self.current_token != Some(Token::BraceL) {
            return Err("Expected { for selection set".to_string());
        }
        self.advance(); // consume {

        while self.current_token.is_some() && self.current_token != Some(Token::BraceR) {
            if let Some(Token::Name(name)) = self.current_token.clone() {
                self.advance();
                let mut arguments = Vec::new();
                if self.current_token == Some(Token::ParenL) {
                    self.advance(); // consume (
                    while self.current_token.is_some() && self.current_token != Some(Token::ParenR)
                    {
                        if let Some(Token::Name(arg_name)) = self.current_token.clone() {
                            self.advance();
                            if self.current_token == Some(Token::Colon) {
                                self.advance(); // consume :
                                let val = match self.current_token.clone() {
                                    Some(Token::String(s)) => {
                                        self.advance();
                                        Value::String(s)
                                    }
                                    Some(Token::Int(i)) => {
                                        self.advance();
                                        Value::Int(i)
                                    }
                                    _ => return Err("Expected argument value".to_string()),
                                };
                                arguments.push(Argument {
                                    name: arg_name,
                                    value: val,
                                });
                            } else {
                                return Err("Expected : after argument name".to_string());
                            }
                        } else {
                            return Err("Expected argument name".to_string());
                        }
                    }
                    if self.current_token == Some(Token::ParenR) {
                        self.advance(); // consume )
                    } else {
                        return Err("Expected ) after arguments".to_string());
                    }
                }

                let mut selection_set = Vec::new();
                if self.current_token == Some(Token::BraceL) {
                    selection_set = self.parse_selection_set()?;
                }

                fields.push(Field {
                    name,
                    arguments,
                    selection_set,
                });
            } else {
                return Err("Expected field name".to_string());
            }
        }

        if self.current_token == Some(Token::BraceR) {
            self.advance(); // consume }
            Ok(fields)
        } else {
            Err("Expected }".to_string())
        }
    }
}

// --- ENGINE / TRAITS ---

// RUST INSIGHT:
// By defining a generic trait `GraphQLObject`, we leverage Rust's dynamic dispatch
// (`&dyn GraphQLObject`) or static dispatch to map AST fields to arbitrary data sources.
pub trait GraphQLObject {
    fn resolve_field(&self, name: &str, args: &[Argument]) -> Value;
    // For nested resolution, returning a boxed trait object or `None` if it's a scalar.
    // In a real implementation, we'd use Associated Types or an Enum to avoid Boxing overhead.
    // PRODUCTION NOTE:
    // A production crate like async-graphql uses macros to generate these implementations
    // statically, eliminating `Box<dyn GraphQLObject>` allocations.
    fn resolve_child(&self, name: &str, args: &[Argument]) -> Option<Box<dyn GraphQLObject>>;
}

pub struct Executor;

impl Executor {
    /// Executes a query on the root object.
    ///
    /// # Errors
    /// Returns a string error if the parsing fails.
    pub fn execute(root: &dyn GraphQLObject, query: &str) -> Result<Value, String> {
        let lexer = Lexer::new(query);
        let mut parser = Parser::new(lexer);
        let fields = parser.parse_selection_set()?;
        Ok(Self::execute_selection_set(root, &fields))
    }

    fn execute_selection_set(obj: &dyn GraphQLObject, fields: &[Field]) -> Value {
        let mut result_map = HashMap::new();
        for field in fields {
            if !field.selection_set.is_empty() {
                // It's a complex object requiring recursive resolution
                if let Some(child_obj) = obj.resolve_child(&field.name, &field.arguments) {
                    let child_res =
                        Self::execute_selection_set(child_obj.as_ref(), &field.selection_set);
                    result_map.insert(field.name.clone(), child_res);
                } else {
                    result_map.insert(field.name.clone(), Value::Null);
                }
            } else {
                // It's a scalar field
                let val = obj.resolve_field(&field.name, &field.arguments);
                result_map.insert(field.name.clone(), val);
            }
        }
        Value::Object(result_map)
    }
}

// --- TESTING ---

#[cfg(test)]
mod tests {
    use super::*;

    struct User {
        id: i64,
        name: String,
    }

    impl GraphQLObject for User {
        fn resolve_field(&self, name: &str, _args: &[Argument]) -> Value {
            match name {
                "id" => Value::Int(self.id),
                "name" => Value::String(self.name.clone()),
                _ => Value::Null,
            }
        }
        fn resolve_child(&self, _name: &str, _args: &[Argument]) -> Option<Box<dyn GraphQLObject>> {
            None
        }
    }

    struct QueryRoot;

    impl GraphQLObject for QueryRoot {
        fn resolve_field(&self, _name: &str, _args: &[Argument]) -> Value {
            Value::Null
        }

        fn resolve_child(&self, name: &str, args: &[Argument]) -> Option<Box<dyn GraphQLObject>> {
            if name == "user" {
                let id = args
                    .iter()
                    .find(|a| a.name == "id")
                    .and_then(|a| match a.value {
                        Value::Int(i) => Some(i),
                        _ => None,
                    })
                    .unwrap_or(0);

                Some(Box::new(User {
                    id,
                    name: format!("Alice {id}"),
                }))
            } else {
                None
            }
        }
    }

    #[test]
    fn test_lexer() {
        let query = "{ user(id: 1) { name } }";
        let mut lexer = Lexer::new(query);
        assert_eq!(lexer.next_token(), Some(Token::BraceL));
        assert_eq!(lexer.next_token(), Some(Token::Name("user".to_string())));
        assert_eq!(lexer.next_token(), Some(Token::ParenL));
        assert_eq!(lexer.next_token(), Some(Token::Name("id".to_string())));
        assert_eq!(lexer.next_token(), Some(Token::Colon));
        assert_eq!(lexer.next_token(), Some(Token::Int(1)));
        assert_eq!(lexer.next_token(), Some(Token::ParenR));
        assert_eq!(lexer.next_token(), Some(Token::BraceL));
        assert_eq!(lexer.next_token(), Some(Token::Name("name".to_string())));
        assert_eq!(lexer.next_token(), Some(Token::BraceR));
        assert_eq!(lexer.next_token(), Some(Token::BraceR));
        assert_eq!(lexer.next_token(), None);
    }

    #[test]
    fn test_lexer_invalid_chars() {
        // Should skip '@' and '!'
        let query = "{ @user ! }";
        let mut lexer = Lexer::new(query);
        assert_eq!(lexer.next_token(), Some(Token::BraceL));
        assert_eq!(lexer.next_token(), Some(Token::Name("user".to_string())));
        assert_eq!(lexer.next_token(), Some(Token::BraceR));
        assert_eq!(lexer.next_token(), None);
    }

    #[test]
    fn test_parser() {
        let query = "{ user(id: 1) { name } }";
        let lexer = Lexer::new(query);
        let mut parser = Parser::new(lexer);
        let ast = parser.parse_selection_set().unwrap();

        assert_eq!(ast.len(), 1);
        assert_eq!(ast[0].name, "user");
        assert_eq!(ast[0].arguments.len(), 1);
        assert_eq!(ast[0].arguments[0].name, "id");
        assert_eq!(ast[0].arguments[0].value, Value::Int(1));
        assert_eq!(ast[0].selection_set.len(), 1);
        assert_eq!(ast[0].selection_set[0].name, "name");
    }

    #[test]
    fn test_executor() {
        let query = "{ user(id: 42) { id name } }";
        let root = QueryRoot;
        let res = Executor::execute(&root, query).unwrap();

        if let Value::Object(map) = res {
            if let Some(Value::Object(user_map)) = map.get("user") {
                assert_eq!(user_map.get("id"), Some(&Value::Int(42)));
                assert_eq!(
                    user_map.get("name"),
                    Some(&Value::String("Alice 42".to_string()))
                );
            } else {
                panic!("Expected user object");
            }
        } else {
            panic!("Expected root object");
        }
    }
}
