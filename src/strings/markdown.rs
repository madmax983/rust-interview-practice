//! # Markdown Parser
//!
//! Implements a minimal Markdown to HTML compiler using a pull-based event stream approach.
//! It supports basic block and inline formatting without full Abstract Syntax Tree (AST) overhead.
//!
//! **Replaces Crates:** `pulldown-cmark`, `comrak`
//!
//! **Real-world Usage:**
//! - Static site generators (Hugo, Zola).
//! - Documentation platforms (docs.rs).
//! - Content Management Systems parsing user input to display formatted text.
//!
//! **Why build it yourself?**
//! Implementing a markdown parser teaches you about complex text processing and stream parsing.
//! You learn how to use a pull-based iterator to yield events (Start/End tags, Text)
//! instead of allocating an entire AST in memory, which drastically improves performance and reduces allocations.

// =========================================================================================
// Architecture
// =========================================================================================
//
// Flow:
//
//   [Markdown String]
//         │
//         ▼
//   [Parser Iterator] ──► (Yields stream of `Event`s)
//         │
//         ▼
//   [HTML Renderer] ──► (Builds HTML String)
//
// Invariants:
// 1. Every `Start` tag event MUST have a corresponding `End` tag event in the stream.
// 2. The parser index MUST advance using `char.len_utf8()` to prevent out-of-bounds panics
//    when slicing around multibyte unicode characters.
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ Parsing       │ O(N)        │ O(N) output │
// │ Rendering     │ O(N)        │ O(N)        │
// └───────────────┴─────────────┴─────────────┘
// N = Length of input text
//
// Design Decisions:
// - **Event Stream**: We use an `Iterator` yielding `Event` enums (similar to `pulldown-cmark`).
//   - *Tradeoff*: Harder to write than a recursive descent AST builder, but highly memory efficient.
// - **Inline Parsing**: Handles Bold (`**`) and Italic (`*`) using lookaheads.

// =========================================================================================
// Types
// =========================================================================================

#[derive(Debug, PartialEq, Eq)]
pub enum Tag {
    Paragraph,
    Heading(u8), // Level 1-6
    Strong,
    Emphasis,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Event<'a> {
    Start(Tag),
    End(Tag),
    Text(&'a str),
    SoftBreak,
    HardBreak,
}

// =========================================================================================
// Parser Iterator
// =========================================================================================

pub struct Parser<'a> {
    text: &'a str,
    pos: usize,
    // Stack of active blocks/inlines that need to be closed
    stack: Vec<Tag>,
    // Buffer of pending events (e.g. text + inline tags) extracted from a block
    pending_events: Vec<Event<'a>>,
}

impl<'a> Parser<'a> {
    pub fn new(text: &'a str) -> Self {
        Self {
            text,
            pos: 0,
            stack: Vec::new(),
            pending_events: Vec::new(),
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.text[self.pos..].chars().next()
    }

    fn advance(&mut self, n: usize) {
        self.pos += n;
    }

    // GOTCHA: Multibyte characters. We must use `char.len_utf8()` to advance byte indices correctly.
    // Simply doing `pos += 1` will panic if the text contains emoji or non-ASCII characters.

    fn parse_block(&mut self) -> Option<Event<'a>> {
        // Skip leading whitespace (newlines, spaces)
        while let Some(c) = self.peek_char() {
            if c.is_whitespace() && c != '\n' {
                self.advance(c.len_utf8());
            } else if c == '\n' {
                self.advance(c.len_utf8());
            } else {
                break;
            }
        }

        if self.pos >= self.text.len() {
            return None; // EOF
        }

        // Check for Heading
        let start_pos = self.pos;
        let mut hashes = 0;
        let mut temp_pos = self.pos;

        while let Some(c) = self.text[temp_pos..].chars().next() {
            if c == '#' {
                hashes += 1;
                temp_pos += c.len_utf8();
            } else {
                break;
            }
        }

        if hashes > 0 && hashes <= 6 && self.text[temp_pos..].starts_with(' ') {
            self.pos = temp_pos + 1; // Skip the hashes and the space
            self.stack.push(Tag::Heading(hashes));

            // Extract the rest of the line as text
            let end_line = self.find_next_newline();
            let content = &self.text[self.pos..end_line];
            self.pos = end_line;

            self.parse_inline(content);
            self.pending_events.push(Event::End(Tag::Heading(hashes)));
            self.stack.pop();

            return Some(Event::Start(Tag::Heading(hashes)));
        }

        // Default to Paragraph
        self.stack.push(Tag::Paragraph);

        // Find end of paragraph (double newline)
        let mut end_para = self.pos;
        while let Some(c) = self.text[end_para..].chars().next() {
            if self.text[end_para..].starts_with("\n\n") {
                break;
            }
            end_para += c.len_utf8();
        }

        let content = &self.text[self.pos..end_para];
        self.pos = end_para;

        self.parse_inline(content);
        self.pending_events.push(Event::End(Tag::Paragraph));
        self.stack.pop();

        Some(Event::Start(Tag::Paragraph))
    }

    fn find_next_newline(&self) -> usize {
        let mut current = self.pos;
        while let Some(c) = self.text[current..].chars().next() {
            if c == '\n' {
                return current;
            }
            current += c.len_utf8();
        }
        self.text.len()
    }

    fn parse_inline(&mut self, content: &'a str) {
        let mut i = 0;
        let mut text_start = 0;

        while i < content.len() {
            let c = content[i..].chars().next().unwrap();
            let c_len = c.len_utf8();

            if content[i..].starts_with("**") {
                // PRODUCTION NOTE: A real implementation would parse blocks then do inline parsing in a second pass, and would support full html escaping.
                // RUST INSIGHT: Lookahead loop must use char boundary advancement to prevent infinite loops
                // and out-of-bounds panics on multibyte characters.
                let mut closing_idx = i + 2;
                let mut found_close = false;

                while closing_idx < content.len() {
                    if content[closing_idx..].starts_with("**") {
                        found_close = true;
                        break;
                    }
                    closing_idx += content[closing_idx..].chars().next().unwrap().len_utf8();
                }

                if found_close {
                    // Flush pending text
                    if i > text_start {
                        self.pending_events.push(Event::Text(&content[text_start..i]));
                    }

                    self.pending_events.push(Event::Start(Tag::Strong));
                    self.parse_inline(&content[i + 2..closing_idx]); // Recursive parsing inside Strong
                    self.pending_events.push(Event::End(Tag::Strong));

                    i = closing_idx + 2;
                    text_start = i;
                    continue;
                }
            } else if content[i..].starts_with("*") {
                let mut closing_idx = i + 1;
                let mut found_close = false;

                while closing_idx < content.len() {
                    if content[closing_idx..].starts_with("*") && !content[closing_idx..].starts_with("**") {
                        found_close = true;
                        break;
                    }
                    closing_idx += content[closing_idx..].chars().next().unwrap().len_utf8();
                }

                if found_close {
                    if i > text_start {
                        self.pending_events.push(Event::Text(&content[text_start..i]));
                    }

                    self.pending_events.push(Event::Start(Tag::Emphasis));
                    self.parse_inline(&content[i + 1..closing_idx]);
                    self.pending_events.push(Event::End(Tag::Emphasis));

                    i = closing_idx + 1;
                    text_start = i;
                    continue;
                }
            } else if content[i..].starts_with("  \n") {
                 if i > text_start {
                    self.pending_events.push(Event::Text(&content[text_start..i]));
                }
                self.pending_events.push(Event::HardBreak);
                i += 3;
                text_start = i;
                continue;
            } else if c == '\n' {
                if i > text_start {
                    self.pending_events.push(Event::Text(&content[text_start..i]));
                }
                self.pending_events.push(Event::SoftBreak);
                i += 1;
                text_start = i;
                continue;
            }

            i += c_len;
        }

        if i > text_start {
            self.pending_events.push(Event::Text(&content[text_start..i]));
        }
    }
}

impl<'a> Iterator for Parser<'a> {
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if !self.pending_events.is_empty() {
            return Some(self.pending_events.remove(0));
        }

        self.parse_block()
    }
}

// =========================================================================================
// HTML Renderer
// =========================================================================================

pub fn render_html<'a, I>(events: I) -> String
where
    I: Iterator<Item = Event<'a>>,
{
    let mut output = String::new();

    for event in events {
        match event {
            Event::Start(tag) => match tag {
                Tag::Paragraph => output.push_str("<p>"),
                Tag::Heading(level) => output.push_str(&format!("<h{}>", level)),
                Tag::Strong => output.push_str("<strong>"),
                Tag::Emphasis => output.push_str("<em>"),
            },
            Event::End(tag) => match tag {
                Tag::Paragraph => output.push_str("</p>\n"),
                Tag::Heading(level) => output.push_str(&format!("</h{}>\n", level)),
                Tag::Strong => output.push_str("</strong>"),
                Tag::Emphasis => output.push_str("</em>"),
            },
            Event::Text(text) => {
                // Basic HTML escaping would go here in production
                output.push_str(text);
            }
            Event::SoftBreak => output.push('\n'),
            Event::HardBreak => output.push_str("<br />\n"),
        }
    }

    output
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `pulldown-cmark` is vastly more complex, fully compliant with the CommonMark spec,
//   and highly optimized. It uses a state machine and handles edge cases (like mismatched tags,
//   links, blockquotes, lists) which our minimal version ignores.
//
// Missing vs. Production:
// - No support for Links, Images, Lists, Blockquotes, or Code Blocks.
// - No HTML escaping (vulnerable to XSS).
// - Very naive Inline parsing (O(N^2) in worst case due to lookahead without memoization).
// - Does not pass the CommonMark test suite.
//
// Benchmark Note: Use `std::hint::black_box` when benchmarking `render_html(Parser::new(text))`.
// Suggested next steps:
// 1. Implement HTML escaping for `Event::Text`.
// 2. Add support for Links `[text](url)`.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paragraph() {
        let markdown = "Hello world";
        let parser = Parser::new(markdown);
        let html = render_html(parser);
        assert_eq!(html, "<p>Hello world</p>\n");
    }

    #[test]
    fn test_heading() {
        let markdown = "## Subtitle here";
        let parser = Parser::new(markdown);
        let html = render_html(parser);
        assert_eq!(html, "<h2>Subtitle here</h2>\n");
    }

    #[test]
    fn test_strong_and_emphasis() {
        let markdown = "This is **bold** and *italic*.";
        let parser = Parser::new(markdown);
        let html = render_html(parser);
        assert_eq!(html, "<p>This is <strong>bold</strong> and <em>italic</em>.</p>\n");
    }

    #[test]
    fn test_nested_inline() {
        let markdown = "**Bold *and* italic**";
        let parser = Parser::new(markdown);
        let html = render_html(parser);
        assert_eq!(html, "<p><strong>Bold <em>and</em> italic</strong></p>\n");
    }

    #[test]
    fn test_multibyte_characters() {
        let markdown = "Hello 🌍! **Bold 🦀**";
        let parser = Parser::new(markdown);
        let html = render_html(parser);
        assert_eq!(html, "<p>Hello 🌍! <strong>Bold 🦀</strong></p>\n");
    }

    #[test]
    fn test_multiple_blocks() {
        let markdown = "# Title\n\nFirst paragraph.\n\nSecond paragraph.";
        let parser = Parser::new(markdown);
        let html = render_html(parser);
        assert_eq!(
            html,
            "<h1>Title</h1>\n<p>First paragraph.</p>\n<p>Second paragraph.</p>\n"
        );
    }

    #[test]
    fn test_soft_and_hard_breaks() {
        let markdown = "Line one  \nLine two\nLine three";
        let parser = Parser::new(markdown);
        let html = render_html(parser);
        assert_eq!(html, "<p>Line one<br />\nLine two\nLine three</p>\n");
    }
}
