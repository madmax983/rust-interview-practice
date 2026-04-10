//! # CSV Parser Implementation
//!
//! Implements a robust, state-machine-based CSV parser that handles quoted fields,
//! escaped quotes, and embedded newlines.
//!
//! **Replaces Crates:** `csv`
//!
//! **Real-world Usage:**
//! - Data ingestion pipelines (ETL).
//! - Spreadsheet import/export.
//! - Configuration data loading.
//!
//! **Why build it yourself?**
//! Parsing CSV seems trivial until you hit quoted fields containing commas, escaped quotes (`""`),
//! and embedded newlines that span across physical lines. Building a state machine for CSV parsing
//! teaches you how to manage parsing state elegantly and process chunked text without allocating
//! excessively.

use std::io::{self, BufRead};

// =========================================================================================
// Architecture
// =========================================================================================
//
// State Machine:
// The parser operates as a state machine processing character by character.
// States: UnquotedField, QuotedField, QuoteInQuotedField
//
// Flow:
// 1. Read a chunk of text (a line).
// 2. Iterate through characters.
// 3. Accumulate characters into the current field buffer.
// 4. Handle state transitions based on commas, quotes, and newlines.
// 5. Yield a row (`Vec<String>`) when a logical line ends.
//
// Invariants:
// 1. A quoted field ignores commas and newlines until a closing quote is found.
// 2. Double quotes inside a quoted field (`""`) evaluate to a single literal quote.
// 3. The iterator yields `Result<Vec<String>, Error>` to properly handle I/O issues.
//
// Complexity:
// ┌───────────┬──────────────┬────────┐
// │ Operation │ Time         │ Space  │
// ├───────────┼──────────────┼────────┤
// │ next()    │ O(Row Len)   │ O(Row) │
// └───────────┴──────────────┴────────┘
//
// Design Decisions:
// - **Iterator interface**: We implement `Iterator` to allow lazy evaluation.
// - **String Allocation**: We allocate a `String` for each field.
//   - *Alternative*: Zero-copy parsing yielding `&str` slices of a buffer. This is significantly
//     harder for fields with escaped quotes or embedded newlines because they require mutation
//     or discontiguous slices. `csv` crate uses a specialized byte record.

/// Configuration for the CSV parser.
#[derive(Clone, Copy, Debug)]
pub struct CsvConfig {
    pub delimiter: char,
    pub quote: char,
}

impl Default for CsvConfig {
    fn default() -> Self {
        Self {
            delimiter: ',',
            quote: '"',
        }
    }
}

/// A CSV reader that implements Iterator to yield rows.
pub struct CsvReader<R: BufRead> {
    reader: R,
    config: CsvConfig,
    // Buffer for reading lines
    line_buf: String,
    // Buffer for the current field being parsed
    field_buf: String,
    // Track if we reached EOF
    eof: bool,
}

impl<R: BufRead> CsvReader<R> {
    /// Creates a new CSV reader with default configuration.
    pub fn new(reader: R) -> Self {
        Self::with_config(reader, CsvConfig::default())
    }

    /// Creates a new CSV reader with custom configuration.
    pub fn with_config(reader: R, config: CsvConfig) -> Self {
        Self {
            reader,
            config,
            line_buf: String::new(),
            field_buf: String::new(),
            eof: false,
        }
    }
}

// RUST INSIGHT: Implementing Iterator is the idiomatic way to provide lazy, stream-like access
// in Rust. It integrates seamlessly with `for` loops and combinators like `.map()` and `.filter()`.

impl<R: BufRead> Iterator for CsvReader<R> {
    type Item = io::Result<Vec<String>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.eof {
            return None;
        }

        let mut row = Vec::new();
        let mut in_quotes = false;

        // We use a state machine to parse the CSV properly.
        // Instead of reading one char at a time from I/O (which is slow),
        // we read line by line and process the characters in memory.

        self.field_buf.clear();

        loop {
            self.line_buf.clear();
            let bytes_read = match self.reader.read_line(&mut self.line_buf) {
                Ok(b) => b,
                Err(e) => return Some(Err(e)),
            };

            if bytes_read == 0 {
                self.eof = true;
                // If we hit EOF but we have accumulated a field, push it.
                // Or if we have a completely empty file, return None.
                if !row.is_empty() || !self.field_buf.is_empty() || in_quotes {
                    // Even if in quotes, we reached EOF, so we close the field.
                    // This handles malformed CSVs with missing closing quotes gracefully.
                    // BOLT OPTIMIZATION: Avoid `.clone()` and `.clear()` allocation overhead.
                    // Using `std::mem::take` directly moves the string buffer into the row, leaving an empty string
                    // behind and saving us a heap allocation per field. This prevents String reallocation when `field_buf.push(c)` runs later.
                    row.push(std::mem::take(&mut self.field_buf));
                    return Some(Ok(row));
                }
                return None;
            }

            let mut chars = self.line_buf.chars().peekable();

            while let Some(c) = chars.next() {
                if in_quotes {
                    if c == self.config.quote {
                        // Lookahead to check for escaped quote (`""`)
                        if let Some(&next_c) = chars.peek()
                            && next_c == self.config.quote {
                                // Escaped quote
                                self.field_buf.push(self.config.quote);
                                chars.next(); // Consume the second quote
                                continue;
                            }
                        // End of quoted section
                        in_quotes = false;
                    } else {
                        self.field_buf.push(c);
                    }
                } else {
                    if c == self.config.quote {
                        // Start of quoted section
                        in_quotes = true;
                    } else if c == self.config.delimiter {
                        // End of field
                        // PRODUCTION NOTE: We use `std::mem::take` to directly take ownership of the field buffer
                        // and push it to the row, leaving an empty String in `self.field_buf`.
                        // This entirely removes the `.clone()` and `.clear()` overhead.
                        row.push(std::mem::take(&mut self.field_buf));
                    } else if c == '\r' || c == '\n' {
                        // End of line.
                        // If it's `\r`, check if next is `\n`
                        if c == '\r'
                            && let Some(&'\n') = chars.peek() {
                                chars.next();
                            }

                        // We finish the row
                        row.push(std::mem::take(&mut self.field_buf));
                        return Some(Ok(row));
                    } else {
                        self.field_buf.push(c);
                    }
                }
            }

            // If we finish the line and we are NOT in quotes, it means the line
            // ended without a trailing newline character (EOF reached without newline).
            if !in_quotes {
                row.push(std::mem::take(&mut self.field_buf));
                self.eof = true;
                return Some(Ok(row));
            }
            // If we ARE in quotes, the newline is part of the field because it's in `line_buf`.
            // The loop continues and reads the next line.
        }
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `csv`: The standard Rust CSV crate is heavily optimized, using SIMD (via `csv-core`)
//   and zero-allocation byte records to achieve Gbps parsing speeds. It also handles
//   header rows, Serde integration (to deserialize directly to structs), and byte-level parsing.
//
// Missing vs. Production:
// - **Zero-copy/Allocation avoidance**: We allocate a `String` for every field.
// - **Serde Integration**: No automatic deserialization to structs.
// - **Byte-level parsing**: We parse using `String` and `chars()`, assuming valid UTF-8.
//   Real CSVs often have weird encodings, so parsing raw `&[u8]` is safer and faster.
// - **Headers**: No built-in logic to skip or map headers.
//
// Next Steps:
// 1. Convert to byte-level parsing (`read_until(b'\n')`).
// 2. Implement `ByteRecord` to avoid per-field allocations.
// 3. Add `Deserialize` support using `serde`.

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn parse_csv(data: &str) -> Vec<Vec<String>> {
        let cursor = Cursor::new(data);
        let reader = CsvReader::new(cursor);
        reader.map(|r| r.unwrap()).collect()
    }

    #[test]
    fn test_basic_csv() {
        let data = "name,age,city\nAlice,30,New York\nBob,25,Los Angeles";
        let rows = parse_csv(data);

        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0], vec!["name", "age", "city"]);
        assert_eq!(rows[1], vec!["Alice", "30", "New York"]);
        assert_eq!(rows[2], vec!["Bob", "25", "Los Angeles"]);
    }

    #[test]
    fn test_quoted_fields() {
        let data = "name,location\n\"Smith, Alice\",\"New York, NY\"\nBob,LA";
        let rows = parse_csv(data);

        assert_eq!(rows.len(), 3);
        assert_eq!(rows[1], vec!["Smith, Alice", "New York, NY"]);
        assert_eq!(rows[2], vec!["Bob", "LA"]);
    }

    #[test]
    fn test_escaped_quotes() {
        let data = "id,description\n1,\"This is a \"\"great\"\" product\"";
        let rows = parse_csv(data);

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1], vec!["1", "This is a \"great\" product"]);
    }

    #[test]
    fn test_embedded_newlines() {
        let data = "id,notes\n1,\"These are\nmultiple\r\nlines\"\n2,single";
        let rows = parse_csv(data);

        assert_eq!(rows.len(), 3);
        assert_eq!(rows[1], vec!["1", "These are\nmultiple\r\nlines"]);
        assert_eq!(rows[2], vec!["2", "single"]);
    }

    #[test]
    fn test_empty_fields() {
        let data = "a,b,c\n1,,3\n,,";
        let rows = parse_csv(data);

        assert_eq!(rows.len(), 3);
        assert_eq!(rows[1], vec!["1", "", "3"]);
        assert_eq!(rows[2], vec!["", "", ""]);
    }

    #[test]
    fn test_no_trailing_newline() {
        let data = "a,b\n1,2";
        let rows = parse_csv(data);

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1], vec!["1", "2"]);
    }
}
