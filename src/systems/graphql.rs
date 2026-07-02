//! GraphQL Execution Engine
//!
//! # What this implements and what crate(s) it replaces
//! This is a minimal GraphQL execution engine, complete with a custom lexer, parser, and
//! a recursive, trait-based resolver system. It replaces heavy-duty GraphQL crates
//! like `async-graphql` and `juniper` for educational purposes or highly embedded contexts.
//!
//! # Real-world usage
//! GraphQL is heavily utilized in modern API gateways, mobile backend-as-a-service providers
//! (like Apollo, Relay), and public APIs (GitHub, Shopify) to allow clients to request
//! exactly the data they need, reducing over-fetching and under-fetching.
//!
//! # Why build it yourself?
//! Building a GraphQL engine from scratch teaches you how to construct a robust parser
//! without relying on external parser-combinator libraries. It also exposes the deep
//! complexities of dynamic dispatch (`Box<dyn Trait>`) and recursive evaluation in Rust's
//! strict type system when mapping a query AST against unknown data sources.
//!
//! # Architecture
//! ```text
//! Query String -> [Lexer] -> Tokens -> [Parser] -> AST (Document) -> [Executor] + Resolvers -> JSON String
//! ```
//!
//! The engine operates in three distinct phases:
//! 1. **Lexical Analysis**: An iterative lexer tokenizes the raw query string into a stream of tokens.
//!    We avoid tail recursion here to prevent stack overflows on malformed inputs.
//! 2. **Parsing**: A recursive descent parser converts the tokens into an Abstract Syntax Tree (AST),
//!    representing the hierarchical structure of the query.
//! 3. **Execution**: The executor walks the AST, recursively calling `.resolve()` on objects that
//!    implement the `Resolver` trait. It uses owned data types (`String`) for evaluated output
//!    to avoid complicated lifetime issues when dealing with trait objects.
//!
//! # Complexity
//! | Phase         | Time Complexity | Space Complexity |
//! |---------------|-----------------|------------------|
//! | Lexer         | O(N)            | O(N)             |
//! | Parser        | O(N)            | O(N)             |
//! | Execution     | O(R)            | O(R)             |
//! *Where N is the query length, and R is the number of resolved fields.*
//!
//! # Invariants
//! - The lexer must always terminate, gracefully handling unrecognized characters.
//! - The parser must correctly nest selection sets based on braces.
//! - The execution engine must never panic, returning `null` or an error message for missing fields.
//!
//! # Benchmarking Note
//! In a real environment, performance would be measured using Criterion (`cargo bench`),
//! testing query parsing speed and deep resolver execution time against various dataset sizes.
//!
//! # Footer
//!
//! # Crates Replaced
//! - `async-graphql`
//! - `juniper`
//!
//! # Missing Features vs. Production
//! - Complete lack of type checking or schema validation. A real GraphQL server verifies queries against a strictly typed schema before executing.
//! - Does not support Arguments, Variables, Fragments, Mutations, Subscriptions, or Directives.
//! - Errors are ignored or serialized as `null`, instead of returning a formal GraphQL `errors` array.
//! - Synchronous execution. Production crates evaluate non-dependent fields concurrently using `async`/`await`.
//!
//! # Next Steps
//! - Introduce a `Schema` struct to validate queries before execution.
//! - Add support for Field Arguments (e.g., `user(id: "123") { name }`).
//! - Implement an asynchronous execution model.

use std::fmt::Write;

/// Represents the tokens produced by the lexer.
#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    Ident(String),
    LBrace,
    RBrace,
    Eof,
    // Note: A full implementation would include arguments, variables, fragments, etc.
}

/// The lexer for our minimal GraphQL language.
pub struct Lexer<'a> {
    input: &'a [u8],
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            // RUST INSIGHT: Working with bytes is zero-cost for ASCII structures.
            input: input.as_bytes(),
            pos: 0,
        }
    }

    /// Fetches the next token from the input.
    pub fn next_token(&mut self) -> Token {
        // GOTCHA: Using an iterative loop instead of tail recursion to skip whitespace
        // prevents stack overflow on inputs with massive amounts of whitespace or invalid chars.
        loop {
            self.skip_whitespace();
            if self.pos >= self.input.len() {
                return Token::Eof;
            }

            let ch = self.input[self.pos];

            match ch {
                b'{' => {
                    self.pos += 1;
                    return Token::LBrace;
                }
                b'}' => {
                    self.pos += 1;
                    return Token::RBrace;
                }
                _ if ch.is_ascii_alphabetic() || ch == b'_' => {
                    return self.read_ident();
                }
                _ => {
                    // Skip invalid characters instead of panicking
                    self.pos += 1;
                }
            }
        }
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.input.len() {
            let ch = self.input[self.pos];
            if ch == b' ' || ch == b'\t' || ch == b'\n' || ch == b'\r' || ch == b',' {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn read_ident(&mut self) -> Token {
        let start = self.pos;
        while self.pos < self.input.len() {
            let ch = self.input[self.pos];
            if ch.is_ascii_alphanumeric() || ch == b'_' {
                self.pos += 1;
            } else {
                break;
            }
        }

        // UNSAFE JUSTIFICATION: Safe because we only advanced on valid ASCII alphanumeric/underscore.
        let ident = unsafe { std::str::from_utf8_unchecked(&self.input[start..self.pos]) };
        Token::Ident(ident.to_string())
    }
}

/// AST Representation of a Field.
#[derive(Debug, PartialEq)]
pub struct Field {
    pub name: String,
    pub selection_set: Vec<Field>,
}

/// A minimal GraphQL parser.
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

    fn next(&mut self) {
        self.current_token = self.lexer.next_token();
    }

    /// Parses the query into a list of root fields (a selection set).
    pub fn parse(&mut self) -> Result<Vec<Field>, String> {
        // Skip an optional "query" keyword
        if let Token::Ident(ref name) = self.current_token {
            if name == "query" || name == "mutation" {
                self.next();
            }
        }

        // If the query starts with `{`, it's an anonymous query.
        if self.current_token == Token::LBrace {
            self.parse_selection_set()
        } else {
            // Standard GraphQL requires a block for root fields if unnamed.
            self.parse_selection_set()
        }
    }

    fn parse_selection_set(&mut self) -> Result<Vec<Field>, String> {
        let mut fields = Vec::new();

        if self.current_token != Token::LBrace {
            return Err("Expected { at start of selection set".to_string());
        }
        self.next(); // Consume '{'

        while self.current_token != Token::RBrace && self.current_token != Token::Eof {
            fields.push(self.parse_field()?);
        }

        if self.current_token != Token::RBrace {
            return Err("Expected } at end of selection set".to_string());
        }
        self.next(); // Consume '}'

        Ok(fields)
    }

    fn parse_field(&mut self) -> Result<Field, String> {
        let name = match &self.current_token {
            Token::Ident(n) => n.clone(),
            _ => {
                return Err(format!(
                    "Expected field name, found {:?}",
                    self.current_token
                ));
            }
        };
        self.next(); // Consume field name

        let mut selection_set = Vec::new();
        if self.current_token == Token::LBrace {
            selection_set = self.parse_selection_set()?;
        }

        Ok(Field {
            name,
            selection_set,
        })
    }
}

/// A dynamically resolvable value.
pub enum ResolvedValue {
    String(String),
    Int(i64),
    Float(f64),
    Boolean(bool),
    // PRODUCTION NOTE: We use Box<dyn Resolver> to allow dynamic dispatch over arbitrary user types.
    // In canonical crates, this is often handled via macros generating static dispatch maps,
    // which is significantly more performant but complex to implement.
    Object(Box<dyn Resolver>),
    List(Vec<ResolvedValue>),
    Null,
}

/// The core trait that any data-providing object must implement.
pub trait Resolver {
    fn resolve(&self, field_name: &str) -> Option<ResolvedValue>;
}

/// Executes an AST against a root resolver, returning a JSON-like string.
pub fn execute(ast: &[Field], root: &dyn Resolver) -> String {
    let mut output = String::new();
    output.push_str("{");
    execute_selection_set(ast, root, &mut output);
    output.push_str("}");
    output
}

fn execute_selection_set(fields: &[Field], resolver: &dyn Resolver, output: &mut String) {
    for (i, field) in fields.iter().enumerate() {
        if i > 0 {
            output.push_str(",");
        }

        // Write key
        write!(output, "\"{}\":", field.name).unwrap();

        // Resolve value
        match resolver.resolve(&field.name) {
            Some(value) => serialize_value(&value, &field.selection_set, output),
            None => output.push_str("null"),
        }
    }
}

fn serialize_value(value: &ResolvedValue, selection_set: &[Field], output: &mut String) {
    match value {
        ResolvedValue::String(s) => {
            write!(output, "\"{}\"", s.replace("\"", "\\\"")).unwrap();
        }
        ResolvedValue::Int(n) => {
            write!(output, "{}", n).unwrap();
        }
        ResolvedValue::Float(f) => {
            write!(output, "{}", f).unwrap();
        }
        ResolvedValue::Boolean(b) => {
            write!(output, "{}", b).unwrap();
        }
        ResolvedValue::Null => {
            output.push_str("null");
        }
        ResolvedValue::Object(obj_resolver) => {
            if selection_set.is_empty() {
                // If we hit an object but no selection set was provided, return empty.
                output.push_str("{}");
            } else {
                output.push_str("{");
                execute_selection_set(selection_set, obj_resolver.as_ref(), output);
                output.push_str("}");
            }
        }
        ResolvedValue::List(list) => {
            output.push_str("[");
            for (i, item) in list.iter().enumerate() {
                if i > 0 {
                    output.push_str(",");
                }
                serialize_value(item, selection_set, output);
            }
            output.push_str("]");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct User {
        id: i64,
        name: String,
        is_active: bool,
    }

    impl Resolver for User {
        fn resolve(&self, field_name: &str) -> Option<ResolvedValue> {
            match field_name {
                "id" => Some(ResolvedValue::Int(self.id)),
                "name" => Some(ResolvedValue::String(self.name.clone())),
                "isActive" => Some(ResolvedValue::Boolean(self.is_active)),
                _ => None,
            }
        }
    }

    struct RootQuery;

    impl Resolver for RootQuery {
        fn resolve(&self, field_name: &str) -> Option<ResolvedValue> {
            match field_name {
                "version" => Some(ResolvedValue::String("1.0".to_string())),
                "currentUser" => {
                    let user = User {
                        id: 42,
                        name: "Alice".to_string(),
                        is_active: true,
                    };
                    Some(ResolvedValue::Object(Box::new(user)))
                }
                "users" => {
                    let list = vec![
                        ResolvedValue::Object(Box::new(User {
                            id: 1,
                            name: "Bob".to_string(),
                            is_active: false,
                        })),
                        ResolvedValue::Object(Box::new(User {
                            id: 2,
                            name: "Charlie".to_string(),
                            is_active: true,
                        })),
                    ];
                    Some(ResolvedValue::List(list))
                }
                _ => None,
            }
        }
    }

    #[test]
    fn test_lexer() {
        let mut lexer = Lexer::new("query { user_name }");
        assert_eq!(lexer.next_token(), Token::Ident("query".to_string()));
        assert_eq!(lexer.next_token(), Token::LBrace);
        assert_eq!(lexer.next_token(), Token::Ident("user_name".to_string()));
        assert_eq!(lexer.next_token(), Token::RBrace);
        assert_eq!(lexer.next_token(), Token::Eof);
    }

    #[test]
    fn test_parser() {
        let lexer = Lexer::new("{ currentUser { id name } }");
        let mut parser = Parser::new(lexer);
        let ast = parser.parse().unwrap();

        assert_eq!(ast.len(), 1);
        assert_eq!(ast[0].name, "currentUser");
        assert_eq!(ast[0].selection_set.len(), 2);
        assert_eq!(ast[0].selection_set[0].name, "id");
        assert_eq!(ast[0].selection_set[1].name, "name");
    }

    #[test]
    fn test_execution() {
        let query = "
        {
            version
            currentUser {
                name
                isActive
                missingField
            }
        }
        ";

        let lexer = Lexer::new(query);
        let mut parser = Parser::new(lexer);
        let ast = parser.parse().unwrap();

        let root = RootQuery;
        let json = execute(&ast, &root);

        assert_eq!(
            json,
            r#"{"version":"1.0","currentUser":{"name":"Alice","isActive":true,"missingField":null}}"#
        );
    }

    #[test]
    fn test_execution_lists() {
        let query = "{ users { name } }";
        let lexer = Lexer::new(query);
        let mut parser = Parser::new(lexer);
        let ast = parser.parse().unwrap();

        let root = RootQuery;
        let json = execute(&ast, &root);

        assert_eq!(json, r#"{"users":[{"name":"Bob"},{"name":"Charlie"}]}"#);
    }

    #[test]
    fn test_parser_error() {
        let lexer = Lexer::new("{ currentUser { id name ");
        let mut parser = Parser::new(lexer);
        let result = parser.parse();
        assert!(result.is_err());
    }
}
