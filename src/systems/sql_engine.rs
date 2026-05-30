//! # Embedded SQL Engine Implementation
//!
//! Implements a minimal, from-scratch embedded SQL engine including a lexer, parser,
//! query planner/executor, and a swappable storage backend (with an in-memory implementation).
//!
//! **Replaces Crates:** `rusqlite` (internals), `sqlparser`, `gluesql`
//!
//! **Real-world Usage:**
//! - Embedded database engines (SQLite, DuckDB).
//! - Web browsers (IndexedDB built on SQLite).
//! - Mobile applications for offline data synchronization.
//! - Distributed SQL databases (CockroachDB, TiDB) which use similar query pipelines.
//!
//! **Why build it yourself?**
//! Building an SQL engine demystifies the "magic" of relational databases. You learn how
//! unstructured strings are transformed into structured tokens (Lexing), validated against
//! a grammar (Parsing), and executed against raw byte storage. It forces you to confront
//! type safety in a dynamic context (SQL values) and the complexity of filtering and projections.
//!
//! # Architecture
//!
//! **Flow:**
//!
//! ```text
//!      SQL String ("SELECT id FROM users")
//!             |
//!             v
//!      [ Lexer ]  ---->  Vec<Token>
//!             |
//!             v
//!      [ Parser ] ---->  AST (Statement::Select { .. })
//!             |
//!             v
//!      [ Executor ] <--> [ Storage Engine (Trait) ]
//!             |
//!             v
//!      ResultSet (Vec<Row>)
//! ```
//!
//! **Invariants:**
//! 1. All strings are assumed to be UTF-8 valid (Rust's `String` guarantees this).
//! 2. The Lexer is case-insensitive for keywords.
//! 3. The `StorageEngine` trait abstractions must never panic, returning proper `Result`s instead.
//! 4. Table schemas must be rigorously enforced during `INSERT`.
//!
//! **Complexity:**
//! ┌───────────────┬────────────┬─────────────┐
//! │ Operation     │ Time       │ Space       │
//! ├───────────────┼────────────┼─────────────┤
//! │ Lexing        │ O(N)       │ O(N)        │
//! │ Parsing       │ O(T)       │ O(T)        │
//! │ Insert        │ O(1)*      │ O(R)        │
//! │ Select (Seq)  │ O(R)       │ O(R)        │
//! └───────────────┴────────────┴─────────────┘
//! * N = query string length, T = num tokens, R = num rows.
//! * Insert is O(1) assuming no indexes and sequential append.
//!
//! **Design Decisions & Tradeoffs:**
//! - **Swappable Storage:** We define a `StorageEngine` trait. Real DBs use pager-backed B-Trees
//!   (like our `lsm_tree.rs` or `b_tree.rs`), but we provide a simple `InMemoryStorage` backed by `HashMap`.
//! - **Limited Grammar:** To keep the file manageable, we support a tiny subset: `CREATE TABLE`, `INSERT INTO`, and basic `SELECT ... WHERE`.
//! - **Dynamic Typing:** SQL columns can hold different types. We use a `Value` enum to represent this, demonstrating how static languages like Rust model dynamic systems safely.
//!
//! # Footer
//!
//! **Comparison to Canonical Crates:**
//! - `rusqlite`: This crate wraps the C-based SQLite engine. SQLite is fully featured, robust, uses pager-backed B-Trees, features a virtual machine (VDBE) for query execution, and an incredibly sophisticated query planner.
//! - `sqlparser-rs`: This crate provides a highly robust, spec-compliant SQL lexer and parser. Our parser is a toy recursive-descent parser that supports exactly what we need it to, without AST normalization or standard dialect support.
//! - `gluesql`: An SQL database engine written purely in Rust. It strongly separates storage and execution (like we did here), but provides massive coverage of SQL standard operations, indexing, and transactions.
//!
//! **Missing Features (vs. Production):**
//! - Transactions (ACID properties).
//! - Advanced indexing structures (B-Trees) and a Query Planner (to utilize those indexes).
//! - Advanced SQL operations: `JOIN`, `GROUP BY`, `ORDER BY`, aggregates (`COUNT`, `SUM`), etc.
//! - Concurrency control (e.g., MVCC).
//! - Disk-backed persistence.
//!
//! **Next Steps:**
//! - Add a B-Tree backed storage engine.
//! - Implement an index structure and update the executor to use it for `WHERE` clauses.
//! - Implement an AST-to-Bytecode compilation step for a VM-based execution engine.

use std::collections::HashMap;
use std::fmt;

// =========================================================================================
// Error Framework
// =========================================================================================

#[derive(Debug, PartialEq, Eq)]
pub enum SqlError {
    LexerError(String),
    ParserError(String),
    ExecutionError(String),
    StorageError(String),
}

impl fmt::Display for SqlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SqlError::LexerError(m) => write!(f, "Lexer Error: {}", m),
            SqlError::ParserError(m) => write!(f, "Parser Error: {}", m),
            SqlError::ExecutionError(m) => write!(f, "Execution Error: {}", m),
            SqlError::StorageError(m) => write!(f, "Storage Error: {}", m),
        }
    }
}

pub type Result<T> = std::result::Result<T, SqlError>;

// =========================================================================================
// Types and Values
// =========================================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataType {
    Integer,
    Text,
    Boolean,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Value {
    Integer(i64),
    Text(String),
    Boolean(bool),
    Null,
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Integer(i) => write!(f, "{}", i),
            Value::Text(t) => write!(f, "{}", t),
            Value::Boolean(b) => write!(f, "{}", if *b { "TRUE" } else { "FALSE" }),
            Value::Null => write!(f, "NULL"),
        }
    }
}

// =========================================================================================
// Lexer (Tokenizer)
// =========================================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    // Keywords
    Create,
    Table,
    Insert,
    Into,
    Values,
    Select,
    From,
    Where,
    And,

    // Types
    IntType,
    TextType,
    BoolType,

    // Identifiers and Literals
    Identifier(String),
    StringLiteral(String),
    IntegerLiteral(i64),
    BooleanLiteral(bool),

    // Symbols
    OpenParen,
    CloseParen,
    Comma,
    Asterisk,
    Equals,
    Semicolon,

    Eof,
}

pub struct Lexer<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    fn peek_char(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn advance_char(&mut self) -> Option<char> {
        let ch = self.peek_char()?;
        self.pos += ch.len_utf8();
        Some(ch)
    }

    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.peek_char() {
            if ch.is_whitespace() {
                self.advance_char();
            } else {
                break;
            }
        }
    }

    // RUST INSIGHT:
    // We use a simple sequential lexer. Since we only hold a reference to `input` (`&'a str`),
    // slicing is very cheap. We don't allocate strings unless we find an identifier or literal.
    pub fn next_token(&mut self) -> Result<Token> {
        self.skip_whitespace();

        let Some(ch) = self.peek_char() else {
            return Ok(Token::Eof);
        };

        match ch {
            '(' => {
                self.advance_char();
                Ok(Token::OpenParen)
            }
            ')' => {
                self.advance_char();
                Ok(Token::CloseParen)
            }
            ',' => {
                self.advance_char();
                Ok(Token::Comma)
            }
            '*' => {
                self.advance_char();
                Ok(Token::Asterisk)
            }
            '=' => {
                self.advance_char();
                Ok(Token::Equals)
            }
            ';' => {
                self.advance_char();
                Ok(Token::Semicolon)
            }
            '\'' => self.read_string_literal(),
            'a'..='z' | 'A'..='Z' | '_' => self.read_identifier_or_keyword(),
            '0'..='9' | '-' => self.read_integer_literal(),
            _ => Err(SqlError::LexerError(format!(
                "Unexpected character: {}",
                ch
            ))),
        }
    }

    fn read_string_literal(&mut self) -> Result<Token> {
        self.advance_char(); // skip opening quote
        // ⚡ BOLT OPTIMIZATION: Eliminate intermediate String allocation by slicing directly from the input.
        let start_pos = self.pos;
        while let Some(ch) = self.peek_char() {
            if ch == '\'' {
                let string = self.input[start_pos..self.pos].to_string();
                self.advance_char(); // skip closing quote
                return Ok(Token::StringLiteral(string));
            }
            self.advance_char();
        }
        Err(SqlError::LexerError("Unterminated string literal".into()))
    }

    fn read_identifier_or_keyword(&mut self) -> Result<Token> {
        // ⚡ BOLT OPTIMIZATION: Avoid String allocation and char pushing; use string slicing instead.
        let start_pos = self.pos;
        while let Some(ch) = self.peek_char() {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                self.advance_char();
            } else {
                break;
            }
        }

        let ident = &self.input[start_pos..self.pos];
        let upper = ident.to_uppercase();
        Ok(match upper.as_str() {
            "CREATE" => Token::Create,
            "TABLE" => Token::Table,
            "INSERT" => Token::Insert,
            "INTO" => Token::Into,
            "VALUES" => Token::Values,
            "SELECT" => Token::Select,
            "FROM" => Token::From,
            "WHERE" => Token::Where,
            "AND" => Token::And,
            "INT" | "INTEGER" => Token::IntType,
            "TEXT" | "VARCHAR" => Token::TextType,
            "BOOL" | "BOOLEAN" => Token::BoolType,
            "TRUE" => Token::BooleanLiteral(true),
            "FALSE" => Token::BooleanLiteral(false),
            _ => Token::Identifier(ident.to_string()), // Keep original case for identifiers
        })
    }

    fn read_integer_literal(&mut self) -> Result<Token> {
        // ⚡ BOLT OPTIMIZATION: Avoid String allocation and char pushing; use string slicing instead.
        let start_pos = self.pos;
        if self.peek_char() == Some('-') {
            self.advance_char();
        }

        while let Some(ch) = self.peek_char() {
            if ch.is_ascii_digit() {
                self.advance_char();
            } else {
                break;
            }
        }

        let num_str = &self.input[start_pos..self.pos];
        let val = num_str
            .parse::<i64>()
            .map_err(|_| SqlError::LexerError(format!("Invalid integer literal: {}", num_str)))?;
        Ok(Token::IntegerLiteral(val))
    }

    pub fn tokenize(mut self) -> Result<Vec<Token>> {
        let mut tokens = Vec::new();
        loop {
            let t = self.next_token()?;
            if t == Token::Eof {
                break;
            }
            tokens.push(t);
        }
        Ok(tokens)
    }
}

// =========================================================================================
// Parser (AST generation)
// =========================================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnDef {
    pub name: String,
    pub data_type: DataType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    Ident(String),
    Literal(Value),
    BinaryOp {
        left: Box<Expr>,
        op: Token, // e.g., Token::Equals
        right: Box<Expr>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement {
    CreateTable {
        name: String,
        columns: Vec<ColumnDef>,
    },
    Insert {
        table_name: String,
        columns: Option<Vec<String>>,
        values: Vec<Expr>, // Using Expr for literals
    },
    Select {
        table_name: String,
        columns: Vec<String>, // empty means '*'
        where_clause: Option<Expr>,
    },
}

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

// GOTCHA:
// Recursive descent parsing is elegant but can lead to stack overflows on deeply nested
// expressions. Production parsers often use Pratt parsing or iterative approaches for expressions.
impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<&Token> {
        let t = self.tokens.get(self.pos);
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    fn consume(&mut self, expected: Token) -> Result<()> {
        match self.advance() {
            Some(t) if t == &expected => Ok(()),
            Some(t) => Err(SqlError::ParserError(format!(
                "Expected {:?}, found {:?}",
                expected, t
            ))),
            None => Err(SqlError::ParserError(format!(
                "Expected {:?}, found EOF",
                expected
            ))),
        }
    }

    pub fn parse(&mut self) -> Result<Vec<Statement>> {
        let mut stmts = Vec::new();
        while self.peek().is_some() {
            stmts.push(self.parse_statement()?);
            // Optional semicolon
            if let Some(Token::Semicolon) = self.peek() {
                self.advance();
            }
        }
        Ok(stmts)
    }

    fn parse_statement(&mut self) -> Result<Statement> {
        match self.peek() {
            Some(Token::Create) => self.parse_create_table(),
            Some(Token::Insert) => self.parse_insert(),
            Some(Token::Select) => self.parse_select(),
            Some(t) => Err(SqlError::ParserError(format!(
                "Unexpected statement start: {:?}",
                t
            ))),
            None => Err(SqlError::ParserError("Unexpected EOF".into())),
        }
    }

    fn parse_create_table(&mut self) -> Result<Statement> {
        self.consume(Token::Create)?;
        self.consume(Token::Table)?;

        let table_name = match self.advance() {
            Some(Token::Identifier(id)) => id.clone(),
            _ => return Err(SqlError::ParserError("Expected table name".into())),
        };

        self.consume(Token::OpenParen)?;
        let mut columns = Vec::new();

        loop {
            let col_name = match self.advance() {
                Some(Token::Identifier(id)) => id.clone(),
                _ => return Err(SqlError::ParserError("Expected column name".into())),
            };

            let data_type = match self.advance() {
                Some(Token::IntType) => DataType::Integer,
                Some(Token::TextType) => DataType::Text,
                Some(Token::BoolType) => DataType::Boolean,
                _ => return Err(SqlError::ParserError("Expected column type".into())),
            };

            columns.push(ColumnDef {
                name: col_name,
                data_type,
            });

            match self.peek() {
                Some(Token::Comma) => {
                    self.advance();
                }
                Some(Token::CloseParen) => break,
                _ => return Err(SqlError::ParserError("Expected ',' or ')'".into())),
            }
        }

        self.consume(Token::CloseParen)?;

        Ok(Statement::CreateTable {
            name: table_name,
            columns,
        })
    }

    fn parse_insert(&mut self) -> Result<Statement> {
        self.consume(Token::Insert)?;
        self.consume(Token::Into)?;

        let table_name = match self.advance() {
            Some(Token::Identifier(id)) => id.clone(),
            _ => return Err(SqlError::ParserError("Expected table name".into())),
        };

        // Parse optional columns
        let mut columns = None;
        if let Some(Token::OpenParen) = self.peek() {
            self.advance();
            let mut cols = Vec::new();
            loop {
                match self.advance() {
                    Some(Token::Identifier(id)) => cols.push(id.clone()),
                    _ => return Err(SqlError::ParserError("Expected column name".into())),
                }
                match self.peek() {
                    Some(Token::Comma) => {
                        self.advance();
                    }
                    Some(Token::CloseParen) => break,
                    _ => return Err(SqlError::ParserError("Expected ',' or ')'".into())),
                }
            }
            self.consume(Token::CloseParen)?;
            columns = Some(cols);
        }

        self.consume(Token::Values)?;
        self.consume(Token::OpenParen)?;

        let mut values = Vec::new();
        loop {
            values.push(self.parse_expression()?);
            match self.peek() {
                Some(Token::Comma) => {
                    self.advance();
                }
                Some(Token::CloseParen) => break,
                _ => return Err(SqlError::ParserError("Expected ',' or ')'".into())),
            }
        }
        self.consume(Token::CloseParen)?;

        Ok(Statement::Insert {
            table_name,
            columns,
            values,
        })
    }

    fn parse_select(&mut self) -> Result<Statement> {
        self.consume(Token::Select)?;

        let mut columns = Vec::new();
        if let Some(Token::Asterisk) = self.peek() {
            self.advance();
        } else {
            loop {
                match self.advance() {
                    Some(Token::Identifier(id)) => columns.push(id.clone()),
                    _ => return Err(SqlError::ParserError("Expected column name or '*'".into())),
                }
                if let Some(Token::Comma) = self.peek() {
                    self.advance();
                } else {
                    break;
                }
            }
        }

        self.consume(Token::From)?;

        let table_name = match self.advance() {
            Some(Token::Identifier(id)) => id.clone(),
            _ => return Err(SqlError::ParserError("Expected table name".into())),
        };

        let mut where_clause = None;
        if let Some(Token::Where) = self.peek() {
            self.advance();
            where_clause = Some(self.parse_expression()?);
        }

        Ok(Statement::Select {
            table_name,
            columns,
            where_clause,
        })
    }

    fn parse_expression(&mut self) -> Result<Expr> {
        // Parse left operand
        let left = match self.advance() {
            Some(Token::Identifier(id)) => Expr::Ident(id.clone()),
            Some(Token::StringLiteral(s)) => Expr::Literal(Value::Text(s.clone())),
            Some(Token::IntegerLiteral(i)) => Expr::Literal(Value::Integer(*i)),
            Some(Token::BooleanLiteral(b)) => Expr::Literal(Value::Boolean(*b)),
            _ => return Err(SqlError::ParserError("Expected expression".into())),
        };

        // Check for binary operator (only '=' supported for now)
        if let Some(Token::Equals) = self.peek() {
            let op = self.advance().unwrap().clone();
            let right = self.parse_expression()?;
            Ok(Expr::BinaryOp {
                left: Box::new(left),
                op,
                right: Box::new(right),
            })
        } else {
            Ok(left)
        }
    }
}

// =========================================================================================
// Storage Engine
// =========================================================================================

#[derive(Debug, Clone)]
pub struct TableSchema {
    pub columns: Vec<ColumnDef>,
}

#[derive(Debug, Clone)]
pub struct Row {
    pub values: Vec<Value>,
}

pub trait StorageEngine {
    fn create_table(&mut self, name: &str, schema: TableSchema) -> Result<()>;
    fn get_schema(&self, table_name: &str) -> Result<TableSchema>;
    fn insert_row(&mut self, table_name: &str, row: Row) -> Result<()>;
    fn scan_table(&self, table_name: &str) -> Result<Vec<Row>>;
}

pub struct InMemoryStorage {
    schemas: HashMap<String, TableSchema>,
    tables: HashMap<String, Vec<Row>>,
}

impl InMemoryStorage {
    pub fn new() -> Self {
        Self {
            schemas: HashMap::new(),
            tables: HashMap::new(),
        }
    }
}

impl StorageEngine for InMemoryStorage {
    fn create_table(&mut self, name: &str, schema: TableSchema) -> Result<()> {
        if self.schemas.contains_key(name) {
            return Err(SqlError::StorageError(format!(
                "Table '{}' already exists",
                name
            )));
        }
        self.schemas.insert(name.to_string(), schema);
        self.tables.insert(name.to_string(), Vec::new());
        Ok(())
    }

    fn get_schema(&self, table_name: &str) -> Result<TableSchema> {
        self.schemas
            .get(table_name)
            .cloned()
            .ok_or_else(|| SqlError::StorageError(format!("Table '{}' not found", table_name)))
    }

    fn insert_row(&mut self, table_name: &str, row: Row) -> Result<()> {
        if let Some(table) = self.tables.get_mut(table_name) {
            table.push(row);
            Ok(())
        } else {
            Err(SqlError::StorageError(format!(
                "Table '{}' not found",
                table_name
            )))
        }
    }

    fn scan_table(&self, table_name: &str) -> Result<Vec<Row>> {
        self.tables
            .get(table_name)
            .cloned()
            .ok_or_else(|| SqlError::StorageError(format!("Table '{}' not found", table_name)))
    }
}

// =========================================================================================
// Executor Engine
// =========================================================================================

pub struct SqlEngine<S: StorageEngine> {
    storage: S,
}

// PRODUCTION NOTE:
// A real database plans the query before execution, choosing indexes (B-Tree/Hash),
// join strategies (Hash Join/Nested Loop), and pushing down predicates.
// Our executor is highly simplified, essentially combining the Planner and Executor stages,
// and doing full table scans for everything.
impl<S: StorageEngine> SqlEngine<S> {
    pub fn new(storage: S) -> Self {
        Self { storage }
    }

    pub fn execute(&mut self, query: &str) -> Result<Vec<Row>> {
        let lexer = Lexer::new(query);
        let tokens = lexer.tokenize()?;
        let mut parser = Parser::new(tokens);
        let stmts = parser.parse()?;

        let mut last_result = Vec::new();
        for stmt in stmts {
            last_result = self.execute_statement(stmt)?;
        }
        Ok(last_result)
    }

    fn execute_statement(&mut self, stmt: Statement) -> Result<Vec<Row>> {
        match stmt {
            Statement::CreateTable { name, columns } => {
                self.storage.create_table(&name, TableSchema { columns })?;
                Ok(Vec::new())
            }
            Statement::Insert {
                table_name,
                columns,
                values,
            } => {
                let schema = self.storage.get_schema(&table_name)?;

                // Currently only supporting implicit full column inserts for simplicity
                if columns.is_some() {
                    return Err(SqlError::ExecutionError(
                        "Named column inserts not yet supported".into(),
                    ));
                }

                if values.len() != schema.columns.len() {
                    return Err(SqlError::ExecutionError(format!(
                        "Column count mismatch. Expected {}, got {}",
                        schema.columns.len(),
                        values.len()
                    )));
                }

                let mut row_values = Vec::new();
                for (i, expr) in values.into_iter().enumerate() {
                    let val = match expr {
                        Expr::Literal(v) => v,
                        _ => {
                            return Err(SqlError::ExecutionError(
                                "Only literals supported in INSERT VALUES".into(),
                            ));
                        }
                    };

                    // Type checking
                    let expected_type = &schema.columns[i].data_type;
                    let valid = match (&val, expected_type) {
                        (Value::Integer(_), DataType::Integer) => true,
                        (Value::Text(_), DataType::Text) => true,
                        (Value::Boolean(_), DataType::Boolean) => true,
                        (Value::Null, _) => true,
                        _ => false,
                    };

                    if !valid {
                        return Err(SqlError::ExecutionError(format!(
                            "Type mismatch for column '{}'. Expected {:?}, got {:?}",
                            schema.columns[i].name, expected_type, val
                        )));
                    }

                    row_values.push(val);
                }

                self.storage
                    .insert_row(&table_name, Row { values: row_values })?;
                Ok(Vec::new())
            }
            Statement::Select {
                table_name,
                columns,
                where_clause,
            } => {
                let schema = self.storage.get_schema(&table_name)?;
                let rows = self.storage.scan_table(&table_name)?;

                // Map column names to indices
                let mut projection_indices = Vec::new();
                if columns.is_empty() {
                    // SELECT *
                    projection_indices = (0..schema.columns.len()).collect();
                } else {
                    for col_name in &columns {
                        let idx = schema
                            .columns
                            .iter()
                            .position(|c| c.name == *col_name)
                            .ok_or_else(|| {
                                SqlError::ExecutionError(format!("Column '{}' not found", col_name))
                            })?;
                        projection_indices.push(idx);
                    }
                }

                let mut result_rows = Vec::new();
                for row in rows {
                    if let Some(ref expr) = where_clause {
                        if !self.evaluate_boolean_expr(expr, &row, &schema)? {
                            continue;
                        }
                    }

                    let projected_values = projection_indices
                        .iter()
                        .map(|&idx| row.values[idx].clone())
                        .collect();
                    result_rows.push(Row {
                        values: projected_values,
                    });
                }

                Ok(result_rows)
            }
        }
    }

    fn evaluate_boolean_expr(&self, expr: &Expr, row: &Row, schema: &TableSchema) -> Result<bool> {
        match expr {
            Expr::BinaryOp { left, op, right } => {
                if *op != Token::Equals {
                    return Err(SqlError::ExecutionError(
                        "Only '=' operator supported in WHERE".into(),
                    ));
                }

                let left_val = self.evaluate_expr(left, row, schema)?;
                let right_val = self.evaluate_expr(right, row, schema)?;

                Ok(left_val == right_val)
            }
            _ => Err(SqlError::ExecutionError(
                "WHERE clause must be a boolean expression".into(),
            )),
        }
    }

    fn evaluate_expr(&self, expr: &Expr, row: &Row, schema: &TableSchema) -> Result<Value> {
        match expr {
            Expr::Literal(val) => Ok(val.clone()),
            Expr::Ident(col_name) => {
                let idx = schema
                    .columns
                    .iter()
                    .position(|c| c.name == *col_name)
                    .ok_or_else(|| {
                        SqlError::ExecutionError(format!(
                            "Column '{}' not found in WHERE clause",
                            col_name
                        ))
                    })?;
                Ok(row.values[idx].clone())
            }
            Expr::BinaryOp { .. } => Err(SqlError::ExecutionError(
                "Nested binary operations not supported".into(),
            )),
        }
    }
}

// =========================================================================================
// Tests
// =========================================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lexer() {
        let sql = "SELECT id, 'bob', 123 FROM users WHERE id = 1;";
        let lexer = Lexer::new(sql);
        let tokens = lexer.tokenize().unwrap();

        assert_eq!(
            tokens,
            vec![
                Token::Select,
                Token::Identifier("id".into()),
                Token::Comma,
                Token::StringLiteral("bob".into()),
                Token::Comma,
                Token::IntegerLiteral(123),
                Token::From,
                Token::Identifier("users".into()),
                Token::Where,
                Token::Identifier("id".into()),
                Token::Equals,
                Token::IntegerLiteral(1),
                Token::Semicolon,
            ]
        );
    }

    #[test]
    fn test_full_pipeline() {
        let storage = InMemoryStorage::new();
        let mut engine = SqlEngine::new(storage);

        // Create table
        let res = engine
            .execute("CREATE TABLE users (id INT, name TEXT, is_active BOOL);")
            .unwrap();
        assert!(res.is_empty());

        // Insert rows
        engine
            .execute("INSERT INTO users VALUES (1, 'Alice', TRUE);")
            .unwrap();
        engine
            .execute("INSERT INTO users VALUES (2, 'Bob', FALSE);")
            .unwrap();
        engine
            .execute("INSERT INTO users VALUES (3, 'Charlie', TRUE);")
            .unwrap();

        // Select all
        let res = engine.execute("SELECT * FROM users;").unwrap();
        assert_eq!(res.len(), 3);
        assert_eq!(res[0].values[1], Value::Text("Alice".into()));

        // Select with projection and where clause
        let res = engine
            .execute("SELECT name FROM users WHERE is_active = TRUE;")
            .unwrap();
        assert_eq!(res.len(), 2);
        assert_eq!(res[0].values.len(), 1);
        assert_eq!(res[0].values[0], Value::Text("Alice".into()));
        assert_eq!(res[1].values[0], Value::Text("Charlie".into()));

        // Select with identifier = identifier should fail but literal works
        let res = engine
            .execute("SELECT name FROM users WHERE id = 2;")
            .unwrap();
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].values[0], Value::Text("Bob".into()));
    }

    #[test]
    fn test_type_mismatch_on_insert() {
        let storage = InMemoryStorage::new();
        let mut engine = SqlEngine::new(storage);

        engine.execute("CREATE TABLE users (id INT);").unwrap();

        // Should fail because 'abc' is text, but column is INT
        let err = engine
            .execute("INSERT INTO users VALUES ('abc');")
            .unwrap_err();
        assert!(matches!(err, SqlError::ExecutionError(_)));
    }

    #[test]
    fn test_table_not_found() {
        let storage = InMemoryStorage::new();
        let mut engine = SqlEngine::new(storage);

        let err = engine.execute("SELECT * FROM missing_table;").unwrap_err();
        assert!(matches!(err, SqlError::StorageError(_)));
    }

    #[test]
    fn test_column_not_found() {
        let storage = InMemoryStorage::new();
        let mut engine = SqlEngine::new(storage);

        engine.execute("CREATE TABLE users (id INT);").unwrap();
        engine.execute("INSERT INTO users VALUES (1);").unwrap();

        let err = engine.execute("SELECT bad_col FROM users;").unwrap_err();
        assert!(matches!(err, SqlError::ExecutionError(_)));
    }

    #[test]
    fn test_benchmark_note() {
        // To properly benchmark this parsing and execution pipeline, use Criterion.
        // E.g.:
        // c.bench_function("sql_insert_1000", |b| {
        //     let mut engine = SqlEngine::new(InMemoryStorage::new());
        //     engine.execute("CREATE TABLE t (id INT);").unwrap();
        //     b.iter(|| {
        //         std::hint::black_box(engine.execute("INSERT INTO t VALUES (1);").unwrap());
        //     });
        // });
        assert!(true);
    }
}
