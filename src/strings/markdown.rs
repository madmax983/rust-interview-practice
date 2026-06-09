//! # Markdown Parser
//!
//! Implements a minimal Markdown to HTML compiler using a pull-based event stream.
//!
//! **Replaces Crates:** `pulldown-cmark`, `comrak`
//!
//! **Real-world Usage:**
//! - Rendering user comments and READMEs in web applications.
//! - Static site generators converting Markdown to HTML.
//! - Documentation generators like rustdoc.
//!
//! **Why build it yourself?**
//! Building a Markdown parser teaches you about state machines, pull-based lexing,
//! and handling text encodings. You learn how to avoid full AST memory overhead
//! by streaming events directly to an HTML renderer.
//!
//! ## Architecture
//!
//! The parser uses a pull-based event stream approach:
//! 1. `Parser` acts as an iterator, yielding `Event`s (Start, End, Text).
//! 2. Block-level parsing handles paragraphs, headers, and lists.
//! 3. Inline-level parsing handles bold, italic, and code formatting.
//!
//! This avoids building a complete abstract syntax tree (AST) in memory,
//! emitting HTML incrementally as the document is parsed.
//!
//! | Operation       | Time Complexity | Space Complexity |
//! |-----------------|-----------------|------------------|
//! | Parse Document  | O(N)            | O(M)             |
//!
//! *N = length of document, M = size of current block (bound by max block size)*
//!
//! **Design Decisions:**
//! - **Event-driven:** By returning events, the parser doesn't mandate a specific
//!   output format (HTML, ANSI, etc.).
//! - **Zero-copy:** The parser primarily yields slices (`&str`) to the original
//!   markdown text, avoiding allocations for text tokens.
//!
//! ---
//!
//! **Comparison to Production Crates:**
//! - A true `pulldown-cmark` implementation handles the full CommonMark specification
//!   which is notoriously complex (handling deeply nested links, indented code blocks, etc.).
//! - This implementation focuses on the core structure and avoids creating an AST.
//!
//! ---
//!
//! **Benchmarks:**
//! To benchmark, use `criterion` with a suite of varied Markdown files (small, large,
//! deeply nested). Measure the throughput (bytes/sec) of the `Parser` iterator. The main
//! performance driver here is avoiding allocations in the `Text` event by yielding string slices.
//!
//! **Missing vs Production:**
//! - **CommonMark Compliance:** Many edge cases in nested lists and blockquotes are not handled.
//! - **HTML Escaping:** We don't automatically escape raw HTML found in the markdown text.
//! - **Links and Images:** Basic inline tags are supported, but complex link resolution is missing.
//!
//! **Next Steps:**
//! - Add support for link syntax (`[text](url)`).
//! - Implement an AST-building wrapper around the parser for tooling that requires structure.
//! - Enhance HTML escaping for secure web rendering.

use std::collections::VecDeque;

/// A Markdown event representing a parsed token.
#[derive(Debug, PartialEq, Clone)]
pub enum Event<'a> {
    Start(Tag),
    End(Tag),
    Text(&'a str),
    SoftBreak,
}

/// Supported Markdown tags.
#[derive(Debug, PartialEq, Clone)]
pub enum Tag {
    Paragraph,
    Heading(usize),
    Strong,
    Emphasis,
    Code,
    BlockQuote,
    List(bool), // true for ordered, false for unordered
    ListItem,
}

/// A pull-based Markdown parser that streams events.
pub struct Parser<'a> {
    text: &'a str,
    pos: usize,
    events: VecDeque<Event<'a>>,
}

impl<'a> Parser<'a> {
    pub fn new(text: &'a str) -> Self {
        Parser {
            text,
            pos: 0,
            // RUST INSIGHT: Pre-allocating VecDeque capacity based on a reasonable
            // heuristic minimizes intermediate growth overhead.
            events: VecDeque::with_capacity(32),
        }
    }

    fn parse_next_block(&mut self) {
        // Skip leading newlines and spaces
        while self.pos < self.text.len() {
            if self.text[self.pos..].starts_with('\n') {
                self.pos += 1;
            } else if self.text[self.pos..].starts_with('\r') {
                self.pos += 1;
            } else {
                break;
            }
        }

        if self.pos >= self.text.len() {
            return;
        }

        let rest = &self.text[self.pos..];

        let end_offset = rest.find("\n\n").unwrap_or(rest.len());
        let block = &rest[..end_offset];

        let mut lines = block.lines();
        let first_line = lines.next().unwrap_or("");

        if first_line.starts_with('#') {
            let mut level = 0;
            for c in first_line.chars() {
                if c == '#' {
                    level += 1;
                } else {
                    break;
                }
            }
            if level > 0 && level <= 6 && first_line[level..].starts_with(' ') {
                self.events.push_back(Event::Start(Tag::Heading(level)));
                self.parse_inline(&first_line[level + 1..]);
                self.events.push_back(Event::End(Tag::Heading(level)));

                // Address Data Loss: if there are more lines, parse them as a paragraph
                let mut has_rest = false;
                for line in lines {
                    if !has_rest {
                        self.events.push_back(Event::Start(Tag::Paragraph));
                        has_rest = true;
                    } else {
                        self.events.push_back(Event::SoftBreak);
                    }
                    self.parse_inline(line);
                }
                if has_rest {
                    self.events.push_back(Event::End(Tag::Paragraph));
                }
            } else {
                self.events.push_back(Event::Start(Tag::Paragraph));
                self.parse_inline(block);
                self.events.push_back(Event::End(Tag::Paragraph));
            }
        } else if first_line.starts_with("> ") {
            self.events.push_back(Event::Start(Tag::BlockQuote));
            self.parse_inline(&first_line[2..]);

            for line in lines {
                self.events.push_back(Event::SoftBreak);
                if line.starts_with("> ") {
                    self.parse_inline(&line[2..]);
                } else {
                    self.parse_inline(line);
                }
            }
            self.events.push_back(Event::End(Tag::BlockQuote));
        } else if first_line.starts_with("- ") || first_line.starts_with("* ") {
            self.events.push_back(Event::Start(Tag::List(false)));
            let mut in_list_item = false;
            for line in block.lines() {
                if line.starts_with("- ") || line.starts_with("* ") {
                    if in_list_item {
                        self.events.push_back(Event::End(Tag::ListItem));
                    }
                    self.events.push_back(Event::Start(Tag::ListItem));
                    self.parse_inline(&line[2..]);
                    in_list_item = true;
                } else {
                    self.events.push_back(Event::SoftBreak);
                    self.parse_inline(line);
                }
            }
            if in_list_item {
                self.events.push_back(Event::End(Tag::ListItem));
            }
            self.events.push_back(Event::End(Tag::List(false)));
        } else {
            self.events.push_back(Event::Start(Tag::Paragraph));
            self.parse_inline(block);
            self.events.push_back(Event::End(Tag::Paragraph));
        }

        self.pos += end_offset;
    }

    fn parse_inline(&mut self, text: &'a str) {
        let mut i = 0;
        let mut start = 0;

        while i < text.len() {
            let rest = &text[i..];

            if rest.starts_with("**") {
                let mut found = false;
                // GOTCHA: Using `.char_indices()` on a slice means yielded indices are relative
                // to the slice. We map them by adding `i + 2` to prevent out-of-bounds panics
                // on multibyte characters.
                for (offset, _) in text[i + 2..].char_indices() {
                    let j = i + 2 + offset;
                    if text[j..].starts_with("**") {
                        if start < i {
                            self.events.push_back(Event::Text(&text[start..i]));
                        }
                        self.events.push_back(Event::Start(Tag::Strong));
                        self.parse_inline(&text[i + 2..j]);
                        self.events.push_back(Event::End(Tag::Strong));
                        i = j + 2;
                        start = i;
                        found = true;
                        break;
                    }
                }
                if found { continue; }
            }

            if rest.starts_with('*') && !rest.starts_with("**") {
                let mut found = false;
                let mut skip_next = false;
                for (offset, _) in text[i + 1..].char_indices() {
                    if skip_next {
                        skip_next = false;
                        continue;
                    }
                    let j = i + 1 + offset;
                    if text[j..].starts_with("**") {
                        skip_next = true; // Avoid matching the second '*' of '**'
                        continue;
                    }
                    if text[j..].starts_with('*') {
                        if start < i {
                            self.events.push_back(Event::Text(&text[start..i]));
                        }
                        self.events.push_back(Event::Start(Tag::Emphasis));
                        self.parse_inline(&text[i + 1..j]);
                        self.events.push_back(Event::End(Tag::Emphasis));
                        i = j + 1;
                        start = i;
                        found = true;
                        break;
                    }
                }
                if found { continue; }
            }

            if rest.starts_with('`') {
                let mut found = false;
                for (offset, _) in text[i + 1..].char_indices() {
                    let j = i + 1 + offset;
                    if text[j..].starts_with('`') {
                        if start < i {
                            self.events.push_back(Event::Text(&text[start..i]));
                        }
                        self.events.push_back(Event::Start(Tag::Code));
                        self.events.push_back(Event::Text(&text[i + 1..j]));
                        self.events.push_back(Event::End(Tag::Code));
                        i = j + 1;
                        start = i;
                        found = true;
                        break;
                    }
                }
                if found { continue; }
            }

            // Safely advance by one character boundary
            let c = rest.chars().next().unwrap();
            i += c.len_utf8();
        }

        if start < text.len() {
            self.events.push_back(Event::Text(&text[start..]));
        }
    }
}

impl<'a> Iterator for Parser<'a> {
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(event) = self.events.pop_front() {
            return Some(event);
        }

        if self.pos >= self.text.len() {
            return None;
        }

        self.parse_next_block();
        self.events.pop_front()
    }
}

/// Renders Markdown text to an HTML string.
pub fn push_html(html: &mut String, markdown: &str) {
    let parser = Parser::new(markdown);
    for event in parser {
        match event {
            Event::Start(tag) => match tag {
                Tag::Paragraph => html.push_str("<p>"),
                Tag::Heading(l) => html.push_str(&format!("<h{}>", l)),
                Tag::Strong => html.push_str("<strong>"),
                Tag::Emphasis => html.push_str("<em>"),
                Tag::Code => html.push_str("<code>"),
                Tag::BlockQuote => html.push_str("<blockquote>\n"),
                Tag::List(true) => html.push_str("<ol>\n"),
                Tag::List(false) => html.push_str("<ul>\n"),
                Tag::ListItem => html.push_str("<li>"),
            },
            Event::End(tag) => match tag {
                Tag::Paragraph => html.push_str("</p>\n"),
                Tag::Heading(l) => html.push_str(&format!("</h{}>\n", l)),
                Tag::Strong => html.push_str("</strong>"),
                Tag::Emphasis => html.push_str("</em>"),
                Tag::Code => html.push_str("</code>"),
                Tag::BlockQuote => html.push_str("</blockquote>\n"),
                Tag::List(true) => html.push_str("</ol>\n"),
                Tag::List(false) => html.push_str("</ul>\n"),
                Tag::ListItem => html.push_str("</li>\n"),
            },
            Event::Text(t) => html.push_str(t),
            Event::SoftBreak => html.push('\n'),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paragraph() {
        let mut html = String::new();
        push_html(&mut html, "Hello world");
        assert_eq!(html, "<p>Hello world</p>\n");
    }

    #[test]
    fn test_heading() {
        let mut html = String::new();
        push_html(&mut html, "## Hello");
        assert_eq!(html, "<h2>Hello</h2>\n");
    }

    #[test]
    fn test_inline_formatting() {
        let mut html = String::new();
        push_html(&mut html, "This is **bold** and *italic* and `code`!");
        assert_eq!(html, "<p>This is <strong>bold</strong> and <em>italic</em> and <code>code</code>!</p>\n");
    }

    #[test]
    fn test_lists() {
        let mut html = String::new();
        push_html(&mut html, "- Item 1\n- Item 2");
        assert_eq!(html, "<ul>\n<li>Item 1</li>\n<li>Item 2</li>\n</ul>\n");
    }

    #[test]
    fn test_blockquote() {
        let mut html = String::new();
        push_html(&mut html, "> Quote line 1\n> Quote line 2");
        assert_eq!(html, "<blockquote>\nQuote line 1\nQuote line 2</blockquote>\n");
    }

    #[test]
    fn test_multibyte_chars() {
        let mut html = String::new();
        push_html(&mut html, "Hello 🌍 **world**");
        assert_eq!(html, "<p>Hello 🌍 <strong>world</strong></p>\n");
    }
}
