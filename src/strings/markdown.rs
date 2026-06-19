//! # Markdown Parser Implementation
//!
//! Implements a minimal, pull-based event stream Markdown to HTML compiler.
//!
//! **Replaces Crates:** `pulldown-cmark`, `comrak`
//!
//! **Real-world Usage:**
//! - Static site generators (Hugo, Zola)
//! - Forums and comment sections (Reddit, GitHub)
//! - Documentation engines (mdBook, Rustdoc)
//!
//! **Why build it yourself?**
//! Parsing Markdown teaches you about state machines and stream processing.
//! Unlike traditional parsers that build a full Abstract Syntax Tree (AST) in memory,
//! an event-based parser emits start/end tokens dynamically, drastically reducing
//! memory overhead. You'll also confront the complexity of lookaheads and inline
//! formatting bounds parsing.

use std::str::CharIndices;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure: Event Stream (Pull Parser)
// Instead of building a full AST tree (Document -> Block -> Inline), we generate a stream
// of events (Start(Header), Text("Hello"), End(Header)).
//
// Block Parser:
// Identifies line-level structures (Headers, Lists, Paragraphs).
//
// Inline Parser:
// Handles text-level formatting (**bold**, *italic*, [links](url)) within a block.
//
// Invariants:
// - Every `Event::Start` must have a corresponding `Event::End`.
// - Lookahead must never panic or enter an infinite loop on invalid UTF-8 or unmatched tags.
//
// Complexity:
// - Time Complexity: O(N) where N is the length of the string (with some small O(K) lookahead bounded overhead)
// - Space Complexity: O(1) beyond the input string size and output buffer

#[derive(Debug, PartialEq)]
pub enum Event<'a> {
    Start(Tag<'a>),
    End(Tag<'a>),
    Text(&'a str),
    Html(&'a str),
}

#[derive(Debug, PartialEq, Clone)]
pub enum Tag<'a> {
    Paragraph,
    Heading(u8),
    BlockQuote,
    CodeBlock(&'a str),
    List(Option<u64>), // None for unordered, Some(start) for ordered
    Item,
    Strong,
    Emphasis,
    Code,
    Link(&'a str, &'a str),  // url, title
    Image(&'a str, &'a str), // url, title
}

pub struct Parser<'a> {
    text: &'a str,
    pos: usize,
    events: Vec<Event<'a>>,
}

impl<'a> Parser<'a> {
    pub fn new(text: &'a str) -> Self {
        Self {
            text,
            pos: 0,
            events: Vec::new(),
        }
    }

    pub fn parse(mut self) -> Vec<Event<'a>> {
        self.parse_blocks();
        self.events
    }

    fn parse_blocks(&mut self) {
        while self.pos < self.text.len() {
            self.skip_whitespace_lines();
            if self.pos >= self.text.len() {
                break;
            }

            let start = self.pos;

            // Check heading
            let mut heading_level = 0;
            let mut h_pos = self.pos;
            while h_pos < self.text.len() && self.text[h_pos..].starts_with('#') {
                heading_level += 1;
                h_pos += 1;
            }
            if heading_level > 0 && heading_level <= 6 && self.text[h_pos..].starts_with(' ') {
                self.pos = h_pos + 1; // skip # and space
                self.events.push(Event::Start(Tag::Heading(heading_level)));
                self.parse_inline_until_newline();
                // Ensure heading doesn't double print text if it was caught somehow
                self.events.push(Event::End(Tag::Heading(heading_level)));
                continue;
            }

            // Check Blockquote
            if self.text[self.pos..].starts_with("> ") {
                self.events.push(Event::Start(Tag::BlockQuote));
                while self.pos < self.text.len() {
                    if self.text[self.pos..].starts_with("> ") {
                        self.pos += 2;
                        self.parse_inline_until_newline();
                        // ensure we append a newline if the next is also a blockquote line to prevent merging words
                        if self.pos < self.text.len() && self.text[self.pos..].starts_with("> ") {
                            self.events.push(Event::Text("\n"));
                        }
                    } else if self.text[self.pos..].starts_with('\n') {
                        // Empty line ends blockquote
                        break;
                    } else {
                        // Continuation line
                        self.parse_inline_until_newline();
                    }
                }
                self.events.push(Event::End(Tag::BlockQuote));
                continue;
            }

            // Paragraph
            self.events.push(Event::Start(Tag::Paragraph));
            while self.pos < self.text.len() {
                self.parse_inline_until_newline();
                if self.pos < self.text.len() && self.text[self.pos..].starts_with('\n') {
                    // Check if next line is empty (end of paragraph)
                    let next_line = self.pos + 1;
                    if next_line < self.text.len() && self.text[next_line..].starts_with('\n') {
                        break; // End of paragraph
                    }
                } else {
                    break;
                }
            }
            self.events.push(Event::End(Tag::Paragraph));
        }
    }

    fn parse_inline_until_newline(&mut self) {
        let mut text_start = self.pos;

        while self.pos < self.text.len() {
            let ch = self.text[self.pos..].chars().next().unwrap();
            let ch_len = ch.len_utf8();

            if ch == '\n' {
                if text_start < self.pos {
                    self.events
                        .push(Event::Text(&self.text[text_start..self.pos]));
                }
                self.pos += ch_len; // Consume newline
                return;
            } else if self.text[self.pos..].starts_with("**") {
                if text_start < self.pos {
                    self.events
                        .push(Event::Text(&self.text[text_start..self.pos]));
                }
                self.pos += 2;
                self.events.push(Event::Start(Tag::Strong));
                self.parse_inline_until("**");
                self.events.push(Event::End(Tag::Strong));
                if self.pos < self.text.len() && self.text[self.pos..].starts_with("**") {
                    self.pos += 2; // Consume closing
                }
                text_start = self.pos;
            } else if self.text[self.pos..].starts_with('*') {
                if text_start < self.pos {
                    self.events
                        .push(Event::Text(&self.text[text_start..self.pos]));
                }
                self.pos += 1;
                self.events.push(Event::Start(Tag::Emphasis));
                self.parse_inline_until("*");
                self.events.push(Event::End(Tag::Emphasis));
                if self.pos < self.text.len() && self.text[self.pos..].starts_with('*') {
                    self.pos += 1; // Consume closing
                }
                text_start = self.pos;
            } else if self.text[self.pos..].starts_with('`') {
                if text_start < self.pos {
                    self.events
                        .push(Event::Text(&self.text[text_start..self.pos]));
                }
                self.pos += 1;
                self.events.push(Event::Start(Tag::Code));
                self.parse_inline_until("`");
                self.events.push(Event::End(Tag::Code));
                if self.pos < self.text.len() && self.text[self.pos..].starts_with('`') {
                    self.pos += 1; // Consume closing
                }
                text_start = self.pos;
            } else if self.text[self.pos..].starts_with('[') {
                if text_start < self.pos {
                    self.events
                        .push(Event::Text(&self.text[text_start..self.pos]));
                }

                // Try parsing link
                let saved_pos = self.pos;
                self.pos += 1;
                let mut title_end = None;

                // GOTCHA: Use char.len_utf8() to advance, avoiding string slicing panics
                let mut search_pos = self.pos;
                while search_pos < self.text.len() {
                    if self.text[search_pos..].starts_with(']') {
                        title_end = Some(search_pos);
                        break;
                    }
                    if self.text[search_pos..].starts_with('\n') {
                        break; // No newlines in links for this simple parser
                    }
                    search_pos += self.text[search_pos..].chars().next().unwrap().len_utf8();
                }

                if let Some(te) = title_end {
                    if te + 1 < self.text.len() && self.text[te + 1..].starts_with('(') {
                        let url_start = te + 2;
                        let mut url_end = None;

                        let mut url_search = url_start;
                        while url_search < self.text.len() {
                            if self.text[url_search..].starts_with(')') {
                                url_end = Some(url_search);
                                break;
                            }
                            if self.text[url_search..].starts_with('\n') {
                                break;
                            }
                            url_search +=
                                self.text[url_search..].chars().next().unwrap().len_utf8();
                        }

                        if let Some(ue) = url_end {
                            let title = &self.text[self.pos..te];
                            let url = &self.text[url_start..ue];
                            self.events.push(Event::Start(Tag::Link(url, title)));
                            self.events.push(Event::Text(title)); // In a real parser we'd parse this recursively, simplify for now
                            self.events.push(Event::End(Tag::Link(url, title)));
                            self.pos = ue + 1;
                            text_start = self.pos;
                            continue;
                        }
                    }
                }

                // Fallback, not a valid link
                self.pos = saved_pos + 1;
            } else {
                self.pos += ch_len;
            }
        }

        if text_start < self.pos
            && text_start < self.text.len()
            && !self.text[text_start..self.pos].starts_with('\n')
        {
            // Trim trailing newline from the last text node if any
            let mut text_slice = &self.text[text_start..self.pos];
            if text_slice.ends_with('\n') {
                text_slice = &text_slice[..text_slice.len() - 1];
            }
            // trim carriage returns too
            if text_slice.ends_with('\r') {
                text_slice = &text_slice[..text_slice.len() - 1];
            }
            if !text_slice.is_empty() {
                self.events.push(Event::Text(text_slice));
            }
        }
    }

    fn parse_inline_until(&mut self, closing_tag: &str) {
        let text_start = self.pos;
        while self.pos < self.text.len() {
            if self.text[self.pos..].starts_with(closing_tag) {
                break;
            }
            if self.text[self.pos..].starts_with('\n') {
                break; // Don't cross newlines for inline tags in this simple parser
            }
            // RUST INSIGHT: Safely advance using char.len_utf8() to avoid multibyte slicing panics
            self.pos += self.text[self.pos..].chars().next().unwrap().len_utf8();
        }
        if text_start < self.pos {
            self.events
                .push(Event::Text(&self.text[text_start..self.pos]));
        }
    }

    fn skip_whitespace_lines(&mut self) {
        while self.pos < self.text.len() {
            let ch = self.text[self.pos..].chars().next().unwrap();
            if ch == '\n' || ch == '\r' {
                self.pos += ch.len_utf8();
            } else {
                break;
            }
        }
    }
}

pub fn push_html<'a>(html: &mut String, events: impl Iterator<Item = Event<'a>>) {
    for event in events {
        match event {
            Event::Start(tag) => match tag {
                Tag::Paragraph => html.push_str("<p>"),
                Tag::Heading(level) => html.push_str(&format!("<h{}>", level)),
                Tag::BlockQuote => html.push_str("<blockquote>\n"),
                Tag::Strong => html.push_str("<strong>"),
                Tag::Emphasis => html.push_str("<em>"),
                Tag::Code => html.push_str("<code>"),
                Tag::Link(url, _) => html.push_str(&format!("<a href=\"{}\">", url)),
                _ => {}
            },
            Event::End(tag) => match tag {
                Tag::Paragraph => html.push_str("</p>\n"),
                Tag::Heading(level) => html.push_str(&format!("</h{}>\n", level)),
                Tag::BlockQuote => html.push_str("</blockquote>\n"),
                Tag::Strong => html.push_str("</strong>"),
                Tag::Emphasis => html.push_str("</em>"),
                Tag::Code => html.push_str("</code>"),
                Tag::Link(_, _) => html.push_str("</a>"),
                _ => {}
            },
            Event::Text(text) => {
                // RUST INSIGHT: Use write! to avoid intermediate string allocations
                use std::fmt::Write;
                write!(html, "{}", text).unwrap();
            }
            Event::Html(html_str) => {
                html.push_str(html_str);
            }
        }
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `pulldown-cmark`: A full CommonMark compliant parser. It handles hundreds of edge cases (nested lists, table extensions)
//   that this simple parser ignores.
// - `comrak`: Supports GFM (GitHub Flavored Markdown) and full AST manipulation.
//
// Missing vs. Production:
// - Nested elements (e.g., strong within emphasis) are not parsed recursively here.
// - Lists (ordered and unordered) and Code Blocks are omitted for brevity.
// - Link references and image tags are not fully supported.
//
// Next Steps:
// 1. Implement full CommonMark compliance.
// 2. Add an AST construction phase for users who need to modify the document before rendering.
// 3. Add table support.
//
// Benchmarking Note:
// To benchmark this parser, use the `criterion` crate. Create a suite of Markdown documents
// of varying sizes (1KB, 10KB, 1MB) with different distributions of inline vs block elements.
// Run `cargo bench` and compare throughput (MB/s) against `pulldown-cmark`. Because this
// parser allocates minimally (mostly for output strings), it should exhibit high throughput.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heading_and_paragraph() {
        let markdown = "# Title\n\nSome text.";
        let parser = Parser::new(markdown);
        let events = parser.parse();

        let mut html = String::new();
        push_html(&mut html, events.into_iter());

        assert_eq!(html, "<h1>Title</h1>\n<p>Some text.</p>\n");
    }

    #[test]
    fn test_inline_formatting() {
        let markdown = "This is **bold** and *italic* and `code`.";
        let parser = Parser::new(markdown);
        let events = parser.parse();

        let mut html = String::new();
        push_html(&mut html, events.into_iter());

        assert_eq!(
            html,
            "<p>This is <strong>bold</strong> and <em>italic</em> and <code>code</code>.</p>\n"
        );
    }

    #[test]
    fn test_links() {
        let markdown = "Click [here](https://rust-lang.org) to learn Rust.";
        let parser = Parser::new(markdown);
        let events = parser.parse();

        let mut html = String::new();
        push_html(&mut html, events.into_iter());

        assert_eq!(
            html,
            "<p>Click <a href=\"https://rust-lang.org\">here</a> to learn Rust.</p>\n"
        );
    }

    #[test]
    fn test_blockquote() {
        let markdown = "> Quote line 1\n> Quote line 2";
        let parser = Parser::new(markdown);
        let events = parser.parse();

        let mut html = String::new();
        push_html(&mut html, events.into_iter());

        assert_eq!(
            html,
            "<blockquote>\nQuote line 1\nQuote line 2</blockquote>\n"
        );
    }

    #[test]
    fn test_multibyte_characters() {
        let markdown = "Hello 世界 **Rust**";
        let parser = Parser::new(markdown);
        let events = parser.parse();

        let mut html = String::new();
        push_html(&mut html, events.into_iter());

        assert_eq!(html, "<p>Hello 世界 <strong>Rust</strong></p>\n");
    }
}
