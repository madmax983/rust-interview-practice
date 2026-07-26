//! # Markdown Parser
//!
//! Difficulty: Hard
//!
//! Why this matters in Rust: Writing parsers in Rust is a joy thanks to its pattern matching,
//! iterators, and strict ownership model. This module demonstrates a minimal, memory-efficient
//! state-machine-based parser that avoids full AST construction, functioning similarly to a
//! stream processor.
//!
//! It replaces basic usage of crates like `pulldown-cmark` or `comrak` when only a small
//! subset of Markdown (like headers and code blocks) needs to be parsed, demonstrating how to
//! build robust parsers from scratch.

use std::fmt::Write;

/// **Approach: Stream Processing State Machine**
///
/// **Algorithm:**
/// We process the Markdown text line-by-line using an iterator. We maintain state (e.g., whether
/// we are currently inside a code block) and match on line prefixes to determine the block type.
/// Instead of building an AST and walking it, we immediately output HTML strings.
///
/// **Complexity:**
/// - **Time:** O(N) where N is the length of the input string, as we do a single pass.
/// - **Space:** O(M) where M is the size of the generated HTML output, avoiding intermediate AST allocations.
///
/// **Idiomatic Rust:**
/// The use of `lines()` iterator combined with `strip_prefix` and `match` makes the parsing logic
/// declarative and easy to read.
pub fn parse_markdown(input: &str) -> String {
    let mut output = String::new();
    let mut in_code_block = false;

    for line in input.lines() {
        if in_code_block {
            if line.starts_with("```") {
                in_code_block = false;
                output.push_str("</code></pre>\n");
            } else {
                // GOTCHA: `.lines()` strips newlines, so we must re-append them for code blocks
                writeln!(&mut output, "{line}").unwrap();
            }
            continue;
        }

        if line.starts_with("```") {
            in_code_block = true;
            output.push_str("<pre><code>\n");
        } else if let Some(content) = line.strip_prefix("# ") {
            writeln!(&mut output, "<h1>{content}</h1>").unwrap();
        } else if let Some(content) = line.strip_prefix("## ") {
            writeln!(&mut output, "<h2>{content}</h2>").unwrap();
        } else if let Some(content) = line.strip_prefix("### ") {
            writeln!(&mut output, "<h3>{content}</h3>").unwrap();
        } else if !line.is_empty() {
            // Basic paragraph handling
            writeln!(&mut output, "<p>{line}</p>").unwrap();
        }
    }

    // In case the input ends without closing the code block
    if in_code_block {
        output.push_str("</code></pre>\n");
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_headers() {
        let input = "# Header 1\n## Header 2\n### Header 3\n";
        let expected = "<h1>Header 1</h1>\n<h2>Header 2</h2>\n<h3>Header 3</h3>\n";
        assert_eq!(parse_markdown(input), expected);
    }

    #[test]
    fn test_paragraphs() {
        let input = "Hello world\n\nThis is a test.";
        let expected = "<p>Hello world</p>\n<p>This is a test.</p>\n";
        assert_eq!(parse_markdown(input), expected);
    }

    #[test]
    fn test_code_blocks() {
        let input = "```rust\nfn main() {}\n```\n";
        let expected = "<pre><code>\nfn main() {}\n</code></pre>\n";
        assert_eq!(parse_markdown(input), expected);
    }

    #[test]
    fn test_unclosed_code_block() {
        let input = "```\nsome code";
        let expected = "<pre><code>\nsome code\n</code></pre>\n";
        assert_eq!(parse_markdown(input), expected);
    }
}
