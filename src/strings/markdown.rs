//! Markdown Parser
//!
//! # What this implements and what crate(s) it replaces
//! This module provides a minimal, custom Markdown to HTML compiler.
//! It replaces the fundamental parts of crates like `pulldown-cmark` and `comrak`.
//!
//! # Real-world systems that use this
//! Real-world applications like static site generators (Hugo, Zola), documentation renderers (rustdoc),
//! and chat applications use Markdown parsers to convert user input into formatted HTML.
//!
//! # Why build it yourself?
//! By building a Markdown parser, you learn how to implement state machines for parsing unstructured text.
//! You also gain mastery over string manipulation and iterative parsing without building a massive AST.
//!
//! # Architecture
//!
//! ```text
//! [Markdown Text] -> [Block Parser] -> [Event Stream] -> [HTML Renderer] -> [HTML String]
//! ```
//!
//! **Invariants:**
//! 1. The block parser reads line by line, maintaining block state (e.g., paragraph, list, code block).
//! 2. Inline formatting (bold, italic) is tracked across contiguous text sequences.
//! 3. Trailing whitespaces and newlines are explicitly trimmed before wrapping text in block HTML tags to avoid empty artifacts like `<p></p>`.
//!
//! **Complexity:**
//! - **Time:** O(N) where N is the length of the Markdown string. Each character is examined a constant number of times.
//! - **Space:** O(N) for the resulting HTML output buffer, plus minimal state tracking overhead. We avoid building a full tree AST in memory.
//!
//! **Design Decisions and Tradeoffs:**
//! We chose a pull-based event stream approach over a full AST representation. This minimizes intermediate heap
//! allocations at the cost of slightly more complex inline state management.
//!
//! # Footer
//!
//! **Comparison to Canonical Crates:**
//! Production parsers like `pulldown-cmark` support the full CommonMark specification, including nested lists, tables,
//! link references, and raw HTML blocks. This implementation only covers a subset: Headers, Paragraphs, Blockquotes, and basic Bold/Italic.
//!
//! **What's Missing:**
//! Lists, Links, Images, Code blocks, Horizontal rules, and strict CommonMark compliance.
//!
//! **Next Steps:**
//! Implement a robust inline parser that handles nested formatting correctly and add support for Links and Images.
//!
//! # Benchmarking Note
//! You could benchmark the parser using `criterion` against `pulldown-cmark` using large markdown files
//! to analyze the throughput (MB/s) and memory footprint.

use std::fmt::Write;

#[derive(Debug, PartialEq)]
enum BlockState {
    Paragraph,
    Header(usize),
    Blockquote,
    CodeBlock,
    None,
}

#[derive(Debug, PartialEq, Clone)]
struct InlineState {
    bold: bool,
    italic: bool,
}

impl InlineState {
    fn new() -> Self {
        Self {
            bold: false,
            italic: false,
        }
    }
}

pub struct MarkdownParser<'a> {
    input: &'a str,
}

impl<'a> MarkdownParser<'a> {
    pub fn new(input: &'a str) -> Self {
        Self { input }
    }

    // RUST INSIGHT:
    // We use a stream-based parsing approach where we emit strings to a buffer.
    // Instead of collecting `.split()` into a `Vec`, we consume the iterator directly
    // within a `for` loop to eliminate intermediate allocations.
    pub fn render_html(&self) -> String {
        let mut html = String::with_capacity(self.input.len() * 2);
        let mut current_block = BlockState::None;
        let mut block_content = String::new();
        // Inline state is kept across lines in a block
        let mut inline_state = InlineState::new();

        for line in self.input.lines() {
            let trimmed = line.trim();

            if trimmed.is_empty() && current_block != BlockState::CodeBlock {
                self.flush_block(&mut html, &mut current_block, &mut block_content, &mut inline_state);
                continue;
            }

            match current_block {
                BlockState::None => {
                    if let Some(level) = self.parse_header_level(trimmed) {
                        current_block = BlockState::Header(level);
                        block_content.push_str(trimmed[level..].trim());
                        // Headers are immediately flushed because they are single-line
                        self.flush_block(&mut html, &mut current_block, &mut block_content, &mut inline_state);
                    } else if trimmed.starts_with("> ") {
                        current_block = BlockState::Blockquote;
                        block_content.push_str(&trimmed[2..]);
                    } else if trimmed.starts_with("```") {
                        current_block = BlockState::CodeBlock;
                    } else {
                        current_block = BlockState::Paragraph;
                        block_content.push_str(trimmed);
                    }
                }
                BlockState::Paragraph => {
                    block_content.push(' ');
                    block_content.push_str(trimmed);
                }
                BlockState::Header(_) => {
                    // This is unreachable due to flushing immediately after parsing a header
                }
                BlockState::Blockquote => {
                    if trimmed.starts_with("> ") {
                        block_content.push('\n');
                        block_content.push_str(&trimmed[2..]);
                    } else {
                        block_content.push('\n');
                        block_content.push_str(trimmed);
                    }
                }
                BlockState::CodeBlock => {
                    if trimmed.starts_with("```") {
                        self.flush_block(&mut html, &mut current_block, &mut block_content, &mut inline_state);
                    } else {
                        // For CodeBlock we want to push the line directly without trimming trailing \n from block_content initially
                        // but wait, `line` doesn't contain `\n` in `.lines()`.
                        block_content.push_str(line); // preserve original formatting
                        block_content.push('\n');
                    }
                }
            }
        }

        self.flush_block(&mut html, &mut current_block, &mut block_content, &mut inline_state);
        html
    }

    fn parse_header_level(&self, line: &str) -> Option<usize> {
        let mut count = 0;
        for c in line.chars() {
            if c == '#' {
                count += 1;
            } else {
                break;
            }
        }
        if count > 0 && count <= 6 && line[count..].starts_with(' ') {
            Some(count)
        } else {
            None
        }
    }

    fn flush_block(
        &self,
        html: &mut String,
        state: &mut BlockState,
        content: &mut String,
        inline_state: &mut InlineState,
    ) {
        if content.is_empty() && *state != BlockState::CodeBlock {
            *state = BlockState::None;
            return;
        }

        match state {
            BlockState::Paragraph => {
                let text_to_render = content.trim_end();
                let parsed_inline = self.parse_inline(text_to_render, inline_state);
                write!(html, "<p>{}</p>\n", parsed_inline).unwrap();
            }
            BlockState::Header(level) => {
                let text_to_render = content.trim_end();
                let parsed_inline = self.parse_inline(text_to_render, inline_state);
                write!(html, "<h{}>{}</h{}>\n", level, parsed_inline, level).unwrap();
            }
            BlockState::Blockquote => {
                let text_to_render = content.trim_end();
                let parsed_inline = self.parse_inline(text_to_render, inline_state);
                write!(html, "<blockquote>{}</blockquote>\n", parsed_inline).unwrap();
            }
            BlockState::CodeBlock => {
                // Code blocks do not parse inline styling, and they may preserve trailing newlines differently.
                // To pass the test `"<pre><code>let x = 1;\n</code></pre>\n"`, we use the raw content.
                write!(html, "<pre><code>{}</code></pre>\n", content).unwrap();
            }
            BlockState::None => {}
        }

        content.clear();
        *state = BlockState::None;
        // Reset inline state for the next block
        *inline_state = InlineState::new();
    }

    fn parse_inline(&self, text: &str, state: &mut InlineState) -> String {
        let mut result = String::with_capacity(text.len());
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            if i + 1 < chars.len() && chars[i] == '*' && chars[i + 1] == '*' {
                state.bold = !state.bold;
                if state.bold {
                    result.push_str("<strong>");
                } else {
                    result.push_str("</strong>");
                }
                i += 2;
            } else if chars[i] == '*' {
                state.italic = !state.italic;
                if state.italic {
                    result.push_str("<em>");
                } else {
                    result.push_str("</em>");
                }
                i += 1;
            } else if chars[i] == '<' {
                result.push_str("&lt;");
                i += 1;
            } else if chars[i] == '>' {
                result.push_str("&gt;");
                i += 1;
            } else if chars[i] == '&' {
                result.push_str("&amp;");
                i += 1;
            } else {
                result.push(chars[i]);
                i += 1;
            }
        }

        // Close any dangling tags
        if state.bold {
            result.push_str("</strong>");
            state.bold = false;
        }
        if state.italic {
            result.push_str("</em>");
            state.italic = false;
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paragraph() {
        let md = "Hello World!";
        let parser = MarkdownParser::new(md);
        assert_eq!(parser.render_html(), "<p>Hello World!</p>\n");
    }

    #[test]
    fn test_multiple_paragraphs() {
        let md = "First paragraph.\n\nSecond paragraph.";
        let parser = MarkdownParser::new(md);
        assert_eq!(
            parser.render_html(),
            "<p>First paragraph.</p>\n<p>Second paragraph.</p>\n"
        );
    }

    #[test]
    fn test_headers() {
        let md = "# Header 1\n## Header 2\n###### Header 6";
        let parser = MarkdownParser::new(md);
        assert_eq!(
            parser.render_html(),
            "<h1>Header 1</h1>\n<h2>Header 2</h2>\n<h6>Header 6</h6>\n"
        );
    }

    #[test]
    fn test_inline_formatting() {
        let md = "This is **bold** and *italic* text.";
        let parser = MarkdownParser::new(md);
        assert_eq!(
            parser.render_html(),
            "<p>This is <strong>bold</strong> and <em>italic</em> text.</p>\n"
        );
    }

    #[test]
    fn test_inline_persistence() {
        let md = "Start **bold across\nlines**";
        let parser = MarkdownParser::new(md);
        assert_eq!(
            parser.render_html(),
            "<p>Start <strong>bold across lines</strong></p>\n"
        );
    }

    #[test]
    fn test_blockquote() {
        let md = "> This is a quote.\n> It spans multiple lines.";
        let parser = MarkdownParser::new(md);
        assert_eq!(
            parser.render_html(),
            "<blockquote>This is a quote.\nIt spans multiple lines.</blockquote>\n"
        );
    }

    #[test]
    fn test_code_block() {
        let md = "```\nlet x = 1;\n```";
        let parser = MarkdownParser::new(md);
        assert_eq!(
            parser.render_html(),
            "<pre><code>let x = 1;\n</code></pre>\n"
        );
    }

    #[test]
    fn test_escaping() {
        let md = "5 < 10 & 10 > 5";
        let parser = MarkdownParser::new(md);
        assert_eq!(
            parser.render_html(),
            "<p>5 &lt; 10 &amp; 10 &gt; 5</p>\n"
        );
    }
}
