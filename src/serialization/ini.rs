//! # Simple INI Parser
//!
//! Implements a basic INI file parser from scratch.
//!
//! **Replaces Crates:** `rust-ini`, `config`
//!
//! **Why build it yourself?**
//! INI is the simplest structured configuration format.
//! Implementing it teaches string parsing, state machines, and error handling.
//!
//! # Grammar
//!
//! ```ini
//! ; Comment
//! [section]
//! key = value
//! ```

use std::collections::HashMap;
use std::fmt;

#[derive(Debug, PartialEq, Eq)]
pub struct Ini {
    pub sections: HashMap<String, HashMap<String, String>>,
}

impl Default for Ini {
    fn default() -> Self {
        Self::new()
    }
}

impl Ini {
    #[must_use]
    pub fn new() -> Self {
        Self {
            sections: HashMap::new(),
        }
    }

    /// Parses INI-formatted text into an [`Ini`] document.
    ///
    /// # Errors
    ///
    /// Returns a [`ParseError`] if a line is neither a comment, a section
    /// header, nor a valid `key = value` assignment.
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        let mut ini = Self::new();
        let mut current_section = "default".to_string();

        for (line_idx, line) in input.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
                continue;
            }

            if line.starts_with('[') && line.ends_with(']') {
                // Section
                current_section = line[1..line.len() - 1].trim().to_string();
            } else if let Some((key, value)) = line.split_once('=') {
                // Key-Value
                let key = key.trim().to_string();
                let value = value.trim().to_string();

                ini.sections
                    .entry(current_section.clone())
                    .or_default()
                    .insert(key, value);
            } else {
                return Err(ParseError {
                    line: line_idx + 1,
                    message: "Invalid syntax".to_string(),
                });
            }
        }

        Ok(ini)
    }

    #[must_use]
    pub fn get(&self, section: &str, key: &str) -> Option<&String> {
        self.sections.get(section).and_then(|s| s.get(key))
    }
}

#[derive(Debug)]
pub struct ParseError {
    line: usize,
    message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Error at line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for ParseError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic() {
        let input = r"
            ; This is a comment
            [server]
            host = localhost
            port = 8080

            [database]
            url = postgres://user:pass@localhost/db
        ";

        let ini = Ini::parse(input).unwrap();

        assert_eq!(ini.get("server", "host"), Some(&"localhost".to_string()));
        assert_eq!(ini.get("server", "port"), Some(&"8080".to_string()));
        assert_eq!(
            ini.get("database", "url"),
            Some(&"postgres://user:pass@localhost/db".to_string())
        );
    }

    #[test]
    fn test_default_section() {
        let input = "debug = true";
        let ini = Ini::parse(input).unwrap();
        assert_eq!(ini.get("default", "debug"), Some(&"true".to_string()));
    }

    #[test]
    fn test_invalid_syntax() {
        let input = "invalid line without equals";
        assert!(Ini::parse(input).is_err());
    }
}
