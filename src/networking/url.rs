//! # URL Parser Implementation
//!
//! Implements RFC 3986 URL parsing and serialization.
//!
//! **Replaces Crates:** `url`
//!
//! **Real-world Usage:**
//! - Web browsers parsing user input in the address bar.
//! - HTTP clients (like `reqwest` or `curl`) constructing requests.
//! - Routers in web frameworks matching request paths.
//!
//! **Why build it yourself?**
//! URLs are notoriously complex due to edge cases, encoding rules, and historical baggage.
//! Building a parser teaches you about state machines for string processing, the difference
//! between a URI and a URL, and how to safely handle user-controlled input (like percent-decoding).

use std::fmt;

// =========================================================================================
// Architecture
// =========================================================================================
//
// URL Structure (RFC 3986):
//      foo://example.com:8042/over/there?name=ferret#nose
//      \_/   \______________/\_________/ \_________/ \__/
//       |           |            |            |        |
//    scheme     authority       path        query   fragment
//
// Authority Structure:
//      user:password@example.com:8042
//      \___________/ \_________/ \__/
//            |            |        |
//        userinfo       host      port
//
// Invariants:
// 1. `scheme` is required and must only contain valid characters (alpha, digit, '+', '-', '.').
// 2. `path` always exists, even if empty.
// 3. Modifying parts of the URL (like `path` or `query`) correctly updates the serialized string representation.
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ parse         │ O(N)        │ O(N)        │
// │ format        │ O(N)        │ O(N)        │
// │ encode/decode │ O(N)        │ O(N)        │
// └───────────────┴─────────────┴─────────────┘
//
// Design Decisions:
// - Strings are owned (`String`) for simplicity when mutating. A zero-allocation parser
//   would use `&str` and lifetimes, but mutating it would require allocation anyway.
// - `percent_encode` and `percent_decode` are implemented manually.
//
// Benchmarking Note:
// To benchmark parsing, use `criterion` to measure `Url::parse(input)`. Provide a mix of
// simple URLs, URLs with extensive percent-encoding, and invalid URLs to evaluate both the
// happy and error paths.

/// Trait defining the core interface for a URL parser.
pub trait UrlParser: Sized {
    /// Parses a URL string into a URL object.
    fn parse(input: &str) -> Result<Self, ParseError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Url {
    pub scheme: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub path: String,
    pub query: Option<String>,
    pub fragment: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    MissingScheme,
    InvalidScheme,
    InvalidPort,
    InvalidEncoding,
}

impl UrlParser for Url {
    fn parse(input: &str) -> Result<Self, ParseError> {
        let mut url = Url {
            scheme: String::new(),
            username: None,
            password: None,
            host: None,
            port: None,
            path: String::new(),
            query: None,
            fragment: None,
        };

        let mut current = input;

        // 1. Extract Fragment
        if let Some(idx) = current.find('#') {
            url.fragment = Some(current[idx + 1..].to_string());
            current = &current[..idx];
        }

        // 2. Extract Query
        if let Some(idx) = current.find('?') {
            url.query = Some(current[idx + 1..].to_string());
            current = &current[..idx];
        }

        // 3. Extract Scheme
        let scheme_end = current.find(':').ok_or(ParseError::MissingScheme)?;
        let scheme = &current[..scheme_end];
        if scheme.is_empty() {
            return Err(ParseError::MissingScheme);
        }
        if !scheme.chars().next().unwrap().is_ascii_alphabetic()
            || !scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.')
        {
            return Err(ParseError::InvalidScheme);
        }
        url.scheme = scheme.to_lowercase();
        current = &current[scheme_end + 1..];

        // 4. Extract Authority (if present)
        if current.starts_with("//") {
            current = &current[2..];
            let authority_end = current.find('/').unwrap_or(current.len());
            let authority = &current[..authority_end];
            current = &current[authority_end..];

            Self::parse_authority(&mut url, authority)?;
        }

        // 5. Extract Path
        // RUST INSIGHT: The remainder is the path. We don't eagerly decode it here
        // because the encoded form might be needed to differentiate separators.
        url.path = current.to_string();

        Ok(url)
    }
}

impl Url {
    fn parse_authority(url: &mut Url, authority: &str) -> Result<(), ParseError> {
        let mut host_port = authority;

        // userinfo
        if let Some(at_idx) = authority.find('@') {
            let userinfo = &authority[..at_idx];
            host_port = &authority[at_idx + 1..];

            if let Some(colon_idx) = userinfo.find(':') {
                url.username = Some(percent_decode(&userinfo[..colon_idx])?);
                url.password = Some(percent_decode(&userinfo[colon_idx + 1..])?);
            } else {
                url.username = Some(percent_decode(userinfo)?);
            }
        }

        // port
        if let Some(colon_idx) = host_port.rfind(':') {
            // Check if this colon is part of an IPv6 address (which would be inside brackets)
            // For simplicity, we assume basic IPv4/DNS formats here, but we should handle
            // brackets correctly in a full implementation.
            let port_str = &host_port[colon_idx + 1..];
            if !port_str.is_empty() {
                url.port = Some(
                    port_str
                        .parse::<u16>()
                        .map_err(|_| ParseError::InvalidPort)?,
                );
            }
            host_port = &host_port[..colon_idx];
        }

        // host
        if !host_port.is_empty() {
            url.host = Some(percent_decode(host_port)?);
        }

        Ok(())
    }
}

impl fmt::Display for Url {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:", self.scheme)?;

        if self.host.is_some()
            || self.username.is_some()
            || self.password.is_some()
            || self.port.is_some()
        {
            write!(f, "//")?;

            if let Some(user) = &self.username {
                write!(f, "{}", percent_encode(user))?;
                if let Some(pass) = &self.password {
                    write!(f, ":{}", percent_encode(pass))?;
                }
                write!(f, "@")?;
            }

            if let Some(host) = &self.host {
                // Technically, we should encode only non-host characters, but
                // this simple implementation encodes everything not safe.
                write!(f, "{}", host)?;
            }

            if let Some(port) = self.port {
                write!(f, ":{}", port)?;
            }
        }

        write!(f, "{}", self.path)?;

        if let Some(query) = &self.query {
            write!(f, "?{}", query)?;
        }

        if let Some(fragment) = &self.fragment {
            write!(f, "#{}", fragment)?;
        }

        Ok(())
    }
}

/// Decodes a percent-encoded string.
pub fn percent_decode(input: &str) -> Result<String, ParseError> {
    let mut result = Vec::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;

    // GOTCHA: Percent decoding can fail if the string contains a `%` not followed
    // by two valid hex digits. It is critical to bounds-check and validate.
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if i + 2 < bytes.len() {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3])
                    .map_err(|_| ParseError::InvalidEncoding)?;
                let byte = u8::from_str_radix(hex, 16).map_err(|_| ParseError::InvalidEncoding)?;
                result.push(byte);
                i += 3;
            } else {
                return Err(ParseError::InvalidEncoding);
            }
        } else {
            result.push(bytes[i]);
            i += 1;
        }
    }

    String::from_utf8(result).map_err(|_| ParseError::InvalidEncoding)
}

/// Percent-encodes a string.
pub fn percent_encode(input: &str) -> String {
    // PRODUCTION NOTE: A production encoder uses a static lookup table (`[bool; 256]`)
    // to determine if a byte needs encoding, which is significantly faster than calling multiple
    // functions like `is_ascii_alphanumeric()`.
    let mut result = String::with_capacity(input.len());
    for byte in input.bytes() {
        if byte.is_ascii_alphanumeric()
            || byte == b'-'
            || byte == b'.'
            || byte == b'_'
            || byte == b'~'
        {
            result.push(byte as char);
        } else {
            result.push_str(&format!("%{:02X}", byte));
        }
    }
    result
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `url`: The official WHATWG URL standard implementation for Rust. It handles thousands of edge
//   cases, IDNA (internationalized domain names), and base URL resolution.
//
// Missing vs. Production:
// - **WHATWG Compliance**: Real web browsers use the WHATWG URL standard, not just RFC 3986.
//   WHATWG specifies exact error recovery and normalization rules.
// - **IPv6 Parsing**: We don't properly handle `[::1]` IPv6 bracket notation.
// - **IDNA**: No Punycode encoding/decoding for non-ASCII domain names.
// - **Base URLs**: We don't implement `.join()` to resolve relative URLs against a base URL.
//
// Next Steps:
// 1. Add `join(relative_path)` to handle relative URL resolution.
// 2. Implement robust IPv6 bracket parsing.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple() {
        let url = Url::parse("http://example.com/").unwrap();
        assert_eq!(url.scheme, "http");
        assert_eq!(url.host.as_deref(), Some("example.com"));
        assert_eq!(url.path, "/");
        assert_eq!(url.port, None);
    }

    #[test]
    fn test_parse_full() {
        let url =
            Url::parse("https://user:pass@example.com:8080/path/to/resource?query=1#frag").unwrap();
        assert_eq!(url.scheme, "https");
        assert_eq!(url.username.as_deref(), Some("user"));
        assert_eq!(url.password.as_deref(), Some("pass"));
        assert_eq!(url.host.as_deref(), Some("example.com"));
        assert_eq!(url.port, Some(8080));
        assert_eq!(url.path, "/path/to/resource");
        assert_eq!(url.query.as_deref(), Some("query=1"));
        assert_eq!(url.fragment.as_deref(), Some("frag"));
    }

    #[test]
    fn test_parse_no_authority() {
        let url = Url::parse("mailto:john.doe@example.com").unwrap();
        assert_eq!(url.scheme, "mailto");
        assert_eq!(url.host, None);
        assert_eq!(url.path, "john.doe@example.com");
    }

    #[test]
    fn test_percent_decoding() {
        assert_eq!(percent_decode("hello%20world").unwrap(), "hello world");
        assert_eq!(
            percent_decode("user%40email.com").unwrap(),
            "user@email.com"
        );
    }

    #[test]
    fn test_percent_encoding() {
        assert_eq!(percent_encode("hello world"), "hello%20world");
        assert_eq!(percent_encode("user@email.com"), "user%40email.com");
    }

    #[test]
    fn test_url_display() {
        let input = "https://user:pass@example.com:8080/path?query#frag";
        let url = Url::parse(input).unwrap();
        assert_eq!(url.to_string(), input);
    }

    #[test]
    fn test_invalid_scheme() {
        assert_eq!(
            Url::parse("123://example.com"),
            Err(ParseError::InvalidScheme)
        );
        assert_eq!(Url::parse("://example.com"), Err(ParseError::MissingScheme));
    }
}
