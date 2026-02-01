//! Fundamental Rust patterns for interview practice
//!
//! These modules contain common idioms to practice with gittype.

#![allow(clippy::missing_errors_doc)] // Examples don't need full docs
#![allow(clippy::needless_pass_by_value)] // Examples use common signatures

pub mod borrowing;
pub mod closures;
pub mod collections;
pub mod concurrency;
pub mod error_handling;
pub mod iterators;
pub mod numeric_ops;
pub mod pattern_matching;
pub mod smart_pointers;
pub mod strings;
pub mod types_and_traits;

// Optional: requires 'async-parallel' feature
#[cfg(feature = "async-parallel")]
pub mod async_and_parallel;
