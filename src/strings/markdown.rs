//! # Markdown Parser
//!
//! **Replaces:** `pulldown-cmark`, `comrak`
//!
//! **Real-world systems:** GitHub (for rendering READMEs), Ghost, Discourse, static site generators.
//!
//! **Why build it yourself?** To understand pull-based event stream parsing vs. full AST parsing,
//! and how simple state machines can handle complex block/inline formatting without recursion blowups.
//!
//! ## Architecture
//!
//! The parser is implemented as an iterator over `Event`s (a stream), directly converting to HTML.
//! It uses a two-pass approach on a per-block level:
//! 1. **Block phase**: Groups lines into paragraphs, headers, lists, etc.
//! 2. **Inline phase**: Parses emphasis, strong text, code, and links within those blocks.
//!
//! ### Complexity
//! | Operation | Time | Space |
//! |-----------|------|-------|
//! | Parsing   | O(N) | O(B)  |
//! Where N is the total text length and B is the size of the current block.
//!
//! ### Design Decisions
//! - **Event Stream**: Instead of building a massive DOM-like tree in memory, we emit events (Start, End, Text).
//!   This saves memory and allows fast transformation.
//! - **Iterative Inline Parsing**: Inline parsing searches for matching delimiters (e.g., `**`) using loops
//!   and `char.len_utf8()` to advance safely over multibyte characters, preventing infinite loops.
//!
//! ## Comparison to Canonical Crates
//! - `pulldown-cmark` is an industry-standard, fully CommonMark-compliant parser written in Rust.
//!   It is heavily optimized and handles all edge cases.
//! - This implementation focuses on the core features (headers, paragraphs, bold, italic)
//!   using the same fundamental event stream idea but skipping complex nesting (like lists within blockquotes).
//!
//! ## Missing vs. Production
//! - **Full CommonMark Spec**: Does not handle HTML entities, reference links, deeply nested structures, etc.
//! - **Performance**: A production parser uses aggressive zero-copy slicing (`&'a str`), whereas we use `String`
//!   for simplicity in some places (though we use `&'a str` where easy).
//!
//! ## Next Steps
//! 1. Add support for bulleted and numbered lists.
//! 2. Add blockquotes (`> text`).
//! 3. Convert output generation to directly write to a `fmt::Write` trait instead of building strings in memory.
//!
//! ## Benchmarking Note
//! Can be benchmarked using `criterion` by parsing a large markdown file (like a book) and comparing throughput against `pulldown-cmark`.

#[derive(Debug, PartialEq, Clone)]
pub enum Event<'a> {
    Start(Tag<'a>),
    End(Tag<'a>),
    Text(&'a str),
}

#[derive(Debug, PartialEq, Clone)]
pub enum Tag<'a> {
    Paragraph,
    Header(u8),
    Strong,
    Emphasis,
    Link(&'a str), // URL
    Code,
}

pub struct Parser<'a> {
    input: &'a str,
    events: Vec<Event<'a>>,
}

impl<'a> Parser<'a> {
    #[must_use]
    pub fn new(input: &'a str) -> Self {
        let mut parser = Self {
            input,
            events: Vec::new(),
        };
        parser.parse_blocks();
        // Since we parsed everything into `events` internally during initialization,
        // we reverse it so we can easily pop elements during iteration (or just keep an index).
        // A real pull parser would yield them lazily. For simplicity, we parse all to an event queue.
        parser.events.reverse();
        parser
    }

    fn parse_blocks(&mut self) {
        let lines: Vec<&str> = self.input.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i];

            if line.trim().is_empty() {
                i += 1;
                continue;
            }

            // Check for headers
            if line.starts_with('#') {
                let mut level = 0;
                for c in line.chars() {
                    if c == '#' {
                        level += 1;
                    } else {
                        break;
                    }
                }

                if level > 0 && level <= 6 && line[level as usize..].starts_with(' ') {
                    let content = line[level as usize + 1..].trim();
                    self.events.push(Event::Start(Tag::Header(level)));
                    self.parse_inline(content);
                    self.events.push(Event::End(Tag::Header(level)));
                    i += 1;
                    continue;
                }
            }

            // Paragraph
            // Accumulate lines until an empty line
            self.events.push(Event::Start(Tag::Paragraph));
            let mut para_text = String::new();
            while i < lines.len() && !lines[i].trim().is_empty() {
                if !para_text.is_empty() {
                    para_text.push('\n');
                }
                para_text.push_str(lines[i]);
                i += 1;
            }

            // To keep lifetimes simple, we should ideally reference the original string.
            // Since paragraph lines are contiguous (except we added '\n'),
            // if we are just looking for inline formatting, we can parse it inline.
            // But we collected it into a `String` which breaks our `&'a str` events.
            // GOTCHA: A true zero-copy parser wouldn't allocate here. It would yield Text slices.
            // For this pedagogical version, we'll parse line by line for paragraphs to avoid allocation,
            // or use a static approach. Let's parse inline directly on slices.

            // Re-evaluating paragraph logic to remain zero-copy:
            // Let's find the end of the block in the original text to get a slice.
            i -= 1; // back up to last non-empty line
            // But we can't easily do block parsing line-by-line while maintaining exact slice bounds
            // unless we track byte offsets carefully.
        }

        // Let's rewrite `parse_blocks` to work strictly on byte offsets to maintain zero-copy.
        self.events.clear();
        let mut pos = 0;
        let bytes = self.input.as_bytes();

        while pos < self.input.len() {
            // Skip empty lines
            while pos < self.input.len() && (bytes[pos] == b'\n' || bytes[pos] == b'\r') {
                pos += 1;
            }

            if pos >= self.input.len() {
                break;
            }

            // Check header
            if bytes[pos] == b'#' {
                let mut level = 0;
                let mut peek = pos;
                while peek < self.input.len() && bytes[peek] == b'#' {
                    level += 1;
                    peek += 1;
                }

                if level <= 6 && peek < self.input.len() && bytes[peek] == b' ' {
                    // It's a header
                    pos = peek + 1; // skip space
                    let end = self.find_line_end(pos);

                    self.events.push(Event::Start(Tag::Header(level)));
                    self.parse_inline(&self.input[pos..end]);
                    self.events.push(Event::End(Tag::Header(level)));

                    pos = end;
                    continue;
                }
            }

            // It's a paragraph block. Find where it ends (double newline)
            let start = pos;
            let mut end = pos;
            while pos < self.input.len() {
                let line_end = self.find_line_end(pos);
                let next_line_start = self.skip_newlines(line_end);

                if next_line_start == self.input.len() || self.is_empty_line(next_line_start) {
                    end = line_end;
                    pos = next_line_start;
                    break;
                }
                pos = next_line_start;
            }

            if start < end {
                self.events.push(Event::Start(Tag::Paragraph));
                self.parse_inline(&self.input[start..end]);
                self.events.push(Event::End(Tag::Paragraph));
            }
        }
    }

    fn find_line_end(&self, start: usize) -> usize {
        let mut i = start;
        let bytes = self.input.as_bytes();
        while i < self.input.len() && bytes[i] != b'\n' && bytes[i] != b'\r' {
            i += 1;
        }
        i
    }

    fn skip_newlines(&self, start: usize) -> usize {
        let mut i = start;
        let bytes = self.input.as_bytes();
        while i < self.input.len() && (bytes[i] == b'\n' || bytes[i] == b'\r') {
            i += 1;
        }
        i
    }

    fn is_empty_line(&self, start: usize) -> bool {
        let line_end = self.find_line_end(start);
        self.input[start..line_end].trim().is_empty()
    }

    fn parse_inline(&mut self, text: &'a str) {
        let mut i = 0;
        let mut last_text = 0;

        while i < text.len() {
            let rest = &text[i..];

            // RUST INSIGHT: Searching for tags requires advancing by character length,
            // not just `+ 1`, to avoid panicking when slicing multibyte UTF-8 characters.

            // Strong: **
            if rest.starts_with("**") {
                if i > last_text {
                    self.events.push(Event::Text(&text[last_text..i]));
                }

                // Find closing **
                if let Some(end) = text[i + 2..].find("**") {
                    let inner_start = i + 2;
                    let inner_end = inner_start + end;

                    self.events.push(Event::Start(Tag::Strong));
                    self.parse_inline(&text[inner_start..inner_end]);
                    self.events.push(Event::End(Tag::Strong));

                    i = inner_end + 2;
                    last_text = i;
                    continue;
                }
            }

            // Emphasis: *
            if rest.starts_with("*") && !rest.starts_with("**") {
                if i > last_text {
                    self.events.push(Event::Text(&text[last_text..i]));
                }

                // Find closing *
                let mut found = false;
                let mut search_idx = i + 1;
                while search_idx < text.len() {
                    if text[search_idx..].starts_with("*") && !text[search_idx..].starts_with("**") {
                        let inner_start = i + 1;
                        let inner_end = search_idx;

                        self.events.push(Event::Start(Tag::Emphasis));
                        self.parse_inline(&text[inner_start..inner_end]);
                        self.events.push(Event::End(Tag::Emphasis));

                        i = inner_end + 1;
                        last_text = i;
                        found = true;
                        break;
                    }
                    search_idx += text[search_idx..].chars().next().unwrap().len_utf8();
                }
                if found { continue; }
            }

            // Code: `
            if rest.starts_with("`") {
                if i > last_text {
                    self.events.push(Event::Text(&text[last_text..i]));
                }

                if let Some(end) = text[i + 1..].find('`') {
                    let inner_start = i + 1;
                    let inner_end = inner_start + end;

                    self.events.push(Event::Start(Tag::Code));
                    self.events.push(Event::Text(&text[inner_start..inner_end]));
                    self.events.push(Event::End(Tag::Code));

                    i = inner_end + 1;
                    last_text = i;
                    continue;
                }
            }

            // Advance by one character safely
            i += rest.chars().next().unwrap().len_utf8();
        }

        if last_text < text.len() {
            self.events.push(Event::Text(&text[last_text..]));
        }
    }
}

impl<'a> Iterator for Parser<'a> {
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.events.pop()
    }
}

/// Converts a stream of Markdown events into HTML.
pub fn push_html<'a, I>(html: &mut String, iter: I)
where
    I: Iterator<Item = Event<'a>>,
{
    for event in iter {
        match event {
            Event::Start(tag) => match tag {
                Tag::Paragraph => html.push_str("<p>"),
                Tag::Header(level) => html.push_str(&format!("<h{}>", level)),
                Tag::Strong => html.push_str("<strong>"),
                Tag::Emphasis => html.push_str("<em>"),
                Tag::Code => html.push_str("<code>"),
                Tag::Link(url) => html.push_str(&format!("<a href=\"{}\">", url)),
            },
            Event::End(tag) => match tag {
                Tag::Paragraph => html.push_str("</p>\n"),
                Tag::Header(level) => html.push_str(&format!("</h{}>\n", level)),
                Tag::Strong => html.push_str("</strong>"),
                Tag::Emphasis => html.push_str("</em>"),
                Tag::Code => html.push_str("</code>"),
                Tag::Link(_) => html.push_str("</a>"),
            },
            Event::Text(text) => html.push_str(text),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paragraph() {
        let input = "Hello world";
        let parser = Parser::new(input);
        let mut html = String::new();
        push_html(&mut html, parser);
        assert_eq!(html, "<p>Hello world</p>\n");
    }

    #[test]
    fn test_headers() {
        let input = "# Header 1\n## Header 2";
        let parser = Parser::new(input);
        let mut html = String::new();
        push_html(&mut html, parser);
        assert_eq!(html, "<h1>Header 1</h1>\n<h2>Header 2</h2>\n");
    }

    #[test]
    fn test_inline_strong() {
        let input = "Hello **world**!";
        let parser = Parser::new(input);
        let mut html = String::new();
        push_html(&mut html, parser);
        assert_eq!(html, "<p>Hello <strong>world</strong>!</p>\n");
    }

    #[test]
    fn test_inline_emphasis() {
        let input = "Hello *world*!";
        let parser = Parser::new(input);
        let mut html = String::new();
        push_html(&mut html, parser);
        assert_eq!(html, "<p>Hello <em>world</em>!</p>\n");
    }

    #[test]
    fn test_inline_code() {
        let input = "Use `code` here";
        let parser = Parser::new(input);
        let mut html = String::new();
        push_html(&mut html, parser);
        assert_eq!(html, "<p>Use <code>code</code> here</p>\n");
    }

    #[test]
    fn test_multibyte_chars() {
        let input = "Hello **世界**";
        let parser = Parser::new(input);
        let mut html = String::new();
        push_html(&mut html, parser);
        assert_eq!(html, "<p>Hello <strong>世界</strong></p>\n");
    }

    #[test]
    fn test_nested_inline() {
        let input = "Hello **bold *and italic***";
        let parser = Parser::new(input);
        let mut html = String::new();
        push_html(&mut html, parser);
        assert_eq!(html, "<p>Hello <strong>bold <em>and italic</em></strong></p>\n");
    }
}
