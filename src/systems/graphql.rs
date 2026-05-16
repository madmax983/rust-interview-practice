//! # GraphQL Execution Engine
//!
//! Implements a minimal GraphQL query parser and execution engine from scratch.
//!
//! **Replaces Crates:** `async-graphql`, `juniper`
//!
//! **Real-world Usage:**
//! - Core engine of API gateways (Apollo Router).
//! - Backend-for-Frontend (BFF) layers resolving data from multiple microservices.
//! - Typed data fetching in modern web applications.
//!
//! **Why build it yourself?**
//! GraphQL seems like magic until you build the engine. By building it, you learn how to parse a
//! domain-specific language (DSL) into an Abstract Syntax Tree (AST), and how to dynamically traverse
//! that tree while executing code (resolvers) at each node. You'll understand the tradeoff between
//! compile-time macro magic (like `async-graphql`) and runtime dynamic resolution.

use std::collections::HashMap;

// =========================================================================================
// Architecture
// =========================================================================================
//
// 1. Parsing Phase:
//    Query String  -->  Lexer (Tokens)  -->  Parser (AST)
//
//    AST Structure:
//    Document -> Operation (Query/Mutation) -> SelectionSet -> Fields -> (Nested SelectionSet)
//
// 2. Execution Phase:
//    Takes the AST + Root Resolver  -->  JSON-like Value
//
//    Execution is a recursive walk of the SelectionSet. For each Field:
//    - Call `resolver.resolve_field(field_name)`
//    - If the result is an Object, recursively execute its SelectionSet.
//
// Invariants:
// - A field without a selection set must resolve to a scalar.
// - A field with a selection set must resolve to an object or list of objects.
//
// Complexity:
// - Parsing: O(N) where N is the length of the query string.
// - Execution: O(V + E) where V is the number of resolved fields and E is the number of edges (nested resolutions).

// -----------------------------------------------------------------------------------------
// 1. AST Definition
// -----------------------------------------------------------------------------------------

/// A GraphQL Document contains one or more operations.
#[derive(Debug, PartialEq, Clone)]
pub struct Document<'a> {
    pub operations: Vec<OperationDefinition<'a>>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct OperationDefinition<'a> {
    pub operation_type: OperationType,
    pub name: Option<&'a str>,
    pub selection_set: Vec<Selection<'a>>,
}

#[derive(Debug, PartialEq, Clone)]
pub enum OperationType {
    Query,
    Mutation,
}

#[derive(Debug, PartialEq, Clone)]
pub enum Selection<'a> {
    Field(Field<'a>),
    // Fragments not implemented in this minimal version
}

#[derive(Debug, PartialEq, Clone)]
pub struct Field<'a> {
    pub name: &'a str,
    pub alias: Option<&'a str>,
    pub arguments: HashMap<&'a str, Value>,
    pub selection_set: Option<Vec<Selection<'a>>>,
}

/// Represents a resolved value (JSON-like)
#[derive(Debug, PartialEq, Clone)]
pub enum Value {
    Null,
    Int(i32),
    Float(f64),
    String(String),
    Boolean(bool),
    Enum(String),
    List(Vec<Value>),
    Object(HashMap<String, Value>),
}

// -----------------------------------------------------------------------------------------
// 2. Parser
// -----------------------------------------------------------------------------------------

// RUST INSIGHT: We use `&'a str` for parsing to achieve zero-copy deserialization.
// The AST borrows slices directly from the input query string, minimizing heap allocations.
// This is exactly how `serde_json` achieves high performance.

pub struct Parser<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    pub fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    pub fn parse(&mut self) -> Result<Document<'a>, String> {
        self.skip_whitespace();
        let mut operations = Vec::new();

        while self.pos < self.input.len() {
            operations.push(self.parse_operation()?);
            self.skip_whitespace();
        }

        Ok(Document { operations })
    }

    fn parse_operation(&mut self) -> Result<OperationDefinition<'a>, String> {
        let start_pos = self.pos;
        let keyword = self.parse_name().unwrap_or("");
        let operation_type = match keyword {
            "query" => OperationType::Query,
            "mutation" => OperationType::Mutation,
            // Shorthand query
            _ => {
                // Rewind and parse as selection set
                self.pos = start_pos;
                return Ok(OperationDefinition {
                    operation_type: OperationType::Query,
                    name: None,
                    selection_set: self.parse_selection_set()?,
                });
            }
        };

        self.skip_whitespace();
        let name = if self.peek() == Some('{') {
            None
        } else {
            Some(self.parse_name()?)
        };

        let selection_set = self.parse_selection_set()?;

        Ok(OperationDefinition {
            operation_type,
            name,
            selection_set,
        })
    }

    fn parse_selection_set(&mut self) -> Result<Vec<Selection<'a>>, String> {
        self.skip_whitespace();
        self.expect('{')?;
        self.skip_whitespace();

        let mut selections = Vec::new();
        while self.peek() != Some('}') && self.pos < self.input.len() {
            selections.push(Selection::Field(self.parse_field()?));
            self.skip_whitespace();
        }

        self.expect('}')?;
        Ok(selections)
    }

    fn parse_field(&mut self) -> Result<Field<'a>, String> {
        let mut name = self.parse_name()?;
        self.skip_whitespace();

        let mut alias = None;
        if self.peek() == Some(':') {
            self.expect(':')?;
            self.skip_whitespace();
            alias = Some(name);
            name = self.parse_name()?;
        }

        self.skip_whitespace();
        let mut arguments = HashMap::new();
        if self.peek() == Some('(') {
            self.expect('(')?;
            self.skip_whitespace();
            while self.peek() != Some(')') && self.pos < self.input.len() {
                let arg_name = self.parse_name()?;
                self.skip_whitespace();
                self.expect(':')?;
                self.skip_whitespace();
                let arg_val = self.parse_value()?;
                arguments.insert(arg_name, arg_val);
                self.skip_whitespace();
                // Optional comma
                if self.peek() == Some(',') {
                    self.pos += 1;
                    self.skip_whitespace();
                }
            }
            self.expect(')')?;
        }

        self.skip_whitespace();
        let selection_set = if self.peek() == Some('{') {
            Some(self.parse_selection_set()?)
        } else {
            None
        };

        Ok(Field {
            name,
            alias,
            arguments,
            selection_set,
        })
    }

    fn parse_value(&mut self) -> Result<Value, String> {
        self.skip_whitespace();
        match self.peek() {
            Some('"') => {
                self.pos += 1; // skip quote
                let start = self.pos;
                while let Some(c) = self.peek() {
                    if c == '"' {
                        break;
                    }
                    self.pos += c.len_utf8();
                }
                let val = &self.input[start..self.pos];
                self.expect('"')?;
                Ok(Value::String(val.to_string()))
            }
            Some(c) if c.is_ascii_digit() => {
                let start = self.pos;
                while let Some(ch) = self.peek() {
                    if ch.is_ascii_digit() {
                        self.pos += ch.len_utf8();
                    } else {
                        break;
                    }
                }
                let val = &self.input[start..self.pos];
                Ok(Value::Int(val.parse().unwrap()))
            }
            Some('t') | Some('f') => {
                let name = self.parse_name()?;
                match name {
                    "true" => Ok(Value::Boolean(true)),
                    "false" => Ok(Value::Boolean(false)),
                    _ => Err(format!("Invalid boolean value: {}", name)),
                }
            }
            _ => Err("Unsupported value type".to_string()),
        }
    }

    fn parse_name(&mut self) -> Result<&'a str, String> {
        self.skip_whitespace();
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c.is_ascii_alphanumeric() || c == '_' {
                self.pos += c.len_utf8();
            } else {
                break;
            }
        }
        if start == self.pos {
            return Err("Expected a name".to_string());
        }
        Ok(&self.input[start..self.pos])
    }

    fn peek(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn expect(&mut self, expected: char) -> Result<(), String> {
        if self.peek() == Some(expected) {
            self.pos += expected.len_utf8();
            Ok(())
        } else {
            Err(format!("Expected '{}'", expected))
        }
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_whitespace() || c == ',' {
                self.pos += c.len_utf8();
            } else {
                break;
            }
        }
    }
}

// -----------------------------------------------------------------------------------------
// 3. Execution Engine
// -----------------------------------------------------------------------------------------

/// Represents any type that can resolve a GraphQL field.
// RUST INSIGHT: Using a trait object `&dyn Resolver` or trait bounds allows for
// heterogeneous schema definitions without massive enum definitions.
pub trait Resolver {
    fn resolve_field(&self, name: &str, args: &HashMap<&str, Value>) -> Option<Value>;
}

/// Executes a GraphQL document against a root resolver.
pub fn execute<'a>(doc: &'a Document<'a>, root: &dyn Resolver) -> Result<Value, String> {
    // GOTCHA: A real engine must validate the query against a Schema before execution.
    // We assume the query is valid and execute the first operation.

    let op = doc.operations.first().ok_or("No operations found")?;

    if op.operation_type != OperationType::Query {
        return Err("Only Query operations are supported in this example".to_string());
    }

    let result = execute_selection_set(&op.selection_set, root)?;
    Ok(Value::Object(result))
}

fn execute_selection_set<'a>(
    selections: &'a [Selection<'a>],
    resolver: &dyn Resolver,
) -> Result<HashMap<String, Value>, String> {
    let mut result_map = HashMap::new();

    for selection in selections {
        match selection {
            Selection::Field(field) => {
                let response_key = field.alias.unwrap_or(field.name).to_string();

                // GOTCHA: Real resolvers might be async and return a Future.
                // We use synchronous resolution here for simplicity.
                let resolved_value = resolver.resolve_field(field.name, &field.arguments)
                    .unwrap_or(Value::Null);

                // If the field has a sub-selection, the resolved value MUST provide a nested resolver.
                // In our simplified typed model, we'd represent nested resolvers via an Object variant or similar.
                // For this exercise, if it's an object and has sub-selections, we assume the object
                // data contains what we need, but standard GraphQL needs a resolver per object.
                // We'll treat Object(HashMap) as a simple map resolver.

                let final_val = match (resolved_value, &field.selection_set) {
                    (Value::Object(map), Some(sub_selections)) => {
                        let sub_resolver = MapResolver(&map);
                        let sub_result = execute_selection_set(sub_selections, &sub_resolver)?;
                        Value::Object(sub_result)
                    }
                    // For lists of objects
                    (Value::List(list), Some(sub_selections)) => {
                        let mut final_list = Vec::new();
                        for item in list {
                            if let Value::Object(map) = item {
                                let sub_resolver = MapResolver(&map);
                                let sub_result = execute_selection_set(sub_selections, &sub_resolver)?;
                                final_list.push(Value::Object(sub_result));
                            } else {
                                final_list.push(Value::Null);
                            }
                        }
                        Value::List(final_list)
                    }
                    (val, None) => val,
                    (_, Some(_)) => Value::Null, // Sub-selection requested on scalar
                };

                result_map.insert(response_key, final_val);
            }
        }
    }

    Ok(result_map)
}

/// A simple resolver that looks up fields in a HashMap.
struct MapResolver<'a>(&'a HashMap<String, Value>);

impl<'a> Resolver for MapResolver<'a> {
    fn resolve_field(&self, name: &str, _args: &HashMap<&str, Value>) -> Option<Value> {
        self.0.get(name).cloned()
    }
}

// =========================================================================================
// Comparison to Canonical Crates & Next Steps
// =========================================================================================
//
// **async-graphql / juniper**:
// - **What they do**: Production crates use powerful procedural macros to map Rust structs
//   and async functions directly to a GraphQL schema at compile time. They handle query
//   validation against the schema, type coercion, async resolvers, and complex features like
//   interfaces, unions, and directives.
// - **What we omitted**: We lack schema validation. Our engine assumes the query is valid
//   and executes it directly against a dynamic resolver map. We also omitted fragments,
//   variables, async resolution, and complex input types.
//
// **Benchmarking Note:**
// Benchmarking a GraphQL engine involves two phases: parsing and execution. You would use
// `criterion` to benchmark `Parser::parse` with a large, deeply nested query string to test
// AST allocation overhead. Separately, you'd benchmark `execute` with a mock resolver to
// measure the cost of traversing the AST and building the result map.
//
// **Next Steps:**
// - Implement a `Schema` representation and a validation phase before execution.
// - Update `Resolver` to return `std::future::Future` to support async data fetching (e.g., DB calls).

// =========================================================================================
// Tests
// =========================================================================================

#[cfg(test)]
mod tests {
    use super::*;

    struct MockQueryRoot;

    impl Resolver for MockQueryRoot {
        fn resolve_field(&self, name: &str, args: &HashMap<&str, Value>) -> Option<Value> {
            match name {
                "user" => {
                    let id = args.get("id");
                    if let Some(Value::Int(1)) = id {
                        let mut map = HashMap::new();
                        map.insert("id".to_string(), Value::Int(1));
                        map.insert("name".to_string(), Value::String("Alice".to_string()));
                        Some(Value::Object(map))
                    } else {
                        None
                    }
                }
                "version" => Some(Value::String("1.0".to_string())),
                _ => None,
            }
        }
    }

    #[test]
    fn test_parser_and_execution() {
        let query = r#"
            query GetUser {
                version
                user(id: 1) {
                    id
                    userName: name
                }
            }
        "#;

        let mut parser = Parser::new(query);
        let ast = parser.parse().expect("Failed to parse query");

        assert_eq!(ast.operations.len(), 1);
        assert_eq!(ast.operations[0].name, Some("GetUser"));

        let result = execute(&ast, &MockQueryRoot).expect("Execution failed");

        if let Value::Object(map) = result {
            assert_eq!(map.get("version"), Some(&Value::String("1.0".to_string())));

            if let Some(Value::Object(user_map)) = map.get("user") {
                assert_eq!(user_map.get("id"), Some(&Value::Int(1)));
                assert_eq!(user_map.get("userName"), Some(&Value::String("Alice".to_string())));
            } else {
                panic!("User object missing");
            }
        } else {
            panic!("Expected object result");
        }
    }

    #[test]
    fn test_shorthand_query() {
        let query = r#"{ version }"#;
        let mut parser = Parser::new(query);
        let ast = parser.parse().unwrap();

        let result = execute(&ast, &MockQueryRoot).unwrap();
        if let Value::Object(map) = result {
            assert_eq!(map.get("version"), Some(&Value::String("1.0".to_string())));
        } else {
            panic!("Expected object result");
        }
    }
}
