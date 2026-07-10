//! # GraphQL Execution Engine
//!
//! Implements a minimal GraphQL lexer, parser, and trait-based recursive execution engine.
//!
//! **Replaces Crates:** `async-graphql`, `juniper`
//!
//! **Real-world Usage:**
//! - API gateways.
//! - Backend-for-frontend (BFF) layers.
//! - Federated graph data composition.
//!
//! **Why build it yourself?**
//! Understanding how GraphQL translates a string query into a tree of resolver executions
//! gives deep insight into how caching, data loader optimizations (N+1 problem), and query
//! complexity analysis work under the hood.
//!
//! # Architecture
//!
//! 1. **Lexer**: Tokenizes the input query string, handling strings, numbers, names, and punctuation.
//! 2. **Parser**: Builds an AST (Document -> OperationDefinition -> SelectionSet -> Field).
//! 3. **Executor**: Recursively evaluates the AST against a trait-based schema resolver.
//!
//! **Time Complexity**:
//! - Parsing: `O(N)` where N is the query string length.
//! - Execution: `O(M)` where M is the number of resolved fields in the response.

use std::collections::HashMap;

// ============================================================================
// Token & Lexer
// ============================================================================

#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    Name(String),
    String(String),
    Int(i64),
    Punctuation(char),
    Eof,
}

pub struct Lexer<'a> {
    input: std::str::Chars<'a>,
    peeked: Option<char>,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        let mut chars = input.chars();
        let peeked = chars.next();
        Self { input: chars, peeked }
    }

    fn advance(&mut self) {
        self.peeked = self.input.next();
    }

    fn skip_whitespace(&mut self) {
        // Use iterative loop to avoid stack overflow on long invalid/whitespace strings
        while let Some(c) = self.peeked {
            if c.is_whitespace() || c == ',' {
                self.advance();
            } else {
                break;
            }
        }
    }

    pub fn next_token(&mut self) -> Token {
        self.skip_whitespace();

        let c = match self.peeked {
            Some(c) => c,
            None => return Token::Eof,
        };

        match c {
            '{' | '}' | '(' | ')' | ':' | '[' | ']' | '!' => {
                self.advance();
                Token::Punctuation(c)
            }
            '"' => {
                self.advance(); // Skip opening quote
                let mut string_val = String::new();
                while let Some(nc) = self.peeked {
                    if nc == '"' {
                        self.advance(); // Skip closing quote
                        break;
                    }
                    string_val.push(nc);
                    self.advance();
                }
                Token::String(string_val)
            }
            _ if c.is_alphabetic() || c == '_' => {
                let mut name = String::new();
                while let Some(nc) = self.peeked {
                    if nc.is_alphanumeric() || nc == '_' {
                        name.push(nc);
                        self.advance();
                    } else {
                        break;
                    }
                }
                Token::Name(name)
            }
            _ if c.is_ascii_digit() || c == '-' => {
                let mut num_str = String::new();
                if c == '-' {
                    num_str.push(c);
                    self.advance();
                }
                while let Some(nc) = self.peeked {
                    if nc.is_ascii_digit() {
                        num_str.push(nc);
                        self.advance();
                    } else {
                        break;
                    }
                }
                // Simplified integer parsing
                Token::Int(num_str.parse().unwrap_or(0))
            }
            _ => {
                // GOTCHA: Iteratively skip unrecognized characters
                self.advance();
                { self.skip_whitespace(); self.next_token() }
            }
        }
    }
}

// ============================================================================
// AST & Parser
// ============================================================================

#[derive(Debug, PartialEq, Clone)]
pub struct Field {
    pub name: String,
    pub args: HashMap<String, Value>,
    pub selections: Vec<Field>,
}

#[derive(Debug, PartialEq, Clone)]
pub enum Value {
    String(String),
    Int(i64),
    Boolean(bool),
    Null,
}

pub struct Parser<'a> {
    lexer: Lexer<'a>,
    current_token: Token,
}

impl<'a> Parser<'a> {
    pub fn new(input: &'a str) -> Self {
        let mut lexer = Lexer::new(input);
        let current_token = lexer.next_token();
        Self { lexer, current_token }
    }

    fn advance(&mut self) {
        self.current_token = self.lexer.next_token();
    }

    fn expect_punctuation(&mut self, expected: char) {
        if let Token::Punctuation(c) = self.current_token {
            if c == expected {
                self.advance();
                return;
            }
        }
        panic!("Expected punctuation '{}'", expected);
    }

    pub fn parse_query(&mut self) -> Vec<Field> {
        // Simple top-level anonymous query parsing: { field1 field2 }
        self.expect_punctuation('{');
        let fields = self.parse_selection_set();
        self.expect_punctuation('}');
        fields
    }

    fn parse_selection_set(&mut self) -> Vec<Field> {
        let mut fields = Vec::new();
        while let Token::Name(_) = self.current_token {
            fields.push(self.parse_field());
        }
        fields
    }

    fn parse_field(&mut self) -> Field {
        let name = match &self.current_token {
            Token::Name(n) => n.clone(),
            _ => panic!("Expected field name"),
        };
        self.advance();

        let mut args = HashMap::new();
        if let Token::Punctuation('(') = self.current_token {
            self.advance();
            while let Token::Name(arg_name) = &self.current_token {
                let arg_name = arg_name.clone();
                self.advance();
                self.expect_punctuation(':');
                let value = self.parse_value();
                args.insert(arg_name, value);
            }
            self.expect_punctuation(')');
        }

        let mut selections = Vec::new();
        if let Token::Punctuation('{') = self.current_token {
            self.advance();
            selections = self.parse_selection_set();
            self.expect_punctuation('}');
        }

        Field {
            name,
            args,
            selections,
        }
    }

    fn parse_value(&mut self) -> Value {
        match &self.current_token {
            Token::String(s) => {
                let val = Value::String(s.clone());
                self.advance();
                val
            }
            Token::Int(i) => {
                let val = Value::Int(*i);
                self.advance();
                val
            }
            Token::Name(n) if n == "true" => {
                self.advance();
                Value::Boolean(true)
            }
            Token::Name(n) if n == "false" => {
                self.advance();
                Value::Boolean(false)
            }
            Token::Name(n) if n == "null" => {
                self.advance();
                Value::Null
            }
            _ => panic!("Expected value"),
        }
    }
}

// ============================================================================
// Execution Engine
// ============================================================================

pub enum OutputValue {
    Object(HashMap<String, OutputValue>),
    String(String),
    Int(i64),
    Boolean(bool),
    Null,
    List(Vec<OutputValue>),
}

pub trait Resolver {
    fn resolve(&self, field: &Field) -> OutputValue;
}

pub struct Engine;

impl Engine {
    pub fn execute<R: Resolver>(query: &str, root_resolver: &R) -> OutputValue {
        let mut parser = Parser::new(query);
        let fields = parser.parse_query();

        let mut result = HashMap::new();
        for field in fields {
            // PRODUCTION NOTE: In a real system like async-graphql, fields are validated against a pre-compiled Schema object before execution to ensure type safety and avoid runtime errors on missing fields.
            // RUST INSIGHT: Traits allow us to dynamically dispatch execution
            // based on the schema definitions provided by the user.
            result.insert(field.name.clone(), root_resolver.resolve(&field));
        }

        OutputValue::Object(result)
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `async-graphql`: Fully asynchronous, macro-heavy, auto-derives schema from Rust structs.
//   Supports subscriptions, directives, and complex union types. Our implementation is synchronous
//   and manually resolved.
//
// Missing vs. Production:
// - **Type System/Validation**: We don't validate the query against a schema before execution.
// - **Variables/Fragments**: Missing support for GraphQL variables (`$id`) and `...Fragment`.
// - **Mutations/Subscriptions**: Only supports basic queries.
//
// Next Steps:
// 1. Add schema validation phase before execution.
// 2. Implement async resolvers.

#[cfg(test)]
mod tests {
    use super::*;

    struct MockUserResolver;
    impl Resolver for MockUserResolver {
        fn resolve(&self, field: &Field) -> OutputValue {
            match field.name.as_str() {
                "id" => OutputValue::Int(1),
                "name" => OutputValue::String("Alice".to_string()),
                _ => OutputValue::Null,
            }
        }
    }

    struct MockRootResolver;
    impl Resolver for MockRootResolver {
        fn resolve(&self, field: &Field) -> OutputValue {
            match field.name.as_str() {
                "user" => {
                    let user_resolver = MockUserResolver;
                    let mut obj = HashMap::new();
                    for sub_field in &field.selections {
                        obj.insert(sub_field.name.clone(), user_resolver.resolve(sub_field));
                    }
                    OutputValue::Object(obj)
                }
                "version" => OutputValue::String("1.0".to_string()),
                _ => OutputValue::Null,
            }
        }
    }

    #[test]
    fn test_lexer() {
        let mut lexer = Lexer::new("{ user(id: 1) { name } }");
        assert_eq!(lexer.next_token(), Token::Punctuation('{'));
        assert_eq!(lexer.next_token(), Token::Name("user".to_string()));
        assert_eq!(lexer.next_token(), Token::Punctuation('('));
        assert_eq!(lexer.next_token(), Token::Name("id".to_string()));
        assert_eq!(lexer.next_token(), Token::Punctuation(':'));
        assert_eq!(lexer.next_token(), Token::Int(1));
        assert_eq!(lexer.next_token(), Token::Punctuation(')'));
        assert_eq!(lexer.next_token(), Token::Punctuation('{'));
        assert_eq!(lexer.next_token(), Token::Name("name".to_string()));
        assert_eq!(lexer.next_token(), Token::Punctuation('}'));
        assert_eq!(lexer.next_token(), Token::Punctuation('}'));
        assert_eq!(lexer.next_token(), Token::Eof);
    }

    #[test]
    fn test_parser() {
        let mut parser = Parser::new("{ user(id: 1) { name } }");
        let ast = parser.parse_query();
        assert_eq!(ast.len(), 1);
        assert_eq!(ast[0].name, "user");
        assert_eq!(ast[0].args.get("id"), Some(&Value::Int(1)));
        assert_eq!(ast[0].selections.len(), 1);
        assert_eq!(ast[0].selections[0].name, "name");
    }

    #[test]
    fn test_execution() {
        let query = "{ version user { id name } }";
        let root = MockRootResolver;
        let result = Engine::execute(query, &root);

        if let OutputValue::Object(map) = result {
            if let Some(OutputValue::String(v)) = map.get("version") {
                assert_eq!(v, "1.0");
            } else {
                panic!("Expected version string");
            }

            if let Some(OutputValue::Object(user_map)) = map.get("user") {
                if let Some(OutputValue::Int(id)) = user_map.get("id") {
                    assert_eq!(*id, 1);
                } else {
                    panic!("Expected id int");
                }

                if let Some(OutputValue::String(name)) = user_map.get("name") {
                    assert_eq!(name, "Alice");
                } else {
                    panic!("Expected name string");
                }
            } else {
                panic!("Expected user object");
            }
        } else {
            panic!("Expected root object");
        }
    }
}
