//! # Markdown Parser
//!
//! A minimal Markdown to HTML compiler using a pull-based event stream and state machines.
//!
//! **Replaces Crates:** `pulldown-cmark`, `comrak`
//!
//! **Real-world Usage:**
//! - Static site generators (Hugo, Zola, Gatsby)
//! - Chat applications (Slack, Discord, Teams)
//! - Forums (Reddit, Discourse)
//!
//! **Why build it yourself?**
//! Parsing Markdown isn't as simple as regex replacements. You need to understand block vs inline
//! formatting, state transitions (like nested bold/italic), and how to process text sequentially
//! without generating massive Abstract Syntax Trees (ASTs) for small snippets.
//!
//! # Architecture
//!
//! ```text
//! Markdown Text -> Line Iteration -> Block Parser -> Inline Parser -> HTML Output Buffer
//! ```
//!
//! - **Block Parser**: Identifies structural elements (Headers, Paragraphs, Code Blocks, Lists).
//! - **Inline Parser**: Applies formatting (Bold, Italic) to the text content within blocks.
//! - **Event Stream**: Instead of a full AST, we push HTML directly as we parse lines.
//!
//! **Invariants:**
//! - Inline formatting state (e.g., in a bold block) persists across words but resets per block.
//! - Single-line elements (Headers) flush immediately; multi-line elements (Paragraphs) buffer text.
//! - Code blocks preserve whitespace and internal newlines.
//!
//! **Complexity:**
//! - Time Complexity: O(N) where N is the length of the document text.
//! - Space Complexity: O(B) where B is the size of the largest single block (for buffering).
//!
//! **Design Decisions:**
//! - We use a simple `enum BlockState` to manage whether we are currently accumulating a paragraph or code block.
//! - Inline parsing is applied to the accumulated block text only right before flushing it to HTML.
//!
//! # Footer
//!
//! **Comparison to `pulldown-cmark`:**
//! `pulldown-cmark` is a spec-compliant CommonMark parser that emits an event stream. It's incredibly robust against edge cases (like malicious deeply nested emphasis) and handles links, images, tables, etc. Our implementation handles a tiny subset of Markdown for demonstration.
//!
//! **Missing Features:**
//! - Links, images, blockquotes, unordered/ordered lists.
//! - Nested formatting correctly resolving (e.g., `***bold italic***`).
//! - HTML escaping for security against XSS.
//!
//! **Benchmarking Note:**
//! To benchmark, measure the parsing of a large Markdown document using `criterion`. Monitor the allocation overhead of the output HTML string and internal block buffers.

use std::fmt::Write;

#[derive(Debug, PartialEq)]
enum BlockState {
    None,
    Paragraph(String),
    CodeBlock(String),
}

/// Parses Markdown text and returns an HTML string.
pub fn parse_markdown(input: &str) -> String {
    let mut html = String::new();
    let mut state = BlockState::None;

    for line in input.lines() {
        if line.starts_with("```") {
            state = match state {
                BlockState::CodeBlock(content) => {
                    // Close code block
                    write!(&mut html, "<pre><code>{}</code></pre>", content).unwrap();
                    BlockState::None
                }
                BlockState::Paragraph(p) => {
                    // Flush paragraph before starting code block
                    if !p.is_empty() {
                        let parsed_p = parse_inline(&p);
                        write!(&mut html, "<p>{}</p>", parsed_p.trim_end()).unwrap();
                    }
                    BlockState::CodeBlock(String::new())
                }
                BlockState::None => BlockState::CodeBlock(String::new()),
            };
            continue;
        }

        match &mut state {
            BlockState::CodeBlock(content) => {
                // RUST INSIGHT: `.lines()` strips newlines, so we must manually append them
                // to preserve verbatim formatting inside code blocks.
                content.push_str(line);
                content.push('\n');
            }
            _ => {
                if line.trim().is_empty() {
                    // Empty line means flush the current block
                    if let BlockState::Paragraph(p) = &mut state {
                        if !p.is_empty() {
                            let parsed_p = parse_inline(p);
                            // GOTCHA: Always trim trailing space before wrapping in tags
                            write!(&mut html, "<p>{}</p>", parsed_p.trim_end()).unwrap();
                            state = BlockState::None;
                        }
                    }
                } else if line.starts_with('#') {
                    // Header line
                    // Flush existing paragraph if any
                    if let BlockState::Paragraph(p) = &mut state {
                        if !p.is_empty() {
                            let parsed_p = parse_inline(p);
                            write!(&mut html, "<p>{}</p>", parsed_p.trim_end()).unwrap();
                            state = BlockState::None;
                        }
                    }

                    // Count '#' to determine header level
                    let mut level = 0;
                    for c in line.chars() {
                        if c == '#' {
                            level += 1;
                        } else {
                            break;
                        }
                    }

                    let content = line[level..].trim();
                    let parsed_content = parse_inline(content);
                    // Single-line element: flush immediately
                    write!(&mut html, "<h{}>{}</h{}>", level, parsed_content, level).unwrap();
                } else {
                    // Accumulate paragraph
                    match &mut state {
                        BlockState::Paragraph(p) => {
                            p.push_str(line);
                            p.push(' ');
                        }
                        BlockState::None => {
                            let mut p = String::new();
                            p.push_str(line);
                            p.push(' ');
                            state = BlockState::Paragraph(p);
                        }
                        _ => unreachable!(),
                    }
                }
            }
        }
    }

    // Flush remaining state
    match state {
        BlockState::Paragraph(p) => {
            if !p.is_empty() {
                let parsed_p = parse_inline(&p);
                write!(&mut html, "<p>{}</p>", parsed_p.trim_end()).unwrap();
            }
        }
        BlockState::CodeBlock(content) => {
            write!(&mut html, "<pre><code>{}</code></pre>", content).unwrap();
        }
        BlockState::None => {}
    }

    html
}

fn parse_inline(text: &str) -> String {
    let mut html = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();

    let mut in_bold = false;
    let mut in_italic = false;

    while let Some(c) = chars.next() {
        match c {
            '*' => {
                if let Some(&'*') = chars.peek() {
                    // Bold
                    chars.next(); // consume second '*'
                    if in_bold {
                        html.push_str("</strong>");
                    } else {
                        html.push_str("<strong>");
                    }
                    in_bold = !in_bold;
                } else {
                    // Italic
                    if in_italic {
                        html.push_str("</em>");
                    } else {
                        html.push_str("<em>");
                    }
                    in_italic = !in_italic;
                }
            }
            _ => html.push(c),
        }
    }

    html
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_headers() {
        let input = "# Header 1\n## Header 2";
        let html = parse_markdown(input);
        assert_eq!(html, "<h1>Header 1</h1><h2>Header 2</h2>");
    }

    #[test]
    fn test_paragraphs_and_inline() {
        let input = "This is a **bold** and *italic* text.\n\nAnother paragraph.";
        let html = parse_markdown(input);
        assert_eq!(html, "<p>This is a <strong>bold</strong> and <em>italic</em> text.</p><p>Another paragraph.</p>");
    }

    #[test]
    fn test_code_block() {
        let input = "```\nfn main() {\n    println!(\"Hello\");\n}\n```";
        let html = parse_markdown(input);
        assert_eq!(html, "<pre><code>fn main() {\n    println!(\"Hello\");\n}\n</code></pre>");
    }

    #[test]
    fn test_mixed_content() {
        let input = "# Title\nParagraph one.\n\n```\ncode\n```\nParagraph two.";
        let html = parse_markdown(input);
        assert_eq!(
            html,
            "<h1>Title</h1><p>Paragraph one.</p><pre><code>code\n</code></pre><p>Paragraph two.</p>"
        );
    }
}
