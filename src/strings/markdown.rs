//! # Minimal State-Machine Markdown Parser
//!
//! Replaces Crates: `pulldown-cmark`, `comrak`
//!
//! Real-world Usage:
//! - Static site generators (Hugo, Zola)
//! - Chat applications for rich text rendering (Discord, Slack)
//! - Code forges and issue trackers (GitHub, GitLab)
//!
//! Why build it yourself?
//! Parsing Markdown is notoriously complex due to its context-sensitive rules and
//! lack of a formal, clean grammar. Building a parser from scratch using a state machine
//! teaches you how to avoid allocating massive Abstract Syntax Trees (ASTs) by emitting
//! output incrementally. It highlights the power of Rust's enums for state representation
//! and lifetimes for zero-copy string processing.
//!
//! ## Architecture
//!
//! The parser uses a single-pass, state-machine-based approach. Instead of building an
//! AST (like a tree of `Node` structs) and then rendering it, it processes the input
//! line-by-line (or chunk-by-chunk) and directly mutates an output HTML buffer.
//!
//! State Machine Transitions:
//! ```text
//! [Normal]
//!   |-- "# " --> [Header] (flushed immediately)
//!   |-- "```" -> [CodeBlock] (buffers lines until "```")
//!   |-- "* " --> [List] (buffers items until non-list line)
//!   |-- "\n" --> (flush buffer as paragraph)
//! ```
//!
//! Invariants:
//! - Block-level elements must close and flush their buffered content when transitioning.
//! - Inline formatting state (bold/italic) persists across sequential text chunks *within*
//!   the same block, but resets across blocks.
//! - Single-line elements (like Headers) flush immediately to the output buffer to
//!   prevent missing newlines or improper accumulation.
//!
//! Complexity:
//! - Time Complexity: O(N) where N is the length of the Markdown string. Each character
//!   is processed a constant number of times.
//! - Space Complexity: O(B) where B is the size of the largest block being buffered
//!   (e.g., a massive paragraph or code block). Output buffer is O(N). We avoid full O(N)
//!   AST overhead.
//!
//! Tradeoffs:
//! - This avoids an AST, meaning we cannot easily perform transformations on the document
//!   structure before rendering (e.g., building a Table of Contents requires a separate pass or AST).
//! - We handle a subset of Markdown to demonstrate the core architecture.
//!
//! ## Comparison to Canonical Crates
//!
//! - `pulldown-cmark` is a highly compliant CommonMark parser that emits an event stream
//!   (similar to our state-machine approach but far more robust).
//! - `comrak` builds an AST, allowing full AST manipulation before rendering to HTML.
//!
//! ## Missing for Production
//!
//! - Full CommonMark compliance (nested lists, blockquotes, HTML blocks, link references).
//! - Complex inline parsing (nested bold/italic overlaps).
//! - Security: production parsers must sanitize output to prevent XSS (e.g., `<script>` tags).
//!
//! ## Next Steps / Extensions
//!
//! - Implement a pull-parser iterator (like `pulldown-cmark`) that yields `Event` enums
//!   instead of writing directly to a string.
//! - Add support for inline links `[text](url)` and images `![alt](url)`.
//!
//! ## Benchmarking Note
//!
//! To benchmark this parser, one would use `criterion` to parse large `.md` files
//! (e.g., 10MB of text) and compare the allocations against `pulldown-cmark`.
//! The lack of AST should show lower peak memory usage.

use std::fmt::Write;

/// Represents the current block-level state of the parser.
#[derive(Debug, PartialEq)]
enum BlockState {
    Normal,
    CodeBlock { language: String },
    Paragraph,
    List,
}

/// A minimal state-machine based Markdown to HTML parser.
pub struct StateMachineParser<'a> {
    input: &'a str,
}

impl<'a> StateMachineParser<'a> {
    /// Creates a new parser for the given markdown string.
    pub fn new(input: &'a str) -> Self {
        Self { input }
    }

    /// Parses the markdown input and returns the rendered HTML string.
    pub fn parse(&self) -> String {
        // PRODUCTION NOTE: A production parser might take a `&mut dyn std::fmt::Write`
        // to write directly to a file/network socket, avoiding `String` allocation entirely.
        let mut output = String::with_capacity(self.input.len() * 2);

        let mut state = BlockState::Normal;
        let mut buffer = String::new();

        // RUST INSIGHT:
        // We use `.lines()` but remember that it strips `\n`. For blocks like CodeBlock
        // where formatting matters, we must manually re-append newlines.
        let lines = self.input.lines().peekable();

        for line in lines {
            let trimmed = line.trim();

            match state {
                BlockState::Normal => {
                    if trimmed.is_empty() {
                        continue;
                    }

                    if let Some(stripped) = line.strip_prefix("```") {
                        let language = stripped.trim().to_string();
                        state = BlockState::CodeBlock { language };
                    } else if let Some(heading_level) = self.parse_heading(line) {
                        // Memory Constraint: Single-line elements like Headers should be
                        // flushed immediately to the output buffer rather than buffering.
                        let content = line[heading_level + 1..].trim();
                        let inline_html = self.parse_inline(content);
                        // GOTCHA: We use format_args! or write! to avoid intermediate format! String allocation
                        let _ = writeln!(
                            &mut output,
                            "<h{}>{}</h{}>",
                            heading_level, inline_html, heading_level
                        );
                    } else if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
                        state = BlockState::List;
                        let _ = writeln!(&mut output, "<ul>");
                        let content = trimmed[2..].trim();
                        let inline_html = self.parse_inline(content);
                        let _ = writeln!(&mut output, "<li>{}</li>", inline_html);
                    } else {
                        state = BlockState::Paragraph;
                        buffer.push_str(line);
                    }
                }
                BlockState::CodeBlock { ref language } => {
                    if line.starts_with("```") {
                        let lang_class = if language.is_empty() {
                            String::new()
                        } else {
                            format!(" class=\"language-{}\"", language)
                        };

                        let _ = writeln!(&mut output, "<pre><code{}>", lang_class);
                        // Memory Constraint: explicitly trim trailing whitespace/newlines from consumed text
                        // before wrapping. However, for CodeBlocks we actually want the content verbatim,
                        // but trailing empty lines can be trimmed.
                        output.push_str(buffer.trim_end());
                        output.push('\n');
                        let _ = writeln!(&mut output, "</code></pre>");

                        buffer.clear();
                        state = BlockState::Normal;
                    } else {
                        // Memory Constraint: To preserve multiline formatting in verbatim blocks,
                        // explicit \n must be manually re-appended.
                        buffer.push_str(line);
                        buffer.push('\n');
                    }
                }
                BlockState::Paragraph => {
                    if trimmed.is_empty() {
                        // End of paragraph
                        // Memory Constraint: explicitly trim trailing whitespace/newlines
                        let content = buffer.trim_end();
                        if !content.is_empty() {
                            let inline_html = self.parse_inline(content);
                            let _ = writeln!(&mut output, "<p>{}</p>", inline_html);
                        }
                        buffer.clear();
                        state = BlockState::Normal;
                    } else if line.starts_with("```")
                        || self.parse_heading(line).is_some()
                        || trimmed.starts_with("- ")
                        || trimmed.starts_with("* ")
                    {
                        // Block interrupt
                        let content = buffer.trim_end();
                        if !content.is_empty() {
                            let inline_html = self.parse_inline(content);
                            let _ = writeln!(&mut output, "<p>{}</p>", inline_html);
                        }
                        buffer.clear();

                        // Re-evaluate line in Normal state
                        state = BlockState::Normal;
                        if let Some(stripped) = line.strip_prefix("```") {
                            let language = stripped.trim().to_string();
                            state = BlockState::CodeBlock { language };
                        } else if let Some(heading_level) = self.parse_heading(line) {
                            let content = line[heading_level + 1..].trim();
                            let inline_html = self.parse_inline(content);
                            let _ = writeln!(
                                &mut output,
                                "<h{}>{}</h{}>",
                                heading_level, inline_html, heading_level
                            );
                        } else if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
                            state = BlockState::List;
                            let _ = writeln!(&mut output, "<ul>");
                            let content = trimmed[2..].trim();
                            let inline_html = self.parse_inline(content);
                            let _ = writeln!(&mut output, "<li>{}</li>", inline_html);
                        }
                    } else {
                        // Continuation of paragraph
                        buffer.push('\n');
                        buffer.push_str(line);
                    }
                }
                BlockState::List => {
                    if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
                        let content = trimmed[2..].trim();
                        let inline_html = self.parse_inline(content);
                        let _ = writeln!(&mut output, "<li>{}</li>", inline_html);
                    } else if trimmed.is_empty() {
                        // End of list
                        let _ = writeln!(&mut output, "</ul>");
                        state = BlockState::Normal;
                    } else {
                        // Interrupt list
                        let _ = writeln!(&mut output, "</ul>");

                        if let Some(stripped) = line.strip_prefix("```") {
                            let language = stripped.trim().to_string();
                            state = BlockState::CodeBlock { language };
                        } else if let Some(heading_level) = self.parse_heading(line) {
                            let content = line[heading_level + 1..].trim();
                            let inline_html = self.parse_inline(content);
                            let _ = writeln!(
                                &mut output,
                                "<h{}>{}</h{}>",
                                heading_level, inline_html, heading_level
                            );
                        } else {
                            state = BlockState::Paragraph;
                            buffer.push_str(line);
                        }
                    }
                }
            }
        }

        // Flush remaining state
        match state {
            BlockState::Paragraph => {
                let content = buffer.trim_end();
                if !content.is_empty() {
                    let inline_html = self.parse_inline(content);
                    let _ = writeln!(&mut output, "<p>{}</p>", inline_html);
                }
            }
            BlockState::CodeBlock { language } => {
                // Unclosed code block.
                let lang_class = if language.is_empty() {
                    String::new()
                } else {
                    format!(" class=\"language-{}\"", language)
                };
                let _ = writeln!(&mut output, "<pre><code{}>", lang_class);
                output.push_str(buffer.trim_end());
                output.push('\n');
                let _ = writeln!(&mut output, "</code></pre>");
            }
            BlockState::List => {
                let _ = writeln!(&mut output, "</ul>");
            }
            BlockState::Normal => {}
        }

        output
    }

    /// Determines if a line is a heading and returns its level (1-6).
    fn parse_heading(&self, line: &str) -> Option<usize> {
        let mut level = 0;
        for c in line.chars() {
            if c == '#' {
                level += 1;
            } else {
                break;
            }
        }
        if level > 0 && level <= 6 && line[level..].starts_with(' ') {
            Some(level)
        } else {
            None
        }
    }

    /// Parses inline formatting (bold, italic, code) within a block.
    fn parse_inline(&self, text: &str) -> String {
        let mut output = String::with_capacity(text.len());
        let mut chars = text.chars().peekable();

        // Memory Constraint: inline formatting state must persist across sequential text chunks
        // within the same block element. Since `parse_inline` is called per paragraph/header block,
        // it persists for the whole block content passed to it.
        let mut in_bold = false;
        let mut in_italic = false;
        let mut in_code = false;

        while let Some(c) = chars.next() {
            match c {
                '`' => {
                    if in_code {
                        output.push_str("</code>");
                        in_code = false;
                    } else {
                        output.push_str("<code>");
                        in_code = true;
                    }
                }
                '*' if !in_code => {
                    if let Some(&'*') = chars.peek() {
                        // It's bold
                        chars.next(); // consume second '*'
                        if in_bold {
                            output.push_str("</strong>");
                            in_bold = false;
                        } else {
                            output.push_str("<strong>");
                            in_bold = true;
                        }
                    } else {
                        // It's italic
                        if in_italic {
                            output.push_str("</em>");
                            in_italic = false;
                        } else {
                            output.push_str("<em>");
                            in_italic = true;
                        }
                    }
                }
                '_' if !in_code => {
                    if let Some(&'_') = chars.peek() {
                        chars.next(); // consume second '_'
                        if in_bold {
                            output.push_str("</strong>");
                            in_bold = false;
                        } else {
                            output.push_str("<strong>");
                            in_bold = true;
                        }
                    } else {
                        if in_italic {
                            output.push_str("</em>");
                            in_italic = false;
                        } else {
                            output.push_str("<em>");
                            in_italic = true;
                        }
                    }
                }
                '<' => {
                    // Escape HTML
                    output.push_str("&lt;");
                }
                '>' => {
                    // Escape HTML
                    output.push_str("&gt;");
                }
                '&' => {
                    // Escape HTML
                    output.push_str("&amp;");
                }
                _ => {
                    output.push(c);
                }
            }
        }

        // Close any unclosed tags
        if in_code {
            output.push_str("</code>");
        }
        if in_bold {
            output.push_str("</strong>");
        }
        if in_italic {
            output.push_str("</em>");
        }

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_headings() {
        let input = "# Heading 1\n## Heading 2\n### Heading 3";
        let parser = StateMachineParser::new(input);
        let html = parser.parse();
        assert_eq!(
            html,
            "<h1>Heading 1</h1>\n<h2>Heading 2</h2>\n<h3>Heading 3</h3>\n"
        );
    }

    #[test]
    fn test_paragraph_and_inline() {
        let input = "This is a **bold** and *italic* paragraph with `code`.\n\nAnother paragraph.";
        let parser = StateMachineParser::new(input);
        let html = parser.parse();
        assert_eq!(
            html,
            "<p>This is a <strong>bold</strong> and <em>italic</em> paragraph with <code>code</code>.</p>\n<p>Another paragraph.</p>\n"
        );
    }

    #[test]
    fn test_code_block() {
        let input = "```rust\nfn main() {\n    println!(\"Hello\");\n}\n```";
        let parser = StateMachineParser::new(input);
        let html = parser.parse();
        assert_eq!(
            html,
            "<pre><code class=\"language-rust\">\nfn main() {\n    println!(\"Hello\");\n}\n</code></pre>\n"
        );
    }

    #[test]
    fn test_lists() {
        let input = "- Item 1\n- Item 2\n* Item 3\n\nParagraph after.";
        let parser = StateMachineParser::new(input);
        let html = parser.parse();
        assert_eq!(
            html,
            "<ul>\n<li>Item 1</li>\n<li>Item 2</li>\n<li>Item 3</li>\n</ul>\n<p>Paragraph after.</p>\n"
        );
    }

    #[test]
    fn test_html_escaping() {
        let input = "Some <script> alert(1); </script> & > code";
        let parser = StateMachineParser::new(input);
        let html = parser.parse();
        assert_eq!(
            html,
            "<p>Some &lt;script&gt; alert(1); &lt;/script&gt; &amp; &gt; code</p>\n"
        );
    }

    #[test]
    fn test_inline_state_persistence_in_block() {
        // "inline formatting state (e.g., toggling bold or italic) must persist across sequential text chunks within the same block element"
        let input = "This is **bold\nand still bold** text.";
        let parser = StateMachineParser::new(input);
        let html = parser.parse();
        assert_eq!(
            html,
            "<p>This is <strong>bold\nand still bold</strong> text.</p>\n"
        );
    }
}
