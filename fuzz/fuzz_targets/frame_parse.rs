//! Raw-bytes fuzz target for the frame parser.
//!
//! `cargo +nightly fuzz run frame_parse` (from the repository root).
//! Any panic — including a violated round-trip assertion — is a finding.

#![no_main]

use libfuzzer_sys::fuzz_target;
use rust_interview_practice::testing_craft::fuzz_target::fuzz_parse;

fuzz_target!(|data: &[u8]| {
    fuzz_parse(data);
});
