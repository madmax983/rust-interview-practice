//! # Markdown Parser
//!
//! Implements a minimal Markdown to HTML compiler using a pull-based event stream approach.
//!
//! **Replaces Crates:** `pulldown-cmark`, `comrak`
//!
//! **Real-world Usage:**
//! - Static site generators (Zola, Hugo) rendering blog posts.
//! - Documentation generators (Rustdoc, mdBook).
//! - Web forums and chat applications for rich text rendering.
//!
//! **Why build it yourself?**
//! Parsing Markdown teaches you how to implement a pull-parser. Instead of building a full AST
//! (which is memory intensive), the parser yields a stream of `Event`s as it processes text chunks.
//! You learn state machine mechanics, balancing tags (like bold/italic), and how to cleanly separate
//! the parsing phase from the rendering phase.

// =========================================================================================
// Architecture
// =========================================================================================
//
// 1. Parsing Phase (Pull Parser):
//    Markdown String  -->  `Parser` (Iterator)  -->  Stream of `Event`s
//
//    Events are flat, e.g.:
//    `Start(Heading(1))`, `Text("Hello")`, `End(Heading(1))`
//
//    This avoids full tree allocation in memory.
//
// 2. Rendering Phase:
//    Stream of `Event`s  -->  `html::push_html()`  -->  HTML String
//
// Invariants:
// - Every `Start(Tag)` must eventually be followed by a matching `End(Tag)`.
// - The parser state machine handles nested inline formatting (like `**bold *italic***`).
//
// Complexity:
// - Time: O(N) where N is the length of the Markdown string.
// - Space: O(D) where D is the maximum depth of nested tags (usually small).

// -----------------------------------------------------------------------------------------
// 1. AST & Events
// -----------------------------------------------------------------------------------------

#[derive(Debug, PartialEq, Clone)]
pub enum Tag {
    Paragraph,
    Heading(u8),
    List,
    Item,
    Strong,
    Emphasis,
}

#[derive(Debug, PartialEq, Clone)]
pub enum Event<'a> {
    Start(Tag),
    End(Tag),
    Text(&'a str),
    Html(&'a str),
}

// -----------------------------------------------------------------------------------------
// 2. Parser
// -----------------------------------------------------------------------------------------

// RUST INSIGHT: `Parser` borrows the input string (`&'a str`), so `Event::Text` can just be
// a slice without allocating Strings. This is a massive performance win.

pub struct Parser<'a> {
    input: &'a str,
    pos: usize,
    state_stack: Vec<Tag>,
    inline_stack: Vec<Tag>,
    buffered_events: std::collections::VecDeque<Event<'a>>,
}

impl<'a> Parser<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            input,
            pos: 0,
            state_stack: Vec::new(),
            inline_stack: Vec::new(),
            buffered_events: std::collections::VecDeque::new(),
        }
    }

    fn peek_line(&self) -> Option<&'a str> {
        if self.pos >= self.input.len() {
            return None;
        }
        let rest = &self.input[self.pos..];
        let end = rest.find('\n').unwrap_or(rest.len());
        Some(&rest[..end])
    }

    fn advance_line(&mut self) {
        if let Some(line) = self.peek_line() {
            self.pos += line.len();
            if self.pos < self.input.len() && self.input[self.pos..].starts_with('\n') {
                self.pos += 1;
            }
        }
    }

    /// Parses inline formatting (*italic*, **bold**) within a line of text
    fn parse_inline(&mut self, text: &'a str) {
        let mut start = 0;
        let mut chars = text.char_indices().peekable();

        while let Some((i, c)) = chars.next() {
            if text[i..].starts_with("**") {
                if i > start {
                    self.buffered_events.push_back(Event::Text(&text[start..i]));
                }

                if let Some(Tag::Strong) = self.inline_stack.last() {
                    self.inline_stack.pop();
                    self.buffered_events.push_back(Event::End(Tag::Strong));
                } else {
                    self.inline_stack.push(Tag::Strong);
                    self.buffered_events.push_back(Event::Start(Tag::Strong));
                }

                chars.next(); // Skip the second '*'
                start = i + 2;
            } else if text[i..].starts_with('*') {
                if i > start {
                    self.buffered_events.push_back(Event::Text(&text[start..i]));
                }

                if let Some(Tag::Emphasis) = self.inline_stack.last() {
                    self.inline_stack.pop();
                    self.buffered_events.push_back(Event::End(Tag::Emphasis));
                } else {
                    self.inline_stack.push(Tag::Emphasis);
                    self.buffered_events.push_back(Event::Start(Tag::Emphasis));
                }

                start = i + 1;
            }
        }

        if start < text.len() {
            self.buffered_events.push_back(Event::Text(&text[start..]));
        }
    }

    // Process the next chunk of the markdown string
    fn process_next(&mut self) -> bool {
        if self.pos >= self.input.len() {
            // Close any open blocks
            if let Some(tag) = self.state_stack.pop() {
                self.buffered_events.push_back(Event::End(tag));
                return true;
            }
            return false;
        }

        let line = self.peek_line().unwrap();

        // 1. Empty lines close paragraphs/lists
        if line.trim().is_empty() {
            if let Some(tag) = self.state_stack.pop() {
                self.buffered_events.push_back(Event::End(tag));
            }
            self.advance_line();
            return true;
        }

        // 2. Headings (#)
        if line.starts_with('#') {
            // GOTCHA: Need to handle closing existing state first
            if let Some(tag) = self.state_stack.pop() {
                self.buffered_events.push_back(Event::End(tag));
            }

            let mut level = 0;
            while line[level..].starts_with('#') {
                level += 1;
            }

            if line[level..].starts_with(' ') {
                self.buffered_events.push_back(Event::Start(Tag::Heading(level as u8)));
                let content = line[level + 1..].trim();
                self.parse_inline(content);
                self.buffered_events.push_back(Event::End(Tag::Heading(level as u8)));
                self.advance_line();
                return true;
            }
        }

        // 3. Lists (- item)
        if line.starts_with("- ") {
            if self.state_stack.last() != Some(&Tag::List) {
                if let Some(tag) = self.state_stack.pop() {
                    self.buffered_events.push_back(Event::End(tag));
                }
                self.state_stack.push(Tag::List);
                self.buffered_events.push_back(Event::Start(Tag::List));
            }

            self.buffered_events.push_back(Event::Start(Tag::Item));
            let content = line[2..].trim();
            self.parse_inline(content);
            self.buffered_events.push_back(Event::End(Tag::Item));
            self.advance_line();
            return true;
        }

        // 4. Paragraphs (Fallback)
        if self.state_stack.last() != Some(&Tag::Paragraph) {
            // Not in a paragraph, maybe close current block
            if let Some(tag) = self.state_stack.pop() {
                self.buffered_events.push_back(Event::End(tag));
            }
            self.state_stack.push(Tag::Paragraph);
            self.buffered_events.push_back(Event::Start(Tag::Paragraph));
        }

        // Inside paragraph, just parse text and inline
        // GOTCHA: In real markdown, consecutive lines in a paragraph are joined by spaces
        if !self.buffered_events.is_empty() && matches!(self.buffered_events.back(), Some(Event::Text(_))) {
            self.buffered_events.push_back(Event::Text(" "));
        }

        self.parse_inline(line.trim());
        self.advance_line();
        true
    }
}

impl<'a> Iterator for Parser<'a> {
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        while self.buffered_events.is_empty() {
            if !self.process_next() {
                return None;
            }
        }
        self.buffered_events.pop_front()
    }
}

// -----------------------------------------------------------------------------------------
// 3. HTML Renderer
// -----------------------------------------------------------------------------------------

pub mod html {
    use super::{Event, Tag};

    pub fn push_html<'a, I>(out: &mut String, iter: I)
    where
        I: Iterator<Item = Event<'a>>,
    {
        for event in iter {
            match event {
                Event::Start(tag) => match tag {
                    Tag::Paragraph => out.push_str("<p>"),
                    Tag::Heading(level) => out.push_str(&format!("<h{}>", level)),
                    Tag::List => out.push_str("<ul>\n"),
                    Tag::Item => out.push_str("<li>"),
                    Tag::Strong => out.push_str("<strong>"),
                    Tag::Emphasis => out.push_str("<em>"),
                },
                Event::End(tag) => match tag {
                    Tag::Paragraph => out.push_str("</p>\n"),
                    Tag::Heading(level) => out.push_str(&format!("</h{}>\n", level)),
                    Tag::List => out.push_str("</ul>\n"),
                    Tag::Item => out.push_str("</li>\n"),
                    Tag::Strong => out.push_str("</strong>"),
                    Tag::Emphasis => out.push_str("</em>"),
                },
                Event::Text(text) => {
                    // Escape HTML in text
                    let escaped = text.replace("&", "&amp;")
                                      .replace("<", "&lt;")
                                      .replace(">", "&gt;");
                    out.push_str(&escaped);
                }
                Event::Html(html) => out.push_str(html),
            }
        }
    }
}

// =========================================================================================
// Comparison to Canonical Crates & Next Steps
// =========================================================================================
//
// **pulldown-cmark**:
// - **What they do**: pulldown-cmark provides a highly optimized, fully CommonMark compliant
//   pull parser. It uses an arena allocator for strings to minimize allocations and handles
//   extreme edge cases in the spec (like deep nested links and tricky HTML blocks).
// - **What we omitted**: Our implementation is extremely minimal. It doesn't handle links,
//   images, blockquotes, complex nested lists, inline HTML, or full CommonMark edge cases.
//   It also allocates `String`s in some inline text processing instead of strictly borrowing.
//
// **Benchmarking Note:**
// To benchmark this, you would use `criterion`. You'd construct a large synthetic markdown
// file (e.g., 1MB of text with mixed formatting) and measure the time it takes to consume
// the entire `Parser` iterator. `std::hint::black_box` should be used to prevent the
// compiler from optimizing away the loop if you don't collect the events.
//
// **Next Steps:**
// - Implement a proper state machine for handling nested tags like `[links](...)`.
// - Transition `Event::Text` entirely to zero-copy `Cow<'a, str>` to allow for unescaping
//   entities while minimizing allocations.

// =========================================================================================
// Tests
// =========================================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paragraph() {
        let input = "Hello world";
        let parser = Parser::new(input);
        let mut html = String::new();
        html::push_html(&mut html, parser);

        assert_eq!(html, "<p>Hello world</p>\n");
    }

    #[test]
    fn test_headings() {
        let input = "# Heading 1\n## Heading 2\n### Heading 3";
        let parser = Parser::new(input);
        let mut html = String::new();
        html::push_html(&mut html, parser);

        assert_eq!(html, "<h1>Heading 1</h1>\n<h2>Heading 2</h2>\n<h3>Heading 3</h3>\n");
    }

    #[test]
    fn test_lists() {
        let input = "- Item 1\n- Item 2";
        let parser = Parser::new(input);
        let mut html = String::new();
        html::push_html(&mut html, parser);

        assert_eq!(html, "<ul>\n<li>Item 1</li>\n<li>Item 2</li>\n</ul>\n");
    }

    #[test]
    fn test_inline_formatting() {
        let input = "This is **bold** and *italic*.";
        let parser = Parser::new(input);
        let mut html = String::new();
        html::push_html(&mut html, parser);

        assert_eq!(html, "<p>This is <strong>bold</strong> and <em>italic</em>.</p>\n");
    }

    #[test]
    fn test_mixed_document() {
        let input = "# My Blog\n\nWelcome to my **awesome** blog.\n\n- Rust\n- Performance";
        let parser = Parser::new(input);
        let mut html = String::new();
        html::push_html(&mut html, parser);

        assert_eq!(html, "<h1>My Blog</h1>\n<p>Welcome to my <strong>awesome</strong> blog.</p>\n<ul>\n<li>Rust</li>\n<li>Performance</li>\n</ul>\n");
    }
}
    #[test]
    fn test_multibyte_markdown() {
        let input = "This is **bóld** and *îtalic*.";
        let parser = Parser::new(input);
        let mut html = String::new();
        html::push_html(&mut html, parser);
        assert_eq!(html, "<p>This is <strong>bóld</strong> and <em>îtalic</em>.</p>\n");
    }
