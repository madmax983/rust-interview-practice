//! # Markdown Parser
//!
//! Implements a minimal Markdown to HTML compiler using a pull-based event stream approach.
//!
//! **Replaces Crates:** `pulldown-cmark`, `comrak`
//!
//! **Real-world Usage:**
//! - Static site generators (Hugo, Zola)
//! - Blog rendering engines
//! - Documentation generators (rustdoc)
//!
//! **Why build it yourself?**
//! Parsing Markdown teaches you how to handle semi-structured text using state machines.
//! Real Markdown is context-sensitive (e.g. knowing if you are in a blockquote affects how
//! you parse a list). Building a pull-based parser (like `pulldown-cmark`) teaches you how to
//! stream events rather than building a massive intermediate Abstract Syntax Tree (AST), which
//! is highly memory efficient.
//!
//! # Architecture
//!
//! **Data Structure & Flow:**
//!
//!      Input String
//!           │
//!           ▼
//!      Event Iterator (`Parser<'a>`)
//!           │ (Yields `Start(Tag)`, `Text(&str)`, `End(Tag)`)
//!           ▼
//!      HTML Compiler (consumes events, builds String)
//!
//! **Invariants:**
//! 1. The parser must yield correctly nested `Start` and `End` events.
//! 2. Inline lookaheads for formatting (like `**bold**`) must advance using `char.len_utf8()` to avoid multi-byte slice panics.
//!
//! **Complexity:**
//! - Parsing: O(N) where N is the length of the string.
//! - Space: O(D) where D is the maximum depth of nested elements (due to the state stack), rather than O(N) for a full AST.
//!
//! **Design Decisions:**
//! - **Pull-Based Parsing:** Instead of building an AST tree in memory, the parser is an Iterator that yields formatting events. The HTML renderer just `fold`s over these events.
//! - **UTF-8 Safety:** Uses explicit byte indexing combined with `char.len_utf8()` to jump ahead in the string safely.

#[derive(Debug, PartialEq, Clone)]
pub enum Tag {
    Paragraph,
    Header(u8), // H1 to H6
    Strong,
    Emphasis,
}

#[derive(Debug, PartialEq, Clone)]
pub enum Event<'a> {
    Start(Tag),
    End(Tag),
    Text(&'a str),
}

// =========================================================================================
// Parser
// =========================================================================================

pub struct Parser<'a> {
    input: &'a str,
    pos: usize,
    stack: Vec<Tag>,
    events_queue: Vec<Event<'a>>, // For buffering inline events
}

impl<'a> Parser<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            input,
            pos: 0,
            stack: Vec::new(),
            events_queue: Vec::new(),
        }
    }

    fn peek(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn advance(&mut self, char_len: usize) {
        self.pos += char_len;
    }

    /// Reads until the next newline or EOF
    fn read_line(&mut self) -> &'a str {
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c == '\n' {
                let end = self.pos;
                self.advance(c.len_utf8()); // Consume newline
                return &self.input[start..end];
            }
            self.advance(c.len_utf8());
        }
        &self.input[start..self.pos]
    }

    /// Helper to parse inline formatting within a text block
    fn parse_inline(text: &'a str) -> Vec<Event<'a>> {
        let mut events = Vec::new();
        let mut pos = 0;
        let mut text_start = 0;

        while pos < text.len() {
            // RUST INSIGHT: We must be careful to index at char boundaries.
            // `chars()` handles the decoding, but to slice the original string,
            // we need byte offsets. We advance `pos` by `c.len_utf8()`.
            let tail = &text[pos..];
            if tail.starts_with("**") {
                // Find closing **
                let mut lookahead = pos + 2;
                let mut found = false;
                while lookahead < text.len() {
                    if text[lookahead..].starts_with("**") {
                        found = true;
                        break;
                    }
                    lookahead += text[lookahead..].chars().next().unwrap().len_utf8();
                }

                if found {
                    if text_start < pos {
                        events.push(Event::Text(&text[text_start..pos]));
                    }
                    events.push(Event::Start(Tag::Strong));

                    // Recursive call to handle nested emphasis inside strong, though omitted here for simplicity
                    events.push(Event::Text(&text[pos + 2..lookahead]));

                    events.push(Event::End(Tag::Strong));

                    pos = lookahead + 2;
                    text_start = pos;
                    continue;
                }
            } else if tail.starts_with("*") {
                 // Find closing *
                 let mut lookahead = pos + 1;
                 let mut found = false;
                 while lookahead < text.len() {
                     if text[lookahead..].starts_with("*") && !text[lookahead..].starts_with("**") {
                         found = true;
                         break;
                     }
                     lookahead += text[lookahead..].chars().next().unwrap().len_utf8();
                 }

                 if found {
                     if text_start < pos {
                         events.push(Event::Text(&text[text_start..pos]));
                     }
                     events.push(Event::Start(Tag::Emphasis));
                     events.push(Event::Text(&text[pos + 1..lookahead]));
                     events.push(Event::End(Tag::Emphasis));

                     pos = lookahead + 1;
                     text_start = pos;
                     continue;
                 }
            }

            // Advance by character length
            pos += text[pos..].chars().next().unwrap().len_utf8();
        }

        if text_start < text.len() {
            events.push(Event::Text(&text[text_start..]));
        }

        events
    }
}

impl<'a> Iterator for Parser<'a> {
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        // Drain buffered events first
        if !self.events_queue.is_empty() {
            return Some(self.events_queue.remove(0));
        }

        // Close open tags if at EOF
        if self.pos >= self.input.len() {
            return self.stack.pop().map(Event::End);
        }

        let start_pos = self.pos;
        let mut is_header = false;
        let mut header_level = 0;

        // Check for Headers
        while let Some(c) = self.peek() {
            if c == '#' {
                header_level += 1;
                self.advance(c.len_utf8());
            } else if c == ' ' && header_level > 0 {
                is_header = true;
                self.advance(c.len_utf8());
                break;
            } else {
                // Not a valid header, rollback (simplification: we just don't rollback, treat as text)
                break;
            }
        }

        if is_header && header_level <= 6 {
            let tag = Tag::Header(header_level as u8);
            self.stack.push(tag.clone());

            let line = self.read_line();

            // Queue up the contents and the end tag
            let mut inline_events = Self::parse_inline(line);
            self.events_queue.append(&mut inline_events);
            self.events_queue.push(Event::End(self.stack.pop().unwrap()));

            return Some(Event::Start(tag));
        }

        // Rollback if it wasn't a header
        self.pos = start_pos;

        // Otherwise, it's a paragraph
        let tag = Tag::Paragraph;
        self.stack.push(tag.clone());

        // Read until blank line
        let p_start = self.pos;
        while self.pos < self.input.len() {
            let line = self.read_line();
            if line.trim().is_empty() {
                break;
            }
        }

        // RUST INSIGHT: We handle the trailing blank line properly by using trim_end()
        // to avoid empty `<p></p>` blocks for extra newlines between paragraphs.
        let raw_slice = &self.input[p_start..self.pos];
        let text_slice = raw_slice.trim(); // Trim spaces/newlines

        if text_slice.is_empty() {
             // Rollback the tag push and skip to next token
             self.stack.pop();
             return self.next();
        }

        let mut inline_events = Self::parse_inline(text_slice);
        self.events_queue.append(&mut inline_events);
        self.events_queue.push(Event::End(self.stack.pop().unwrap()));

        Some(Event::Start(tag))
    }
}

// =========================================================================================
// HTML Renderer
// =========================================================================================

pub fn render_html(parser: Parser) -> String {
    let mut html = String::with_capacity(parser.input.len() * 2);

    // RUST INSIGHT: The `format!` macro allocates a new string. By using `push_str`
    // onto a pre-allocated buffer, we save countless intermediate allocations.
    for event in parser {
        match event {
            Event::Start(Tag::Paragraph) => html.push_str("<p>"),
            Event::End(Tag::Paragraph) => html.push_str("</p>\n"),
            Event::Start(Tag::Header(level)) => html.push_str(&format!("<h{}>", level)),
            Event::End(Tag::Header(level)) => html.push_str(&format!("</h{}>\n", level)),
            Event::Start(Tag::Strong) => html.push_str("<strong>"),
            Event::End(Tag::Strong) => html.push_str("</strong>"),
            Event::Start(Tag::Emphasis) => html.push_str("<em>"),
            Event::End(Tag::Emphasis) => html.push_str("</em>"),
            Event::Text(text) => html.push_str(text),
        }
    }
    html
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `pulldown-cmark`: A fully spec-compliant (CommonMark) pull parser. Highly optimized, uses zero-copy string slices everywhere.
// - Our implementation is extremely naive. It doesn't handle lists, blockquotes, code blocks, or nested inline formatting properly.
//
// Missing vs. Production:
// - **CommonMark Spec:** We are nowhere close to passing the 600+ test cases of the CommonMark spec.
// - **Zero-Copy Blocks:** True zero-copy parsing of multiline paragraphs requires yielding multiple text events or building a smart Cow representation.
// - **Escaping:** We don't handle HTML escaping (e.g. turning `<` into `&lt;`).
//
// Next Steps:
// 1. Add support for lists (requires maintaining more state in the parser).
// 2. Implement HTML escaping for text nodes.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header() {
        let parser = Parser::new("# Hello World");
        let html = render_html(parser);
        assert_eq!(html, "<h1>Hello World</h1>\n");
    }

    #[test]
    fn test_paragraph() {
        let parser = Parser::new("Just some text.");
        let html = render_html(parser);
        assert_eq!(html, "<p>Just some text.</p>\n");
    }

    #[test]
    fn test_inline_strong() {
        let parser = Parser::new("This is **bold** text.");
        let html = render_html(parser);
        assert_eq!(html, "<p>This is <strong>bold</strong> text.</p>\n");
    }

    #[test]
    fn test_inline_emphasis() {
        let parser = Parser::new("This is *italic* text.");
        let html = render_html(parser);
        assert_eq!(html, "<p>This is <em>italic</em> text.</p>\n");
    }

    #[test]
    fn test_multiple_blocks() {
        let markdown = "# Title\n\nA paragraph.\n\n## Subtitle\n\nMore text.";
        let parser = Parser::new(markdown);
        let html = render_html(parser);
        assert_eq!(
            html,
            "<h1>Title</h1>\n<p>A paragraph.</p>\n<h2>Subtitle</h2>\n<p>More text.</p>\n"
        );
    }

    #[test]
    fn test_multibyte_characters() {
        let parser = Parser::new("Hello **世界**.");
        let html = render_html(parser);
        assert_eq!(html, "<p>Hello <strong>世界</strong>.</p>\n");
    }
}
