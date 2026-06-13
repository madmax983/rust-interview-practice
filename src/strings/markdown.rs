//! # Markdown Parser
//!
//! What this implements and what crate(s) it replaces:
//! This implements a minimal, pull-based Markdown to HTML compiler.
//! It replaces crates like `pulldown-cmark` and `comrak`.
//!
//! Real-world systems that use this:
//! Static site generators (Hugo, Zola), documentation systems (rustdoc),
//! and chat applications (Discord, Slack) rely on Markdown parsers to
//! convert user-generated text into formatted HTML.
//!
//! Why build it yourself?
//! Building a Markdown parser teaches you how to implement a pull-based event stream
//! (similar to SAX parsing for XML) and manage block vs. inline state machines.
//! It highlights the complexities of parsing text formats without strict grammars.
//!
//! ## Architecture
//!
//! ```text
//! [ Markdown Text ] -> (Lexer/Event Iterator) -> [ Stream of Events ] -> (HTML Renderer) -> [ HTML String ]
//! ```
//!
//! ### Invariants
//! - **Event Stream**: The parser yields events (`Start(Tag)`, `End(Tag)`, `Text(String)`) lazily.
//! - **Block vs Inline**: Block elements (Headers, Lists) contain inline elements (Bold, Italics, Links).
//! - **Escaping**: HTML special characters (`<`, `>`, `&`) must be escaped to prevent XSS.
//!
//! ### Complexity
//! - **Time**: `O(N)` where N is the length of the input string.
//! - **Space**: `O(D)` where D is the maximum depth of nested tags (very small), avoiding full AST overhead.
//!
//! ### Design Decisions
//! - **Pull-Based API**: Instead of building a full Abstract Syntax Tree (AST) in memory,
//!   we use an `Iterator` of events. This requires less memory and allows streaming processing.
//! - **Simplified Rules**: Standard CommonMark is famously complex. We implement a subset
//!   (Headers, Bold, Italics, Links, Lists) to demonstrate the core state machine mechanics.
//!
//! ## Implementation

use std::iter::Peekable;
use std::str::CharIndices;

// =========================================================================================
// Types
// =========================================================================================

#[derive(Debug, PartialEq, Clone)]
pub enum Tag {
    Paragraph,
    Header(u8),
    Strong,
    Emphasis,
    Link(String, String), // url, title
    List,
    Item,
}

#[derive(Debug, PartialEq)]
pub enum Event<'a> {
    Start(Tag),
    End(Tag),
    Text(&'a str),
    Html(&'a str),
}

// =========================================================================================
// Parser
// =========================================================================================

/// A pull-based Markdown parser that yields an iterator of events.
pub struct Parser<'a> {
    text: &'a str,
    chars: Peekable<CharIndices<'a>>,
    // Used to simulate an event queue when a single token parses into multiple events.
    event_queue: Vec<Event<'a>>,
    // Stack of open block/inline tags to ensure correct closing.
    open_tags: Vec<Tag>,
}

impl<'a> Parser<'a> {
    pub fn new(text: &'a str) -> Self {
        Self {
            text,
            chars: text.char_indices().peekable(),
            event_queue: Vec::new(),
            open_tags: Vec::new(),
        }
    }

    /// RUST INSIGHT: `char.len_utf8()`
    /// We must use `char.len_utf8()` to advance byte indices safely.
    /// Attempting to use `index + 1` causes panics on multibyte characters.
    fn consume_while<F>(&mut self, condition: F) -> &'a str
    where
        F: Fn(char) -> bool,
    {
        let start = match self.chars.peek() {
            Some(&(i, _)) => i,
            None => return "",
        };

        let mut end = start;
        while let Some(&(i, c)) = self.chars.peek() {
            if condition(c) {
                end = i + c.len_utf8();
                self.chars.next(); // consume
            } else {
                break;
            }
        }

        &self.text[start..end]
    }

    fn skip_whitespace(&mut self) {
        self.consume_while(|c| c == ' ' || c == '\t');
    }

    fn parse_inline(&mut self, text: &'a str) -> Vec<Event<'a>> {
        let mut events = Vec::new();
        let mut i = 0;
        let mut start_text = 0;

        while i < text.len() {
            let rest = &text[i..];

            // ⚡ BOLT OPTIMIZATION: Use `starts_with` and manual index advancement (`char.len_utf8()`)
            // to avoid allocating strings or panicking on boundaries.
            if rest.starts_with("**") {
                if start_text < i {
                    events.push(Event::Text(&text[start_text..i]));
                }
                // Find closing **
                if let Some(end) = text[i + 2..].find("**") {
                    let inner_start = i + 2;
                    let inner_end = inner_start + end;
                    events.push(Event::Start(Tag::Strong));
                    events.push(Event::Text(&text[inner_start..inner_end]));
                    events.push(Event::End(Tag::Strong));
                    i = inner_end + 2;
                    start_text = i;
                    continue;
                }
            } else if rest.starts_with('*') {
                if start_text < i {
                    events.push(Event::Text(&text[start_text..i]));
                }
                if let Some(end) = text[i + 1..].find('*') {
                    let inner_start = i + 1;
                    let inner_end = inner_start + end;
                    events.push(Event::Start(Tag::Emphasis));
                    events.push(Event::Text(&text[inner_start..inner_end]));
                    events.push(Event::End(Tag::Emphasis));
                    i = inner_end + 1;
                    start_text = i;
                    continue;
                }
            } else if rest.starts_with('[') {
                if start_text < i {
                    events.push(Event::Text(&text[start_text..i]));
                }

                // Parse Link [text](url)
                if let Some(bracket_end) = rest.find(']') {
                    let link_text = &rest[1..bracket_end];
                    let after_bracket = &rest[bracket_end + 1..];

                    if after_bracket.starts_with('(') {
                        if let Some(paren_end) = after_bracket.find(')') {
                            let url = &after_bracket[1..paren_end];

                            events.push(Event::Start(Tag::Link(url.to_string(), "".to_string())));
                            events.push(Event::Text(link_text));
                            events.push(Event::End(Tag::Link(url.to_string(), "".to_string())));

                            i += bracket_end + 1 + paren_end + 1;
                            start_text = i;
                            continue;
                        }
                    }
                }
            }

            // Move to next character boundary
            let c = text[i..].chars().next().unwrap();
            i += c.len_utf8();
        }

        if start_text < text.len() {
            events.push(Event::Text(&text[start_text..]));
        }

        events
    }

    fn parse_block(&mut self) {
        self.skip_whitespace();

        if self.chars.peek().is_none() {
            return;
        }

        // Check for Header
        let start_idx = self.chars.peek().unwrap().0;
        let mut hashes = 0;
        while let Some(&(_, '#')) = self.chars.peek() {
            hashes += 1;
            self.chars.next();
        }

        if hashes > 0 && hashes <= 6 {
            if let Some(&(_, ' ')) = self.chars.peek() {
                self.chars.next(); // consume space
                let content = self.consume_while(|c| c != '\n');
                if let Some(&(_, '\n')) = self.chars.peek() {
                    self.chars.next(); // consume newline
                }

                let inline_events = self.parse_inline(content);
                self.event_queue
                    .push(Event::Start(Tag::Header(hashes as u8)));
                self.event_queue.extend(inline_events);
                self.event_queue.push(Event::End(Tag::Header(hashes as u8)));
                return;
            }
        }

        // Backtrack if not a header
        self.chars = self.text[start_idx..].char_indices().peekable();
        // GOTCHA: `char_indices` yields indices relative to the slice `start_idx`.
        // However, we only use `consume_while` within `parse_block`, which recalculates
        // offsets correctly or we just consume lines directly.
        // Actually, replacing `self.chars` with a sub-slice iterator breaks global indices.
        // Let's reset properly.
        self.chars = self.text.char_indices().peekable();
        for _ in 0..self.text[..start_idx].chars().count() {
            self.chars.next();
        }

        // Check for Unordered List
        if let Some(&(_i, '-')) = self.chars.peek() {
            self.chars.next();
            if let Some(&(_, ' ')) = self.chars.peek() {
                self.chars.next();
                let content = self.consume_while(|c| c != '\n');
                if let Some(&(_, '\n')) = self.chars.peek() {
                    self.chars.next();
                }

                let inline_events = self.parse_inline(content);
                // Simplified list: just emit item. Real parser tracks list state.
                self.event_queue.push(Event::Start(Tag::Item));
                self.event_queue.extend(inline_events);
                self.event_queue.push(Event::End(Tag::Item));
                return;
            }
        }

        // Reset again
        self.chars = self.text.char_indices().peekable();
        for _ in 0..self.text[..start_idx].chars().count() {
            self.chars.next();
        }

        // Paragraph
        let mut content_end = start_idx;
        while let Some(&(i, c)) = self.chars.peek() {
            if c == '\n' {
                self.chars.next();
                if let Some(&(_, '\n')) = self.chars.peek() {
                    // Double newline ends paragraph
                    break;
                }
                content_end = i + 1; // Include single newline
            } else {
                content_end = i + c.len_utf8();
                self.chars.next();
            }
        }

        let content = &self.text[start_idx..content_end].trim_end();
        if !content.is_empty() {
            let inline_events = self.parse_inline(content);
            self.event_queue.push(Event::Start(Tag::Paragraph));
            self.event_queue.extend(inline_events);
            self.event_queue.push(Event::End(Tag::Paragraph));
        }
    }
}

impl<'a> Iterator for Parser<'a> {
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if !self.event_queue.is_empty() {
            return Some(self.event_queue.remove(0));
        }

        if self.chars.peek().is_none() {
            return None;
        }

        self.parse_block();

        if !self.event_queue.is_empty() {
            return Some(self.event_queue.remove(0));
        }

        None
    }
}

// =========================================================================================
// HTML Renderer
// =========================================================================================

pub fn push_html(html: &mut String, parser: Parser) {
    for event in parser {
        match event {
            Event::Start(tag) => match tag {
                Tag::Paragraph => html.push_str("<p>"),
                Tag::Header(level) => html.push_str(&format!("<h{}>", level)),
                Tag::Strong => html.push_str("<strong>"),
                Tag::Emphasis => html.push_str("<em>"),
                Tag::Link(url, _) => html.push_str(&format!("<a href=\"{}\">", escape_html(&url))),
                Tag::List => html.push_str("<ul>\n"),
                Tag::Item => html.push_str("<li>"),
            },
            Event::End(tag) => match tag {
                Tag::Paragraph => html.push_str("</p>\n"),
                Tag::Header(level) => html.push_str(&format!("</h{}>\n", level)),
                Tag::Strong => html.push_str("</strong>"),
                Tag::Emphasis => html.push_str("</em>"),
                Tag::Link(_, _) => html.push_str("</a>"),
                Tag::List => html.push_str("</ul>\n"),
                Tag::Item => html.push_str("</li>\n"),
            },
            Event::Text(text) => html.push_str(&escape_html(text)),
            Event::Html(raw) => html.push_str(raw),
        }
    }
}

fn escape_html(s: &str) -> String {
    let mut escaped = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '&' => escaped.push_str("&amp;"),
            '"' => escaped.push_str("&quot;"),
            _ => escaped.push(c),
        }
    }
    escaped
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `pulldown-cmark`: The standard Rust Markdown parser. It is strictly CommonMark compliant,
//   extremely fast, and uses a similar pull-based event iterator.
// - `comrak`: Wraps a C library (cmark) or implements a full AST. Good for manipulating the DOM
//   before rendering.
//
// What's missing vs. production:
// - **CommonMark Compliance**: Real markdown parsing requires complex rules for nested lists,
//   blockquotes, indented code blocks, and HTML tags.
// - **Backtracking**: Our simplistic backtracking on header failure uses `chars().count()`, which is `O(N)`.
//   Production parsers avoid backtracking or use efficient state tracking.
// - **References**: `[link][ref]` style links require two passes: one to collect refs, one to render.
//
// Next steps:
// 1. Support ` ``` ` fenced code blocks.
// 2. Properly manage nested `Tag::List` elements around `Tag::Item`.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_paragraph() {
        let parser = Parser::new("Hello world");
        let events: Vec<_> = parser.collect();
        assert_eq!(
            events,
            vec![
                Event::Start(Tag::Paragraph),
                Event::Text("Hello world"),
                Event::End(Tag::Paragraph),
            ]
        );
    }

    #[test]
    fn test_parse_header() {
        let parser = Parser::new("## Header 2\nText");
        let events: Vec<_> = parser.collect();
        assert_eq!(
            events,
            vec![
                Event::Start(Tag::Header(2)),
                Event::Text("Header 2"),
                Event::End(Tag::Header(2)),
                Event::Start(Tag::Paragraph),
                Event::Text("Text"),
                Event::End(Tag::Paragraph),
            ]
        );
    }

    #[test]
    fn test_parse_inline_formatting() {
        let parser = Parser::new("This is **bold** and *italic*.");
        let events: Vec<_> = parser.collect();
        assert_eq!(
            events,
            vec![
                Event::Start(Tag::Paragraph),
                Event::Text("This is "),
                Event::Start(Tag::Strong),
                Event::Text("bold"),
                Event::End(Tag::Strong),
                Event::Text(" and "),
                Event::Start(Tag::Emphasis),
                Event::Text("italic"),
                Event::End(Tag::Emphasis),
                Event::Text("."),
                Event::End(Tag::Paragraph),
            ]
        );
    }

    #[test]
    fn test_parse_link() {
        let parser = Parser::new("[Rust](https://rust-lang.org)");
        let events: Vec<_> = parser.collect();
        assert_eq!(
            events,
            vec![
                Event::Start(Tag::Paragraph),
                Event::Start(Tag::Link(
                    "https://rust-lang.org".to_string(),
                    "".to_string()
                )),
                Event::Text("Rust"),
                Event::End(Tag::Link(
                    "https://rust-lang.org".to_string(),
                    "".to_string()
                )),
                Event::End(Tag::Paragraph),
            ]
        );
    }

    #[test]
    fn test_html_rendering() {
        let text = "## Hello\nThis is **Rust**.";
        let parser = Parser::new(text);
        let mut html = String::new();
        push_html(&mut html, parser);

        assert_eq!(
            html,
            "<h2>Hello</h2>\n<p>This is <strong>Rust</strong>.</p>\n"
        );
    }

    #[test]
    fn test_escape_html() {
        let text = "<script>alert('XSS & more');</script>";
        let parser = Parser::new(text);
        let mut html = String::new();
        push_html(&mut html, parser);

        assert_eq!(
            html,
            "<p>&lt;script&gt;alert('XSS &amp; more');&lt;/script&gt;</p>\n"
        );
    }

    #[test]
    fn test_multibyte_characters() {
        // Ensure that char.len_utf8() advancement handles emojis correctly.
        let parser = Parser::new("Hello 🦀 **World**");
        let events: Vec<_> = parser.collect();
        assert_eq!(
            events,
            vec![
                Event::Start(Tag::Paragraph),
                Event::Text("Hello 🦀 "),
                Event::Start(Tag::Strong),
                Event::Text("World"),
                Event::End(Tag::Strong),
                Event::End(Tag::Paragraph),
            ]
        );
    }
}
