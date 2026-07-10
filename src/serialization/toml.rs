//! # Simple TOML Parser
//!
//! # Header
//!
//! *   **Problem Name**: TOML Parser (Mini)
//! *   **Difficulty**: Hard (Parsing)
//! *   **Link**: <https://toml.io/en/>
//! *   **Why this matters in Rust**: Configuration parsing is ubiquitous. Writing a parser from scratch teaches lexing, grammar, and error handling.
//!
//! # Architecture
//!
//! The implementation is split into two phases:
//! 1.  **Lexer (Tokenizer)**: Scans raw text and produces a stream of `Token`s with position information (`Span`).
//! 2.  **Parser**: Consumes tokens to build an Abstract Syntax Tree (AST), represented here as `TomlValue`.
//!
//! **Grammar Subset:**
//! *   Key-Value pairs: `key = "value"`, `count = 123`
//! *   Sections: `[database]`
//! *   Comments: `# This is a comment`
//! *   Types: String, Integer, Table (Section).
//!
//! **Invariants:**
//! *   Duplicate keys in the same section are invalid.
//! *   Strings must be quoted.
//!
//! # Rust Insight
//!
//! *   **Enums**: `Token` and `TomlValue` use enums to represent variant data types safely.
//! *   **Result/Error**: Custom error types with `Span` allow precise error reporting (Line/Col).
//! *   **Peekable Iterator**: Used in the parser to look ahead without consuming tokens.

use std::collections::HashMap;
use std::fmt;
use std::iter::Peekable;
use std::str::Chars;

// =========================================================================================
// Data Structures
// =========================================================================================

#[derive(Debug, Clone, PartialEq)]
pub enum TomlValue {
    String(String),
    Integer(i64),
    Table(HashMap<String, Self>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub col: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    Identifier(String),
    StringLiteral(String),
    IntegerLiteral(i64),
    Equals,
    LBracket,
    RBracket,
    EOF,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

#[derive(Debug)]
pub enum ParseError {
    UnexpectedChar(char, Span),
    UnexpectedToken(TokenKind, Span),
    UnterminatedString(Span),
    InvalidNumber(String, Span),
    Generic(String, Span),
    UnexpectedEOF,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedChar(c, s) => {
                write!(f, "Unexpected character '{}' at {}:{}", c, s.line, s.col)
            }
            Self::UnexpectedToken(t, s) => {
                write!(f, "Unexpected token {:?} at {}:{}", t, s.line, s.col)
            }
            Self::UnterminatedString(s) => {
                write!(f, "Unterminated string starting at {}:{}", s.line, s.col)
            }
            Self::InvalidNumber(n, s) => {
                write!(f, "Invalid number '{}' at {}:{}", n, s.line, s.col)
            }
            Self::Generic(msg, s) => write!(f, "Error '{}' at {}:{}", msg, s.line, s.col),
            Self::UnexpectedEOF => write!(f, "Unexpected End of File"),
        }
    }
}

// =========================================================================================
// Lexer
// =========================================================================================

pub struct Lexer<'a> {
    _input: &'a str,
    chars: Peekable<Chars<'a>>,
    pos: usize,
    line: usize,
    col: usize,
}

impl<'a> Lexer<'a> {
    #[must_use]
    pub fn new(input: &'a str) -> Self {
        Self {
            _input: input,
            chars: input.chars().peekable(),
            pos: 0,
            line: 1,
            col: 1,
        }
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.chars.next();
        if let Some(ch) = c {
            self.pos += ch.len_utf8();
            if ch == '\n' {
                self.line += 1;
                self.col = 1;
            } else {
                self.col += 1;
            }
        }
        c
    }

    fn peek(&mut self) -> Option<&char> {
        self.chars.peek()
    }

    /// Lexes and returns the next token from the input stream.
    ///
    /// # Errors
    ///
    /// Returns a [`ParseError`] if the input contains an unexpected character,
    /// an unterminated string, or a malformed number.
    pub fn next_token(&mut self) -> Result<Token, ParseError> {
        self.skip_whitespace_and_comments();

        let start_span = Span {
            start: self.pos,
            end: self.pos,
            line: self.line,
            col: self.col,
        };

        let Some(c) = self.advance() else {
            return Ok(Token {
                kind: TokenKind::EOF,
                span: start_span,
            });
        };

        let kind = match c {
            '=' => TokenKind::Equals,
            '[' => TokenKind::LBracket,
            ']' => TokenKind::RBracket,
            '"' => self.read_string(start_span)?,
            '0'..='9' | '-' => self.read_number(c, start_span)?,
            c if is_ident_start(c) => self.read_ident(c),
            c => return Err(ParseError::UnexpectedChar(c, start_span)),
        };

        Ok(Token {
            kind,
            span: Span {
                end: self.pos,
                ..start_span
            },
        })
    }

    fn skip_whitespace_and_comments(&mut self) {
        while let Some(&c) = self.peek() {
            if c.is_whitespace() {
                self.advance();
            } else if c == '#' {
                // Comment until end of line
                while let Some(&c) = self.peek() {
                    if c == '\n' {
                        break;
                    }
                    self.advance();
                }
            } else {
                break;
            }
        }
    }

    fn read_string(&mut self, start_span: Span) -> Result<TokenKind, ParseError> {
        let mut s = String::new();
        loop {
            match self.advance() {
                Some('"') => break, // End of string
                Some(c) => s.push(c),
                None => return Err(ParseError::UnterminatedString(start_span)),
            }
        }
        Ok(TokenKind::StringLiteral(s))
    }

    fn read_number(&mut self, first: char, start_span: Span) -> Result<TokenKind, ParseError> {
        let mut s = String::new();
        s.push(first);

        while let Some(&c) = self.peek() {
            if c.is_ascii_digit() {
                self.advance();
                s.push(c);
            } else {
                break;
            }
        }

        s.parse::<i64>().map_or_else(
            |_| Err(ParseError::InvalidNumber(s, start_span)),
            |n| Ok(TokenKind::IntegerLiteral(n)),
        )
    }

    fn read_ident(&mut self, first: char) -> TokenKind {
        let mut s = String::new();
        s.push(first);

        while let Some(&c) = self.peek() {
            if is_ident_char(c) {
                self.advance();
                s.push(c);
            } else {
                break;
            }
        }

        TokenKind::Identifier(s)
    }
}

fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '-'
}

// =========================================================================================
// Parser
// =========================================================================================

pub struct Parser<'a> {
    lexer: Lexer<'a>,
    current_token: Token,
}

impl<'a> Parser<'a> {
    /// Creates a new parser, priming it with the first token.
    ///
    /// # Errors
    ///
    /// Returns a [`ParseError`] if the very first token cannot be lexed.
    pub fn new(input: &'a str) -> Result<Self, ParseError> {
        let mut lexer = Lexer::new(input);
        let current_token = lexer.next_token()?;
        Ok(Self {
            lexer,
            current_token,
        })
    }

    fn advance(&mut self) -> Result<(), ParseError> {
        self.current_token = self.lexer.next_token()?;
        Ok(())
    }

    fn expect(&mut self, expected_kind: &TokenKind) -> Result<(), ParseError> {
        // Simple comparison ignoring internal data for Identifier/Literal.
        // We only use `expect` for the symbol tokens (Equals, brackets, EOF), so a
        // structural `matches!` over the paired discriminants is sufficient and
        // avoids the manual PartialEq needed for the data-carrying variants.
        let matches = matches!(
            (&self.current_token.kind, expected_kind),
            (TokenKind::Equals, TokenKind::Equals)
                | (TokenKind::LBracket, TokenKind::LBracket)
                | (TokenKind::RBracket, TokenKind::RBracket)
                | (TokenKind::EOF, TokenKind::EOF)
        );

        if matches {
            self.advance()
        } else {
            Err(ParseError::UnexpectedToken(
                self.current_token.kind.clone(),
                self.current_token.span,
            ))
        }
    }

    /// Parses the token stream into a [`TomlValue`] table hierarchy.
    ///
    /// # Errors
    ///
    /// Returns a [`ParseError`] on any malformed input: unexpected tokens,
    /// duplicate keys, or lexing failures encountered while advancing.
    ///
    /// # Panics
    ///
    /// Panics only if the internal root section is missing, which cannot happen
    /// because it is inserted unconditionally at the start of parsing.
    pub fn parse(&mut self) -> Result<TomlValue, ParseError> {
        let mut current_section = String::new(); // Empty string = root section

        // We need to store sections. A flat map of "section" -> Hashmap might be easier internally,
        // then convert to nested.
        // Or simpler: root is a map. If key is simple, insert to root.
        // If we see [section], we start inserting into that section in the root map.

        // Structure: "key" -> Val, "section" -> Table(Map)

        // Since we process sequentially, we can maintain a pointer to the current active table.
        // But in Rust, mutable pointers to parts of HashMap are hard while inserting.
        // Easier: Build a flattened structure: `section_name` -> `HashMap<key, val>`.
        // Root is section "".

        let mut sections: HashMap<String, HashMap<String, TomlValue>> = HashMap::new();
        sections.insert(String::new(), HashMap::new()); // Root section

        while self.current_token.kind != TokenKind::EOF {
            match &self.current_token.kind {
                TokenKind::LBracket => {
                    // Section header: [name]
                    self.advance()?;
                    if let TokenKind::Identifier(name) = &self.current_token.kind {
                        current_section.clone_from(name);
                        self.advance()?;
                        self.expect(&TokenKind::RBracket)?;

                        // Ensure section exists
                        sections.entry(current_section.clone()).or_default();
                    } else {
                        return Err(ParseError::UnexpectedToken(
                            self.current_token.kind.clone(),
                            self.current_token.span,
                        ));
                    }
                }
                TokenKind::Identifier(key) => {
                    // Key = Value
                    let key = key.clone();
                    self.advance()?;
                    self.expect(&TokenKind::Equals)?;

                    let value = self.parse_value()?;

                    if let Some(table) = sections.get_mut(&current_section) {
                        if table.contains_key(&key) {
                            return Err(ParseError::Generic(
                                format!("Duplicate key '{key}'"),
                                self.current_token.span,
                            ));
                        }
                        table.insert(key, value);
                    }
                }
                _ => {
                    return Err(ParseError::UnexpectedToken(
                        self.current_token.kind.clone(),
                        self.current_token.span,
                    ));
                }
            }
        }

        // Convert flattened sections to TomlValue::Table hierarchy
        // Root items go to top level.
        // Sections become Table entries in top level.

        let mut root_map = sections.remove("").unwrap();

        for (sec_name, sec_map) in sections {
            root_map.insert(sec_name, TomlValue::Table(sec_map));
        }

        Ok(TomlValue::Table(root_map))
    }

    fn parse_value(&mut self) -> Result<TomlValue, ParseError> {
        let val = match &self.current_token.kind {
            TokenKind::StringLiteral(s) => TomlValue::String(s.clone()),
            TokenKind::IntegerLiteral(i) => TomlValue::Integer(*i),
            _ => {
                return Err(ParseError::UnexpectedToken(
                    self.current_token.kind.clone(),
                    self.current_token.span,
                ));
            }
        };
        self.advance()?;
        Ok(val)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lexer_basic() {
        let input = "key = \"value\"";
        let mut lexer = Lexer::new(input);

        let t1 = lexer.next_token().unwrap();
        match t1.kind {
            TokenKind::Identifier(s) => assert_eq!(s, "key"),
            _ => panic!("Expected Identifier"),
        }

        let t2 = lexer.next_token().unwrap();
        assert_eq!(t2.kind, TokenKind::Equals);

        let t3 = lexer.next_token().unwrap();
        match t3.kind {
            TokenKind::StringLiteral(s) => assert_eq!(s, "value"),
            _ => panic!("Expected StringLiteral"),
        }
    }

    #[test]
    fn test_parser_basic() {
        let input = r#"
            title = "TOML Example"
            count = 123
        "#;
        let mut parser = Parser::new(input).unwrap();
        let result = parser.parse().unwrap();

        if let TomlValue::Table(map) = result {
            assert_eq!(
                map.get("title"),
                Some(&TomlValue::String("TOML Example".to_string()))
            );
            assert_eq!(map.get("count"), Some(&TomlValue::Integer(123)));
        } else {
            panic!("Expected Table");
        }
    }

    #[test]
    fn test_parser_sections() {
        let input = r#"
            global = "init"

            [database]
            server = "192.168.1.1"
            ports = 8080

            [users]
            admin = "root"
        "#;
        let mut parser = Parser::new(input).unwrap();
        let result = parser.parse().unwrap();

        if let TomlValue::Table(root) = result {
            assert_eq!(
                root.get("global"),
                Some(&TomlValue::String("init".to_string()))
            );

            if let Some(TomlValue::Table(db)) = root.get("database") {
                assert_eq!(
                    db.get("server"),
                    Some(&TomlValue::String("192.168.1.1".to_string()))
                );
                assert_eq!(db.get("ports"), Some(&TomlValue::Integer(8080)));
            } else {
                panic!("Missing database section");
            }

            if let Some(TomlValue::Table(users)) = root.get("users") {
                assert_eq!(
                    users.get("admin"),
                    Some(&TomlValue::String("root".to_string()))
                );
            } else {
                panic!("Missing users section");
            }
        }
    }

    #[test]
    fn test_comments() {
        let input = r"
            # This is a comment
            key = 1 # Inline comment
        ";
        let mut parser = Parser::new(input).unwrap();
        let result = parser.parse().unwrap();

        if let TomlValue::Table(root) = result {
            assert_eq!(root.get("key"), Some(&TomlValue::Integer(1)));
        }
    }
}
