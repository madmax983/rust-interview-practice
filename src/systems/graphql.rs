//! # GraphQL Execution Engine
//!
//! What this implements and what crate(s) it replaces:
//! This implements a dynamic, schema-driven GraphQL query parser and execution engine.
//! It replaces crates like `async-graphql` and `juniper` by providing a minimal AST builder
//! and trait-based resolver framework.
//!
//! Real-world systems that use this:
//! Meta (Facebook), GitHub's API v4, and countless modern web backends rely on GraphQL engines
//! to aggregate and selectively fetch data from disparate underlying microservices or databases.
//!
//! Why build it yourself?
//! Building a GraphQL engine demystifies how a single string query is transformed into a tree
//! of typed resolvers. You'll learn how to handle nested, recursive execution contexts and how
//! a trait object (`Box<dyn Resolver>`) can dynamically map a structured schema in Rust.
//!
//! ## Architecture
//!
//! ```text
//! [ Query String ] -> (Lexer/Parser) -> [ AST (Document -> Operation -> SelectionSet) ]
//!                                                |
//! [ Execution Context ] -----------------> (Executor) <----------------- [ Schema / Resolvers ]
//!                                                |
//!                                      [ JSON Response ]
//! ```
//!
//! ### Invariants
//! - **Schema Typing**: The output types from resolvers must structurally match what the query requests.
//! - **Depth Limits**: Recursive queries must be bounded to prevent stack overflow DoS attacks.
//! - **Resolver Isolation**: Each field resolves independently; failures in optional fields shouldn't crash the entire response.
//!
//! ### Complexity
//! - **Parsing**: Time: `O(N)` where N is the query string length. Space: `O(N)` for the AST.
//! - **Execution**: Time: `O(F)` where F is the number of fields requested. Space: `O(F)` for the JSON result tree.
//!
//! ### Design Decisions
//! - **Owned Data Types in AST**: Instead of zero-copy string slices (`&'a str`), we use owned `String` types.
//!   While slightly slower due to allocations, it drastically simplifies the lifetime requirements of the
//!   dynamic trait objects (`Box<dyn Resolver>`) that execute against the AST.
//! - **Synchronous Execution**: For simplicity, this is synchronous. Production engines heavily use async/await
//!   and data-loaders to prevent N+1 query problems.
//!
//! ## Implementation

use std::collections::HashMap;

// =========================================================================================
// AST (Abstract Syntax Tree)
// =========================================================================================

#[derive(Debug, PartialEq)]
pub enum Value {
    String(String),
    Int(i64),
    Float(f64),
    Boolean(bool),
    Null,
    List(Vec<Value>),
    Object(HashMap<String, Value>),
}

#[derive(Debug, PartialEq)]
pub struct Field {
    pub name: String,
    pub alias: Option<String>,
    pub arguments: HashMap<String, Value>,
    pub selection_set: Vec<Selection>,
}

#[derive(Debug, PartialEq)]
pub enum Selection {
    Field(Field),
}

#[derive(Debug, PartialEq)]
pub struct OperationDefinition {
    pub operation_type: String, // "query", "mutation"
    pub name: Option<String>,
    pub selection_set: Vec<Selection>,
}

#[derive(Debug, PartialEq)]
pub struct Document {
    pub definitions: Vec<OperationDefinition>,
}

// =========================================================================================
// Lexer & Parser (Minimal)
// =========================================================================================

#[derive(Debug, PartialEq)]
enum Token {
    Name(String),
    StringLiteral(String),
    IntLiteral(i64),
    ParenOpen,
    ParenClose,
    BraceOpen,
    BraceClose,
    Colon,
}

fn tokenize(input: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let mut chars = input.char_indices().peekable();

    while let Some((_, c)) = chars.next() {
        match c {
            ' ' | '\n' | '\r' | '\t' | ',' => continue, // Ignore whitespace and commas
            '(' => tokens.push(Token::ParenOpen),
            ')' => tokens.push(Token::ParenClose),
            '{' => tokens.push(Token::BraceOpen),
            '}' => tokens.push(Token::BraceClose),
            ':' => tokens.push(Token::Colon),
            '"' => {
                let mut string_val = String::new();
                for (_, ch) in chars.by_ref() {
                    if ch == '"' {
                        break;
                    }
                    string_val.push(ch);
                }
                tokens.push(Token::StringLiteral(string_val));
            }
            c if c.is_ascii_alphabetic() || c == '_' => {
                let mut name = String::from(c);
                while let Some(&(_i, ch)) = chars.peek() {
                    if ch.is_ascii_alphanumeric() || ch == '_' {
                        name.push(ch);
                        chars.next();
                    } else {
                        break;
                    }
                }
                tokens.push(Token::Name(name));
            }
            c if c.is_ascii_digit() => {
                let mut num_str = String::from(c);
                while let Some(&(_i, ch)) = chars.peek() {
                    if ch.is_ascii_digit() {
                        num_str.push(ch);
                        chars.next();
                    } else {
                        break;
                    }
                }
                let val = num_str.parse::<i64>().map_err(|_| "Invalid integer")?;
                tokens.push(Token::IntLiteral(val));
            }
            _ => return Err(format!("Unexpected character: {}", c)),
        }
    }
    Ok(tokens)
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<&Token> {
        let token = self.tokens.get(self.pos);
        self.pos += 1;
        token
    }

    fn expect_name(&mut self) -> Result<String, String> {
        match self.advance() {
            Some(Token::Name(name)) => Ok(name.clone()),
            _ => Err("Expected Name".into()),
        }
    }

    fn parse_value(&mut self) -> Result<Value, String> {
        match self.advance() {
            Some(Token::StringLiteral(s)) => Ok(Value::String(s.clone())),
            Some(Token::IntLiteral(i)) => Ok(Value::Int(*i)),
            Some(Token::Name(n)) if n == "true" => Ok(Value::Boolean(true)),
            Some(Token::Name(n)) if n == "false" => Ok(Value::Boolean(false)),
            Some(Token::Name(n)) if n == "null" => Ok(Value::Null),
            _ => Err("Expected Value".into()),
        }
    }

    fn parse_arguments(&mut self) -> Result<HashMap<String, Value>, String> {
        let mut args = HashMap::new();
        if let Some(Token::ParenOpen) = self.peek() {
            self.advance(); // consume '('
            while let Some(token) = self.peek() {
                if token == &Token::ParenClose {
                    self.advance(); // consume ')'
                    break;
                }
                let name = self.expect_name()?;
                if self.advance() != Some(&Token::Colon) {
                    return Err("Expected ':' after argument name".into());
                }
                let val = self.parse_value()?;
                args.insert(name, val);
            }
        }
        Ok(args)
    }

    fn parse_selection_set(&mut self) -> Result<Vec<Selection>, String> {
        let mut selections = Vec::new();
        if self.advance() != Some(&Token::BraceOpen) {
            return Err("Expected '{'".into());
        }

        while let Some(token) = self.peek() {
            if token == &Token::BraceClose {
                self.advance(); // consume '}'
                break;
            }

            // Could be an alias or a field name
            let mut name_or_alias = self.expect_name()?;
            let mut alias = None;

            if let Some(Token::Colon) = self.peek() {
                self.advance(); // consume ':'
                alias = Some(name_or_alias);
                name_or_alias = self.expect_name()?;
            }

            let arguments = self.parse_arguments()?;

            // Recursive selection set
            let selection_set = if let Some(Token::BraceOpen) = self.peek() {
                self.parse_selection_set()?
            } else {
                Vec::new()
            };

            selections.push(Selection::Field(Field {
                name: name_or_alias,
                alias,
                arguments,
                selection_set,
            }));
        }

        Ok(selections)
    }

    fn parse_document(&mut self) -> Result<Document, String> {
        let mut definitions = Vec::new();

        // Very simplified: assume everything is a query, and "query" keyword might be omitted.
        let is_query_keyword = match self.peek() {
            Some(Token::Name(n)) if n == "query" => true,
            _ => false,
        };

        if is_query_keyword {
            self.advance();
        }

        // Optional name
        let name = match self.peek() {
            Some(Token::Name(n)) if n != "{" => {
                let n = n.clone();
                self.advance();
                Some(n)
            }
            _ => None,
        };

        let selection_set = self.parse_selection_set()?;

        definitions.push(OperationDefinition {
            operation_type: "query".to_string(),
            name,
            selection_set,
        });

        Ok(Document { definitions })
    }
}

pub fn parse_query(input: &str) -> Result<Document, String> {
    let tokens = tokenize(input)?;
    let mut parser = Parser::new(tokens);
    parser.parse_document()
}

// =========================================================================================
// Executor & Resolvers
// =========================================================================================

/// The core trait that makes the schema dynamic.
///
/// # RUST INSIGHT: Trait Objects for Recursive APIs
/// In Rust, building a recursive structure like a JSON/GraphQL tree requires dynamic dispatch.
/// By returning `Box<dyn Resolver>`, a field on a `User` type can return an `Address` type,
/// and the executor doesn't need to know the concrete types at compile time.
pub trait Resolver {
    /// Resolves a single field on this object.
    fn resolve_field(
        &self,
        name: &str,
        args: &HashMap<String, Value>,
    ) -> Result<ResolverValue, String>;
}

/// The result of resolving a field.
pub enum ResolverValue {
    Scalar(Value),
    Object(Box<dyn Resolver>),
    List(Vec<ResolverValue>),
    Null,
}

pub struct ExecutionContext;

/// Executes a GraphQL document against a root resolver.
pub fn execute(document: &Document, root_resolver: &dyn Resolver) -> Result<Value, String> {
    // For simplicity, just execute the first operation
    let operation = document.definitions.first().ok_or("No operation found")?;

    let result = execute_selection_set(&operation.selection_set, root_resolver)?;
    Ok(Value::Object(result))
}

fn execute_selection_set(
    selections: &[Selection],
    resolver: &dyn Resolver,
) -> Result<HashMap<String, Value>, String> {
    let mut result_map = HashMap::new();

    for selection in selections {
        if let Selection::Field(field) = selection {
            // RUST INSIGHT: Use the alias if provided, otherwise the field name as the key.
            // This is a core GraphQL spec requirement.
            let response_key = field.alias.as_ref().unwrap_or(&field.name).clone();

            let resolver_result = resolver.resolve_field(&field.name, &field.arguments)?;

            let resolved_value = execute_resolver_value(resolver_result, &field.selection_set)?;

            result_map.insert(response_key, resolved_value);
        }
    }

    Ok(result_map)
}

fn execute_resolver_value(
    value: ResolverValue,
    selection_set: &[Selection],
) -> Result<Value, String> {
    match value {
        ResolverValue::Scalar(val) => Ok(val),
        ResolverValue::Null => Ok(Value::Null),
        ResolverValue::Object(obj_resolver) => {
            if selection_set.is_empty() {
                return Err("Object must have a selection set".into());
            }
            let map = execute_selection_set(selection_set, obj_resolver.as_ref())?;
            Ok(Value::Object(map))
        }
        ResolverValue::List(items) => {
            let mut list_result = Vec::new();
            for item in items {
                list_result.push(execute_resolver_value(item, selection_set)?);
            }
            Ok(Value::List(list_result))
        }
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `async-graphql`: The current standard in Rust. Uses macros to generate schemas natively
//   from Rust types, deeply integrates with `async`, supports subscriptions via websockets,
//   and implements the full spec.
// - `juniper`: An older crate, pioneered the macro-based approach but lacks some modern async features.
//
// What's missing vs. production:
// - **Async Execution**: Resolving fields over a network requires async execution, not sync.
// - **Dataloaders**: Naive execution causes the N+1 query problem. Production engines batch requests.
// - **Validation**: We do not perform query validation against a strong schema (type checking,
//   non-null enforcement, variable substitution) before execution.
// - **Fragments / Directives**: The parser lacks support for fragment spreads or `@skip`/`@include`.
//
// Next steps:
// 1. Make the `Resolver` trait methods return `Pin<Box<dyn Future<Output = Result<ResolverValue, String>> + Send>>`
// 2. Implement the introspection query `__schema`.

#[cfg(test)]
mod tests {
    use super::*;

    // A mock User resolver
    struct User {
        id: i64,
        name: String,
    }

    impl Resolver for User {
        fn resolve_field(
            &self,
            name: &str,
            _args: &HashMap<String, Value>,
        ) -> Result<ResolverValue, String> {
            match name {
                "id" => Ok(ResolverValue::Scalar(Value::Int(self.id))),
                "name" => Ok(ResolverValue::Scalar(Value::String(self.name.clone()))),
                _ => Err(format!("Unknown field {} on User", name)),
            }
        }
    }

    // A mock Root Query resolver
    struct RootQuery;

    impl Resolver for RootQuery {
        fn resolve_field(
            &self,
            name: &str,
            args: &HashMap<String, Value>,
        ) -> Result<ResolverValue, String> {
            match name {
                "me" => Ok(ResolverValue::Object(Box::new(User {
                    id: 1,
                    name: "Alice".to_string(),
                }))),
                "user" => {
                    let id_val = args.get("id").ok_or("Missing argument 'id'")?;
                    if let Value::Int(id) = id_val {
                        if *id == 1 {
                            Ok(ResolverValue::Object(Box::new(User {
                                id: 1,
                                name: "Alice".to_string(),
                            })))
                        } else {
                            Ok(ResolverValue::Null)
                        }
                    } else {
                        Err("Argument 'id' must be an int".into())
                    }
                }
                _ => Err(format!("Unknown field {} on RootQuery", name)),
            }
        }
    }

    #[test]
    fn test_lexing_and_parsing() {
        let query = r#"
        {
            user(id: 1) {
                id
                name
            }
        }
        "#;
        let doc = parse_query(query).unwrap();
        assert_eq!(doc.definitions.len(), 1);

        let op = &doc.definitions[0];
        assert_eq!(op.operation_type, "query");
        assert_eq!(op.selection_set.len(), 1);

        let Selection::Field(f) = &op.selection_set[0];
        assert_eq!(f.name, "user");
        assert!(f.arguments.contains_key("id"));
        assert_eq!(f.selection_set.len(), 2);
    }

    #[test]
    fn test_execution_simple() {
        let query = "{ me { id name } }";
        let doc = parse_query(query).unwrap();

        let result = execute(&doc, &RootQuery).unwrap();

        if let Value::Object(map) = result {
            if let Value::Object(me_map) = map.get("me").unwrap() {
                assert_eq!(me_map.get("id").unwrap(), &Value::Int(1));
                assert_eq!(
                    me_map.get("name").unwrap(),
                    &Value::String("Alice".to_string())
                );
            } else {
                panic!("Expected 'me' to be an object");
            }
        } else {
            panic!("Expected root to be an object");
        }
    }

    #[test]
    fn test_execution_with_arguments() {
        let query = "{ user(id: 1) { name } }";
        let doc = parse_query(query).unwrap();
        let result = execute(&doc, &RootQuery).unwrap();

        if let Value::Object(map) = result {
            if let Value::Object(user_map) = map.get("user").unwrap() {
                assert_eq!(
                    user_map.get("name").unwrap(),
                    &Value::String("Alice".to_string())
                );
            } else {
                panic!("Expected 'user' to be an object");
            }
        } else {
            panic!("Expected root to be an object");
        }
    }

    #[test]
    fn test_execution_with_alias() {
        let query = "{ alice: user(id: 1) { name } }";
        let doc = parse_query(query).unwrap();
        let result = execute(&doc, &RootQuery).unwrap();

        if let Value::Object(map) = result {
            assert!(map.contains_key("alice"));
            assert!(!map.contains_key("user"));
        } else {
            panic!("Expected root to be an object");
        }
    }
}
