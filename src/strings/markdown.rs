//! Markdown Parser
//!
//! # What this implements and what it replaces
//! This implements a minimal, state-machine-based Markdown to HTML parser.
//! It replaces full-featured parsers like `pulldown-cmark` and `comrak`.
//!
//! # Real-world systems that use this
//! Markdown parsers are ubiquitous. Static site generators (Hugo, Zola), documentation systems (rustdoc),
//! forums (Discourse), and chat apps (Slack/Discord subsets) all rely on them.
//!
//! # Why build it yourself?
//! Parsing text requires carefully managing state and avoiding excessive allocations.
//! Implementing a minimal parser teaches you how to transition between block and inline
//! parsing states, and how to use slices (`&str`) to avoid copying strings.
//!
//! # Architecture
//!
//! The parser processes the document line by line, determining the "block" type.
//! Within blocks, it evaluates "inline" formatting (bold, italics, code).
//!
//! ```text
//! Document
//!   |
//!   v
//! +------------------+
//! | Block Parser     |
//! | - Paragraph      |
//! | - Header         |
//! | - Code Block     |
//! | - List Item      |
//! +------------------+
//!   |
//!   v
//! +------------------+
//! | Inline Parser    |
//! | - **Bold**       |
//! | - *Italic*       |
//! | - `Code`         |
//! | - [Link](url)    |
//! +------------------+
//! ```
//!
//! ## Invariants
//! - **State Consistency**: Inline formatting state (e.g., `in_bold`) must be reset across distinct blocks.
//! - **Memory Efficiency**: The parser should stream or build HTML without creating a full Abstract Syntax Tree (AST) when possible.
//!
//! ## Complexity
//! - **Time**: `O(N)` where N is the length of the input string.
//! - **Space**: `O(N)` to store the output HTML string (no intermediate AST allocations).
//!
//! # Tradeoffs vs. Alternatives
//! - **No AST**: This is a direct string-to-string parser. Production parsers like `pulldown-cmark` build an AST (or event stream) to allow filtering, manipulation, and rendering to non-HTML targets.
//! - **Limited Spec**: This only supports a tiny subset of `CommonMark` (no tables, blockquotes, complex nested lists).

use std::fmt::Write;

#[derive(Debug, PartialEq)]
enum BlockState {
    None,
    Paragraph,
    CodeBlock,
    List,
}

/// Parses a simple markdown string into HTML.
#[must_use]
pub fn parse_markdown(input: &str) -> String {
    let mut html = String::with_capacity(input.len() * 2);
    let mut block_state = BlockState::None;

    let lines = input.lines();

    for line in lines {
        let trimmed = line.trim();

        // Handle Code Blocks first
        if trimmed.starts_with("```") {
            if block_state == BlockState::CodeBlock {
                // End code block
                html.push_str("</code></pre>\n");
                block_state = BlockState::None;
            } else {
                // Close previous block if any
                close_block(&mut html, &mut block_state);
                // Start code block
                // GOTCHA: We don't parse language identifiers here for simplicity.
                html.push_str("<pre><code>\n");
                block_state = BlockState::CodeBlock;
            }
            continue;
        }

        if block_state == BlockState::CodeBlock {
            // Inside a code block, preserve formatting and do NOT parse inline styles.
            // RUST INSIGHT: HTML escaping is crucial here in a real parser.
            html.push_str(line);
            html.push('\n');
            continue;
        }

        if trimmed.is_empty() {
            // Empty line closes the current block
            close_block(&mut html, &mut block_state);
            continue;
        }

        // Headers
        if trimmed.starts_with('#') {
            close_block(&mut html, &mut block_state);
            let level = trimmed.chars().take_while(|&c| c == '#').count();
            if level <= 6 && trimmed.len() > level && trimmed.as_bytes()[level] == b' ' {
                // Ensure bounds checking before slicing
                let content = &trimmed[level + 1..];
                // Headers are single-line, flush immediately
                writeln!(html, "<h{level}>{}</h{level}>", parse_inline(content)).unwrap();
                continue;
            }
        }

        // Lists
        if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
            if block_state != BlockState::List {
                close_block(&mut html, &mut block_state);
                html.push_str("<ul>\n");
                block_state = BlockState::List;
            }
            let content = &trimmed[2..];
            writeln!(html, "<li>{}</li>", parse_inline(content)).unwrap();
            continue;
        }

        // Paragraphs
        if block_state == BlockState::Paragraph {
            // Continuation of paragraph
            html.push(' ');
        } else {
            close_block(&mut html, &mut block_state);
            html.push_str("<p>");
            block_state = BlockState::Paragraph;
        }
        html.push_str(&parse_inline(trimmed));
    }

    // EOF: close any remaining block
    close_block(&mut html, &mut block_state);

    html
}

fn close_block(html: &mut String, state: &mut BlockState) {
    match state {
        BlockState::Paragraph => html.push_str("</p>\n"),
        BlockState::List => html.push_str("</ul>\n"),
        BlockState::CodeBlock => html.push_str("</code></pre>\n"),
        BlockState::None => {}
    }
    *state = BlockState::None;
}

/// Parses inline formatting like `**bold**`, `*italic*`, and `` `code` ``.
/// Uses a state machine approach.
fn parse_inline(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 20);
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;

    let mut in_bold = false;
    let mut in_italic = false;
    let mut in_code = false;

    while i < chars.len() {
        // Inline code toggle
        if chars[i] == '`' {
            if in_code {
                out.push_str("</code>");
            } else {
                out.push_str("<code>");
            }
            in_code = !in_code;
            i += 1;
            continue;
        }

        // Skip other formatting if inside code
        if in_code {
            out.push(chars[i]);
            i += 1;
            continue;
        }

        // Bold toggle
        if i + 1 < chars.len() && chars[i] == '*' && chars[i + 1] == '*' {
            if in_bold {
                out.push_str("</strong>");
            } else {
                out.push_str("<strong>");
            }
            in_bold = !in_bold;
            i += 2;
            continue;
        }

        // Italic toggle
        if chars[i] == '*' || chars[i] == '_' {
            // Need to check it's not part of bold if it's '*'
            if in_italic {
                out.push_str("</em>");
            } else {
                out.push_str("<em>");
            }
            in_italic = !in_italic;
            i += 1;
            continue;
        }

        // Link parsing: [text](url)
        if chars[i] == '[' {
            let mut j = i + 1;
            let mut link_text = String::new();
            while j < chars.len() && chars[j] != ']' {
                link_text.push(chars[j]);
                j += 1;
            }
            if j + 1 < chars.len() && chars[j] == ']' && chars[j + 1] == '(' {
                let mut k = j + 2;
                let mut url = String::new();
                while k < chars.len() && chars[k] != ')' {
                    url.push(chars[k]);
                    k += 1;
                }
                if k < chars.len() && chars[k] == ')' {
                    write!(out, "<a href=\"{url}\">{}</a>", parse_inline(&link_text)).unwrap();
                    i = k + 1;
                    continue;
                }
            }
        }

        out.push(chars[i]);
        i += 1;
    }

    // GOTCHA: We don't auto-close unclosed tags here for simplicity,
    // though a robust parser would handle mismatched formatting gracefully.

    out
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `pulldown-cmark`: A fully spec-compliant CommonMark parser that builds an event stream. Highly optimized and extensible.
//
// Missing vs. Production:
// - **AST**: We just dump straight to a string. You can't easily transform the document before rendering.
// - **Spec Compliance**: We don't handle blockquotes, nested lists, ATX header edge cases, HTML escaping, or nested bold/italic overlaps correctly.
// - **Performance**: Converting `&str` to `Vec<char>` for inline parsing is inefficient for large blocks.
//
// Next Steps:
// 1. Refactor `parse_inline` to operate on byte indices (`&str`) instead of `Vec<char>` to avoid allocation.
// 2. Build a real AST (e.g., `enum Node { Text(String), Header(u8, Vec<Node>), ... }`).

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_headers() {
        assert_eq!(parse_markdown("# Header 1"), "<h1>Header 1</h1>\n");
        assert_eq!(parse_markdown("## Header 2"), "<h2>Header 2</h2>\n");
        assert_eq!(parse_markdown("###### Header 6"), "<h6>Header 6</h6>\n");
    }

    #[test]
    fn test_paragraphs() {
        let input = "Hello world.\nThis is a test.\n\nNew paragraph.";
        assert_eq!(
            parse_markdown(input),
            "<p>Hello world. This is a test.</p>\n<p>New paragraph.</p>\n"
        );
    }

    #[test]
    fn test_inline_formatting() {
        assert_eq!(
            parse_markdown("This is **bold** and *italic*."),
            "<p>This is <strong>bold</strong> and <em>italic</em>.</p>\n"
        );
        assert_eq!(
            parse_markdown("Inline `code()` test."),
            "<p>Inline <code>code()</code> test.</p>\n"
        );
    }

    #[test]
    fn test_links() {
        assert_eq!(
            parse_markdown("Check out [Rust](https://rust-lang.org)"),
            "<p>Check out <a href=\"https://rust-lang.org\">Rust</a></p>\n"
        );
    }

    #[test]
    fn test_lists() {
        let input = "- Item 1\n- Item 2\n* Item 3";
        assert_eq!(
            parse_markdown(input),
            "<ul>\n<li>Item 1</li>\n<li>Item 2</li>\n<li>Item 3</li>\n</ul>\n"
        );
    }

    #[test]
    fn test_code_blocks() {
        let input = "```rust\nfn main() {}\n```";
        assert_eq!(
            parse_markdown(input),
            "<pre><code>\nfn main() {}\n</code></pre>\n"
        );
    }

    #[test]
    fn test_mixed_content() {
        let input = "# Title\n\nIntro text with **bold**.\n\n- List A\n- List B\n\n```\ncode\n```";
        let expected = "<h1>Title</h1>\n<p>Intro text with <strong>bold</strong>.</p>\n<ul>\n<li>List A</li>\n<li>List B</li>\n</ul>\n<pre><code>\ncode\n</code></pre>\n";
        assert_eq!(parse_markdown(input), expected);
    }
}
