//! # Markdown Parser
//!
//! An educational implementation of a minimal state-machine-based Markdown parser.
//! This replaces crates like `pulldown-cmark` or `comrak` to demonstrate how to build
//! a zero-allocation parsing engine that streams tokens directly to HTML without building
//! a full AST (Abstract Syntax Tree) intermediate representation.
//!
//! Real-world usage: Lightweight static site generators or chat formatting.
//! Why build it: To understand string slices (`&str`), iterative parsing, bounds checking,
//! and maintaining minimal state across line streams for optimal memory efficiency.
//!
//! ## Architecture
//!
//! ```text
//! +---------------+      Line by line     +----------------+
//! |               | --------------------->|                |
//! |   Input str   |                       | State Machine  |
//! |               |                       |                |
//! +---------------+                       +--------+-------+
//!                                                  | (flush)
//!                                                  v
//!                                         +----------------+
//!                                         |                |
//!                                         |   HTML String  |
//!                                         |                |
//!                                         +----------------+
//! ```
//!
//! - **Invariants:** The parser must handle arbitrary chunks and missing delimiters gracefully.
//!   Inline state (bold/italic) resets at block boundaries.
//! - **Complexity:** Parsing is O(N) where N is the length of the string. Space is O(M) where
//!   M is the output HTML size. No AST nodes are allocated.
//! - **Tradeoffs:** Since we don't build an AST, advanced multi-pass operations (like reference
//!   links defined at the bottom of the document) are not supported.

/// Internal state tracker for block-level elements
#[derive(Debug, PartialEq, Eq)]
enum ParserState {
    Normal,
    InParagraph,
    InCodeBlock,
}

pub struct MarkdownParser {
    output: String,
    state: ParserState,
    code_block_content: String,
}

impl MarkdownParser {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            output: String::new(),
            state: ParserState::Normal,
            code_block_content: String::new(),
        }
    }

    /// Parses a complete markdown string into HTML.
    pub fn parse(&mut self, markdown: &str) -> String {
        // GOTCHA: `str::lines()` strips `\n` characters. We must remember this when parsing
        // multiline structures like CodeBlocks where we want to preserve them.
        for line in markdown.lines() {
            self.process_line(line);
        }

        // Ensure we flush any trailing state at EOF
        self.flush_state();

        std::mem::take(&mut self.output)
    }

    fn process_line(&mut self, line: &str) {
        let trimmed = line.trim();

        // 1. Handle Code Blocks
        if trimmed.starts_with("```") {
            if self.state == ParserState::InCodeBlock {
                // Close block
                self.output.push_str("<pre><code>");
                self.output
                    .push_str(&Self::escape_html(&self.code_block_content));
                self.output.push_str("</code></pre>\n");
                self.code_block_content.clear();
                self.state = ParserState::Normal;
            } else {
                // Open block
                self.flush_state();
                self.state = ParserState::InCodeBlock;
            }
            return;
        }

        if self.state == ParserState::InCodeBlock {
            // RUST INSIGHT: We manually re-append `\n` here because `.lines()` stripped it.
            self.code_block_content.push_str(line);
            self.code_block_content.push('\n');
            return;
        }

        // 2. Empty Lines (break paragraphs)
        if trimmed.is_empty() {
            self.flush_state();
            return;
        }

        // 3. Headers
        if trimmed.starts_with('#') {
            let mut level = 0;
            for c in trimmed.chars() {
                if c == '#' {
                    level += 1;
                } else {
                    break;
                }
            }

            // Ensure bounds checking when slicing!
            if level > 0 && level <= 6 && trimmed.len() > level {
                // Must be followed by space
                if trimmed.as_bytes()[level] == b' ' {
                    self.flush_state();
                    // GOTCHA: Single-line elements must be flushed to the HTML buffer immediately
                    // upon parsing. Do not buffer them.
                    let text = &trimmed[level + 1..];
                    let parsed = Self::parse_inline(text);
                    self.output
                        .push_str(&format!("<h{}>{}</h{}>\n", level, parsed, level));
                    return;
                }
            }
        }

        // 4. Paragraphs (Normal text)
        if self.state == ParserState::Normal {
            self.state = ParserState::InParagraph;
            self.output.push_str("<p>");
        } else if self.state == ParserState::InParagraph {
            self.output.push(' ');
        }

        let parsed = Self::parse_inline(trimmed);
        self.output.push_str(&parsed);
    }

    fn flush_state(&mut self) {
        if self.state == ParserState::InParagraph {
            self.output.push_str("</p>\n");
            self.state = ParserState::Normal;
        }
    }

    /// Parses inline formatting like **bold** and *italic*
    fn parse_inline(text: &str) -> String {
        let mut result = String::with_capacity(text.len());
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;

        let mut in_bold = false;
        let mut in_italic = false;

        while i < chars.len() {
            // Bold
            if i + 1 < chars.len() && chars[i] == '*' && chars[i + 1] == '*' {
                if in_bold {
                    result.push_str("</strong>");
                } else {
                    result.push_str("<strong>");
                }
                in_bold = !in_bold;
                i += 2;
                continue;
            }

            // Italic
            if chars[i] == '*' {
                if in_italic {
                    result.push_str("</em>");
                } else {
                    result.push_str("<em>");
                }
                in_italic = !in_italic;
                i += 1;
                continue;
            }

            result.push(chars[i]);
            i += 1;
        }

        // Auto-close any unclosed tags to prevent formatting leaks
        if in_bold {
            result.push_str("</strong>");
        }
        if in_italic {
            result.push_str("</em>");
        }

        result
    }

    fn escape_html(text: &str) -> String {
        let mut result = String::with_capacity(text.len());
        for c in text.chars() {
            match c {
                '<' => result.push_str("&lt;"),
                '>' => result.push_str("&gt;"),
                '"' => result.push_str("&quot;"),
                '&' => result.push_str("&amp;"),
                _ => result.push(c),
            }
        }
        result
    }
}

impl Default for MarkdownParser {
    fn default() -> Self {
        Self::new()
    }
}

// Footer:
// Missing features: Lists, Blockquotes, Links, Images.
// Compare with: pulldown-cmark (events based, highly compliant).
// Next steps: Add an Event stream interface.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_markdown_headers() {
        let mut parser = MarkdownParser::new();
        let html = parser.parse("# Header 1\n## Header 2");
        assert_eq!(html, "<h1>Header 1</h1>\n<h2>Header 2</h2>\n");
    }

    #[test]
    fn test_markdown_paragraph_and_inline() {
        let mut parser = MarkdownParser::new();
        let html = parser.parse("Hello **world**\nand *universe*!");
        assert_eq!(
            html,
            "<p>Hello <strong>world</strong> and <em>universe</em>!</p>\n"
        );
    }

    #[test]
    fn test_markdown_code_block() {
        let mut parser = MarkdownParser::new();
        let markdown = "```\nfn main() {\n    println!(\"<hello>\");\n}\n```";
        let html = parser.parse(markdown);
        assert_eq!(
            html,
            "<pre><code>fn main() {\n    println!(&quot;&lt;hello&gt;&quot;);\n}\n</code></pre>\n"
        );
    }

    #[test]
    fn test_markdown_unclosed_inline() {
        let mut parser = MarkdownParser::new();
        let html = parser.parse("This is **bold text");
        assert_eq!(html, "<p>This is <strong>bold text</strong></p>\n");
    }

    #[test]
    fn test_markdown_missing_space_header() {
        let mut parser = MarkdownParser::new();
        // Missing space after # shouldn't parse as a header
        let html = parser.parse("#NotAHeader");
        assert_eq!(html, "<p>#NotAHeader</p>\n");
    }
}
