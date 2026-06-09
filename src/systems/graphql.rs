//! # GraphQL Execution Engine
//!
//! Implements a dynamic GraphQL execution engine from scratch.
//!
//! **Replaces Crates:** `async-graphql`, `juniper`
//!
//! **Real-world Usage:**
//! - Core routing logic in API gateways.
//! - Backend-for-frontend (BFF) layers aggregating microservices.
//! - Graph databases exposing traversal APIs.
//!
//! **Why build it yourself?**
//! Building a GraphQL engine demystifies how declarative queries are mapped to
//! imperative backend calls. It forces you to build an AST, handle dynamic type
//! resolution (unlike Rust's static typing), and manage the 'N+1 problem' conceptually.
//!
//! ## Architecture
//!
//! The engine consists of three primary components:
//! 1. **Parser:** Converts a string query into a GraphQL AST (`Document`, `Operation`, `Selection`).
//! 2. **Schema:** The type definitions available in the system.
//! 3. **Executor:** Traverses the AST and resolves fields against the Schema using traits.
//!
//! ### Data Structure Diagram
//! ```text
//! Query String
//!      |
//!      v
//!   Parser ---> AST (Document -> Operation -> SelectionSet -> Field)
//!                    |
//!                    v
//!                 Executor <--> Schema (Types & Resolvers)
//!                    |
//!                    v
//!               JSON Response
//! ```
//!
//! | Phase           | Time Complexity | Space Complexity |
//! |-----------------|-----------------|------------------|
//! | Parse Query     | O(N)            | O(N)             |
//! | Execute         | O(D * F)        | O(R)             |
//!
//! *N = length of query, D = depth of query, F = number of fields, R = size of response*
//!
//! **Design Decisions:**
//! - **Trait-based Resolvers:** Instead of compile-time macros (like `async-graphql`),
//!   we use a dynamic `Resolver` trait. This is closer to how reference implementations
//!   in dynamic languages work and provides maximum flexibility.
//! - **AST Representation:** A simplified AST that supports basic operations, fields,
//!   and nested selection sets.
//!
//! ---
//!
//! **Comparison to Production Crates:**
//! - Real crates like `async-graphql` heavily use proc-macros to generate type-safe schemas
//!   at compile time. This implementation is dynamic and interprets queries at runtime.
//! - We skip full validation (type checking the query against the schema before execution)
//!   for simplicity.
//! - No async support here. Production engines are async to handle concurrent I/O.
//!
//! ---
//!
//! **Benchmarks:**
//! To benchmark this engine, use `criterion`. Create a large, nested schema and
//! a complex query string. Measure the time taken by `Executor::execute()`. You
//! will likely find that `HashMap` lookups and AST cloning on nested fields become
//! bottlenecks.
//!
//! **Missing vs Production:**
//! - **Validation:** We don't implement the full GraphQL validation specification.
//! - **Variables and Fragments:** Not supported in this simplified parser.
//! - **Directives:** E.g. `@skip` or `@include`.
//! - **Async Execution:** Necessary for real-world backend integrations.
//!
//! **Next Steps:**
//! - Implement the full validation phase (e.g., verifying fields exist on types).
//! - Add support for Variables to the parser and executor.
//! - Implement an async `Resolver` trait using futures.

use std::collections::HashMap;
use std::fmt;

// --- 1. Abstract Syntax Tree (AST) ---

#[derive(Debug, PartialEq)]
pub struct Document {
    pub operations: Vec<Operation>,
}

#[derive(Debug, PartialEq)]
pub struct Operation {
    pub operation_type: OperationType,
    pub name: Option<String>,
    pub selection_set: SelectionSet,
}

#[derive(Debug, PartialEq)]
pub enum OperationType {
    Query,
    Mutation,
}

#[derive(Debug, PartialEq)]
pub struct SelectionSet {
    pub items: Vec<Selection>,
}

#[derive(Debug, PartialEq)]
pub enum Selection {
    Field(Field),
}

#[derive(Debug, PartialEq)]
pub struct Field {
    pub name: String,
    pub alias: Option<String>,
    pub arguments: HashMap<String, Value>,
    pub selection_set: Option<SelectionSet>,
}

#[derive(Debug, PartialEq, Clone)]
pub enum Value {
    String(String),
    Int(i64),
    Float(f64),
    Boolean(bool),
    Null,
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::String(s) => write!(f, "\"{}\"", s),
            Value::Int(i) => write!(f, "{}", i),
            Value::Float(fl) => write!(f, "{}", fl),
            Value::Boolean(b) => write!(f, "{}", b),
            Value::Null => write!(f, "null"),
        }
    }
}

// --- 2. Parser ---

/// A naive parser for a minimal subset of GraphQL.
pub struct Parser<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    pub fn new(input: &'a str) -> Self {
        Parser { input, pos: 0 }
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.input.len() {
            let c = self.input[self.pos..].chars().next().unwrap();
            if c.is_whitespace() || c == ',' {
                self.pos += c.len_utf8();
            } else {
                break;
            }
        }
    }

    fn peek(&mut self) -> Option<char> {
        self.skip_whitespace();
        self.input[self.pos..].chars().next()
    }

    fn consume(&mut self, expected: char) -> Result<(), String> {
        self.skip_whitespace();
        let c = self.input[self.pos..].chars().next()
            .ok_or_else(|| format!("Expected '{}', got EOF", expected))?;
        if c == expected {
            self.pos += c.len_utf8();
            Ok(())
        } else {
            Err(format!("Expected '{}', got '{}'", expected, c))
        }
    }

    fn parse_name(&mut self) -> Result<String, String> {
        self.skip_whitespace();
        let mut name = String::new();
        while self.pos < self.input.len() {
            let c = self.input[self.pos..].chars().next().unwrap();
            if c.is_alphanumeric() || c == '_' {
                name.push(c);
                self.pos += c.len_utf8();
            } else {
                break;
            }
        }
        if name.is_empty() {
            Err("Expected name".to_string())
        } else {
            Ok(name)
        }
    }

    pub fn parse_document(&mut self) -> Result<Document, String> {
        let mut operations = Vec::new();
        while self.peek().is_some() {
            operations.push(self.parse_operation()?);
        }
        Ok(Document { operations })
    }

    fn parse_operation(&mut self) -> Result<Operation, String> {
        // Simplified: assume unnamed query if it starts with '{'
        if self.peek() == Some('{') {
            return Ok(Operation {
                operation_type: OperationType::Query,
                name: None,
                selection_set: self.parse_selection_set()?,
            });
        }

        let op_type_str = self.parse_name()?;
        let operation_type = match op_type_str.as_str() {
            "query" => OperationType::Query,
            "mutation" => OperationType::Mutation,
            _ => return Err(format!("Unknown operation type: {}", op_type_str)),
        };

        let mut name = None;
        if self.peek() != Some('{') && self.peek() != Some('(') {
            name = Some(self.parse_name()?);
        }

        // Skip variables for now...
        if self.peek() == Some('(') {
            while let Some(c) = self.peek() {
                if c == ')' { break; }
                self.pos += c.len_utf8();
            }
            self.consume(')')?;
        }

        let selection_set = self.parse_selection_set()?;

        Ok(Operation {
            operation_type,
            name,
            selection_set,
        })
    }

    fn parse_selection_set(&mut self) -> Result<SelectionSet, String> {
        self.consume('{')?;
        let mut items = Vec::new();
        while self.peek() != Some('}') {
            items.push(Selection::Field(self.parse_field()?));
        }
        self.consume('}')?;
        Ok(SelectionSet { items })
    }

    fn parse_field(&mut self) -> Result<Field, String> {
        let name_or_alias = self.parse_name()?;
        let mut name = name_or_alias.clone();
        let mut alias = None;

        if self.peek() == Some(':') {
            self.consume(':')?;
            alias = Some(name_or_alias);
            name = self.parse_name()?;
        }

        // Basic argument parsing (mocked for simplicity)
        let arguments = HashMap::new();
        if self.peek() == Some('(') {
            while let Some(c) = self.peek() {
                if c == ')' { break; }
                self.pos += c.len_utf8(); // Skip args for now in this simple parser
            }
            self.consume(')')?;
        }

        let mut selection_set = None;
        if self.peek() == Some('{') {
            selection_set = Some(self.parse_selection_set()?);
        }

        Ok(Field {
            name,
            alias,
            arguments,
            selection_set,
        })
    }
}

// --- 3. Execution Engine ---

/// Represents a resolved value from the schema.
#[derive(Debug, Clone)]
pub enum ResolvedValue {
    Scalar(Value),
    Object(HashMap<String, ResolvedValue>),
    List(Vec<ResolvedValue>),
    Null,
}

impl ResolvedValue {
    pub fn to_json(&self) -> String {
        match self {
            ResolvedValue::Scalar(v) => match v {
                Value::String(s) => format!("\"{}\"", s),
                Value::Int(i) => i.to_string(),
                Value::Float(f) => f.to_string(),
                Value::Boolean(b) => b.to_string(),
                Value::Null => "null".to_string(),
            },
            ResolvedValue::Object(map) => {
                let mut json = String::from("{");
                let mut first = true;
                for (k, v) in map {
                    if !first {
                        json.push(',');
                    }
                    json.push_str(&format!("\"{}\":{}", k, v.to_json()));
                    first = false;
                }
                json.push('}');
                json
            }
            ResolvedValue::List(list) => {
                let mut json = String::from("[");
                let mut first = true;
                for item in list {
                    if !first {
                        json.push(',');
                    }
                    json.push_str(&item.to_json());
                    first = false;
                }
                json.push(']');
                json
            }
            ResolvedValue::Null => "null".to_string(),
        }
    }
}

/// The core trait that allows Rust types to be queried via GraphQL.
// RUST INSIGHT: Using a trait object `&dyn Resolver` allows heterogeneous
// schema trees to be traversed dynamically.
pub trait Resolver {
    fn resolve_field(&self, name: &str, args: &HashMap<String, Value>) -> Option<Box<dyn Resolver>>;
    fn resolve_scalar(&self) -> Option<ResolvedValue> { None }
}

/// Helper struct for root execution.
pub struct Executor;

impl Executor {
    pub fn execute(query: &str, root: &dyn Resolver) -> Result<String, String> {
        let mut parser = Parser::new(query);
        let doc = parser.parse_document()?;

        let mut response = HashMap::new();

        for op in doc.operations {
            if op.operation_type == OperationType::Query {
                for selection in op.selection_set.items {
                    if let Selection::Field(field) = selection {
                        let response_key = field.alias.clone().unwrap_or_else(|| field.name.clone());
                        let resolved = Self::execute_field(&field, root)?;
                        response.insert(response_key, resolved);
                    }
                }
            }
        }

        Ok(ResolvedValue::Object(response).to_json())
    }

    fn execute_field(field: &Field, resolver: &dyn Resolver) -> Result<ResolvedValue, String> {
        if let Some(child_resolver) = resolver.resolve_field(&field.name, &field.arguments) {
            if let Some(selection_set) = &field.selection_set {
                // It's an object, resolve subfields
                let mut object_response = HashMap::new();
                for selection in &selection_set.items {
                    if let Selection::Field(sub_field) = selection {
                        let response_key = sub_field.alias.clone().unwrap_or_else(|| sub_field.name.clone());
                        let resolved = Self::execute_field(sub_field, child_resolver.as_ref())?;
                        object_response.insert(response_key, resolved);
                    }
                }
                Ok(ResolvedValue::Object(object_response))
            } else {
                // It should be a scalar
                Ok(child_resolver.resolve_scalar().unwrap_or(ResolvedValue::Null))
            }
        } else {
            Ok(ResolvedValue::Null)
        }
    }
}

// --- Example Schema Implementations ---

struct User {
    id: i64,
    name: String,
}

impl Resolver for User {
    fn resolve_field(&self, name: &str, _args: &HashMap<String, Value>) -> Option<Box<dyn Resolver>> {
        match name {
            "id" => Some(Box::new(self.id)),
            "name" => Some(Box::new(self.name.clone())),
            _ => None,
        }
    }
}

struct QueryRoot;

impl Resolver for QueryRoot {
    fn resolve_field(&self, name: &str, _args: &HashMap<String, Value>) -> Option<Box<dyn Resolver>> {
        match name {
            "me" => Some(Box::new(User {
                id: 1,
                name: "Alice".to_string(),
            })),
            _ => None,
        }
    }
}

// Blanket impls for scalars
impl Resolver for String {
    fn resolve_field(&self, _name: &str, _args: &HashMap<String, Value>) -> Option<Box<dyn Resolver>> { None }
    fn resolve_scalar(&self) -> Option<ResolvedValue> {
        Some(ResolvedValue::Scalar(Value::String(self.clone())))
    }
}

impl Resolver for i64 {
    fn resolve_field(&self, _name: &str, _args: &HashMap<String, Value>) -> Option<Box<dyn Resolver>> { None }
    fn resolve_scalar(&self) -> Option<ResolvedValue> {
        Some(ResolvedValue::Scalar(Value::Int(*self)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parser_basic() {
        let query = "{ me { id name } }";
        let mut parser = Parser::new(query);
        let doc = parser.parse_document().unwrap();

        assert_eq!(doc.operations.len(), 1);
        let op = &doc.operations[0];
        assert_eq!(op.operation_type, OperationType::Query);
        assert_eq!(op.selection_set.items.len(), 1);

        if let Selection::Field(field) = &op.selection_set.items[0] {
            assert_eq!(field.name, "me");
            assert!(field.selection_set.is_some());
        } else {
            panic!("Expected field");
        }
    }

    #[test]
    fn test_execution_basic() {
        let query = "{ me { id name } }";
        let root = QueryRoot;
        let response = Executor::execute(query, &root).unwrap();

        // Use string parsing to avoid hashmap ordering issues in assertion
        assert!(response.contains("\"id\":1"));
        assert!(response.contains("\"name\":\"Alice\""));
    }

    #[test]
    fn test_execution_alias() {
        let query = "{ user1: me { identifier: id } }";
        let root = QueryRoot;
        let response = Executor::execute(query, &root).unwrap();

        assert!(response.contains("\"user1\":{"));
        assert!(response.contains("\"identifier\":1"));
    }
}
