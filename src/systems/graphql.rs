//! # GraphQL Execution Engine
//!
//! **Replaces:** `async-graphql`, `juniper`
//!
//! **Real-world systems:** Apollo Server, GitHub API, Shopify API.
//!
//! **Why build it yourself?** To understand the complexity of parsing and executing GraphQL queries,
//! how resolvers map to fields, and how the AST is traversed during execution.
//!
//! ## Architecture
//!
//! The GraphQL engine consists of three main components:
//! 1. **Lexer**: Tokenizes the raw query string into tokens (identifiers, scalars, punctuation). Uses iterative loops instead of tail recursion.
//! 2. **Parser**: Converts tokens into an Abstract Syntax Tree (AST) representing queries, fields, and arguments.
//! 3. **Executor**: Traverses the AST and resolves fields against a root object using trait-based resolvers.
//!
//! ### Complexity
//! | Operation | Time | Space |
//! |-----------|------|-------|
//! | Lexing    | O(N) | O(N)  |
//! | Parsing   | O(N) | O(N)  |
//! | Execution | O(N) | O(D)  |
//! Where N is the query length, and D is the query depth.
//!
//! ### Design Decisions
//! - **Iterative Lexer**: Prevents stack overflows on deep or malformed input.
//! - **Owned Values**: Uses owned `Value` types (e.g., `Value::String(String)`) instead of borrows to avoid lifetime issues with dynamic resolvers.
//! - **Trait-based Resolvers**: Allows dynamic resolution via `dyn Resolver` and simplifies returning heterogeneous data.
//!
//! ## Comparison to Canonical Crates
//! - `async-graphql` and `juniper` are fully spec-compliant, handling validation,
//!   introspection, directives, fragments, and complex types.
//! - This implementation is a minimal subset demonstrating lexing, recursive descent parsing,
//!   and trait-based dispatch.
//!
//! ## Missing vs. Production
//! - **Validation**: We do not validate against a GraphQL schema before execution.
//! - **Variables**: Query variables are not supported.
//! - **Fragments**: Inline and named fragments are ignored.
//! - **Async Resolvers**: Real GraphQL engines use async/await for I/O bound resolvers.
//!
//! ## Next Steps
//! 1. Add schema definition and validation.
//! 2. Make resolvers async.
//! 3. Add support for lists in the executor.
//!
//! ## Benchmarking Note
//! Lexing and parsing can be benchmarked using `criterion` by measuring the time taken to parse
//! large, deeply nested queries, ensuring the iterative lexer performs competitively without stack overflows.

use std::collections::HashMap;

// ==========================================
// Lexer
// ==========================================

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Ident(String),
    String(String),
    Int(i64),
    Float(f64),
    Punctuation(char),
    Eof,
}

pub struct Lexer<'a> {
    _input: &'a str,
    chars: std::str::Chars<'a>,
    current: Option<char>,
}

impl<'a> Lexer<'a> {
    #[must_use]
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

    pub fn next_token(&mut self) -> Token {
        // RUST INSIGHT: Using `loop` avoids tail-recursion stack overflows that
        // could occur with deeply nested or malformed input in recursive parsers.
        loop {
            match self.current {
                Some(c) if c.is_whitespace() || c == ',' => {
                    self.advance();
                }
                Some('#') => {
                    // Skip comment
                    while let Some(c) = self.current {
                        if c == '\n' {
                            break;
                        }
                        self.advance();
                    }
                }
                Some(c) if c.is_ascii_alphabetic() || c == '_' => {
                    let mut ident = String::new();
                    while let Some(c) = self.current {
                        if c.is_ascii_alphanumeric() || c == '_' {
                            ident.push(c);
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    return Token::Ident(ident);
                }
                Some(c) if c.is_ascii_digit() || c == '-' => {
                    let mut num = String::new();
                    let mut is_float = false;
                    if c == '-' {
                        num.push(c);
                        self.advance();
                    }
                    while let Some(c) = self.current {
                        if c.is_ascii_digit() {
                            num.push(c);
                            self.advance();
                        } else if c == '.' && !is_float {
                            is_float = true;
                            num.push(c);
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    if is_float {
                        return Token::Float(num.parse().unwrap_or(0.0));
                    } else {
                        return Token::Int(num.parse().unwrap_or(0));
                    }
                }
                Some('"') => {
                    self.advance(); // skip quote
                    let mut s = String::new();
                    while let Some(c) = self.current {
                        if c == '"' {
                            self.advance();
                            break;
                        }
                        s.push(c);
                        self.advance();
                    }
                    return Token::String(s);
                }
                Some(c) => {
                    // Punctuation like {, }, (, ), :, !
                    self.advance();
                    return Token::Punctuation(c);
                }
                None => return Token::Eof,
            }
        }
    }
}

// ==========================================
// AST & Parser
// ==========================================

#[derive(Debug, Clone, PartialEq)]
pub enum ASTNode {
    Document(Vec<Operation>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Operation {
    pub operation_type: String, // e.g., "query", "mutation"
    pub name: Option<String>,
    pub selection_set: Vec<Selection>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Selection {
    pub name: String,
    pub alias: Option<String>,
    pub arguments: HashMap<String, Value>,
    pub selection_set: Vec<Selection>,
}

// Owned value type to avoid lifetime issues
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Int(i64),
    Float(f64),
    String(String),
    Boolean(bool),
    Enum(String),
    List(Vec<Value>),
    Object(HashMap<String, Value>),
}

pub struct Parser<'a> {
    lexer: Lexer<'a>,
    current_token: Token,
}

impl<'a> Parser<'a> {
    #[must_use]
    pub fn new(mut lexer: Lexer<'a>) -> Self {
        let current_token = lexer.next_token();
        Self { lexer, current_token }
    }

    fn advance(&mut self) {
        self.current_token = self.lexer.next_token();
    }

    fn expect_punctuation(&mut self, expected: char) -> Result<(), String> {
        match &self.current_token {
            Token::Punctuation(c) if *c == expected => {
                self.advance();
                Ok(())
            }
            _ => Err(format!("Expected punctuation '{}'", expected)),
        }
    }

    fn parse_ident(&mut self) -> Result<String, String> {
        match &self.current_token {
            Token::Ident(name) => {
                let name = name.clone();
                self.advance();
                Ok(name)
            }
            _ => Err("Expected identifier".to_string()),
        }
    }

    pub fn parse_document(&mut self) -> Result<ASTNode, String> {
        let mut operations = Vec::new();
        while self.current_token != Token::Eof {
            operations.push(self.parse_operation()?);
        }
        Ok(ASTNode::Document(operations))
    }

    fn parse_operation(&mut self) -> Result<Operation, String> {
        let mut operation_type = "query".to_string();
        let mut name = None;

        if let Token::Ident(ident) = &self.current_token {
            if ident == "query" || ident == "mutation" {
                operation_type = ident.clone();
                self.advance();
                if let Token::Ident(op_name) = &self.current_token {
                    name = Some(op_name.clone());
                    self.advance();
                }
            }
        }

        let selection_set = self.parse_selection_set()?;
        Ok(Operation {
            operation_type,
            name,
            selection_set,
        })
    }

    fn parse_selection_set(&mut self) -> Result<Vec<Selection>, String> {
        let mut selections = Vec::new();
        self.expect_punctuation('{')?;
        while self.current_token != Token::Punctuation('}') && self.current_token != Token::Eof {
            selections.push(self.parse_selection()?);
        }
        self.expect_punctuation('}')?;
        Ok(selections)
    }

    fn parse_selection(&mut self) -> Result<Selection, String> {
        let mut name = self.parse_ident()?;
        let mut alias = None;

        if let Token::Punctuation(':') = &self.current_token {
            self.advance();
            alias = Some(name);
            name = self.parse_ident()?;
        }

        let mut arguments = HashMap::new();
        if let Token::Punctuation('(') = &self.current_token {
            self.advance();
            while self.current_token != Token::Punctuation(')') && self.current_token != Token::Eof {
                let arg_name = self.parse_ident()?;
                self.expect_punctuation(':')?;
                let arg_value = self.parse_value()?;
                arguments.insert(arg_name, arg_value);
            }
            self.expect_punctuation(')')?;
        }

        let mut selection_set = Vec::new();
        if let Token::Punctuation('{') = &self.current_token {
            selection_set = self.parse_selection_set()?;
        }

        Ok(Selection {
            name,
            alias,
            arguments,
            selection_set,
        })
    }

    fn parse_value(&mut self) -> Result<Value, String> {
        // GOTCHA: Properly handling multiple value types requires thorough token matching.
        match &self.current_token {
            Token::Int(i) => {
                let val = Value::Int(*i);
                self.advance();
                Ok(val)
            }
            Token::Float(f) => {
                let val = Value::Float(*f);
                self.advance();
                Ok(val)
            }
            Token::String(s) => {
                let val = Value::String(s.clone());
                self.advance();
                Ok(val)
            }
            Token::Ident(s) => {
                let val = if s == "true" {
                    Value::Boolean(true)
                } else if s == "false" {
                    Value::Boolean(false)
                } else if s == "null" {
                    Value::Null
                } else {
                    Value::Enum(s.clone())
                };
                self.advance();
                Ok(val)
            }
            _ => Err("Expected value".to_string()),
        }
    }
}

// ==========================================
// Execution Engine
// ==========================================

pub trait Resolver {
    fn resolve(&self, field: &str, args: &HashMap<String, Value>) -> Result<Option<Value>, String>;
    fn resolve_object(&self, field: &str) -> Result<Option<Box<dyn Resolver>>, String> {
        let _ = field;
        Ok(None)
    }
}

#[derive(Default)]
pub struct Executor;

impl Executor {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    pub fn execute(
        &self,
        query: &str,
        root_resolver: &dyn Resolver,
    ) -> Result<Value, String> {
        let lexer = Lexer::new(query);
        let mut parser = Parser::new(lexer);
        let ast = parser.parse_document()?;

        let ASTNode::Document(operations) = ast;
        if operations.is_empty() {
            return Err("No operations found".to_string());
        }
        let op = &operations[0];
        self.execute_selection_set(&op.selection_set, root_resolver)
    }

    fn execute_selection_set(
        &self,
        selections: &[Selection],
        resolver: &dyn Resolver,
    ) -> Result<Value, String> {
        let mut result_map = HashMap::new();

        for selection in selections {
            let key = selection.alias.as_ref().unwrap_or(&selection.name).clone();

            if !selection.selection_set.is_empty() {
                // Resolve nested object
                if let Some(child_resolver) = resolver.resolve_object(&selection.name)? {
                    let child_result = self.execute_selection_set(&selection.selection_set, child_resolver.as_ref())?;
                    result_map.insert(key, child_result);
                } else {
                    result_map.insert(key, Value::Null);
                }
            } else {
                // Resolve scalar/leaf field
                let val = resolver.resolve(&selection.name, &selection.arguments)?;
                result_map.insert(key, val.unwrap_or(Value::Null));
            }
        }

        Ok(Value::Object(result_map))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct UserResolver {
        id: i64,
        name: String,
    }

    impl Resolver for UserResolver {
        fn resolve(&self, field: &str, _args: &HashMap<String, Value>) -> Result<Option<Value>, String> {
            match field {
                "id" => Ok(Some(Value::Int(self.id))),
                "name" => Ok(Some(Value::String(self.name.clone()))),
                _ => Ok(None),
            }
        }
    }

    struct RootResolver;

    impl Resolver for RootResolver {
        fn resolve(&self, field: &str, args: &HashMap<String, Value>) -> Result<Option<Value>, String> {
            match field {
                "version" => Ok(Some(Value::String("1.0.0".to_string()))),
                "greet" => {
                    if let Some(Value::String(name)) = args.get("name") {
                        Ok(Some(Value::String(format!("Hello, {}!", name))))
                    } else {
                        Ok(Some(Value::String("Hello, World!".to_string())))
                    }
                }
                _ => Ok(None),
            }
        }

        fn resolve_object(&self, field: &str) -> Result<Option<Box<dyn Resolver>>, String> {
            match field {
                "user" => Ok(Some(Box::new(UserResolver {
                    id: 1,
                    name: "Alice".to_string(),
                }))),
                _ => Ok(None),
            }
        }
    }

    #[test]
    fn test_lexer() {
        let mut lexer = Lexer::new("query { user(id: 1) { name } }");
        assert_eq!(lexer.next_token(), Token::Ident("query".to_string()));
        assert_eq!(lexer.next_token(), Token::Punctuation('{'));
        assert_eq!(lexer.next_token(), Token::Ident("user".to_string()));
        assert_eq!(lexer.next_token(), Token::Punctuation('('));
        assert_eq!(lexer.next_token(), Token::Ident("id".to_string()));
        assert_eq!(lexer.next_token(), Token::Punctuation(':'));
        assert_eq!(lexer.next_token(), Token::Int(1));
        assert_eq!(lexer.next_token(), Token::Punctuation(')'));
        assert_eq!(lexer.next_token(), Token::Punctuation('{'));
        assert_eq!(lexer.next_token(), Token::Ident("name".to_string()));
        assert_eq!(lexer.next_token(), Token::Punctuation('}'));
        assert_eq!(lexer.next_token(), Token::Punctuation('}'));
        assert_eq!(lexer.next_token(), Token::Eof);
    }

    #[test]
    fn test_parser() {
        let lexer = Lexer::new("{ user { name } }");
        let mut parser = Parser::new(lexer);
        let ast = parser.parse_document().unwrap();
        let ops = match ast {
            ASTNode::Document(ops) => ops,
        };
        assert_eq!(ops.len(), 1);
        let op = &ops[0];
        assert_eq!(op.operation_type, "query");
        assert_eq!(op.selection_set.len(), 1);
        assert_eq!(op.selection_set[0].name, "user");
        assert_eq!(op.selection_set[0].selection_set.len(), 1);
        assert_eq!(op.selection_set[0].selection_set[0].name, "name");
    }

    #[test]
    fn test_execution_scalar() {
        let executor = Executor::new();
        let root = RootResolver;
        let query = "{ version greet(name: \"Bob\") }";
        let result = executor.execute(query, &root).unwrap();

        if let Value::Object(map) = result {
            assert_eq!(map.get("version"), Some(&Value::String("1.0.0".to_string())));
            assert_eq!(map.get("greet"), Some(&Value::String("Hello, Bob!".to_string())));
        } else {
            panic!("Expected Object");
        }
    }

    #[test]
    fn test_execution_object() {
        let executor = Executor::new();
        let root = RootResolver;
        let query = "{ user { id name } }";
        let result = executor.execute(query, &root).unwrap();

        if let Value::Object(map) = result {
            if let Some(Value::Object(user_map)) = map.get("user") {
                assert_eq!(user_map.get("id"), Some(&Value::Int(1)));
                assert_eq!(user_map.get("name"), Some(&Value::String("Alice".to_string())));
            } else {
                panic!("Expected Object for user");
            }
        } else {
            panic!("Expected Object");
        }
    }

    #[test]
    fn test_execution_alias() {
        let executor = Executor::new();
        let root = RootResolver;
        let query = "{ myUser: user { myId: id } }";
        let result = executor.execute(query, &root).unwrap();

        if let Value::Object(map) = result {
            if let Some(Value::Object(user_map)) = map.get("myUser") {
                assert_eq!(user_map.get("myId"), Some(&Value::Int(1)));
            } else {
                panic!("Expected Object for myUser");
            }
        } else {
            panic!("Expected Object");
        }
    }
}
