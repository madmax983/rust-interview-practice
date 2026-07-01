//! # Markdown Parser
//!
//! A minimal from-scratch Markdown to HTML compiler using a pull-based event stream approach.
//!
//! ## What This Implements
//! This module implements a core subset of a Markdown compiler, taking a markdown string
//! and emitting HTML. It parses block elements (Paragraphs, Headers) and inline elements
//! (Bold, Italic).
//!
//! ## Crates Replaced
//! - `pulldown-cmark`
//! - `comrak`
//!
//! ## Real-World Systems
//! - Static site generators (Hugo, Jekyll, Zola)
//! - Code forges (GitHub, GitLab)
//! - Note-taking apps (Obsidian, Notion)
//!
//! ## Why Build It Yourself?
//! Writing a markdown parser reveals the complexities of state machine-based parsing
//! and why a pure pull-based event stream (like `pulldown-cmark`) is more memory
//! efficient than building a full Abstract Syntax Tree (AST) for every document.
//!
//! ## Architecture
//!
//! ### Event Stream
//! Instead of building an AST, the parser yields a sequence of `Event`s:
//! `Start(Tag)`, `Text(String)`, `End(Tag)`.
//!
//! ### Block & Inline State Machines
//! The parser identifies block boundaries (e.g., newlines separating paragraphs) and
//! then processes the inline content within those blocks, emitting the appropriate events.
//!
//! ### HTML Generation
//! Consumes the event stream to generate an HTML string. It explicitly trims trailing
//! whitespace/newlines from block elements before wrapping them to avoid empty tags.
//!
//! ### Complexity
//! - **Time Complexity:** O(N) where N is the length of the document.
//! - **Space Complexity:** O(M) where M is the size of the output string (and the maximum
//!   depth of the tag stack, which is small). No full AST is built.
//!
//! ## Benchmarking Note
//! Benchmarking a Markdown parser typically focuses on throughput (MB/s). Use `criterion`
//! with large, complex markdown files to measure performance against `pulldown-cmark`.
//!
//! ## Footer
//!
//! ### Comparison to Canonical Crates
//! `pulldown-cmark` is fully CommonMark compliant, highly optimized, and handles complex
//! edge cases (like nested lists, blockquotes, and escaping) flawlessly using a highly
//! tuned state machine.
//!
//! ### What's Missing
//! - Full CommonMark compliance.
//! - Lists, Blockquotes, Code blocks.
//! - Links and Images.
//! - Robust escaping and HTML entity handling.
//!
//! ### Next Steps
//! - Add support for lists and blockquotes.
//! - Implement link and image parsing in the inline state machine.

use std::fmt::Write;

#[derive(Debug, PartialEq, Clone)]
pub enum Tag {
    Paragraph,
    Header(u8),
    Strong,
    Emphasis,
}

#[derive(Debug, PartialEq, Clone)]
pub enum Event {
    Start(Tag),
    End(Tag),
    Text(String),
}

pub struct Parser<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    pub fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    /// Pulls all events from the markdown string.
    pub fn parse(&mut self) -> Vec<Event> {
        let mut events = Vec::new();

        while self.pos < self.input.len() {
            self.skip_whitespace();
            if self.pos >= self.input.len() {
                break;
            }

            // Check for block elements
            if self.input[self.pos..].starts_with('#') {
                self.parse_header(&mut events);
            } else {
                self.parse_paragraph(&mut events);
            }
        }

        events
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.input.len() {
            let c = self.input[self.pos..].chars().next().unwrap();
            if c == ' ' || c == '\n' || c == '\r' {
                self.pos += c.len_utf8();
            } else {
                break;
            }
        }
    }

    fn parse_header(&mut self, events: &mut Vec<Event>) {
        let mut level = 0;
        while self.pos < self.input.len() && self.input[self.pos..].starts_with('#') {
            level += 1;
            self.pos += 1;
        }

        // skip space after #
        if self.pos < self.input.len() && self.input[self.pos..].starts_with(' ') {
            self.pos += 1;
        }

        events.push(Event::Start(Tag::Header(level)));

        let mut text = String::new();
        while self.pos < self.input.len() {
            let c = self.input[self.pos..].chars().next().unwrap();
            if c == '\n' {
                break;
            }
            text.push(c);
            self.pos += c.len_utf8();
        }

        // GOTCHA: Need to explicitly trim trailing whitespace to avoid empty trailing spaces in tags.
        let text = text.trim_end();
        if !text.is_empty() {
             self.parse_inline(text, events);
        }

        events.push(Event::End(Tag::Header(level)));
    }

    fn parse_paragraph(&mut self, events: &mut Vec<Event>) {
        events.push(Event::Start(Tag::Paragraph));

        let mut text = String::new();
        while self.pos < self.input.len() {
            let rest = &self.input[self.pos..];

            // A double newline denotes the end of a paragraph
            if rest.starts_with("\n\n") || rest.starts_with("\r\n\r\n") {
                break;
            }

            let c = rest.chars().next().unwrap();
            text.push(c);
            self.pos += c.len_utf8();
        }

        // PRODUCTION NOTE: Explicitly trimming trailing whitespace/newlines from block elements.
        let text = text.trim_end();
        if !text.is_empty() {
            self.parse_inline(text, events);
        }

        events.push(Event::End(Tag::Paragraph));
    }

    fn parse_inline(&self, mut text: &str, events: &mut Vec<Event>) {
        // Simple inline parsing for **strong** and *emphasis*.
        // A real inline parser needs a more robust state machine to handle nested tags
        // and escaped characters.

        while !text.is_empty() {
            if text.starts_with("**") {
                if let Some(end_idx) = text[2..].find("**") {
                    events.push(Event::Start(Tag::Strong));
                    events.push(Event::Text(text[2..2+end_idx].to_string()));
                    events.push(Event::End(Tag::Strong));
                    text = &text[2+end_idx+2..];
                    continue;
                }
            } else if text.starts_with("*") {
                if let Some(end_idx) = text[1..].find("*") {
                    events.push(Event::Start(Tag::Emphasis));
                    events.push(Event::Text(text[1..1+end_idx].to_string()));
                    events.push(Event::End(Tag::Emphasis));
                    text = &text[1+end_idx+1..];
                    continue;
                }
            }

            // Normal text
            let next_special = text.find('*').unwrap_or(text.len());
            if next_special == 0 {
                // Should be handled above, but just in case it's an unmatched *
                events.push(Event::Text(text[0..1].to_string()));
                text = &text[1..];
            } else {
                events.push(Event::Text(text[0..next_special].to_string()));
                text = &text[next_special..];
            }
        }
    }
}

pub fn push_html(html: &mut String, events: Vec<Event>) {
    for event in events {
        match event {
            Event::Start(Tag::Paragraph) => write!(html, "<p>").unwrap(),
            Event::End(Tag::Paragraph) => write!(html, "</p>\n").unwrap(),
            Event::Start(Tag::Header(level)) => write!(html, "<h{}>", level).unwrap(),
            Event::End(Tag::Header(level)) => write!(html, "</h{}>\n", level).unwrap(),
            Event::Start(Tag::Strong) => write!(html, "<strong>").unwrap(),
            Event::End(Tag::Strong) => write!(html, "</strong>").unwrap(),
            Event::Start(Tag::Emphasis) => write!(html, "<em>").unwrap(),
            Event::End(Tag::Emphasis) => write!(html, "</em>").unwrap(),
            Event::Text(text) => write!(html, "{}", text).unwrap(),
        }
    }
}

pub fn render_html(input: &str) -> String {
    let mut parser = Parser::new(input);
    let events = parser.parse();

    let mut html = String::with_capacity(input.len());
    push_html(&mut html, events);
    html
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_paragraph() {
        let input = "Hello world";
        let html = render_html(input);
        assert_eq!(html, "<p>Hello world</p>\n");
    }

    #[test]
    fn test_parse_multiple_paragraphs() {
        let input = "Para 1\n\nPara 2";
        let html = render_html(input);
        assert_eq!(html, "<p>Para 1</p>\n<p>Para 2</p>\n");
    }

    #[test]
    fn test_parse_headers() {
        let input = "# Header 1\n## Header 2";
        let html = render_html(input);
        assert_eq!(html, "<h1>Header 1</h1>\n<h2>Header 2</h2>\n");
    }

    #[test]
    fn test_parse_inline_strong() {
        let input = "Hello **world**!";
        let html = render_html(input);
        assert_eq!(html, "<p>Hello <strong>world</strong>!</p>\n");
    }

    #[test]
    fn test_parse_inline_emphasis() {
        let input = "Hello *world*!";
        let html = render_html(input);
        assert_eq!(html, "<p>Hello <em>world</em>!</p>\n");
    }

    #[test]
    fn test_trailing_whitespace_trimmed() {
        let input = "# Header \n\nPara  ";
        let html = render_html(input);
        assert_eq!(html, "<h1>Header</h1>\n<p>Para</p>\n");
    }
}
