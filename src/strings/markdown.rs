//! # Markdown Parser
//!
//! Implements a minimal Markdown to HTML compiler.
//!
//! **Replaces Crates:** `pulldown-cmark`, `comrak`
//!
//! **Real-world Usage:**
//! - GitHub flavored markdown rendering
//! - Static site generators (Hugo, Zola)
//! - Chat applications text formatting
//!
//! **Why build it yourself?**
//! Building a markdown parser teaches you how to construct a pull-based event stream and manage state transitions
//! without the immense memory overhead of a full Abstract Syntax Tree (AST). You'll learn to differentiate between
//! block-level elements (paragraphs, headers) and inline elements (bold, italic, links).
//!
//! # Architecture
//!
//! Data Structure:
//!
//! ```text
//! String -> Lexer (Block/Inline) -> Event Stream (Start, End, Text) -> HTML Renderer -> String
//! ```
//!
//! Time/Space Complexity:
//! - Parsing: Time O(N), Space O(1) mostly, leveraging pull-based events over a full AST.
//! - Rendering: Time O(N), Space O(N) for the output HTML string.
//!
//! Invariants:
//! - Events are correctly nested (every `Start` has a corresponding `End`).
//! - Block elements do not cross block boundaries inappropriately.
//!
//! Benchmarking:
//! To benchmark, you could use `criterion` to compare the throughput of parsing huge documents
//! against `pulldown-cmark`, ensuring `Parser::new` scales linearly O(N).
//!
//! Design Decisions:
//! - **Event Stream:** Instead of generating a heavy AST, we emit a stream of `Event`s. This is highly efficient and matches the design of `pulldown-cmark`.
//! - **State Machine:** Uses a simple state machine to track whether we're inside a paragraph, header, or list.

// =========================================================================================
// Events and Tags
// =========================================================================================

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
    Heading(u32),
    BlockQuote,
    List(Option<u64>), // None for unordered, Some(start) for ordered
    Item,
    Emphasis,
    Strong,
    CodeBlock(&'a str),
    Link(&'a str, &'a str), // url, title
}

// =========================================================================================
// Parser
// =========================================================================================

pub struct Parser<'a> {
    text: &'a str,
    pos: usize,
    events: Vec<Event<'a>>,
}

impl<'a> Parser<'a> {
    pub fn new(text: &'a str) -> Self {
        let mut parser = Self {
            text,
            pos: 0,
            events: Vec::new(),
        };
        parser.parse();
        parser
    }

    fn parse(&mut self) {
        let lines: Vec<&'a str> = self.text.lines().collect();
        let mut in_paragraph = false;

        for line in lines {
            if line.trim().is_empty() {
                if in_paragraph {
                    self.events.push(Event::End(Tag::Paragraph));
                    in_paragraph = false;
                }
                continue;
            }

            if line.starts_with("#") {
                if in_paragraph {
                    self.events.push(Event::End(Tag::Paragraph));
                    in_paragraph = false;
                }

                let mut level = 0;
                let mut chars = line.chars();
                while let Some('#') = chars.next() {
                    level += 1;
                }

                let text = line[level..].trim();
                self.events.push(Event::Start(Tag::Heading(level as u32)));
                self.parse_inline(text);
                self.events.push(Event::End(Tag::Heading(level as u32)));
                continue;
            }

            // Simple blockquote
            if line.starts_with("> ") {
                if in_paragraph {
                    self.events.push(Event::End(Tag::Paragraph));
                    in_paragraph = false;
                }
                self.events.push(Event::Start(Tag::BlockQuote));
                self.events.push(Event::Start(Tag::Paragraph));
                self.parse_inline(line[2..].trim());
                self.events.push(Event::End(Tag::Paragraph));
                self.events.push(Event::End(Tag::BlockQuote));
                continue;
            }

            // Otherwise, it's a paragraph
            if !in_paragraph {
                self.events.push(Event::Start(Tag::Paragraph));
                in_paragraph = true;
            } else {
                self.events.push(Event::Text("\n"));
            }
            self.parse_inline(line.trim());
        }

        if in_paragraph {
            self.events.push(Event::End(Tag::Paragraph));
        }
    }

    fn parse_inline(&mut self, text: &'a str) {
        let mut chars = text.char_indices().peekable();
        let mut last_idx = 0;

        while let Some((idx, c)) = chars.next() {
            match c {
                '*' => {
                    if let Some(&(_, '*')) = chars.peek() {
                        // Strong
                        if idx > last_idx {
                            self.events.push(Event::Text(&text[last_idx..idx]));
                        }
                        self.events.push(Event::Start(Tag::Strong));
                        chars.next(); // consume second '*'
                        last_idx = idx + 2;

                        // find closing '**'
                        let mut end_idx = last_idx;
                        let mut found = false;
                        while let Some((i, c2)) = chars.next() {
                            if c2 == '*' {
                                if let Some(&(_, '*')) = chars.peek() {
                                    found = true;
                                    end_idx = i;
                                    chars.next(); // consume second '*'
                                    break;
                                }
                            }
                        }

                        if found {
                            self.parse_inline(&text[last_idx..end_idx]);
                            self.events.push(Event::End(Tag::Strong));
                            last_idx = end_idx + 2;
                        } else {
                            // If no closing '**', just treat it as text
                            self.events.push(Event::Text("**"));
                            last_idx = idx + 2;
                        }
                    } else {
                        // Emphasis
                        if idx > last_idx {
                            self.events.push(Event::Text(&text[last_idx..idx]));
                        }
                        self.events.push(Event::Start(Tag::Emphasis));
                        last_idx = idx + 1;

                        // find closing '*'
                        let mut end_idx = last_idx;
                        let mut found = false;
                        while let Some((i, c2)) = chars.next() {
                            if c2 == '*' {
                                found = true;
                                end_idx = i;
                                break;
                            }
                        }

                        if found {
                            self.parse_inline(&text[last_idx..end_idx]);
                            self.events.push(Event::End(Tag::Emphasis));
                            last_idx = end_idx + 1;
                        } else {
                            self.events.push(Event::Text("*"));
                            last_idx = idx + 1;
                        }
                    }
                }
                '[' => {
                    // Simple Link
                    if idx > last_idx {
                        self.events.push(Event::Text(&text[last_idx..idx]));
                    }

                    let start_text = idx + 1;
                    let mut end_text = start_text;
                    let mut found_bracket = false;

                    while let Some((i, c2)) = chars.next() {
                        if c2 == ']' {
                            found_bracket = true;
                            end_text = i;
                            break;
                        }
                    }

                    if found_bracket {
                        if let Some(&(paren_idx, '(')) = chars.peek() {
                            chars.next(); // consume '('
                            let start_url = paren_idx + 1;
                            let mut end_url = start_url;
                            let mut found_paren = false;

                            while let Some((i, c2)) = chars.next() {
                                if c2 == ')' {
                                    found_paren = true;
                                    end_url = i;
                                    break;
                                }
                            }

                            if found_paren {
                                let link_text = &text[start_text..end_text];
                                let link_url = &text[start_url..end_url];

                                self.events.push(Event::Start(Tag::Link(link_url, "")));
                                self.parse_inline(link_text);
                                self.events.push(Event::End(Tag::Link(link_url, "")));

                                last_idx = end_url + 1;
                                continue;
                            }
                        }
                    }
                    // If parsing link fails, we don't need to do anything special.
                    // We just continue parsing from the next character. The text from `last_idx`
                    // to `idx` was already pushed, so we need to set `last_idx` to `idx`
                    // so the '[' becomes part of the text.
                    // To avoid lifetime issues and trait objects, let's just create a new `char_indices` from `text` and skip up to `idx` bytes.
                    let mut new_chars = text.char_indices().peekable();
                    while let Some(&(i, _)) = new_chars.peek() {
                        if i <= idx {
                            new_chars.next();
                        } else {
                            break;
                        }
                    }
                    chars = new_chars;
                    last_idx = idx;
                }
                _ => {}
            }
        }

        if last_idx < text.len() {
            self.events.push(Event::Text(&text[last_idx..]));
        }
    }
}

impl<'a> Iterator for Parser<'a> {
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos < self.events.len() {
            let event = self.events[self.pos].take();
            self.pos += 1;
            Some(event)
        } else {
            None
        }
    }
}

// Workaround to make 'take' work without Clone on Event
impl<'a> Event<'a> {
    fn take(&mut self) -> Self {
        std::mem::replace(self, Event::Text(""))
    }
}

// =========================================================================================
// HTML Renderer
// =========================================================================================

// RUST INSIGHT:
// By decoupling the Parser (which generates an `Iterator` of `Event`s) from the Renderer,
// we allow consumers to intercept, modify, or filter events before they become HTML,
// exactly like middleware.

pub fn push_html<'a, I>(s: &mut String, iter: I)
where
    I: Iterator<Item = Event<'a>>,
{
    for event in iter {
        match event {
            Event::Start(tag) => {
                match tag {
                    Tag::Paragraph => s.push_str("<p>"),
                    Tag::Heading(level) => s.push_str(&format!("<h{}>", level)),
                    Tag::BlockQuote => s.push_str("<blockquote>"),
                    Tag::Strong => s.push_str("<strong>"),
                    Tag::Emphasis => s.push_str("<em>"),
                    Tag::Link(url, title) => {
                        if title.is_empty() {
                            s.push_str(&format!("<a href=\"{}\">", escape_html(url)));
                        } else {
                            s.push_str(&format!("<a href=\"{}\" title=\"{}\">", escape_html(url), escape_html(title)));
                        }
                    }
                    _ => {}
                }
            }
            Event::End(tag) => {
                match tag {
                    Tag::Paragraph => s.push_str("</p>\n"),
                    Tag::Heading(level) => s.push_str(&format!("</h{}>\n", level)),
                    Tag::BlockQuote => s.push_str("</blockquote>\n"),
                    Tag::Strong => s.push_str("</strong>"),
                    Tag::Emphasis => s.push_str("</em>"),
                    Tag::Link(_, _) => s.push_str("</a>"),
                    _ => {}
                }
            }
            Event::Text(text) => s.push_str(&escape_html(text)),
            Event::Html(html) => s.push_str(html),
        }
    }
}

// GOTCHA:
// Always pre-allocate capacity when building strings dynamically to prevent repeated reallocation overhead.

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
// Alternative Approaches Footer
// =========================================================================================
//
// How this compares to `pulldown-cmark`:
// - `pulldown-cmark` is a fully compliant CommonMark parser with extensive edge case handling.
// - Our implementation handles only a tiny subset of markdown (paragraphs, headers, basic inline styles).
// - We use an intermediate `Vec` of events for simplicity, whereas `pulldown-cmark` streams events lazily.
//
// What's missing vs. production:
// - **Lazy Iterator:** `Parser` should yield events as it parses rather than buffering them all.
// - **Full CommonMark Support:** Missing lists, code blocks, tables, proper emphasis nesting, HTML blocks.
// - **Performance:** String manipulation and indexing can be optimized further.

// =========================================================================================
// Tests
// =========================================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paragraph() {
        let text = "Hello, world!";
        let parser = Parser::new(text);
        let mut html = String::new();
        push_html(&mut html, parser);
        assert_eq!(html, "<p>Hello, world!</p>\n");
    }

    #[test]
    fn test_heading() {
        let text = "## Header 2";
        let parser = Parser::new(text);
        let mut html = String::new();
        push_html(&mut html, parser);
        assert_eq!(html, "<h2>Header 2</h2>\n");
    }

    #[test]
    fn test_strong_and_emphasis() {
        let text = "This is **bold** and *italic*.";
        let parser = Parser::new(text);
        let mut html = String::new();
        push_html(&mut html, parser);
        assert_eq!(html, "<p>This is <strong>bold</strong> and <em>italic</em>.</p>\n");
    }

    #[test]
    fn test_links() {
        let text = "Check out [Rust](https://rust-lang.org).";
        let parser = Parser::new(text);
        let mut html = String::new();
        push_html(&mut html, parser);
        assert_eq!(html, "<p>Check out <a href=\"https://rust-lang.org\">Rust</a>.</p>\n");
    }

    #[test]
    fn test_blockquote() {
        let text = "> To be or not to be";
        let parser = Parser::new(text);
        let mut html = String::new();
        push_html(&mut html, parser);
        assert_eq!(html, "<blockquote><p>To be or not to be</p>\n</blockquote>\n");
    }

    #[test]
    fn test_failed_link_with_multibyte_chars() {
        // This test ensures we don't panic on a failed link slice following a multibyte char
        let text = "a [ ❤️ *b*";
        let parser = Parser::new(text);
        let mut html = String::new();
        push_html(&mut html, parser);
        assert_eq!(html, "<p>a [ ❤️ <em>b</em></p>\n");
    }
}
