//! # Markdown Parser Implementation
//!
//! Implements a minimal Markdown to HTML compiler using a pull-based event stream approach.
//! It supports basic block structures (paragraphs, headings) and inline formatting (bold, italic).
//!
//! **Replaces Crates:** `pulldown-cmark`, `comrak`
//!
//! **Real-world Usage:**
//! - Rendering user comments (Reddit, GitHub).
//! - Static site generators (Hugo, Zola).
//! - Documentation engines (`rustdoc`).
//!
//! **Why build it yourself?**
//! Parsing Markdown is notoriously difficult because it's not a context-free grammar.
//! Building a parser teaches you about state machines, event-driven architectures (pull vs. push parsers),
//! and how to handle ambiguous, highly-nested text formatting efficiently without blowing up the heap with a full AST.

use std::iter::Peekable;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure:
// Event Stream (Pull Parser) -> HTML Renderer
//
// Invariants:
// 1. Emits well-formed HTML tags (every `Start` has a matching `End`).
// 2. Inline parsing gracefully falls back to plain text if a formatting tag is unmatched.
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ Parse & Render│ O(N)        │ O(D)        │
// └───────────────┴─────────────┴─────────────┘
// N = length of input string.
// D = maximum nesting depth of inline tags (very small in practice).
//
// Design Decisions:
// - **Event-driven (Pull Parser)**: Instead of building a large in-memory Abstract Syntax Tree (AST),
//   the parser yields a stream of `Event`s (Start, End, Text). This drastically reduces memory allocations.
// - **Two-pass strategy (implied)**: Markdown technically requires two passes for things like reference links,
//   but our simplified version is single-pass for standard blocks.
// - **String slicing**: We heavily use `&str` to avoid copying the input text, allocating only when constructing the final HTML.

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Tag {
    Paragraph,
    Heading(u8),
    Strong,
    Emphasis,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Event<'a> {
    Start(Tag),
    End(Tag),
    Text(&'a str),
}

/// A simplified Markdown Parser that yields an iterator of Events.
pub struct Parser<'a> {
    text: &'a str,
    offset: usize,
}

impl<'a> Parser<'a> {
    pub fn new(text: &'a str) -> Self {
        Self { text, offset: 0 }
    }

    /// Helper to consume and return a block of text separated by blank lines.
    fn next_block(&mut self) -> Option<&'a str> {
        // Skip leading whitespace/newlines
        while self.offset < self.text.len() && self.text[self.offset..].starts_with('\n') {
            self.offset += 1;
        }

        if self.offset >= self.text.len() {
            return None;
        }

        let start = self.offset;
        // Find next double newline (blank line) marking end of block
        let end = self.text[start..]
            .find("\n\n")
            .map(|i| start + i)
            .unwrap_or(self.text.len());

        self.offset = end;
        Some(self.text[start..end].trim())
    }
}

// RUST INSIGHT:
// Implementing `Iterator` allows users to cleanly `for event in parser`
// or chain combinators like `.filter()`.
// Since our blocks are simple, our iterator just parses block by block,
// then uses a secondary inline parser to yield the inner events.
// For a true streaming pull parser, the state machine would be more complex to yield one event at a time without storing intermediate vecs.
// To keep the code understandable, our Iterator implementation will buffer events for the current block.

pub struct EventIter<'a> {
    parser: Parser<'a>,
    buffer: Vec<Event<'a>>,
}

impl<'a> Iterator for EventIter<'a> {
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if !self.buffer.is_empty() {
            return Some(self.buffer.remove(0));
        }

        let block = self.parser.next_block()?;

        // Block-level parsing
        if block.starts_with("# ") {
            self.buffer.push(Event::Start(Tag::Heading(1)));
            self.parse_inline(&block[2..]);
            self.buffer.push(Event::End(Tag::Heading(1)));
        } else if block.starts_with("## ") {
            self.buffer.push(Event::Start(Tag::Heading(2)));
            self.parse_inline(&block[3..]);
            self.buffer.push(Event::End(Tag::Heading(2)));
        } else {
            self.buffer.push(Event::Start(Tag::Paragraph));
            self.parse_inline(block);
            self.buffer.push(Event::End(Tag::Paragraph));
        }

        Some(self.buffer.remove(0))
    }
}

impl<'a> EventIter<'a> {
    /// Parses inline formatting (*italic*, **bold**) and pushes events to the buffer.
    fn parse_inline(&mut self, text: &'a str) {
        let mut i = 0;
        let mut last_text_start = 0;

        while i < text.len() {
            // Check for **strong**
            if text[i..].starts_with("**") {
                // Find closing **
                // GOTCHA: We must use `.char_indices()` and `len_utf8()` to safely advance indices,
                // or we risk panicking when slicing on multibyte characters.
                let mut end = None;
                let mut search_idx = i + 2;
                while search_idx < text.len() {
                    if text[search_idx..].starts_with("**") {
                        end = Some(search_idx);
                        break;
                    }
                    let c = text[search_idx..].chars().next().unwrap();
                    search_idx += c.len_utf8();
                }

                if let Some(end_idx) = end {
                    // Push preceding text
                    if i > last_text_start {
                        self.buffer.push(Event::Text(&text[last_text_start..i]));
                    }
                    self.buffer.push(Event::Start(Tag::Strong));
                    // Recursively parse inner text
                    self.parse_inline(&text[i + 2..end_idx]);
                    self.buffer.push(Event::End(Tag::Strong));

                    i = end_idx + 2;
                    last_text_start = i;
                    continue;
                }
            }
            // Check for *emphasis*
            else if text[i..].starts_with('*') {
                let mut end = None;
                let mut search_idx = i + 1;
                while search_idx < text.len() {
                    if text[search_idx..].starts_with('*') {
                        end = Some(search_idx);
                        break;
                    }
                    let c = text[search_idx..].chars().next().unwrap();
                    search_idx += c.len_utf8();
                }

                if let Some(end_idx) = end {
                    if i > last_text_start {
                        self.buffer.push(Event::Text(&text[last_text_start..i]));
                    }
                    self.buffer.push(Event::Start(Tag::Emphasis));
                    self.parse_inline(&text[i + 1..end_idx]);
                    self.buffer.push(Event::End(Tag::Emphasis));

                    i = end_idx + 1;
                    last_text_start = i;
                    continue;
                }
            }

            // Advance by one char
            let c = text[i..].chars().next().unwrap();
            i += c.len_utf8();
        }

        if last_text_start < text.len() {
            self.buffer
                .push(Event::Text(&text[last_text_start..text.len()]));
        }
    }
}

/// Convenience function to render a Markdown string directly to HTML.
pub fn render_html(markdown: &str) -> String {
    let parser = Parser::new(markdown);
    let iter = EventIter {
        parser,
        buffer: Vec::new(),
    };

    let mut html = String::with_capacity(markdown.len() * 2);

    for event in iter {
        match event {
            Event::Start(tag) => match tag {
                Tag::Paragraph => html.push_str("<p>"),
                Tag::Heading(level) => {
                    // Use format!() or manual push_str based on level
                    html.push_str(&format!("<h{}>", level));
                }
                Tag::Strong => html.push_str("<strong>"),
                Tag::Emphasis => html.push_str("<em>"),
            },
            Event::End(tag) => match tag {
                Tag::Paragraph => html.push_str("</p>\n"),
                Tag::Heading(level) => html.push_str(&format!("</h{}>\n", level)),
                Tag::Strong => html.push_str("</strong>"),
                Tag::Emphasis => html.push_str("</em>"),
            },
            Event::Text(t) => {
                // PRODUCTION NOTE: A real implementation MUST escape HTML entities here (e.g., < to &lt;)
                // to prevent XSS attacks.
                html.push_str(t);
            }
        }
    }

    html
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `pulldown-cmark`: A fully compliant CommonMark parser. Uses a true zero-allocation state machine
//   and handles complex edge cases (lists, blockquotes, reference links, tables).
//
// Missing vs. Production:
// - **CommonMark Compliance**: We miss 99% of the spec (lists, links, images, code blocks).
// - **Security (HTML Escaping)**: We don't escape `<` or `>` in plain text, making it vulnerable to XSS.
// - **Performance**: Our inline parsing uses recursion and buffers events in a Vec, which allocates.
//   `pulldown-cmark` is completely zero-allocation for most inputs.
//
// Next Steps:
// 1. Implement HTML escaping for `Event::Text`.
// 2. Add support for Links `[text](url)` and Images `![alt](url)`.
// 3. Implement a proper line-by-line state machine to handle lists and blockquotes.
//
// Benchmarking Note:
// Use `criterion` to benchmark `render_html` on various markdown documents. Compare against
// `pulldown-cmark`. Track allocation counts to see the cost of the event buffer vector vs a true streaming parser.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paragraphs() {
        let md = "Hello world\n\nSecond paragraph";
        let html = render_html(md);
        assert_eq!(html, "<p>Hello world</p>\n<p>Second paragraph</p>\n");
    }

    #[test]
    fn test_headings() {
        let md = "# Title\n\n## Subtitle";
        let html = render_html(md);
        assert_eq!(html, "<h1>Title</h1>\n<h2>Subtitle</h2>\n");
    }

    #[test]
    fn test_inline_formatting() {
        let md = "This is **bold** and *italic* text.";
        let html = render_html(md);
        assert_eq!(
            html,
            "<p>This is <strong>bold</strong> and <em>italic</em> text.</p>\n"
        );
    }

    #[test]
    fn test_nested_formatting() {
        let md = "**Bold and *italic* inside**";
        let html = render_html(md);
        assert_eq!(
            html,
            "<p><strong>Bold and <em>italic</em> inside</strong></p>\n"
        );
    }

    #[test]
    fn test_unmatched_formatting() {
        // Should fall back to plain text
        let md = "This is **bold and unmatched";
        let html = render_html(md);
        assert_eq!(html, "<p>This is **bold and unmatched</p>\n");
    }

    #[test]
    fn test_multibyte_characters() {
        let md = "Here is an emoji 🚀 **bold 🚀**";
        let html = render_html(md);
        assert_eq!(html, "<p>Here is an emoji 🚀 <strong>bold 🚀</strong></p>\n");
    }
}
