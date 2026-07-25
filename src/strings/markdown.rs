//! # Markdown Parser Implementation
//!
//! A minimal, educational implementation of a state-machine-based Markdown parser.
//!
//! **Replaces Crates:** `pulldown-cmark`, `comrak` (partial)
//!
//! **Real-world Usage:**
//! - Static site generators (e.g., Hugo, Zola)
//! - Chat applications for rich text rendering (Discord, Slack)
//! - Documentation platforms (rustdoc, mdBook)
//!
//! **Why build it yourself?**
//! Building a Markdown parser teaches you how to design state machines for text processing without building full Abstract Syntax Trees (ASTs) when intermediate heap allocations are undesirable. It highlights bounds checking, string slicing, and the intricacies of inline vs. block-level formatting.
//!
//! # Architecture
//!
//! **Benchmarking Note:**
//! You can benchmark this parser using `criterion` against large markdown files (like the Rust Book markdown sources) to measure throughput (MB/s).
//!
//! **Design decisions and tradeoffs vs. alternatives:**
//! - **State Machine vs. AST:** We generate HTML directly via a state machine rather than building an intermediate AST. This avoids large allocations but makes operations like Table of Contents generation very difficult.
//! - **Line-by-line parsing:** Iterating via `.lines()` strips newlines, requiring us to re-append them for verbatim blocks, but it vastly simplifies paragraph and list parsing.
//!
//! **Components:**
//! 1. **Parser State Machine**: Processes text line by line, recognizing block structures (headings, code blocks, lists).
//! 2. **Inline Formatter**: A secondary state machine that runs within paragraph or heading blocks to format bold, italic, or code spans.
//!
//! **State Machine Diagram:**
//! ```text
//!       [Start]
//!          │
//!          ▼
//!    [Next Line] ◄────────────────────────────────────────┐
//!          │                                              │
//!          ├─ Starts with '#' ─► [Header Block] ──────────┤
//!          │                                              │
//!          ├─ Starts with '```' ─► [Code Block Loop] ─────┤
//!          │                          (append '\n')       │
//!          │                                              │
//!          ├─ Starts with '*' ─► [List Item] ─────────────┤
//!          │                                              │
//!          └─ Else ─► [Paragraph Block] ──────────────────┘
//!                           │
//!                           ▼
//!                    [Inline Formatting]
//!                 (toggle bold/italic state)
//!                           │
//!                    [Reset State & Flush]
//! ```
//!
//! **Invariants:**
//! - Inline formatting state (e.g., `in_bold`) MUST be explicitly reset when flushing out block elements to prevent format leakage into subsequent blocks.
//! - When slicing strings, always check bounds (e.g., `&line[level + 1..]`) to avoid out-of-bounds panics on truncated inputs.
//! - When iterating over `.lines()`, explicit `\n` characters must be manually re-appended to the block content buffer for verbatim blocks like CodeBlocks.
//!
//! **Complexity:**
//! - **Time Complexity**: O(N) where N is the length of the string, as we process each character exactly once or twice.
//! - **Space Complexity**: O(N) to store the rendered HTML output. Intermediate states avoid large AST allocations.

#[derive(Debug, PartialEq)]
enum BlockState {
    Normal,
    CodeBlock,
}

/// A trait for parsing markdown strings into HTML.
pub trait MarkdownParser {
    fn parse(&self, input: &str) -> String;
}

/// A state-machine based implementation of the MarkdownParser trait.
pub struct StateMachineParser;

impl MarkdownParser for StateMachineParser {
    fn parse(&self, input: &str) -> String {
        let mut output = String::new();
        let mut state = BlockState::Normal;
        let mut code_content = String::new();
        let mut in_list = false;

        for line in input.lines() {
            match state {
                BlockState::CodeBlock => {
                    if line.starts_with("```") {
                        output.push_str("<pre><code>");
                        // RUST INSIGHT: HTML escaping is crucial here, but for simplicity we assume safe input
                        // GOTCHA: We need to trim the trailing newline, but since we add `\n` for every line,
                        // the last line will have an extra `\n` which is acceptable for <pre> code blocks.
                        output.push_str(&code_content);
                        output.push_str("</code></pre>\n");
                        code_content.clear();
                        state = BlockState::Normal;
                    } else {
                        code_content.push_str(line);
                        code_content.push('\n');
                    }
                }
                BlockState::Normal => {
                    if line.starts_with("```") {
                        if in_list {
                            output.push_str("</ul>\n");
                            in_list = false;
                        }
                        state = BlockState::CodeBlock;
                    } else if line.starts_with('#') {
                        if in_list {
                            output.push_str("</ul>\n");
                            in_list = false;
                        }

                        let mut level = 0;
                        for c in line.chars() {
                            if c == '#' {
                                level += 1;
                            } else {
                                break;
                            }
                        }

                        if level > 6 {
                            level = 6;
                        }

                        // Prevent panic if there's no space after the hashes
                        let content = if line.len() > level && line[level..].starts_with(' ') {
                            &line[level + 1..]
                        } else if line.len() > level {
                            &line[level..]
                        } else {
                            ""
                        };

                        let formatted = format_inline(content);
                        output.push_str(&format!("<h{}>{}</h{}>\n", level, formatted, level));
                    } else if line.starts_with('*') && line.len() > 1 && line[1..].starts_with(' ')
                    {
                        if !in_list {
                            output.push_str("<ul>\n");
                            in_list = true;
                        }
                        let content = &line[2..];
                        output.push_str(&format!("<li>{}</li>\n", format_inline(content)));
                    } else if line.is_empty() {
                        if in_list {
                            output.push_str("</ul>\n");
                            in_list = false;
                        }
                    } else {
                        if in_list {
                            output.push_str("</ul>\n");
                            in_list = false;
                        }
                        output.push_str(&format!("<p>{}</p>\n", format_inline(line)));
                    }
                }
            }
        }

        if in_list {
            output.push_str("</ul>\n");
        }

        if state == BlockState::CodeBlock {
            output.push_str("<pre><code>");
            output.push_str(&code_content);
            output.push_str("</code></pre>\n");
        }

        output
    }
}

/// Applies inline formatting (bold, italic, code) to a single line or paragraph.
fn format_inline(text: &str) -> String {
    let mut result = String::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;

    let mut in_bold = false;
    let mut in_italic = false;
    let mut in_code = false;

    while i < chars.len() {
        if chars[i] == '`' {
            if in_code {
                result.push_str("</code>");
                in_code = false;
            } else {
                result.push_str("<code>");
                in_code = true;
            }
            i += 1;
        } else if !in_code && chars[i] == '*' {
            if i + 1 < chars.len() && chars[i + 1] == '*' {
                if in_bold {
                    result.push_str("</strong>");
                    in_bold = false;
                } else {
                    result.push_str("<strong>");
                    in_bold = true;
                }
                i += 2;
            } else {
                if in_italic {
                    result.push_str("</em>");
                    in_italic = false;
                } else {
                    result.push_str("<em>");
                    in_italic = true;
                }
                i += 1;
            }
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }

    // In a robust implementation, we would close any unclosed tags at the end of the line,
    // but Markdown typically requires matched pairs.
    if in_code {
        result.push_str("</code>");
    }
    if in_bold {
        result.push_str("</strong>");
    }
    if in_italic {
        result.push_str("</em>");
    }

    result
}

// =========================================================================================
// Tests
// =========================================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_headers() {
        let input = "# Heading 1\n## Heading 2\n### Heading 3";
        let expected = "<h1>Heading 1</h1>\n<h2>Heading 2</h2>\n<h3>Heading 3</h3>\n";
        let parser = StateMachineParser;
        assert_eq!(parser.parse(input), expected);
    }

    #[test]
    fn test_paragraphs() {
        let input = "This is a paragraph.\n\nThis is another paragraph.";
        let expected = "<p>This is a paragraph.</p>\n<p>This is another paragraph.</p>\n";
        let parser = StateMachineParser;
        assert_eq!(parser.parse(input), expected);
    }

    #[test]
    fn test_inline_formatting() {
        let input = "This is **bold** and *italic* and `code`.";
        let expected =
            "<p>This is <strong>bold</strong> and <em>italic</em> and <code>code</code>.</p>\n";
        let parser = StateMachineParser;
        assert_eq!(parser.parse(input), expected);
    }

    #[test]
    fn test_code_blocks() {
        let input = "```\nfn main() {\n    println!(\"Hello\");\n}\n```";
        let expected = "<pre><code>fn main() {\n    println!(\"Hello\");\n}\n</code></pre>\n";
        let parser = StateMachineParser;
        assert_eq!(parser.parse(input), expected);
    }

    #[test]
    fn test_lists() {
        let input = "* Item 1\n* Item 2\n* Item 3";
        let expected = "<ul>\n<li>Item 1</li>\n<li>Item 2</li>\n<li>Item 3</li>\n</ul>\n";
        let parser = StateMachineParser;
        assert_eq!(parser.parse(input), expected);
    }

    #[test]
    fn test_mixed_content() {
        let input = "# Title\n\nSome **bold** text.\n\n* List 1\n* List 2\n\n```\nCode\n```";
        let expected = "<h1>Title</h1>\n<p>Some <strong>bold</strong> text.</p>\n<ul>\n<li>List 1</li>\n<li>List 2</li>\n</ul>\n<pre><code>Code\n</code></pre>\n";
        let parser = StateMachineParser;
        assert_eq!(parser.parse(input), expected);
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// **Comparison to Canonical Crates:**
// - `pulldown-cmark`: A fully spec-compliant CommonMark parser using a pull parser design (events).
//   It's far more robust, handling edge cases, blockquotes, HTML entities, and proper escaping.
// - `comrak`: Full GFM (GitHub Flavored Markdown) support with AST construction.
//
// **What's missing vs Production:**
// - No AST: We stream directly to a string. Real parsers build an AST for manipulation (e.g., TOC generation) before rendering.
// - Escaping: We don't escape `<` or `>` characters in text or code blocks, which is a major security flaw (XSS) for a real renderer.
// - Edge Cases: Nested lists, blockquotes, links (`[text](url)`), images, and thematic breaks are omitted for simplicity.
//
// **Next Steps:**
// - Implement a pull-parser model that yields events (e.g., `Event::Start(Tag::Header(1))`) instead of raw HTML.
// - Add HTML escaping for user-supplied content.
// - Support links and images.
