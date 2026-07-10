//! # Custom Markdown Parser
//!
//! Implements a custom state-machine-based Markdown parser to HTML compiler.
//!
//! **Replaces Crates:** `pulldown-cmark`, `comrak`
//!
//! **Real-world Usage:**
//! - GitHub flavored markdown rendering.
//! - Static site generators.
//! - Content management systems.
//!
//! **Why build it yourself?**
//! It teaches you how to construct a pull-based event stream parsing system and
//! state-machine-based block and inline parsers, rather than using heavy AST memory allocations.
//!
//! # Architecture
//!
//! We use a pull-based, single-pass iteration using `.lines()` combined with inline processing.
//! To avoid fully allocating an AST in memory, we buffer blocks of text until we determine
//! what kind of element they are (Paragraph vs Header vs Code Block), and emit HTML directly.
//!
//! **Time Complexity**: `O(N)` where N is the length of the string.
//! **Space Complexity**: `O(M)` where M is the maximum size of a single block of text buffering,
//! instead of O(N) for a full AST.

#[derive(Debug, PartialEq)]
enum BlockState {
    None,
    Paragraph,
    Header(usize),
    CodeBlock,
    Quote,
    List,
}

pub struct MarkdownParser {
    input: String,
}

impl MarkdownParser {
    pub fn new(input: &str) -> Self {
        Self {
            input: input.to_string(),
        }
    }

    pub fn parse(&self) -> String {
        let mut output = String::new();
        let mut block_buffer = String::new();
        let mut state = BlockState::None;

        let lines: Vec<&str> = self.input.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i];

            if state == BlockState::CodeBlock {
                if line.trim() == "```" {
                    output.push_str("<pre><code>");
                    output.push_str(&block_buffer);
                    output.push_str("</code></pre>\n");
                    block_buffer.clear();
                    state = BlockState::None;
                } else {
                    // Re-append explicit \n for code blocks
                    block_buffer.push_str(line);
                    block_buffer.push('\n');
                }
                i += 1;
                continue;
            }

            if line.trim().is_empty() {
                self.flush_block(&mut state, &mut block_buffer, &mut output);
                i += 1;
                continue;
            }

            if line.starts_with("```") {
                self.flush_block(&mut state, &mut block_buffer, &mut output);
                state = BlockState::CodeBlock;
                i += 1;
                continue;
            }

            // Headers
            if line.starts_with('#') {
                self.flush_block(&mut state, &mut block_buffer, &mut output);
                let count = line.chars().take_while(|&c| c == '#').count();
                if count <= 6 && count > 0 && line.chars().nth(count) == Some(' ') {
                    state = BlockState::Header(count);
                    block_buffer.push_str(line[count + 1..].trim());
                    // Single line elements like headers should be flushed immediately
                    // GOTCHA: Forgetting to immediately flush single-line elements like headers can cause them
                    // to incorrectly merge into subsequent blocks or lose their newlines.
                    self.flush_block(&mut state, &mut block_buffer, &mut output);
                    i += 1;
                    continue;
                }
            }

            if state == BlockState::None {
                state = BlockState::Paragraph;
            }
            if !block_buffer.is_empty() {
                block_buffer.push(' ');
            }
            block_buffer.push_str(line.trim());

            i += 1;
        }

        self.flush_block(&mut state, &mut block_buffer, &mut output);

        output
    }

    fn flush_block(&self, state: &mut BlockState, buffer: &mut String, output: &mut String) {
        if buffer.trim_end().is_empty() && *state != BlockState::None {
            *state = BlockState::None;
            buffer.clear();
            return;
        }

        let content = self.parse_inline(buffer.trim_end());

        match state {
            BlockState::Paragraph => {
                output.push_str("<p>");
                output.push_str(&content);
                output.push_str("</p>\n");
            }
            BlockState::Header(level) => {
                output.push_str(&format!("<h{}>", level));
                output.push_str(&content);
                output.push_str(&format!("</h{}>\n", level));
            }
            _ => {}
        }

        *state = BlockState::None;
        buffer.clear();
    }

    fn parse_inline(&self, text: &str) -> String {
        let mut result = String::new();
        let mut chars = text.chars().peekable();

        // Inline formatting state must persist across sequential text chunks
        // GOTCHA: Resetting inline formatting state per-chunk instead of per-block causes bold/italic markers to break across line continuations.
        // within the same block element.
        let mut is_bold = false;
        let mut is_italic = false;

        while let Some(c) = chars.next() {
            if c == '*' {
                if let Some(&'*') = chars.peek() {
                    chars.next(); // Consume second '*'
                    if is_bold {
                        result.push_str("</strong>");
                    } else {
                        result.push_str("<strong>");
                    }
                    is_bold = !is_bold;
                } else {
                    if is_italic {
                        result.push_str("</em>");
                    } else {
                        result.push_str("<em>");
                    }
                    is_italic = !is_italic;
                }
            } else {
                result.push(c);
            }
        }

        // Close any unclosed tags
        if is_bold { result.push_str("</strong>"); }
        if is_italic { result.push_str("</em>"); }

        result
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `pulldown-cmark`: A fully compliant CommonMark parser. Uses a pull parser strategy.
//   It fully parses all CommonMark rules including links, images, tables, blockquotes,
//   ordered/unordered lists. Our implementation covers just a few rules to show the logic.
//
// Missing vs. Production:
// - **Links/Images**: Missing entirely.
// - **Lists/Quotes**: Missing entirely.
// - **Escaping**: No HTML escaping for things like `<script>`.
//
// Next Steps:
// 1. Add support for blockquotes and lists.
// 2. Implement robust HTML character escaping.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paragraph() {
        let md = MarkdownParser::new("Hello world");
        assert_eq!(md.parse(), "<p>Hello world</p>\n");
    }

    #[test]
    fn test_headers() {
        let md = MarkdownParser::new("# Header 1\n## Header 2");
        assert_eq!(md.parse(), "<h1>Header 1</h1>\n<h2>Header 2</h2>\n");
    }

    #[test]
    fn test_inline_formatting() {
        let md = MarkdownParser::new("This is *italic* and **bold** text.");
        assert_eq!(md.parse(), "<p>This is <em>italic</em> and <strong>bold</strong> text.</p>\n");
    }

    #[test]
    fn test_code_block() {
        let md = MarkdownParser::new("```\nfn main() {\n    println!();\n}\n```");
        assert_eq!(md.parse(), "<pre><code>fn main() {\n    println!();\n}\n</code></pre>\n");
    }

    #[test]
    fn test_empty_elements() {
        // Ensures no empty paragraphs are emitted for multiple blank lines
        let md = MarkdownParser::new("Line 1\n\n\nLine 2");
        assert_eq!(md.parse(), "<p>Line 1</p>\n<p>Line 2</p>\n");
    }
}
