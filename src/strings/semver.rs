//! # Semantic Versioning (SemVer) Parser and Evaluator
//!
//! **Implements:** A robust parser and evaluator for Semantic Versioning 2.0.0 rules.
//! **Replaces Crates:** `semver`
//! **Real-world Usage:**
//! - Package managers like `Cargo`, `npm`, and `bundler` for resolving dependencies.
//! - API version routing.
//! - Kubernetes API group versioning.
//!
//! **Why build it yourself?**
//! Semantic versioning looks like simple string splitting at first glance (`MAJOR.MINOR.PATCH`), but
//! the official specification (semver.org) defines strict rules around pre-release ordering (alphanumeric vs numeric),
//! build metadata (which is ignored for precedence), and validation (e.g., no leading zeros).
//! Implementing it from scratch forces you to understand structural parsing without relying on slow regex,
//! and teaches you how to implement custom, complex `PartialOrd` / `Ord` traits in Rust.
//!
//! # Architecture
//!
//! The SemVer format is defined as: `MAJOR.MINOR.PATCH[-PRERELEASE][+BUILD]`
//!
//! **Data Structure (`Version`):**
//! - `major`: `u64`
//! - `minor`: `u64`
//! - `patch`: `u64`
//! - `pre`: `Vec<Identifier>` (Enum handling numeric vs alphanumeric strings)
//! - `build`: `Vec<String>` (Build metadata is purely alphanumeric strings)
//!
//! **Invariants:**
//! 1. `MAJOR`, `MINOR`, and `PATCH` are non-negative integers without leading zeros.
//! 2. `PRERELEASE` identifiers must not be empty. Numeric identifiers cannot have leading zeros.
//! 3. Two versions are equal if their MAJOR, MINOR, PATCH, and PRERELEASE match. Build metadata is IGNORED for equality and ordering.
//! 4. A pre-release version has LOWER precedence than a normal version (e.g., `1.0.0-alpha` < `1.0.0`).
//!
//! **Complexity:**
//! | Operation | Time | Space |
//! | :--- | :--- | :--- |
//! | Parse (`FromStr`) | O(N) | O(N) |
//! | Compare (`Ord`) | O(min(N, M)) | O(1) |
//!
//! *N is the length of the version string.*

use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

/// Error type for parsing Semantic Versions.
#[derive(Debug, PartialEq, Eq)]
pub enum SemVerError {
    EmptyString,
    MissingMajor,
    MissingMinor,
    MissingPatch,
    InvalidCharacters,
    LeadingZero,
    EmptyIdentifier,
    NumberOverflow,
}

impl fmt::Display for SemVerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SemVerError::EmptyString => write!(f, "Version string is empty"),
            SemVerError::MissingMajor => write!(f, "Missing MAJOR version component"),
            SemVerError::MissingMinor => write!(f, "Missing MINOR version component"),
            SemVerError::MissingPatch => write!(f, "Missing PATCH version component"),
            SemVerError::InvalidCharacters => write!(f, "Invalid characters in version string"),
            SemVerError::LeadingZero => write!(f, "Numeric identifier contains leading zero"),
            SemVerError::EmptyIdentifier => write!(f, "Identifier is empty"),
            SemVerError::NumberOverflow => write!(f, "Numeric component is too large"),
        }
    }
}

impl std::error::Error for SemVerError {}

/// Represents a single identifier in the pre-release section.
/// According to the SemVer spec, numeric identifiers are compared numerically,
/// while alphanumeric identifiers are compared lexically.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Identifier {
    Numeric(u64),
    Alphanumeric(String),
}

// RUST INSIGHT: By implementing `Ord` on the enum directly, we can define the exact precedence rules required by the spec.
impl PartialOrd for Identifier {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Identifier {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            // Numeric identifiers are compared numerically
            (Identifier::Numeric(a), Identifier::Numeric(b)) => a.cmp(b),
            // Alphanumeric identifiers are compared lexically in ASCII sort order
            (Identifier::Alphanumeric(a), Identifier::Alphanumeric(b)) => a.cmp(b),
            // Numeric identifiers always have lower precedence than non-numeric identifiers
            (Identifier::Numeric(_), Identifier::Alphanumeric(_)) => Ordering::Less,
            (Identifier::Alphanumeric(_), Identifier::Numeric(_)) => Ordering::Greater,
        }
    }
}

impl fmt::Display for Identifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Identifier::Numeric(n) => write!(f, "{}", n),
            Identifier::Alphanumeric(s) => write!(f, "{}", s),
        }
    }
}

/// A Semantic Version structure.
#[derive(Debug, Clone)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    pub pre: Vec<Identifier>,
    pub build: Vec<String>,
}

// GOTCHA: We must implement `PartialEq` and `Eq` manually because the spec explicitly states
// that build metadata SHOULD BE IGNORED when determining version precedence or equality.
// If we derived `PartialEq`, it would compare the `build` field, violating the spec.
impl PartialEq for Version {
    fn eq(&self, other: &Self) -> bool {
        self.major == other.major
            && self.minor == other.minor
            && self.patch == other.patch
            && self.pre == other.pre
    }
}

impl Eq for Version {}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        // Compare major, then minor, then patch
        let core_cmp = self
            .major
            .cmp(&other.major)
            .then_with(|| self.minor.cmp(&other.minor))
            .then_with(|| self.patch.cmp(&other.patch));

        if core_cmp != Ordering::Equal {
            return core_cmp;
        }

        // When major, minor, and patch are equal, a pre-release version has
        // LOWER precedence than a normal version.
        match (self.pre.is_empty(), other.pre.is_empty()) {
            (true, true) => Ordering::Equal,
            (true, false) => Ordering::Greater, // self is normal, other is pre-release
            (false, true) => Ordering::Less,    // self is pre-release, other is normal
            (false, false) => {
                // Both are pre-release. Compare identifiers left to right.
                // A larger set of pre-release fields has a higher precedence than a smaller set,
                // if all of the preceding identifiers are equal.
                for (a, b) in self.pre.iter().zip(other.pre.iter()) {
                    match a.cmp(b) {
                        Ordering::Equal => continue,
                        other => return other,
                    }
                }
                self.pre.len().cmp(&other.pre.len())
            }
        }
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;

        if !self.pre.is_empty() {
            write!(f, "-")?;
            for (i, id) in self.pre.iter().enumerate() {
                if i > 0 {
                    write!(f, ".")?;
                }
                write!(f, "{}", id)?;
            }
        }

        if !self.build.is_empty() {
            write!(f, "+")?;
            for (i, meta) in self.build.iter().enumerate() {
                if i > 0 {
                    write!(f, ".")?;
                }
                write!(f, "{}", meta)?;
            }
        }

        Ok(())
    }
}

// RUST INSIGHT: `FromStr` is the idiomatic trait for parsing strings into objects.
// It allows users to write `"1.2.3".parse::<Version>()`.
impl FromStr for Version {
    type Err = SemVerError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Err(SemVerError::EmptyString);
        }

        let mut remainder = s;

        // Parse build metadata first from the back (it comes last)
        let build = if let Some(idx) = remainder.find('+') {
            let build_str = &remainder[idx + 1..];
            remainder = &remainder[..idx];
            parse_build(build_str)?
        } else {
            Vec::new()
        };

        // Parse pre-release metadata
        let pre = if let Some(idx) = remainder.find('-') {
            let pre_str = &remainder[idx + 1..];
            remainder = &remainder[..idx];
            parse_pre(pre_str)?
        } else {
            Vec::new()
        };

        // Parse MAJOR.MINOR.PATCH
        let mut parts = remainder.split('.');

        // PRODUCTION NOTE: In a highly optimized crate, `split('.')` might be replaced by manual byte-level scanning
        // using `&[u8]` indices to avoid the overhead of the `Split` iterator.
        let major = parse_numeric_component(parts.next().ok_or(SemVerError::MissingMajor)?)?;
        let minor = parse_numeric_component(parts.next().ok_or(SemVerError::MissingMinor)?)?;
        let patch = parse_numeric_component(parts.next().ok_or(SemVerError::MissingPatch)?)?;

        if parts.next().is_some() {
            return Err(SemVerError::InvalidCharacters); // Too many dot-separated components
        }

        Ok(Version {
            major,
            minor,
            patch,
            pre,
            build,
        })
    }
}

fn parse_numeric_component(s: &str) -> Result<u64, SemVerError> {
    if s.is_empty() {
        return Err(SemVerError::EmptyString);
    }

    // Check for leading zero unless the string is exactly "0"
    if s.len() > 1 && s.starts_with('0') {
        return Err(SemVerError::LeadingZero);
    }

    if !s.chars().all(|c| c.is_ascii_digit()) {
        return Err(SemVerError::InvalidCharacters);
    }

    s.parse::<u64>().map_err(|_| SemVerError::NumberOverflow)
}

fn parse_pre(s: &str) -> Result<Vec<Identifier>, SemVerError> {
    if s.is_empty() {
        return Err(SemVerError::EmptyIdentifier);
    }

    let mut identifiers = Vec::new();
    for part in s.split('.') {
        if part.is_empty() {
            return Err(SemVerError::EmptyIdentifier);
        }

        if !part.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return Err(SemVerError::InvalidCharacters);
        }

        // Determine if it's strictly numeric
        let is_numeric = part.chars().all(|c| c.is_ascii_digit());

        if is_numeric {
            if part.len() > 1 && part.starts_with('0') {
                return Err(SemVerError::LeadingZero);
            }
            let n = part
                .parse::<u64>()
                .map_err(|_| SemVerError::NumberOverflow)?;
            identifiers.push(Identifier::Numeric(n));
        } else {
            identifiers.push(Identifier::Alphanumeric(part.to_string()));
        }
    }

    Ok(identifiers)
}

fn parse_build(s: &str) -> Result<Vec<String>, SemVerError> {
    if s.is_empty() {
        return Err(SemVerError::EmptyIdentifier);
    }

    let mut metadata = Vec::new();
    for part in s.split('.') {
        if part.is_empty() {
            return Err(SemVerError::EmptyIdentifier);
        }
        if !part.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return Err(SemVerError::InvalidCharacters);
        }
        metadata.push(part.to_string());
    }
    Ok(metadata)
}

// ============================================================================
// Footer
// ============================================================================
// Comparison to Canonical Crates:
// - The canonical `semver` crate implements the exact specification above, but also handles
//   version requirements (e.g., parsing "^1.0.0" and evaluating if a Version satisfies it).
// - The canonical crate is highly optimized for performance and zero-copy where possible.
//
// Missing vs. Production:
// - We use `String` allocations for alphanumeric identifiers and build metadata. A production
//   crate might use a small-string optimization or string slices (`&'a str`) to avoid allocations
//   during parsing, though this ties the `Version` to the lifetime of the input string.
// - We do not support Version Requirements (`VersionReq`) like `~1.2.3` or `>= 1.0.0`.
//
// Next Steps:
// 1. Implement `VersionReq` to parse range operators and evaluate compatibility.
// 2. Optimize allocation strategy by using `Cow<'a, str>` for identifiers.
//
// Benchmarking Notes:
// To benchmark this implementation against the canonical `semver` crate, you would use `criterion`:
// ```rust
// pub fn criterion_benchmark(c: &mut Criterion) {
//     c.bench_function("parse semver custom", |b| b.iter(|| std::hint::black_box("1.2.3-alpha.1+build".parse::<Version>())));
//     c.bench_function("parse semver crate", |b| b.iter(|| std::hint::black_box(semver::Version::parse("1.2.3-alpha.1+build"))));
// }
// ```

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_parsing() {
        let v: Version = "1.2.3".parse().unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
        assert!(v.pre.is_empty());
        assert!(v.build.is_empty());

        let v: Version = "10.20.30-rc.1+build.123".parse().unwrap();
        assert_eq!(v.major, 10);
        assert_eq!(v.minor, 20);
        assert_eq!(v.patch, 30);
        assert_eq!(
            v.pre,
            vec![
                Identifier::Alphanumeric("rc".to_string()),
                Identifier::Numeric(1)
            ]
        );
        assert_eq!(v.build, vec!["build".to_string(), "123".to_string()]);
    }

    #[test]
    fn test_invalid_parsing() {
        assert_eq!("".parse::<Version>(), Err(SemVerError::EmptyString));
        assert_eq!("1".parse::<Version>(), Err(SemVerError::MissingMinor));
        assert_eq!("1.2".parse::<Version>(), Err(SemVerError::MissingPatch));
        assert_eq!(
            "1.2.3.4".parse::<Version>(),
            Err(SemVerError::InvalidCharacters)
        );
        assert_eq!("01.2.3".parse::<Version>(), Err(SemVerError::LeadingZero));
        assert_eq!("1.02.3".parse::<Version>(), Err(SemVerError::LeadingZero));
        assert_eq!("1.2.03".parse::<Version>(), Err(SemVerError::LeadingZero));
        assert_eq!(
            "1.2.3-".parse::<Version>(),
            Err(SemVerError::EmptyIdentifier)
        );
        assert_eq!(
            "1.2.3-a..b".parse::<Version>(),
            Err(SemVerError::EmptyIdentifier)
        );
        assert_eq!(
            "1.2.3+a..b".parse::<Version>(),
            Err(SemVerError::EmptyIdentifier)
        );
        assert_eq!("1.2.3-01".parse::<Version>(), Err(SemVerError::LeadingZero));
        assert_eq!(
            "1.2.3-a$b".parse::<Version>(),
            Err(SemVerError::InvalidCharacters)
        );
    }

    #[test]
    fn test_equality_ignores_build_metadata() {
        let v1: Version = "1.2.3+build.1".parse().unwrap();
        let v2: Version = "1.2.3+other.build".parse().unwrap();
        let v3: Version = "1.2.3".parse().unwrap();

        assert_eq!(v1, v2);
        assert_eq!(v1, v3);
        assert_eq!(v2, v3);
    }

    #[test]
    fn test_precedence_ordering() {
        // Spec precedence example
        let ordered = [
            "1.0.0-alpha",
            "1.0.0-alpha.1",
            "1.0.0-alpha.beta",
            "1.0.0-beta",
            "1.0.0-beta.2",
            "1.0.0-beta.11",
            "1.0.0-rc.1",
            "1.0.0",
        ];

        let mut parsed: Vec<Version> = ordered.iter().map(|s| s.parse().unwrap()).collect();
        let parsed_clone = parsed.clone();

        // Sorting should maintain this exact order
        parsed.sort();

        for (i, v) in parsed.iter().enumerate() {
            assert_eq!(v, &parsed_clone[i], "Ordering failed at index {}", i);
        }

        // Additional edge cases
        let a: Version = "1.0.0".parse().unwrap();
        let b: Version = "2.0.0".parse().unwrap();
        assert!(a < b);

        let a: Version = "1.0.0".parse().unwrap();
        let b: Version = "1.1.0".parse().unwrap();
        assert!(a < b);

        let a: Version = "1.0.0".parse().unwrap();
        let b: Version = "1.0.1".parse().unwrap();
        assert!(a < b);
    }

    #[test]
    fn test_display() {
        let cases = vec![
            "1.2.3",
            "1.2.3-alpha.1",
            "1.2.3+build.xyz",
            "1.2.3-beta.2+build.metadata.88",
        ];

        for s in cases {
            let v: Version = s.parse().unwrap();
            assert_eq!(v.to_string(), s);
        }
    }
}
