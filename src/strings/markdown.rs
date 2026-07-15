//! # Minimal Markdown Parser
//!
//! **Replaces Crates:** `pulldown-cmark`, `comrak`
//!
//! **Real-world systems:**
//! - GitHub comments, Static Site Generators (Hugo, Zola), Documentation tools (Rustdoc)
//!
//! **Why build it yourself?**
//! Building a Markdown parser teaches how to construct finite state machines for text
//! processing and how to emit intermediate events (AST-less pull parsing) to save memory
//! over large documents, which is exactly how `pulldown-cmark` dominates in Rust.
//!
//! ## Architecture
//!
//! ```text
//! [Markdown String]
//!       | (Lines iterator)
//!       v
//! [Block Level Parser] (Paragraphs, Headers, CodeBlocks, Lists)
//!       | (Yields Block Events)
//!       v
//! [Inline Level Parser] (Bold, Italic, Code spans)
//!       | (Yields Inline Events or HTML directly)
//!       v
//! [HTML Output Buffer]
//! ```
//!
//! ### Invariants
//! - Block parsing determines structure (e.g., `# Header`). Inline parsing strictly operates within a block's boundaries.
//! - Single-line elements (like Headers) must be flushed to the HTML output buffer immediately to prevent them from improperly accumulating.
//! - Inline formatting state (like `is_bold` or `is_italic`) must persist across sequential text chunks within the same block element, and not be reset per word.
//!
//! ### Complexity
//! - **Time Complexity:** O(N) where N is the length of the string, traversing characters minimally.
//! - **Space Complexity:** O(M) where M is the size of the output HTML buffer (no intermediate full-AST is built).
//!
//! ### Tradeoffs vs Production
//! - Real parsers fully comply with the massive CommonMark specification, including handling complex nested lists, blockquotes, and HTML passthrough.
//! - We implement a simplified subset (Headers, Code Blocks, Paragraphs, Bold, Italic) without full CommonMark edge cases.
//!
//! ## Footer
//!
//! ### Crate Comparison
//! - `pulldown-cmark`: A robust, spec-compliant pull-parser emitting streams of `Event`s.
//! - This implementation: A direct-to-HTML parser using explicit state machines, skipping the intermediate event stream for simplicity.
//!
//! ### Missing Features
//! - Nested lists, links, images, blockquotes.
//! - Strict CommonMark compliance.
//!
//! ### Next Steps
//! - Refactor into an Iterator over `Event`s to decouple parsing from HTML generation.
//! - Add a Link reference resolution pass.
//!
//! --- Benchmarking Note ---
//! Use `std::hint::black_box` to pass a large raw markdown string into the `render_html`
//! function, testing raw iteration speed and `String::with_capacity` memory allocations.

use std::fmt::Write;

#[derive(Debug, PartialEq)]
enum BlockState {
    Normal,
    InParagraph,
    InCodeBlock,
}

pub struct MarkdownParser<'a> {
    input: &'a str,
}

impl<'a> MarkdownParser<'a> {
    #[must_use]
    pub fn new(input: &'a str) -> Self {
        Self { input }
    }

    /// Renders the parsed Markdown directly to HTML.
    ///
    /// # Errors
    /// Returns a string error if the `write!` macro fails (e.g. out of memory).
    pub fn render_html(&self) -> Result<String, String> {
        let mut output = String::with_capacity(self.input.len() * 2);
        let mut state = BlockState::Normal;
        let mut block_buffer = String::new();

        let lines = self.input.lines().peekable();

        for line in lines {
            match state {
                BlockState::Normal => {
                    if line.starts_with("```") {
                        state = BlockState::InCodeBlock;
                        // GOTCHA:
                        // Be careful to manually emit the `<pre><code>` exactly once when entering.
                        let _ = write!(output, "<pre><code>");
                    } else if let Some(content) = line.strip_prefix("# ") {
                        let _ = writeln!(output, "<h1>{}</h1>", Self::parse_inline(content));
                    } else if let Some(content) = line.strip_prefix("## ") {
                        let _ = writeln!(output, "<h2>{}</h2>", Self::parse_inline(content));
                    } else if !line.trim().is_empty() {
                        state = BlockState::InParagraph;
                        block_buffer.push_str(line);
                        block_buffer.push('\n');
                    }
                }
                BlockState::InParagraph => {
                    if line.trim().is_empty() || line.starts_with('#') || line.starts_with("```") {
                        // End paragraph
                        // RUST INSIGHT:
                        // Trim end ensures we don't output empty or trailing newline paragraphs.
                        let parsed = Self::parse_inline(block_buffer.trim_end());
                        let _ = writeln!(output, "<p>{}</p>", parsed);
                        block_buffer.clear();
                        state = BlockState::Normal;

                        // RUST INSIGHT:
                        // To avoid losing the current line (e.g. a Header right after a Paragraph),
                        // we would ideally use `lines.peek()` or a proper token stream.
                        // Here, we handle the simple case: if it was empty, we just moved to Normal.
                        // If it was a header/code block, we must process it immediately.
                        if line.starts_with("```") {
                            state = BlockState::InCodeBlock;
                            let _ = write!(output, "<pre><code>");
                        } else if let Some(content) = line.strip_prefix("# ") {
                            let _ = writeln!(output, "<h1>{}</h1>", Self::parse_inline(content));
                        } else if let Some(content) = line.strip_prefix("## ") {
                            let _ = writeln!(output, "<h2>{}</h2>", Self::parse_inline(content));
                        }
                    } else {
                        block_buffer.push_str(line);
                        block_buffer.push('\n');
                    }
                }
                BlockState::InCodeBlock => {
                    if line.starts_with("```") {
                        state = BlockState::Normal;
                        let _ = writeln!(output, "</code></pre>");
                    } else {
                        // GOTCHA:
                        // `.lines()` strips newlines. When rendering verbatim blocks like code,
                        // we must manually re-append `\n` to preserve multiline formatting.
                        let _ = writeln!(output, "{}", line);
                    }
                }
            }
        }

        // Flush any remaining paragraph
        if state == BlockState::InParagraph {
            let parsed = Self::parse_inline(block_buffer.trim_end());
            let _ = writeln!(output, "<p>{}</p>", parsed);
        } else if state == BlockState::InCodeBlock {
            // Unclosed code block recovery
            let _ = writeln!(output, "</code></pre>");
        }

        Ok(output)
    }

    /// Parses inline formatting: `**bold**`, `*italic*`, `` `code` ``
    fn parse_inline(text: &str) -> String {
        let mut result = String::with_capacity(text.len());
        let mut chars = text.chars().peekable();

        // State must persist across the whole block
        let mut in_bold = false;
        let mut in_italic = false;
        let mut in_code = false;

        while let Some(c) = chars.next() {
            if c == '`' {
                if in_code {
                    result.push_str("</code>");
                    in_code = false;
                } else {
                    result.push_str("<code>");
                    in_code = true;
                }
            } else if in_code {
                // Ignore formatting inside code spans
                // In a real parser we'd HTML escape here
                result.push(c);
            } else if c == '*' {
                if let Some(&'*') = chars.peek() {
                    chars.next(); // consume second *
                    if in_bold {
                        result.push_str("</b>");
                        in_bold = false;
                    } else {
                        result.push_str("<b>");
                        in_bold = true;
                    }
                } else if in_italic {
                    result.push_str("</i>");
                    in_italic = false;
                } else {
                    result.push_str("<i>");
                    in_italic = true;
                }
            } else {
                result.push(c);
            }
        }

        // Auto-close unclosed tags (simplified recovery)
        if in_bold {
            result.push_str("</b>");
        }
        if in_italic {
            result.push_str("</i>");
        }
        if in_code {
            result.push_str("</code>");
        }

        result
    }
}

// --- TESTING ---

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_headers() {
        let md = "# Header 1\n## Header 2";
        let parser = MarkdownParser::new(md);
        let html = parser.render_html().unwrap();
        assert_eq!(html, "<h1>Header 1</h1>\n<h2>Header 2</h2>\n");
    }

    #[test]
    fn test_paragraphs_and_inline() {
        let md = "This is a paragraph with **bold** and *italic* text.\n\nAnother paragraph.";
        let parser = MarkdownParser::new(md);
        let html = parser.render_html().unwrap();
        assert_eq!(
            html,
            "<p>This is a paragraph with <b>bold</b> and <i>italic</i> text.</p>\n<p>Another paragraph.</p>\n"
        );
    }

    #[test]
    fn test_code_blocks() {
        let md = "```\nfn main() {\n    println!(\"Hello\");\n}\n```";
        let parser = MarkdownParser::new(md);
        let html = parser.render_html().unwrap();
        assert_eq!(
            html,
            "<pre><code>fn main() {\n    println!(\"Hello\");\n}\n</code></pre>\n"
        );
    }

    #[test]
    fn test_inline_code() {
        let md = "Use the `println!` macro.";
        let parser = MarkdownParser::new(md);
        let html = parser.render_html().unwrap();
        assert_eq!(html, "<p>Use the <code>println!</code> macro.</p>\n");
    }

    #[test]
    fn test_state_transition_paragraph_to_header() {
        let md = "Paragraph line 1\nParagraph line 2\n# Header";
        let parser = MarkdownParser::new(md);
        let html = parser.render_html().unwrap();
        assert_eq!(
            html,
            "<p>Paragraph line 1\nParagraph line 2</p>\n<h1>Header</h1>\n"
        );
    }
}
