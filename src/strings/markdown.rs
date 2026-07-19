//! # Markdown Parser
//!
//! Replaces crates like `pulldown-cmark` and `comrak`.
//!
//! **Real-world systems that use this:** GitHub's rendering pipeline, static site generators
//! like Hugo or Zola, and documentation tools like rustdoc.
//!
//! **Why build it yourself?**
//! Writing a Markdown parser demystifies how structured text is converted to HTML.
//! It teaches you state-machine parsing, how to handle inline vs block elements,
//! and the performance benefits of avoiding full AST construction when a simple
//! stream-based approach suffices.
//!
//! ## Architecture
//!
//! We implement a state-machine-based parser that processes text line by line.
//! By avoiding a full Abstract Syntax Tree (AST), we optimize memory efficiency.
//!
//! ### Invariants
//! - Inline formatting state (e.g., bold toggles) persists across chunks within the same block.
//! - Single-line elements (like Headers) are flushed to the output immediately.
//! - Trailing whitespaces are trimmed from consumed text before wrapping in HTML elements like `<p>`.
//! - Code blocks preserve internal newlines.
//!
//! ### Complexity
//! - **Time Complexity:** O(N) where N is the length of the input, as each character is processed
//!   a constant number of times.
//! - **Space Complexity:** O(B) where B is the size of the largest block, since we only buffer
//!   one block at a time before flushing it to the output string.
//!
//! ### Tradeoffs
//! This minimal parser is faster and uses less memory than an AST-based parser but
//! is less extensible and cannot handle complex nested markdown rules (e.g., nested lists
//! or complex table structures).
//!
//! ## Comparison and Next Steps
//! **Comparison:** `pulldown-cmark` uses a pull-parser approach generating events, which are then
//! collected or rendered. Our approach writes directly to an output buffer per block.
//!
//! **Missing vs Production:**
//! - No support for lists, links, images, blockquotes, or tables.
//! - HTML escaping is not implemented.
//!
//! **Next Steps:**
//! - Implement link parsing (`[text](url)`).
//! - Add HTML escaping for safe rendering.

use std::fmt::Write;

pub trait MarkdownParser {
    fn parse(&mut self, input: &str) -> String;
}

#[derive(Debug, PartialEq, Eq)]
enum ParserState {
    Normal,
    CodeBlock,
    Paragraph,
}

pub struct StateMachineParser {
    state: ParserState,
    buffer: String,
    output: String,
    in_bold: bool,
    in_italic: bool,
}

impl Default for StateMachineParser {
    fn default() -> Self {
        Self::new()
    }
}

impl StateMachineParser {
    pub fn new() -> Self {
        Self {
            state: ParserState::Normal,
            // RUST INSIGHT:
            // Pre-allocating capacity avoids intermediate heap allocations during iterative appends.
            buffer: String::with_capacity(1024),
            output: String::with_capacity(1024),
            in_bold: false,
            in_italic: false,
        }
    }

    fn flush_buffer(&mut self) {
        match self.state {
            ParserState::Normal => {}
            ParserState::Paragraph => {
                let trimmed = self.buffer.trim_end();
                if !trimmed.is_empty() {
                    let trimmed_string = trimmed.to_string();
                    let content = self.parse_inline(&trimmed_string);
                    // GOTCHA:
                    // Using write! directly appends to the String buffer without additional allocation.
                    let _ = write!(&mut self.output, "<p>{}</p>", content);
                }
            }
            ParserState::CodeBlock => {
                let trimmed = self.buffer.trim_end();
                let _ = write!(&mut self.output, "<pre><code>{}</code></pre>", trimmed);
            }
        }
        self.buffer.clear();
        self.state = ParserState::Normal;
        self.in_bold = false;
        self.in_italic = false;
    }

    fn parse_inline(&mut self, text: &str) -> String {
        let mut result = String::with_capacity(text.len());
        let mut chars = text.chars().peekable();

        while let Some(c) = chars.next() {
            if c == '*' {
                if let Some(&'*') = chars.peek() {
                    chars.next(); // Consume second '*'
                    if self.in_bold {
                        result.push_str("</b>");
                    } else {
                        result.push_str("<b>");
                    }
                    self.in_bold = !self.in_bold;
                } else {
                    if self.in_italic {
                        result.push_str("</i>");
                    } else {
                        result.push_str("<i>");
                    }
                    self.in_italic = !self.in_italic;
                }
            } else {
                result.push(c);
            }
        }
        result
    }
}

impl MarkdownParser for StateMachineParser {
    fn parse(&mut self, input: &str) -> String {
        self.output.clear();
        self.buffer.clear();
        self.state = ParserState::Normal;
        self.in_bold = false;
        self.in_italic = false;

        // RUST INSIGHT:
        // `.lines()` automatically splits by `\n` or `\r\n` and strips the line endings.
        for line in input.lines() {
            match self.state {
                ParserState::Normal | ParserState::Paragraph => {
                    if line.starts_with("```") {
                        self.flush_buffer();
                        self.state = ParserState::CodeBlock;
                    } else if line.starts_with('#') {
                        self.flush_buffer();

                        let mut level = 0;
                        for c in line.chars() {
                            if c == '#' {
                                level += 1;
                            } else {
                                break;
                            }
                        }

                        // PRODUCTION NOTE:
                        // Proper bounds checking is critical when slicing strings manually, as
                        // missing spaces or truncated delimiters can cause out-of-bounds panics.
                        let mut start_idx = level;
                        if line.len() > level && line[level..].starts_with(' ') {
                            start_idx = level + 1;
                        }

                        if start_idx <= line.len() {
                            let content = self.parse_inline(line[start_idx..].trim_end());
                            let _ =
                                write!(&mut self.output, "<h{}>{}</h{}>", level, content, level);

                            // Reset state because headers are immediately flushed without entering the buffer.
                            self.in_bold = false;
                            self.in_italic = false;
                        }
                    } else if line.trim().is_empty() {
                        self.flush_buffer();
                    } else {
                        if self.state == ParserState::Normal {
                            self.state = ParserState::Paragraph;
                        } else {
                            self.buffer.push('\n');
                        }
                        self.buffer.push_str(line);
                    }
                }
                ParserState::CodeBlock => {
                    if line.starts_with("```") {
                        self.flush_buffer();
                    } else {
                        self.buffer.push_str(line);
                        self.buffer.push('\n');
                    }
                }
            }
        }

        self.flush_buffer();
        self.output.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_headers() {
        let mut parser = StateMachineParser::new();
        let html = parser.parse("# Header 1\n## Header 2");
        assert_eq!(html, "<h1>Header 1</h1><h2>Header 2</h2>");
    }

    #[test]
    fn test_paragraphs() {
        let mut parser = StateMachineParser::new();
        let html = parser.parse("Hello\nWorld\n\nNew Paragraph");
        assert_eq!(html, "<p>Hello\nWorld</p><p>New Paragraph</p>");
    }

    #[test]
    fn test_inline_formatting() {
        let mut parser = StateMachineParser::new();
        let html = parser.parse("This is **bold** and *italic* text.");
        assert_eq!(html, "<p>This is <b>bold</b> and <i>italic</i> text.</p>");
    }

    #[test]
    fn test_code_blocks() {
        let mut parser = StateMachineParser::new();
        let markdown = "```\nfn main() {\n    println!(\"Hello\");\n}\n```";
        let html = parser.parse(markdown);
        assert_eq!(
            html,
            "<pre><code>fn main() {\n    println!(\"Hello\");\n}</code></pre>"
        );
    }

    #[test]
    fn test_mixed_content() {
        let mut parser = StateMachineParser::new();
        let markdown = "# Title\n\nSome **bold** text.\n\n```\ncode\n```";
        let html = parser.parse(markdown);
        assert_eq!(
            html,
            "<h1>Title</h1><p>Some <b>bold</b> text.</p><pre><code>code</code></pre>"
        );
    }

    #[test]
    fn test_bounds_checking_header_malformed() {
        let mut parser = StateMachineParser::new();
        let html = parser.parse("###"); // Header with no text or space
        assert_eq!(html, "<h3></h3>");
    }
}
