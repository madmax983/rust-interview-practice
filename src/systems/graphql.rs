//! # GraphQL Execution Engine
//!
//! What this implements and what crate(s) it replaces:
//! This replaces crates like `async-graphql` and `juniper`. It provides a minimal lexer, parser,
//! and trait-based recursive execution engine for evaluating GraphQL-like queries against a graph schema.
//!
//! Real-world systems that use this:
//! API servers, Hasura, Apollo Server. GraphQL engines power flexible API layers over databases and microservices.
//!
//! Why build it yourself:
//! Building a GraphQL engine from scratch teaches you how lexers and parsers work together without
//! external dependencies, and how to use trait objects to recursively resolve queries over a generic data graph.
//!
//! ## Architecture
//!
//! - **Lexer**: Converts a raw string query into a stream of tokens (Identifiers, Strings, Punctuation).
//!   Uses an iterative approach to prevent stack overflows on invalid input.
//! - **Parser**: Consumes tokens to build an Abstract Syntax Tree (AST) representing the query and its selection sets.
//! - **Execution Engine**: Recursively evaluates the AST against a schema defined by implementing the `ObjectResolver` trait.
//!
//! ### Complexity
//! - Lexical Analysis: O(N) where N is the length of the query string.
//! - Parsing: O(T) where T is the number of tokens.
//! - Execution: O(F) where F is the number of resolved fields in the response.
//!
//! ## Benchmarking Note
//! A full GraphQL server would use criterion to benchmark query parsing vs execution time independently.
//! You can benchmark this locally by generating large nested queries and wrapping execution in `Instant::now()`.
//!
//! ## Footer
//!
//! How this compares to the canonical crate:
//! Unlike `async-graphql` which uses macros and advanced async/await scheduling for parallel field resolution,
//! this implementation is synchronous and strictly sequential. It lacks full schema validation, introspection,
//! variables, fragments, and directives.
//!
//! What's missing vs. production:
//! - Full GraphQL spec compliance (mutations, subscriptions, fragments, variables).
//! - Parallel execution of sibling fields.
//! - Input validation against a typed schema.
//!
//! Suggested next steps:
//! - Add support for parallel field resolution using `async`/`await` and `futures::join_all`.
//! - Implement schema introspection queries.

use std::collections::BTreeMap;

/// Represents a parsed GraphQL Document. In this minimal engine, a document is just one Query.
#[derive(Debug, Clone, PartialEq)]
pub struct Query {
    pub selections: Vec<Field>,
}

/// A single field selection in a query.
#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub name: String,
    pub arguments: BTreeMap<String, Value>,
    pub selections: Vec<Field>,
}

/// JSON-like value used for inputs (arguments) and outputs (resolved data).
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Boolean(bool),
    Int(i64),
    String(String),
    List(Vec<Value>),
    Object(BTreeMap<String, Value>),
}

/// Tokens emitted by the Lexer.
#[derive(Debug, PartialEq)]
pub enum Token {
    Ident(String),
    String(String),
    Int(i64),
    Punct(char),
    Eof,
}

pub struct Lexer<'a> {
    input: &'a str,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self { input }
    }

    // RUST INSIGHT:
    // By returning an iterative stream of tokens, we avoid recursion for skipping whitespace
    // or handling invalid sequences. This prevents stack overflows on deeply malformed inputs.
    pub fn next_token(&mut self) -> Token {
        loop {
            if self.input.is_empty() {
                return Token::Eof;
            }

            let c = self.input.chars().next().unwrap();

            if c.is_whitespace() || c == ',' {
                self.input = &self.input[c.len_utf8()..];
                continue;
            }

            if c == '#' {
                if let Some(newline_idx) = self.input.find('\n') {
                    self.input = &self.input[newline_idx + 1..];
                } else {
                    self.input = "";
                }
                continue;
            }

            if c == '{' || c == '}' || c == '(' || c == ')' || c == ':' {
                self.input = &self.input[c.len_utf8()..];
                return Token::Punct(c);
            }

            if c.is_alphabetic() || c == '_' {
                let end = self
                    .input
                    .char_indices()
                    .take_while(|(_, ch)| ch.is_alphanumeric() || *ch == '_')
                    .last()
                    .map(|(idx, ch)| idx + ch.len_utf8())
                    .unwrap_or(0);

                let ident = self.input[..end].to_string();
                self.input = &self.input[end..];
                return Token::Ident(ident);
            }

            if c.is_ascii_digit() || c == '-' {
                let mut end = if c == '-' { 1 } else { 0 };
                end += self.input[end..]
                    .char_indices()
                    .take_while(|(_, ch)| ch.is_ascii_digit())
                    .last()
                    .map(|(idx, ch)| idx + ch.len_utf8())
                    .unwrap_or(0);

                // GOTCHA:
                // Using standard parse here. In a production engine, you'd want robust error
                // handling for out-of-bounds integers, rather than a default fallback.
                let val: i64 = self.input[..end].parse().unwrap_or(0);
                self.input = &self.input[end..];
                return Token::Int(val);
            }

            if c == '"' {
                self.input = &self.input[c.len_utf8()..]; // skip opening quote
                if let Some(close_idx) = self.input.find('"') {
                    let val = self.input[..close_idx].to_string();
                    self.input = &self.input[close_idx + 1..]; // skip closing quote
                    return Token::String(val);
                } else {
                    // Unclosed string
                    let val = self.input.to_string();
                    self.input = "";
                    return Token::String(val);
                }
            }

            // Skip invalid character iteratively to avoid tail recursion stack overflows
            self.input = &self.input[c.len_utf8()..];
        }
    }
}

pub struct Parser<'a> {
    lexer: Lexer<'a>,
    current: Token,
}

impl<'a> Parser<'a> {
    pub fn new(mut lexer: Lexer<'a>) -> Self {
        let current = lexer.next_token();
        Self { lexer, current }
    }

    fn advance(&mut self) {
        self.current = self.lexer.next_token();
    }

    pub fn parse_query(&mut self) -> Result<Query, String> {
        if self.current != Token::Punct('{') {
            return Err("Expected '{' at start of query".into());
        }
        let selections = self.parse_selection_set()?;
        Ok(Query { selections })
    }

    fn parse_selection_set(&mut self) -> Result<Vec<Field>, String> {
        let mut selections = Vec::new();
        self.advance(); // Consume '{'

        while self.current != Token::Punct('}') && self.current != Token::Eof {
            selections.push(self.parse_field()?);
        }

        if self.current == Token::Punct('}') {
            self.advance();
        } else {
            return Err("Expected '}' to close selection set".into());
        }

        Ok(selections)
    }

    fn parse_field(&mut self) -> Result<Field, String> {
        let name = match &self.current {
            Token::Ident(id) => id.clone(),
            _ => return Err(format!("Expected field name, found {:?}", self.current)),
        };
        self.advance();

        let mut arguments = BTreeMap::new();
        if self.current == Token::Punct('(') {
            self.advance();
            while self.current != Token::Punct(')') && self.current != Token::Eof {
                let arg_name = match &self.current {
                    Token::Ident(id) => id.clone(),
                    _ => return Err("Expected argument name".into()),
                };
                self.advance();

                if self.current != Token::Punct(':') {
                    return Err("Expected ':' after argument name".into());
                }
                self.advance();

                let arg_value = match &self.current {
                    Token::Int(v) => Value::Int(*v),
                    Token::String(v) => Value::String(v.clone()),
                    Token::Ident(id) if id == "true" => Value::Boolean(true),
                    Token::Ident(id) if id == "false" => Value::Boolean(false),
                    Token::Ident(id) if id == "null" => Value::Null,
                    _ => return Err("Expected argument value".into()),
                };
                self.advance();

                arguments.insert(arg_name, arg_value);
            }
            if self.current == Token::Punct(')') {
                self.advance();
            } else {
                return Err("Expected ')' to close arguments".into());
            }
        }

        let mut selections = Vec::new();
        if self.current == Token::Punct('{') {
            selections = self.parse_selection_set()?;
        }

        Ok(Field {
            name,
            arguments,
            selections,
        })
    }
}

pub enum ResolvedValue<'a> {
    Leaf(Value),
    List(Vec<ResolvedValue<'a>>),
    Object(Box<dyn ObjectResolver + 'a>),
}

// RUST INSIGHT:
// By using trait objects (`Box<dyn ObjectResolver>`), we can return heterogeneous
// sub-resolvers from a parent resolver. This models the recursive graph nature of GraphQL.
pub trait ObjectResolver {
    fn resolve_field(
        &self,
        name: &str,
        args: &BTreeMap<String, Value>,
    ) -> Result<ResolvedValue<'_>, String>;
}

pub fn execute_query(query: &Query, root: &dyn ObjectResolver) -> Result<Value, String> {
    execute_selection_set(&query.selections, root)
}

fn execute_selection_set(
    selections: &[Field],
    resolver: &dyn ObjectResolver,
) -> Result<Value, String> {
    let mut result_map = BTreeMap::new();

    for field in selections {
        // PRODUCTION NOTE:
        // A production crate like `async-graphql` would spawn tasks or futures here to resolve
        // sibling fields concurrently. For simplicity, we resolve sequentially.
        let resolved = resolver.resolve_field(&field.name, &field.arguments)?;

        let value = match resolved {
            ResolvedValue::Leaf(v) => {
                if !field.selections.is_empty() {
                    return Err(format!(
                        "Field '{}' is a leaf but selection set provided",
                        field.name
                    ));
                }
                v
            }
            ResolvedValue::List(items) => {
                let mut list_vals = Vec::with_capacity(items.len());
                for item in items {
                    match item {
                        ResolvedValue::Leaf(v) => list_vals.push(v),
                        ResolvedValue::Object(obj) => {
                            if field.selections.is_empty() {
                                return Err(format!(
                                    "Field '{}' is an object but no selection set provided",
                                    field.name
                                ));
                            }
                            list_vals.push(execute_selection_set(&field.selections, obj.as_ref())?);
                        }
                        ResolvedValue::List(_) => return Err("Nested lists not supported".into()),
                    }
                }
                Value::List(list_vals)
            }
            ResolvedValue::Object(obj) => {
                if field.selections.is_empty() {
                    return Err(format!(
                        "Field '{}' is an object but no selection set provided",
                        field.name
                    ));
                }
                execute_selection_set(&field.selections, obj.as_ref())?
            }
        };

        result_map.insert(field.name.clone(), value);
    }

    Ok(Value::Object(result_map))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct UserResolver {
        id: i64,
        name: String,
    }

    impl ObjectResolver for UserResolver {
        fn resolve_field(
            &self,
            name: &str,
            _args: &BTreeMap<String, Value>,
        ) -> Result<ResolvedValue<'_>, String> {
            match name {
                "id" => Ok(ResolvedValue::Leaf(Value::Int(self.id))),
                "name" => Ok(ResolvedValue::Leaf(Value::String(self.name.clone()))),
                _ => Err(format!("Unknown field {}", name)),
            }
        }
    }

    struct RootResolver;

    impl ObjectResolver for RootResolver {
        fn resolve_field(
            &self,
            name: &str,
            args: &BTreeMap<String, Value>,
        ) -> Result<ResolvedValue<'_>, String> {
            match name {
                "user" => {
                    let id = match args.get("id") {
                        Some(Value::Int(v)) => *v,
                        _ => return Err("Missing or invalid argument 'id'".into()),
                    };
                    Ok(ResolvedValue::Object(Box::new(UserResolver {
                        id,
                        name: format!("User {}", id),
                    })))
                }
                "users" => Ok(ResolvedValue::List(vec![
                    ResolvedValue::Object(Box::new(UserResolver {
                        id: 1,
                        name: "Alice".into(),
                    })),
                    ResolvedValue::Object(Box::new(UserResolver {
                        id: 2,
                        name: "Bob".into(),
                    })),
                ])),
                _ => Err(format!("Unknown field {}", name)),
            }
        }
    }

    #[test]
    fn test_lexer() {
        let mut lexer = Lexer::new("{ user(id: 42, active: true) { id name } }");
        assert_eq!(lexer.next_token(), Token::Punct('{'));
        assert_eq!(lexer.next_token(), Token::Ident("user".into()));
        assert_eq!(lexer.next_token(), Token::Punct('('));
        assert_eq!(lexer.next_token(), Token::Ident("id".into()));
        assert_eq!(lexer.next_token(), Token::Punct(':'));
        assert_eq!(lexer.next_token(), Token::Int(42));
        assert_eq!(lexer.next_token(), Token::Ident("active".into()));
        assert_eq!(lexer.next_token(), Token::Punct(':'));
        assert_eq!(lexer.next_token(), Token::Ident("true".into()));
        assert_eq!(lexer.next_token(), Token::Punct(')'));
        assert_eq!(lexer.next_token(), Token::Punct('{'));
        assert_eq!(lexer.next_token(), Token::Ident("id".into()));
        assert_eq!(lexer.next_token(), Token::Ident("name".into()));
        assert_eq!(lexer.next_token(), Token::Punct('}'));
        assert_eq!(lexer.next_token(), Token::Punct('}'));
        assert_eq!(lexer.next_token(), Token::Eof);
    }

    #[test]
    fn test_parser_and_execution() {
        let query_str = "{ user(id: 100) { id name } }";
        let mut parser = Parser::new(Lexer::new(query_str));
        let query = parser.parse_query().unwrap();

        let root = RootResolver;
        let result = execute_query(&query, &root).unwrap();

        let mut expected_user = BTreeMap::new();
        expected_user.insert("id".into(), Value::Int(100));
        expected_user.insert("name".into(), Value::String("User 100".into()));

        let mut expected = BTreeMap::new();
        expected.insert("user".into(), Value::Object(expected_user));

        assert_eq!(result, Value::Object(expected));
    }

    #[test]
    fn test_execution_list() {
        let query_str = "{ users { name } }";
        let mut parser = Parser::new(Lexer::new(query_str));
        let query = parser.parse_query().unwrap();

        let root = RootResolver;
        let result = execute_query(&query, &root).unwrap();

        let mut user1 = BTreeMap::new();
        user1.insert("name".into(), Value::String("Alice".into()));
        let mut user2 = BTreeMap::new();
        user2.insert("name".into(), Value::String("Bob".into()));

        let mut expected = BTreeMap::new();
        expected.insert(
            "users".into(),
            Value::List(vec![Value::Object(user1), Value::Object(user2)]),
        );

        assert_eq!(result, Value::Object(expected));
    }

    #[test]
    fn test_parser_errors() {
        let mut parser = Parser::new(Lexer::new("{ user(id: ) }"));
        assert!(parser.parse_query().is_err());

        let mut parser2 = Parser::new(Lexer::new("{ user(id: 42"));
        assert!(parser2.parse_query().is_err());
    }
}
