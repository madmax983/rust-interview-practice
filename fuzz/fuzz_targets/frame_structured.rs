//! Structure-aware fuzz target: fuzzer bytes are decisions that build a valid
//! frame, so every execution exercises the encoder and the full parser.
//!
//! `cargo +nightly fuzz run frame_structured` (from the repository root).

#![no_main]

use libfuzzer_sys::fuzz_target;
use rust_interview_practice::testing_craft::fuzz_target::fuzz_structured;

fuzz_target!(|data: &[u8]| {
    fuzz_structured(data);
});
