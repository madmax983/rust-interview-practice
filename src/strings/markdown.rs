//! # Markdown Parser
//!
//! Implements a minimal Markdown to HTML compiler using a pull-based event stream
//! approach and block/inline state machines.
//!
//! **Replaces Crates:** `pulldown-cmark`, `comrak`
//!
//! **Real-world Usage:**
//! - Rendering user comments and README files on platforms like GitHub or Reddit.
//! - Static site generators transforming `.md` files to HTML.
//!
//! **Why build it yourself?**
//! Building a Markdown parser teaches you about text state machines and stream processing.
//! You'll see how to avoid creating a massive Abstract Syntax Tree (AST) by emitting
//! rendering events directly, which is crucial for parsing large documents without blowing
//! up memory.
//!
//! ## Architecture
//!
//! The parser processes the input string line by line to identify blocks (Paragraphs, Headings, etc.),
//! and then processes the text within those blocks to handle inline formatting (Bold, Italic).
//!
//! ### Data Structure Diagram
//! ```text
//! Input String -> Block Lexer -> [Block Event, Block Event, ...]
//!                                |
//!                                v
//!                           Inline Lexer -> HTML Renderer -> Output String
//! ```
//!
//! ### Invariants
//! - Must gracefully handle unmatched formatting tags without panicking.
//! - Multibyte characters must not cause string slicing panics.
//!
//! ### Complexity
//! - **Parsing:** O(N) where N is the length of the string, as we do mostly single passes with bounded lookahead.
//! - **Space:** O(B) where B is the size of the current block, as we do not build a full AST.
//!
//! ## Design Decisions
//! - **Event Stream over AST:** We yield events (StartBlock, EndBlock, Text) instead of a deep tree,
//!   keeping memory overhead low, matching the design of `pulldown-cmark`.
//! - **Safe String Advancement:** We use `char.len_utf8()` to advance indices safely across multibyte characters.
//!
//! ## Benchmarking Note
//! Benchmarking typically involves taking large standard markdown files (like a spec or long README)
//! and measuring throughput (MB/s). We can use `criterion` for statistical measurements.
//!
//! ## Footer
//! - **Comparison to Canonical:** `pulldown-cmark` is vastly more complex, strictly adhering to the CommonMark
//!   specification with comprehensive tag coverage and handling of malicious inputs (like incredibly deep nested lists).
//! - **What's Missing:** Lists, blockquotes, code blocks, links, images, tables, escaping, and full CommonMark compliance.
//! - **Next Steps:** Implement parsing for lists and code blocks by tracking block state across multiple lines.

#[derive(Debug, PartialEq, Clone)]
pub enum Event {
    StartHeading(usize),
    EndHeading(usize),
    StartParagraph,
    EndParagraph,
    Text(String),
    StartBold,
    EndBold,
    StartItalic,
    EndItalic,
}

pub struct MarkdownParser<'a> {
    input: &'a str,
}

impl<'a> MarkdownParser<'a> {
    pub fn new(input: &'a str) -> Self {
        Self { input }
    }

    pub fn parse(&self) -> Vec<Event> {
        let mut events = Vec::new();
        let mut lines = self.input.lines().peekable();

        while let Some(line) = lines.next() {
            let line = line.trim_end();
            if line.is_empty() {
                continue;
            }

            // Detect heading
            if line.starts_with('#') {
                let mut level = 0;
                let mut chars = line.chars();
                while let Some('#') = chars.next() {
                    level += 1;
                }

                if level > 0 && line.chars().nth(level) == Some(' ') {
                    let content = &line[level + 1..];
                    events.push(Event::StartHeading(level));
                    self.parse_inline(content, &mut events);
                    events.push(Event::EndHeading(level));
                    continue;
                }
            }

            // Otherwise, it's a paragraph
            events.push(Event::StartParagraph);
            self.parse_inline(line, &mut events);
            events.push(Event::EndParagraph);
        }

        events
    }

    fn parse_inline(&self, text: &str, events: &mut Vec<Event>) {
        let mut i = 0;
        let mut current_text = String::new();

        while i < text.len() {
            let remaining = &text[i..];

            // Bold: **
            if remaining.starts_with("**") {
                if !current_text.is_empty() {
                    events.push(Event::Text(current_text.clone()));
                    current_text.clear();
                }
                events.push(Event::StartBold);
                i += 2;

                // RUST INSIGHT:
                // Lookahead loop explicitly uses char.len_utf8() to prevent slicing
                // panics on multibyte characters.
                let mut end_found = false;
                let mut lookahead = i;
                while lookahead < text.len() {
                    if text[lookahead..].starts_with("**") {
                        let inner_content = &text[i..lookahead];
                        self.parse_inline(inner_content, events);
                        events.push(Event::EndBold);
                        i = lookahead + 2;
                        end_found = true;
                        break;
                    }
                    let c = text[lookahead..].chars().next().unwrap();
                    lookahead += c.len_utf8();
                }

                if !end_found {
                    // Treat as literal if not closed
                    events.pop(); // Remove StartBold
                    current_text.push_str("**");
                }
                continue;
            }

            // Italic: *
            if remaining.starts_with('*') {
                if !current_text.is_empty() {
                    events.push(Event::Text(current_text.clone()));
                    current_text.clear();
                }
                events.push(Event::StartItalic);
                i += 1;

                let mut end_found = false;
                let mut lookahead = i;
                while lookahead < text.len() {
                    if text[lookahead..].starts_with('*') && !text[lookahead..].starts_with("**") {
                        let inner_content = &text[i..lookahead];
                        self.parse_inline(inner_content, events);
                        events.push(Event::EndItalic);
                        i = lookahead + 1;
                        end_found = true;
                        break;
                    }
                    let c = text[lookahead..].chars().next().unwrap();
                    lookahead += c.len_utf8();
                }

                if !end_found {
                    events.pop(); // Remove StartItalic
                    current_text.push('*');
                }
                continue;
            }

            // Normal text
            let c = remaining.chars().next().unwrap();
            current_text.push(c);
            i += c.len_utf8();
        }

        if !current_text.is_empty() {
            events.push(Event::Text(current_text));
        }
    }
}

pub fn render_html(events: &[Event]) -> String {
    use std::fmt::Write;
    let mut output = String::new();

    for event in events {
        match event {
            Event::StartHeading(level) => write!(output, "<h{}>", level).unwrap(),
            Event::EndHeading(level) => writeln!(output, "</h{}>", level).unwrap(),
            Event::StartParagraph => write!(output, "<p>").unwrap(),
            Event::EndParagraph => writeln!(output, "</p>").unwrap(),
            Event::Text(text) => write!(output, "{}", text).unwrap(),
            Event::StartBold => write!(output, "<strong>").unwrap(),
            Event::EndBold => write!(output, "</strong>").unwrap(),
            Event::StartItalic => write!(output, "<em>").unwrap(),
            Event::EndItalic => write!(output, "</em>").unwrap(),
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paragraph_and_bold() {
        let input = "Hello **world**";
        let parser = MarkdownParser::new(input);
        let events = parser.parse();
        let html = render_html(&events);
        assert_eq!(html, "<p>Hello <strong>world</strong></p>\n");
    }

    #[test]
    fn test_heading_and_italic() {
        let input = "## This is *italic*";
        let parser = MarkdownParser::new(input);
        let events = parser.parse();
        let html = render_html(&events);
        assert_eq!(html, "<h2>This is <em>italic</em></h2>\n");
    }

    #[test]
    fn test_multibyte_characters() {
        // Ensures the parser doesn't panic on emoji or other UTF-8 chars
        let input = "Hello 🌍 **Rust**";
        let parser = MarkdownParser::new(input);
        let events = parser.parse();
        let html = render_html(&events);
        assert_eq!(html, "<p>Hello 🌍 <strong>Rust</strong></p>\n");
    }

    #[test]
    fn test_unmatched_tags() {
        let input = "This is **bold and *italic";
        let parser = MarkdownParser::new(input);
        let events = parser.parse();
        let html = render_html(&events);
        assert_eq!(html, "<p>This is **bold and *italic</p>\n");
    }
}
