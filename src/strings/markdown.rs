//! # Markdown Parser Implementation
//!
//! Implements a minimal, state-machine-based Markdown parser that converts Markdown
//! text directly into HTML without building a full Abstract Syntax Tree (AST).
//!
//! **Replaces Crates:** `pulldown-cmark`, `comrak`
//!
//! **Real-world Usage:**
//! - Static site generators rendering blog posts.
//! - Web forums converting user-submitted markdown to safe HTML.
//! - In-browser or server-side document preview services.
//!
//! **Why build it yourself?**
//! Building a custom Markdown parser demystifies text processing and state machines.
//! You learn how to handle context-dependent syntax (like inline vs block elements),
//! manage parser state across lines, and optimize for memory efficiency by streaming
//! output instead of allocating a large intermediate AST.

use std::fmt::Write;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Flow:
//
//      Markdown Text
//           │
//           ▼
//    [ Line Iterator ]
//           │
//           ▼
//    [ Block State Machine ]  ──► [ Inline Parser ] ──► [ HTML Output Buffer ]
//      (Paragraph, List,
//       CodeBlock, etc.)
//
// Invariants:
// 1. Single-line block elements (Headers) are flushed immediately.
// 2. Multi-line block elements (Paragraphs, Lists, CodeBlocks) buffer their contents
//    and flush when the block ends (empty line or type change).
// 3. Inline formatting state (bold, italic) persists within a single block element's content.
// 4. Trailing whitespace/newlines are trimmed from buffered blocks before HTML wrapping.
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ Parse String  │ O(N)        │ O(N)        │
// └───────────────┴─────────────┴─────────────┘
// N is the length of the input string.
// Space complexity is O(N) primarily for the output buffer.
//
// Design Decisions & Tradeoffs:
// - **AST-less Streaming**: We avoid building an AST and write directly to an output buffer.
//   - *Tradeoff*: High memory efficiency and speed.
//   - *Downside*: Difficult to perform complex semantic transformations or validations (like table of contents generation) that require a global view.
// - **Line-by-Line Processing**:
//   - *Tradeoff*: Easy to handle block-level elements.
//   - *Downside*: Inline elements spanning multiple lines inside a paragraph can be slightly trickier, but manageable.

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum BlockState {
    None,
    Paragraph,
    UnorderedList,
    CodeBlock,
    Blockquote,
}

pub trait MarkdownRenderer {
    /// Parses a Markdown string and returns the rendered output.
    fn render(&mut self, input: &str) -> String;
}

pub struct MarkdownParser {
    output: String,
    state: BlockState,
    buffer: String,
}

impl Default for MarkdownParser {
    fn default() -> Self {
        Self::new()
    }
}

impl MarkdownRenderer for MarkdownParser {
    fn render(&mut self, input: &str) -> String {
        self.parse(input)
    }
}

impl MarkdownParser {
    #[must_use]
    pub fn new() -> Self {
        Self {
            output: String::with_capacity(1024),
            state: BlockState::None,
            buffer: String::with_capacity(256),
        }
    }

    #[must_use]
    pub fn parse(&mut self, input: &str) -> String {
        let lines: Vec<&str> = input.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i];

            if self.state == BlockState::CodeBlock {
                if line.starts_with("```") {
                    self.flush_block();
                } else {
                    // Re-append the newline stripped by .lines()
                    self.buffer.push_str(line);
                    self.buffer.push('\n');
                }
                i += 1;
                continue;
            }

            if line.trim().is_empty() {
                self.flush_block();
                i += 1;
                continue;
            }

            // Headers (Single-line elements, flush immediately)
            if line.starts_with('#') {
                self.flush_block();
                let (level, text) = Self::parse_header(line);
                let parsed_text = Self::parse_inline(text.trim());
                // RUST INSIGHT: write! macro with String avoids intermediate allocations
                // compared to format!() which allocates a new string before pushing.
                writeln!(&mut self.output, "<h{level}>{parsed_text}</h{level}>").unwrap();
                i += 1;
                continue;
            }

            // Code Block Start
            if line.starts_with("```") {
                self.flush_block();
                self.state = BlockState::CodeBlock;
                // Optional language tag handling could go here.
                i += 1;
                continue;
            }

            // Blockquote
            if let Some(stripped) = line.strip_prefix("> ") {
                if self.state == BlockState::Blockquote {
                    self.buffer.push(' ');
                } else {
                    self.flush_block();
                    self.state = BlockState::Blockquote;
                }
                self.buffer.push_str(stripped);
                i += 1;
                continue;
            }

            // Unordered List
            if let Some(stripped) = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")) {
                if self.state != BlockState::UnorderedList {
                    self.flush_block();
                    self.state = BlockState::UnorderedList;
                }
                let parsed_item = Self::parse_inline(stripped);
                writeln!(&mut self.buffer, "<li>{parsed_item}</li>").unwrap();
                i += 1;
                continue;
            }

            // Paragraph (Fallback)
            if self.state == BlockState::Paragraph {
                // If appending to existing paragraph, add a space (since .lines() strips newlines)
                self.buffer.push(' ');
            } else {
                self.flush_block();
                self.state = BlockState::Paragraph;
            }
            self.buffer.push_str(line.trim());

            i += 1;
        }

        self.flush_block();
        self.output.clone()
    }

    fn flush_block(&mut self) {
        if self.state == BlockState::None || self.buffer.is_empty() {
            self.state = BlockState::None;
            self.buffer.clear();
            return;
        }

        // GOTCHA: Trim trailing whitespace/newlines before wrapping to avoid empty elements
        // like <p></p> or extra spacing inside blocks.
        let content = self.buffer.trim_end();

        if content.is_empty() {
            self.state = BlockState::None;
            self.buffer.clear();
            return;
        }

        match self.state {
            BlockState::Paragraph => {
                let parsed = Self::parse_inline(content);
                writeln!(&mut self.output, "<p>{parsed}</p>").unwrap();
            }
            BlockState::UnorderedList => {
                write!(&mut self.output, "<ul>\n{content}\n</ul>\n").unwrap();
            }
            BlockState::Blockquote => {
                let parsed = Self::parse_inline(content);
                writeln!(&mut self.output, "<blockquote>{parsed}</blockquote>").unwrap();
            }
            BlockState::CodeBlock => {
                write!(&mut self.output, "<pre><code>{content}\n</code></pre>\n").unwrap();
            }
            BlockState::None => {}
        }

        self.buffer.clear();
        self.state = BlockState::None;
    }

    fn parse_header(line: &str) -> (usize, &str) {
        let mut level = 0;
        let mut chars = line.chars();
        for c in chars.by_ref() {
            if c == '#' {
                level += 1;
            } else if c == ' ' {
                break;
            } else {
                // Invalid header, e.g. "##NoSpace"
                return (0, line);
            }
        }

        if level > 0 && level <= 6 {
            // Guard against strings like "##" which have no space or text after
            if line.len() > level {
                (level, &line[level + 1..]) // Skip the '#'s and the space
            } else {
                (0, line) // Just "#" with nothing else is treated as text
            }
        } else {
            (0, line)
        }
    }

    /// Parses inline formatting like **bold**, *italic*, `code`, and [links](url).
    /// Note: This is a simplified state-machine that processes character by character.
    fn parse_inline(text: &str) -> String {
        let mut result = String::with_capacity(text.len() + 16);
        let mut chars = text.chars().peekable();

        let mut in_bold = false;
        let mut in_italic = false;
        let mut in_code = false;

        while let Some(c) = chars.next() {
            if c == '`' {
                if in_code {
                    result.push_str("</code>");
                } else {
                    result.push_str("<code>");
                }
                in_code = !in_code;
            } else if in_code {
                // Inside code, ignore other formatting
                result.push(c);
            } else if c == '*' {
                if chars.peek() == Some(&'*') {
                    chars.next(); // Consume second '*'
                    if in_bold {
                        result.push_str("</strong>");
                    } else {
                        result.push_str("<strong>");
                    }
                    in_bold = !in_bold;
                } else {
                    if in_italic {
                        result.push_str("</em>");
                    } else {
                        result.push_str("<em>");
                    }
                    in_italic = !in_italic;
                }
            } else if c == '[' {
                // Simplified link parsing
                let mut link_text = String::new();
                let mut link_url = String::new();
                let mut found_end_bracket = false;

                for inner_c in chars.by_ref() {
                    if inner_c == ']' {
                        found_end_bracket = true;
                        break;
                    }
                    link_text.push(inner_c);
                }

                if found_end_bracket && chars.peek() == Some(&'(') {
                    chars.next(); // Consume '('
                    for inner_c in chars.by_ref() {
                        if inner_c == ')' {
                            break;
                        }
                        link_url.push(inner_c);
                    }
                    write!(&mut result, "<a href=\"{link_url}\">{link_text}</a>").unwrap();
                } else {
                    // Fallback if not a valid link
                    result.push('[');
                    result.push_str(&link_text);
                    if found_end_bracket {
                        result.push(']');
                    }
                }
            } else {
                result.push(c);
            }
        }

        // Close unclosed tags to prevent malformed HTML
        if in_bold {
            result.push_str("</strong>");
        }
        if in_italic {
            result.push_str("</em>");
        }
        if in_code {
            result.push_str("</code>");
        }

        result
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `pulldown-cmark`: A full, spec-compliant CommonMark parser. Uses events instead of strings,
//   and builds a full AST for extensions. Highly optimized.
// - `comrak`: Another full AST parser, compatible with GitHub Flavored Markdown (GFM).
//
// Missing vs. Production:
// - **Spec Compliance**: This does not follow the full CommonMark spec (e.g., nested lists, escaped characters).
// - **XSS Protection**: Production parsers must HTML-escape arbitrary text/attributes to prevent XSS. We skip this for brevity.
// - **AST Flexibility**: Since we don't build an AST, we can't easily transform the document before rendering.
//
// Next Steps:
// 1. Add HTML escaping for `&`, `<`, and `>` in normal text.
// 2. Implement nested lists and ordered lists (`1. `).
// 3. Support image tags (`![alt](url)`).

// =========================================================================================
// Benchmarking Note
// =========================================================================================
// To benchmark this parser against `pulldown-cmark`:
// ```rust
// use criterion::{black_box, criterion_group, criterion_main, Criterion};
// fn bench_markdown(c: &mut Criterion) {
//     let input = "# Header\n\nSome **bold** text.";
//     c.bench_function("parse_markdown", |b| b.iter(|| {
//         let mut parser = MarkdownParser::new();
//         black_box(parser.render(black_box(input)));
//     }));
// }
// ```

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_headers() {
        let mut parser = MarkdownParser::new();
        let result = parser.parse("# Header 1\n## Header 2");
        assert_eq!(result, "<h1>Header 1</h1>\n<h2>Header 2</h2>\n");
    }

    #[test]
    fn test_paragraph() {
        let mut parser = MarkdownParser::new();
        let result = parser.parse("Hello\nWorld");
        assert_eq!(result, "<p>Hello World</p>\n");
    }

    #[test]
    fn test_inline_formatting() {
        let mut parser = MarkdownParser::new();
        let result = parser.parse("This is **bold** and *italic* and `code`.");
        assert_eq!(
            result,
            "<p>This is <strong>bold</strong> and <em>italic</em> and <code>code</code>.</p>\n"
        );
    }

    #[test]
    fn test_links() {
        let mut parser = MarkdownParser::new();
        let result = parser.parse("[Rust](https://rust-lang.org)");
        assert_eq!(
            result,
            "<p><a href=\"https://rust-lang.org\">Rust</a></p>\n"
        );
    }

    #[test]
    fn test_unordered_list() {
        let mut parser = MarkdownParser::new();
        let result = parser.parse("- Item 1\n- Item 2");
        assert_eq!(result, "<ul>\n<li>Item 1</li>\n<li>Item 2</li>\n</ul>\n");
    }

    #[test]
    fn test_blockquote() {
        let mut parser = MarkdownParser::new();
        let result = parser.parse("> Quote\n> goes here");
        assert_eq!(result, "<blockquote>Quote goes here</blockquote>\n");
    }

    #[test]
    fn test_code_block() {
        let mut parser = MarkdownParser::new();
        let result = parser.parse("```\nfn main() {}\n```");
        assert_eq!(result, "<pre><code>fn main() {}\n</code></pre>\n");
    }

    #[test]
    fn test_complex_document() {
        let input = "\
# Title

This is a paragraph with **bold** text.

- Item 1
- Item 2

> A quote here

```
code block
```";
        let mut parser = MarkdownParser::new();
        let result = parser.parse(input);

        let expected = "\
<h1>Title</h1>
<p>This is a paragraph with <strong>bold</strong> text.</p>
<ul>
<li>Item 1</li>
<li>Item 2</li>
</ul>
<blockquote>A quote here</blockquote>
<pre><code>code block
</code></pre>
";
        assert_eq!(result, expected);
    }
}
