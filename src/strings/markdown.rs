//! # Markdown Parser
//!
//! Implements a minimal Markdown to HTML compiler using a pull-based event stream approach.
//!
//! **Replaces Crates:** `pulldown-cmark`, `comrak`
//!
//! **Real-world Usage:**
//! - Static site generators (Hugo, Zola)
//! - Comment rendering engines (GitHub, Reddit)
//! - Chat applications (Slack, Discord)
//!
//! **Why build it yourself?**
//! Building a Markdown parser teaches you about stream processing and state machines.
//! You'll learn why pull-based parsing (iterating over an event stream) is more memory efficient than building a full Abstract Syntax Tree (AST) for simple transformations.
//! It also forces you to handle edge cases like inline formatting nested within blocks, and how to track state across multiple lines safely.
//!
//! **Benchmarking Note:**
//! To benchmark the Markdown parser, use the `criterion` crate to measure the time it takes to parse and render a large markdown document.
//! ```rust,ignore
//! b.iter(|| compile_markdown(black_box(large_markdown_string)));
//! ```
//!
//! ## Architecture
//!
//! The parser uses a simplified pull-based approach (inspired by `pulldown-cmark`), consisting of two main phases:
//! 1.  **Block Parsing:** The input string is processed line by line to identify block-level elements (Paragraphs, Headings, Unordered Lists).
//! 2.  **Inline Parsing & Rendering:** Block contents are evaluated for inline formatting (Bold, Italic, Code) while simultaneously being rendered to the final HTML string.
//!
//! ### Data Structures
//! - `Event`: Enum representing parsing events (Start Block, Text, End Block).
//! - `Parser`: An iterator over the input text yielding `Event`s.
//! - `HtmlRenderer`: Consumes the `Event` stream to build the HTML string.
//!
//! ### Invariants
//! - **No Empty Elements:** Block elements must explicitly trim trailing whitespace and newlines before wrapping text in tags to prevent generating empty tags like `<p></p>`.
//! - **Stream Processing:** The parser should yield events incrementally rather than allocating a full AST in memory.
//!
//! ### Complexity
//! - **Parsing & Rendering:** O(N) time and O(N) space, where N is the length of the input string, as we process each character and build the output string.
//!
//! ## Implementation
//!
//! **RUST INSIGHT:**
//! Using an `Iterator` for the parser allows the consumer (`HtmlRenderer`) to drive the parsing process lazily. This is highly idiomatic in Rust, avoiding intermediate allocations for AST nodes.
//!
//! **GOTCHA:**
//! When dealing with paragraphs, Markdown often uses blank lines to separate them. If we just wrap every line in `<p>`, we'll get lots of empty `<p></p>` tags. We must buffer paragraph text and emit it only when the block ends or a new block starts.

#[derive(Debug, PartialEq)]
pub enum Tag {
    Paragraph,
    Heading(u8),
    UnorderedList,
    ListItem,
}

#[derive(Debug, PartialEq)]
pub enum Event<'a> {
    Start(Tag),
    End(Tag),
    Text(&'a str),
}

/// A minimal Markdown parser that yields a stream of `Event`s.
pub struct Parser<'a> {
    lines: std::str::Lines<'a>,
    current_block: Option<Tag>,
    in_list: bool,
}

impl<'a> Parser<'a> {
    pub fn new(text: &'a str) -> Self {
        Self {
            lines: text.lines(),
            current_block: None,
            in_list: false,
        }
    }
}



/// A helper function to parse all events from a text into a Vec.
/// A full implementation would make `Parser` a proper stateful `Iterator`.
pub fn parse_markdown(text: &str) -> Vec<Event> {
    let mut events = Vec::new();
    let mut in_paragraph = false;
    let mut in_list = false;


    let mut lines = text.lines().peekable();

    while let Some(line) = lines.next() {
        let trimmed = line.trim();

        if trimmed.is_empty() {
            if in_paragraph {
                events.push(Event::End(Tag::Paragraph));
                in_paragraph = false;
            }
            if in_list {
                events.push(Event::End(Tag::UnorderedList));
                in_list = false;
            }
            continue;
        }

        // Headings
        if trimmed.starts_with('#') {
            if in_paragraph {
                events.push(Event::End(Tag::Paragraph));
                in_paragraph = false;
            }
            let mut level = 0;
            let mut chars = trimmed.chars();
            while let Some(c) = chars.next() {
                if c == '#' {
                    level += 1;
                } else {
                    break;
                }
            }
            if level > 0 && level <= 6 {
                let text_start = trimmed[level as usize..].trim();
                events.push(Event::Start(Tag::Heading(level)));
                events.push(Event::Text(text_start));
                events.push(Event::End(Tag::Heading(level)));
                continue;
            }
        }

        // Unordered Lists
        if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
            if in_paragraph {
                events.push(Event::End(Tag::Paragraph));
                in_paragraph = false;
            }
            if !in_list {
                events.push(Event::Start(Tag::UnorderedList));
                in_list = true;
            }
            events.push(Event::Start(Tag::ListItem));
            events.push(Event::Text(&trimmed[2..]));
            events.push(Event::End(Tag::ListItem));
            continue;
        }

        // Paragraph continuation or start
        if !in_paragraph {
            if in_list {
                 events.push(Event::End(Tag::UnorderedList));
                 in_list = false;
            }
            events.push(Event::Start(Tag::Paragraph));
            in_paragraph = true;
        }

        events.push(Event::Text(trimmed));

        // If the next line is empty or a new block, we'll end the paragraph in the next iteration.
    }

    if in_paragraph {
        events.push(Event::End(Tag::Paragraph));
    }
    if in_list {
        events.push(Event::End(Tag::UnorderedList));
    }

    events
}


/// Renders inline formatting (bold, italic, code) into the output string.
struct InlineState {
    in_bold: bool,
    in_italic: bool,
    in_code: bool,
}

impl InlineState {
    fn new() -> Self {
        Self {
            in_bold: false,
            in_italic: false,
            in_code: false,
        }
    }

    fn render(&mut self, text: &str, out: &mut String) {
        let mut chars = text.chars().peekable();

        while let Some(c) = chars.next() {
            match c {
                '`' => {
                    if self.in_code {
                        out.push_str("</code>");
                        self.in_code = false;
                    } else {
                        out.push_str("<code>");
                        self.in_code = true;
                    }
                }
                '*' => {
                    if let Some(&'*') = chars.peek() {
                        chars.next(); // consume second '*'
                        if self.in_bold {
                            out.push_str("</b>");
                            self.in_bold = false;
                        } else {
                            out.push_str("<b>");
                            self.in_bold = true;
                        }
                    } else {
                        if self.in_italic {
                            out.push_str("</i>");
                            self.in_italic = false;
                        } else {
                            out.push_str("<i>");
                            self.in_italic = true;
                        }
                    }
                }
                _ => {
                    // Basic HTML escaping
                    match c {
                        '<' => out.push_str("&lt;"),
                        '>' => out.push_str("&gt;"),
                        '&' => out.push_str("&amp;"),
                        _ => out.push(c),
                    }
                }
            }
        }
    }

    fn close_tags(&mut self, out: &mut String) {
        if self.in_code { out.push_str("</code>"); self.in_code = false; }
        if self.in_bold { out.push_str("</b>"); self.in_bold = false; }
        if self.in_italic { out.push_str("</i>"); self.in_italic = false; }
    }
}

/// Renders a stream of Markdown `Event`s into an HTML string.
pub fn render_html(events: Vec<Event>) -> String {
    let mut html = String::with_capacity(1024);
    let mut needs_space = false;
    let mut inline_state = InlineState::new();

    for event in events {
        match event {
            Event::Start(tag) => {
                needs_space = false;
                match tag {
                    Tag::Paragraph => html.push_str("<p>"),
                    Tag::Heading(level) => html.push_str(&format!("<h{}>", level)),
                    Tag::UnorderedList => html.push_str("<ul>\n"),
                    Tag::ListItem => html.push_str("<li>"),
                }
            }
            Event::End(tag) => {
                inline_state.close_tags(&mut html);
                match tag {
                    Tag::Paragraph => html.push_str("</p>\n"),
                    Tag::Heading(level) => html.push_str(&format!("</h{}>\n", level)),
                    Tag::UnorderedList => html.push_str("</ul>\n"),
                    Tag::ListItem => html.push_str("</li>\n"),
                }
            }
            Event::Text(text) => {
                let trimmed = text.trim_end();
                if trimmed.is_empty() {
                    continue;
                }
                if needs_space {
                    html.push(' ');
                }
                inline_state.render(trimmed, &mut html);
                needs_space = true; // paragraphs on multiple lines need a space
            }
        }
    }

    html
}

/// Convenience function to compile Markdown text to HTML.
pub fn compile_markdown(text: &str) -> String {
    let events = parse_markdown(text);
    render_html(events)
}


/// ## Footer
///
/// **Crate Comparison:**
/// Canonical crates like `pulldown-cmark` implement the full CommonMark specification. They use highly optimized state machines, support complex features like nested blockquotes and link reference definitions, and ensure absolute safety against malicious input (ReDoS).
///
/// **Missing Features:**
/// - **Full CommonMark Spec:** Missing blockquotes, ordered lists, links, images, tables, and HTML blocks.
/// - **Robust Iterator:** The current `parse_markdown` function allocates a `Vec<Event>` instead of implementing a true lazy `Iterator<Item = Event>` state machine to avoid complexity.
/// - **Nested Blocks:** Does not handle lists inside lists or lists inside blockquotes properly.
///
/// **Next Steps:**
/// Refactor the parser into a true state-machine `Iterator` that yields events lazily without allocating a `Vec`. Implement link parsing `[text](url)` and blockquotes `> text`.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paragraphs() {
        let md = "Hello\nWorld\n\nNew paragraph.";
        let html = compile_markdown(md);
        assert_eq!(html, "<p>Hello World</p>\n<p>New paragraph.</p>\n");
    }

    #[test]
    fn test_headings() {
        let md = "# Heading 1\n## Heading 2\n### Heading 3";
        let html = compile_markdown(md);
        assert_eq!(html, "<h1>Heading 1</h1>\n<h2>Heading 2</h2>\n<h3>Heading 3</h3>\n");
    }

    #[test]
    fn test_unordered_list() {
        let md = "- Item 1\n- Item 2\n* Item 3";
        let html = compile_markdown(md);
        assert_eq!(html, "<ul>\n<li>Item 1</li>\n<li>Item 2</li>\n<li>Item 3</li>\n</ul>\n");
    }

    #[test]
    fn test_inline_formatting() {
        let md = "This is **bold** and *italic* and `code`.";
        let html = compile_markdown(md);
        assert_eq!(html, "<p>This is <b>bold</b> and <i>italic</i> and <code>code</code>.</p>\n");
    }

    #[test]
    fn test_empty_elements_avoidance() {
        // Ensuring trailing newlines don't create empty paragraphs
        let md = "Line 1\n\n\n\nLine 2";
        let html = compile_markdown(md);
        assert_eq!(html, "<p>Line 1</p>\n<p>Line 2</p>\n");
    }

    #[test]
    fn test_html_escaping() {
        let md = "Use <tag> & symbols";
        let html = compile_markdown(md);
        assert_eq!(html, "<p>Use &lt;tag&gt; &amp; symbols</p>\n");
    }
}
