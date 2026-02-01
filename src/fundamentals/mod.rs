//! Fundamental Rust patterns for interview practice
//!
//! These modules contain common idioms to practice with gittype.

#![allow(clippy::missing_errors_doc)] // Examples don't need full docs
#![allow(clippy::needless_pass_by_value)] // Examples use common signatures

pub mod borrowing;
pub mod closures;
pub mod collections;
pub mod concurrency;
pub mod design_patterns;
pub mod error_handling;
pub mod error_types;
pub mod iterators;
pub mod macros;
pub mod numeric_ops;
pub mod pattern_matching;
pub mod performance;
pub mod smart_pointers;
pub mod strings;
pub mod testing;
pub mod types_and_traits;
pub mod unsafe_rust;

// Optional: requires 'async-parallel' feature
#[cfg(feature = "async-parallel")]
pub mod async_and_parallel;

// Optional: requires 'serde-patterns' feature
#[cfg(feature = "serde-patterns")]
pub mod serde_patterns;

// Optional: requires 'cli-patterns' feature
#[cfg(feature = "cli-patterns")]
pub mod cli_patterns;
