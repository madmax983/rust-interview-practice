//! # Parser Combinator Framework
//!
//! Implements a functional parser combinator framework from scratch.
//!
//! **Replaces Crates:** `nom`, `chumsky`, `combine`
//!
//! **Real-world Usage:**
//! - Parsing network protocols (e.g., HTTP, Redis RESP).
//! - Building compilers and interpreters (Lexing/Parsing ASTs).
//! - Reading custom configuration formats or DSLs.
//!
//! **Why build it yourself?**
//! Parser combinators can seem like "magic" because they heavily rely on closures returning closures and
//! complex type signatures. Building this teaches you how to leverage Rust's trait system to compose
//! simple operations (like matching a single character) into complex recursive descent parsers elegantly.
//! It also demonstrates strict lifetime management, as parsers continuously borrow from the original input string.
//!
//! Note: single canonical implementation; the brute/optimized/optimal progression does not apply.

// =========================================================================================
// Architecture
// =========================================================================================
//
// Core Concept:
// A Parser is simply a function that takes an input string and returns:
// - `Ok((Remaining_Input, Parsed_Output))` if successful.
// - `Err(ParseError)` if it fails.
//
// Combinators:
// Combinators are higher-order functions that take one or more parsers and return a *new* parser.
// For example, `P1.or(P2)` creates a parser that tries P1, and if it fails, tries P2.
//
// Invariants:
// 1. A parser must NEVER consume input if it returns an Error (this is usually handled naturally by returning the original slice).
// 2. Parsed string slices (`&str`) must be zero-copy, referencing the original input via lifetimes (`'a`).
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ Parse         │ O(N)        │ O(D)        │
// └───────────────┴─────────────┴─────────────┘
// N = input length, D = depth of the parser tree (stack space).
//
// Design Decisions:
// - **Zero-copy**: Output strings are strictly `&'a str` tied to the input lifetime.
// - **Boxed Trait Objects vs `impl Trait`**: We use explicit boxed closures (`Box<dyn Parser>`) for complex combinators
//   to avoid deeply nested opaque types that crash the compiler or are unreadable. In production (`nom`), complex
//   macros or zero-cost `impl Trait` chains are used.

/// Represents an error during parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError<'a> {
    pub location: &'a str,
    pub expected: String,
}

/// The result of a parser operation.
/// On success: returns the unparsed remainder of the input, and the extracted value.
pub type ParseResult<'a, Output> = Result<(&'a str, Output), ParseError<'a>>;

/// The core Parser trait.
/// Any type implementing this can parse a string slice.
pub trait Parser<'a, Output> {
    /// Runs the parser against `input`.
    ///
    /// # Errors
    ///
    /// Returns a `ParseError` if the input does not match what this parser expects.
    fn parse(&self, input: &'a str) -> ParseResult<'a, Output>;

    // =========================================================================
    // Combinator Methods
    // =========================================================================

    /// Maps the parsed output to a new type using a function.
    fn map<F, NewOutput>(self, map_fn: F) -> BoxedParser<'a, NewOutput>
    where
        Self: Sized + 'a,
        Output: 'a,
        NewOutput: 'a,
        F: Fn(Output) -> NewOutput + 'a,
    {
        BoxedParser::new(move |input| {
            self.parse(input)
                .map(|(next_input, result)| (next_input, map_fn(result)))
        })
    }

    /// Chains another parser that depends on the output of this parser.
    fn and_then<F, NextParser, NewOutput>(self, f: F) -> BoxedParser<'a, NewOutput>
    where
        Self: Sized + 'a,
        Output: 'a,
        NewOutput: 'a,
        NextParser: Parser<'a, NewOutput> + 'a,
        F: Fn(Output) -> NextParser + 'a,
    {
        BoxedParser::new(move |input| {
            let (next_input, result) = self.parse(input)?;
            f(result).parse(next_input)
        })
    }

    /// Attempts this parser; if it fails, attempts the `other` parser.
    fn or<P>(self, other: P) -> BoxedParser<'a, Output>
    where
        Self: Sized + 'a,
        Output: 'a,
        P: Parser<'a, Output> + 'a,
    {
        BoxedParser::new(move |input| self.parse(input).map_or_else(|_| other.parse(input), Ok))
    }
}

// RUST INSIGHT: Blanket implementation.
// Any closure matching the signature automatically becomes a `Parser`.
impl<'a, F, Output> Parser<'a, Output> for F
where
    F: Fn(&'a str) -> ParseResult<'a, Output>,
{
    fn parse(&self, input: &'a str) -> ParseResult<'a, Output> {
        self(input)
    }
}

/// A boxed parser, useful for returning parsers dynamically or composing them without giant types.
pub struct BoxedParser<'a, Output> {
    parser: Box<dyn Parser<'a, Output> + 'a>,
}

impl<'a, Output> BoxedParser<'a, Output> {
    pub fn new<P>(parser: P) -> Self
    where
        P: Parser<'a, Output> + 'a,
    {
        Self {
            parser: Box::new(parser),
        }
    }
}

impl<'a, Output> Parser<'a, Output> for BoxedParser<'a, Output> {
    fn parse(&self, input: &'a str) -> ParseResult<'a, Output> {
        self.parser.parse(input)
    }
}

// =========================================================================
// Fundamental Parsers
// =========================================================================

/// Matches an exact string literal.
#[must_use]
pub fn tag<'a>(expected: &'a str) -> impl Parser<'a, &'a str> {
    move |input: &'a str| {
        input.strip_prefix(expected).map_or_else(
            || {
                Err(ParseError {
                    location: input,
                    expected: format!("Expected '{expected}'"),
                })
            },
            // Return the remaining input, and the matched prefix
            |rest| Ok((rest, expected)),
        )
    }
}

/// Consumes characters as long as the predicate returns true.
pub fn take_while<'a, P>(predicate: P) -> impl Parser<'a, &'a str>
where
    P: Fn(char) -> bool,
{
    move |input: &'a str| {
        let chars = input.chars();
        let mut matched_len = 0;

        for c in chars {
            if predicate(c) {
                matched_len += c.len_utf8();
            } else {
                break;
            }
        }

        Ok((&input[matched_len..], &input[..matched_len]))
    }
}

/// Applies a parser 0 or more times, collecting the results into a `Vec`.
pub fn many0<'a, P, A>(parser: P) -> impl Parser<'a, Vec<A>>
where
    P: Parser<'a, A>,
{
    move |mut input: &'a str| {
        let mut results = Vec::new();

        // GOTCHA: Infinite Loop Prevention
        // If a parser matches empty input (like `take_while` can), `many0` would loop infinitely.
        // We must ensure the input length actually shrinks.
        loop {
            let initial_len = input.len();

            match parser.parse(input) {
                Ok((next_input, result)) => {
                    if next_input.len() == initial_len {
                        // Parser succeeded but didn't consume anything. Break to avoid infinite loop.
                        break;
                    }
                    input = next_input;
                    results.push(result);
                }
                Err(_) => break, // Stop collecting on first failure
            }
        }

        Ok((input, results))
    }
}

/// Sequence combinator: Runs P1, then P2, returning a tuple of their outputs.
pub fn pair<'a, P1, P2, R1, R2>(parser1: P1, parser2: P2) -> impl Parser<'a, (R1, R2)>
where
    P1: Parser<'a, R1>,
    P2: Parser<'a, R2>,
{
    move |input: &'a str| {
        let (next1, res1) = parser1.parse(input)?;
        let (next2, res2) = parser2.parse(next1)?;
        Ok((next2, (res1, res2)))
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `nom`: Uses heavily optimized zero-cost abstractions. Nom 7 mostly uses functions returning `impl Fn`, similar
//   to this implementation, but deeply optimizes byte slice (`&[u8]`) processing.
// - `chumsky`: Focuses on error recovery and excellent error messages for compiler development.
//
// Missing vs. Production:
// - **Zero-cost Abstractions**: We use `BoxedParser` internally in combinators (`map`, `or`) to keep return types
//   simple. A production crate returns complex nested types `Map<Or<P1, P2>, F>` to avoid heap allocations.
// - **Streaming Input**: Real parsers can handle streaming data (Incomplete results) needing more bytes.
// - **Detailed Errors**: Our `ParseError` is simplistic and doesn't trace the entire parser stack.
//
// Suggested next steps / extensions:
// 1. Implement a `map_err` combinator to improve error reporting context.
// 2. Build a JSON or CSV parser using these combinators.
//
// Benchmarking Note:
// To benchmark the framework against `nom`, use `criterion` to measure parse throughput.
// Construct a deeply nested parser (e.g., parsing a complex arithmetic expression) and parse a large string.
// Use `std::hint::black_box` on the parser input to prevent the compiler from optimizing away the parse execution.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tag() {
        let p = tag("hello");
        assert_eq!(p.parse("hello world"), Ok((" world", "hello")));
        assert!(p.parse("hi").is_err());
    }

    #[test]
    fn test_take_while() {
        let p = take_while(|c| c.is_ascii_digit());
        assert_eq!(p.parse("123abc456"), Ok(("abc456", "123")));
        assert_eq!(p.parse("abc"), Ok(("abc", ""))); // Returns empty string, doesn't fail
    }

    #[test]
    fn test_map() {
        let p = take_while(|c| c.is_ascii_digit()).map(|s: &str| s.parse::<i32>().unwrap_or(0));
        assert_eq!(p.parse("42xyz"), Ok(("xyz", 42)));
    }

    #[test]
    fn test_or() {
        let p = tag("foo").or(tag("bar"));
        assert_eq!(p.parse("foo test"), Ok((" test", "foo")));
        assert_eq!(p.parse("bar test"), Ok((" test", "bar")));
        assert!(p.parse("baz test").is_err());
    }

    #[test]
    fn test_pair() {
        let p = pair(tag("ID:"), take_while(|c| c.is_ascii_digit()));
        assert_eq!(p.parse("ID:999 "), Ok((" ", ("ID:", "999"))));
    }

    #[test]
    fn test_many0() {
        // Custom parser to require at least 1 character for a word
        // Need to specify lifetimes to satisfy Rust's closure type inference
        fn word(input: &str) -> ParseResult<'_, &str> {
            let (next, w) = take_while(char::is_alphabetic).parse(input)?;
            if w.is_empty() {
                Err(ParseError {
                    location: input,
                    expected: "at least one alphabetic character".to_string(),
                })
            } else {
                Ok((next, w))
            }
        }

        let whitespace = take_while(char::is_whitespace);

        // Parse a word, preceded by optional whitespace
        let ws_word = pair(whitespace, word).map(|(_, w)| w);

        let p = many0(ws_word);

        assert_eq!(
            p.parse(" hello   world rust "),
            Ok((" ", vec!["hello", "world", "rust"]))
        );
    }
}
