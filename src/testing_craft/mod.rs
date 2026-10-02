//! # Testing Craft
//!
//! Techniques for testing code that ordinary example-based unit tests miss.
//! Each module pairs a small, realistic subject-under-test with the testing
//! technique that is best at finding its bugs:
//!
//! | Module | Technique | Finds |
//! |--------|-----------|-------|
//! | [`sim_rng`] | Seeded determinism | Flaky tests (every "random" test replays from a seed) |
//! | [`property_testing`] | Properties, oracles, model-based state machines | Forgotten edge cases |
//! | [`fuzz_target`] | Coverage-guided fuzzing + a stable mini-fuzzer | Panics / overflows on hostile bytes |
//! | [`loom_model`] | Exhaustive interleaving model checking (Loom) | Data races, bad memory orderings |
//! | [`crash_injection`] | Simulated disk, crash points, failpoints | Lost/torn writes after power loss |
//!
//! ## Running the heavier tools
//!
//! ```bash
//! cargo test testing_craft                              # stable, dependency-free versions
//! cargo test --features testing-extras testing_craft    # + proptest suites
//! RUSTFLAGS="--cfg loom" cargo test --release --lib testing_craft::loom_model
//! cargo +nightly fuzz run frame_parse                    # from the repo root (needs cargo-fuzz)
//! ```
//!
//! Proofs prevent forbidden states; tests prevent forgotten behavior. These
//! tools sit in between: they *search* for the forgotten behavior for you.

pub mod crash_injection;
pub mod fuzz_target;
pub mod loom_model;
pub mod property_testing;
pub mod sim_rng;
