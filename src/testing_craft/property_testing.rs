//! # Property-Based Testing
//!
//! Example-based tests check the inputs you thought of. Property tests state a
//! rule that must hold for *every* input, generate hundreds of inputs, and —
//! when one fails — *shrink* it to the smallest input that still fails.
//!
//! Four property shapes cover most real code:
//!
//! 1. **Round-trip** — `decode(encode(x)) == x`.
//! 2. **Invariant** — the output is always well-formed (sorted, no empty runs, ...).
//! 3. **Oracle** — the fast implementation agrees with a slow, obviously-correct one.
//! 4. **Model-based (stateful)** — a random *sequence of operations* applied to
//!    the real structure and to a trivial model (`BTreeSet`) gives the same answers.
//!
//! This module contains the subjects under test ([`rle_encode`], [`SortedSet`]),
//! a deliberately buggy encoder ([`rle_encode_u8_counts_buggy`]), and a tiny
//! dependency-free property runner with shrinking ([`check`]). The same
//! properties are also written with `proptest` in the `testing-extras` suite.
//!
//! ```
//! use rust_interview_practice::testing_craft::property_testing::{check, rle_decode, rle_encode, Config};
//!
//! let result = check(
//!     Config::default(),
//!     |rng| {
//!         let len = rng.below(32);
//!         rng.bytes(len)
//!     },
//!     |input: &Vec<u8>| {
//!         let decoded = rle_decode(&rle_encode(input));
//!         if &decoded == input { Ok(()) } else { Err(format!("got {decoded:?}")) }
//!     },
//! );
//! assert!(result.is_ok());
//! ```

use std::fmt::Debug;

use super::sim_rng::SimRng;

// ============================================================================
// Subject under test #1: run-length encoding
// ============================================================================

/// One run of identical bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Run {
    /// The repeated byte.
    pub byte: u8,
    /// How many times it repeats. Always `>= 1` in well-formed output.
    pub len: usize,
}

/// Run-length encodes `input`.
///
/// Time: O(n) - single pass. Space: O(r) - one entry per run.
#[must_use]
pub fn rle_encode(input: &[u8]) -> Vec<Run> {
    let mut runs: Vec<Run> = Vec::new();
    for &byte in input {
        match runs.last_mut() {
            Some(run) if run.byte == byte => run.len += 1,
            _ => runs.push(Run { byte, len: 1 }),
        }
    }
    runs
}

/// Expands runs back into bytes.
///
/// Time: O(n) - n is the decoded length. Space: O(n).
#[must_use]
pub fn rle_decode(runs: &[Run]) -> Vec<u8> {
    let mut out = Vec::with_capacity(runs.iter().map(|r| r.len).sum());
    for run in runs {
        out.extend(std::iter::repeat_n(run.byte, run.len));
    }
    out
}

/// Checks the structural invariant of encoder output: no empty runs and no two
/// adjacent runs with the same byte (they should have been merged).
#[must_use]
pub fn rle_is_canonical(runs: &[Run]) -> bool {
    runs.iter().all(|r| r.len > 0) && runs.windows(2).all(|w| w[0].byte != w[1].byte)
}

/// BUGGY on purpose: stores run lengths in a `u8`.
///
/// A run of 256 identical bytes wraps around to a length of 0. Every hand-written example with short
/// runs passes; a property test with a run-biased generator finds it at once.
#[must_use]
pub fn rle_encode_u8_counts_buggy(input: &[u8]) -> Vec<Run> {
    let mut runs: Vec<(u8, u8)> = Vec::new();
    for &byte in input {
        match runs.last_mut() {
            Some((b, count)) if *b == byte => *count = count.wrapping_add(1),
            _ => runs.push((byte, 1)),
        }
    }
    runs.into_iter()
        .map(|(byte, count)| Run {
            byte,
            len: usize::from(count),
        })
        .collect()
}

/// Generator biased toward long runs. Uniform random bytes almost never repeat
/// 256 times in a row; good generators aim at the interesting region.
pub fn gen_runny_bytes(rng: &mut SimRng) -> Vec<u8> {
    let run_count = rng.below(6);
    let mut out = Vec::new();
    for _ in 0..run_count {
        let byte = rng.byte();
        let len = 1 + rng.below(300);
        out.extend(std::iter::repeat_n(byte, len));
    }
    out
}

// ============================================================================
// Subject under test #2: a sorted-vector set (model-based testing target)
// ============================================================================

/// A set stored as a sorted, deduplicated `Vec`. Binary search for lookups,
/// shifting inserts/removes. Cache-friendly for small sets.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SortedSet<T: Ord> {
    items: Vec<T>,
}

impl<T: Ord> SortedSet<T> {
    /// Creates an empty set.
    #[must_use]
    pub const fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Inserts `value`; returns `true` if it was not already present.
    ///
    /// Time: O(n) - binary search + shift.
    pub fn insert(&mut self, value: T) -> bool {
        match self.items.binary_search(&value) {
            Ok(_) => false,
            Err(pos) => {
                self.items.insert(pos, value);
                true
            }
        }
    }

    /// Removes `value`; returns `true` if it was present.
    ///
    /// Time: O(n) - binary search + shift.
    pub fn remove(&mut self, value: &T) -> bool {
        match self.items.binary_search(value) {
            Ok(pos) => {
                self.items.remove(pos);
                true
            }
            Err(_) => false,
        }
    }

    /// Returns `true` if `value` is in the set. Time: O(log n).
    #[must_use]
    pub fn contains(&self, value: &T) -> bool {
        self.items.binary_search(value).is_ok()
    }

    /// Number of elements.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.items.len()
    }

    /// `true` when the set has no elements.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Iterates in ascending order.
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.items.iter()
    }
}

impl<'a, T: Ord> IntoIterator for &'a SortedSet<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// One step of a model-based test.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetOp {
    /// `insert(v)`
    Insert(u8),
    /// `remove(v)`
    Remove(u8),
    /// `contains(v)`
    Contains(u8),
}

/// Generates an operation over a *small* value domain (0..16) so inserts,
/// removes and lookups actually collide.
pub const fn gen_set_op(rng: &mut SimRng) -> SetOp {
    #[allow(clippy::cast_possible_truncation)] // below(16) < 256
    let value = rng.below(16) as u8;
    match rng.below(3) {
        0 => SetOp::Insert(value),
        1 => SetOp::Remove(value),
        _ => SetOp::Contains(value),
    }
}

/// Applies `ops` to both [`SortedSet`] and the `BTreeSet` model, comparing every
/// return value and the final contents.
///
/// # Errors
///
/// Returns a description of the first divergence between implementation and model.
pub fn run_set_model(ops: &[SetOp]) -> Result<(), String> {
    let mut real = SortedSet::new();
    let mut model = std::collections::BTreeSet::new();
    for (step, op) in ops.iter().enumerate() {
        let (got, want) = match *op {
            SetOp::Insert(v) => (real.insert(v), model.insert(v)),
            SetOp::Remove(v) => (real.remove(&v), model.remove(&v)),
            SetOp::Contains(v) => (real.contains(&v), model.contains(&v)),
        };
        if got != want {
            return Err(format!(
                "step {step}: {op:?} returned {got}, model said {want}"
            ));
        }
        if real.len() != model.len() {
            return Err(format!(
                "step {step}: len {} vs model {}",
                real.len(),
                model.len()
            ));
        }
    }
    if !real.iter().eq(model.iter()) {
        return Err("final contents differ from model".to_string());
    }
    Ok(())
}

// ============================================================================
// A dependency-free property runner with shrinking
// ============================================================================

/// Types that know how to propose *simpler* versions of themselves.
pub trait Shrink: Sized {
    /// Candidates strictly simpler than `self`, most aggressive first.
    fn shrink(&self) -> Vec<Self>;
}

impl Shrink for u8 {
    fn shrink(&self) -> Vec<Self> {
        match *self {
            0 => vec![],
            n => vec![0, n / 2, n - 1],
        }
    }
}

impl Shrink for SetOp {
    fn shrink(&self) -> Vec<Self> {
        match *self {
            Self::Insert(v) => v.shrink().into_iter().map(Self::Insert).collect(),
            Self::Remove(v) => v.shrink().into_iter().map(Self::Remove).collect(),
            Self::Contains(v) => v.shrink().into_iter().map(Self::Contains).collect(),
        }
    }
}

impl<T: Shrink + Clone> Shrink for Vec<T> {
    fn shrink(&self) -> Vec<Self> {
        let mut out = Vec::new();
        if self.is_empty() {
            return out;
        }
        // 1. Drop whole halves (fast progress on big inputs). Skip for len 1:
        //    `self[0..]` is `self`, and a candidate equal to its parent loops forever.
        if self.len() > 1 {
            let mid = self.len() / 2;
            out.push(self[mid..].to_vec());
            out.push(self[..mid].to_vec());
        }
        // 2. Drop single elements.
        for i in 0..self.len() {
            let mut smaller = self.clone();
            smaller.remove(i);
            out.push(smaller);
        }
        // 3. Simplify single elements in place.
        for (i, item) in self.iter().enumerate() {
            for simpler in item.shrink() {
                let mut candidate = self.clone();
                candidate[i] = simpler;
                out.push(candidate);
            }
        }
        out
    }
}

/// Runner settings.
#[derive(Debug, Clone, Copy)]
pub struct Config {
    /// Number of generated cases.
    pub cases: usize,
    /// Seed for case generation — print it on failure, replay it to debug.
    pub seed: u64,
    /// Upper bound on accepted shrink steps (guards against pathological shrinkers).
    pub max_shrink_steps: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            cases: 256,
            seed: 0x5EED,
            max_shrink_steps: 10_000,
        }
    }
}

/// A failing input, before and after shrinking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Counterexample<T> {
    /// Seed that reproduces the run.
    pub seed: u64,
    /// Index of the first failing case.
    pub case: usize,
    /// The input as generated.
    pub original: T,
    /// The minimal failing input found by shrinking.
    pub shrunk: T,
    /// The property's failure message for `shrunk`.
    pub message: String,
}

/// Runs `property` against `config.cases` generated inputs. On the first
/// failure, greedily shrinks the input: take the first simpler candidate that
/// still fails, repeat until no candidate fails.
///
/// # Errors
///
/// Returns the (shrunk) [`Counterexample`] if any case fails.
pub fn check<T, G, P>(config: Config, mut generate: G, property: P) -> Result<(), Counterexample<T>>
where
    T: Shrink + Clone + Debug,
    G: FnMut(&mut SimRng) -> T,
    P: Fn(&T) -> Result<(), String>,
{
    let mut rng = SimRng::new(config.seed);
    for case in 0..config.cases {
        let input = generate(&mut rng);
        if let Err(message) = property(&input) {
            let (shrunk, message) =
                shrink_failure(&input, message, &property, config.max_shrink_steps);
            return Err(Counterexample {
                seed: config.seed,
                case,
                original: input,
                shrunk,
                message,
            });
        }
    }
    Ok(())
}

fn shrink_failure<T, P>(input: &T, message: String, property: &P, max_steps: usize) -> (T, String)
where
    T: Shrink + Clone,
    P: Fn(&T) -> Result<(), String>,
{
    let mut current = input.clone();
    let mut current_message = message;
    let mut steps = 0;
    'outer: while steps < max_steps {
        for candidate in current.shrink() {
            if let Err(msg) = property(&candidate) {
                current = candidate;
                current_message = msg;
                steps += 1;
                continue 'outer;
            }
        }
        break;
    }
    (current, current_message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(encode: fn(&[u8]) -> Vec<Run>) -> impl Fn(&Vec<u8>) -> Result<(), String> {
        move |input: &Vec<u8>| {
            let decoded = rle_decode(&encode(input));
            if &decoded == input {
                Ok(())
            } else {
                Err(format!(
                    "len {} decoded to len {}",
                    input.len(),
                    decoded.len()
                ))
            }
        }
    }

    // ---- example-based anchors ----

    #[test]
    fn test_rle_examples() {
        assert_eq!(rle_encode(b""), vec![]);
        assert_eq!(
            rle_encode(b"aaab"),
            vec![Run { byte: b'a', len: 3 }, Run { byte: b'b', len: 1 }]
        );
        assert_eq!(rle_decode(&rle_encode(b"hello")), b"hello");
    }

    #[test]
    fn test_buggy_encoder_passes_short_examples() {
        // This is exactly why example tests are not enough.
        for example in [&b"aaab"[..], b"", b"xyz", &[7; 255]] {
            assert_eq!(rle_decode(&rle_encode_u8_counts_buggy(example)), example);
        }
    }

    #[test]
    fn test_sorted_set_examples() {
        let mut set = SortedSet::new();
        assert!(set.is_empty());
        assert!(set.insert(3));
        assert!(set.insert(1));
        assert!(!set.insert(3));
        assert!(set.contains(&1));
        assert!(set.remove(&1));
        assert!(!set.remove(&1));
        assert_eq!(set.iter().copied().collect::<Vec<_>>(), vec![3]);
        assert_eq!(set.len(), 1);
    }

    // ---- properties with the dependency-free runner ----

    #[test]
    fn prop_rle_roundtrip() {
        let result = check(Config::default(), gen_runny_bytes, roundtrip(rle_encode));
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn prop_rle_output_is_canonical() {
        let result = check(Config::default(), gen_runny_bytes, |input: &Vec<u8>| {
            if rle_is_canonical(&rle_encode(input)) {
                Ok(())
            } else {
                Err("non-canonical runs".to_string())
            }
        });
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn prop_buggy_rle_is_caught_and_shrunk() {
        let failure = check(
            Config::default(),
            gen_runny_bytes,
            roundtrip(rle_encode_u8_counts_buggy),
        )
        .expect_err("the u8-count bug must be found");
        // Shrinking strips everything except one run of exactly 256 equal bytes.
        assert_eq!(failure.shrunk.len(), 256, "{:?}", failure.message);
        assert!(failure.shrunk.windows(2).all(|w| w[0] == w[1]));
        assert!(failure.original.len() >= failure.shrunk.len());
    }

    #[test]
    fn prop_sorted_set_matches_btreeset_model() {
        let result = check(
            Config::default(),
            |rng| {
                (0..rng.below(64))
                    .map(|_| gen_set_op(rng))
                    .collect::<Vec<_>>()
            },
            |ops: &Vec<SetOp>| run_set_model(ops),
        );
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn test_shrinker_finds_minimal_vec() {
        // Property: "no element is >= 10". Minimal counterexample is [10].
        let failure = check(
            Config::default(),
            |rng| {
                let len = 1 + rng.below(20);
                rng.bytes(len)
            },
            |v: &Vec<u8>| {
                if v.iter().all(|&b| b < 10) {
                    Ok(())
                } else {
                    Err("found big".to_string())
                }
            },
        )
        .expect_err("random bytes exceed 10");
        assert_eq!(failure.shrunk, vec![10]);
    }

    #[test]
    fn test_same_seed_same_counterexample() {
        let run = || {
            check(
                Config::default(),
                gen_runny_bytes,
                roundtrip(rle_encode_u8_counts_buggy),
            )
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn test_shrink_step_limit_is_respected() {
        let config = Config {
            max_shrink_steps: 0,
            ..Config::default()
        };
        let failure = check(
            config,
            gen_runny_bytes,
            roundtrip(rle_encode_u8_counts_buggy),
        )
        .expect_err("bug is found");
        assert_eq!(failure.original, failure.shrunk);
    }

    #[test]
    fn test_shrink_candidates_are_strictly_smaller_or_simpler() {
        // Regression: a 1-element vec used to propose itself as a candidate.
        let one = vec![5u8];
        assert!(one.shrink().iter().all(|c| c != &one));
        let many = vec![1u8, 2, 3];
        assert!(many.shrink().iter().all(|c| c != &many));
    }

    #[test]
    fn test_shrink_candidates() {
        assert_eq!(0u8.shrink(), Vec::<u8>::new());
        assert_eq!(9u8.shrink(), vec![0, 4, 8]);
        assert_eq!(Vec::<u8>::new().shrink(), Vec::<Vec<u8>>::new());
        assert_eq!(SetOp::Insert(0).shrink(), Vec::<SetOp>::new());
        assert_eq!(
            SetOp::Remove(2).shrink(),
            vec![SetOp::Remove(0), SetOp::Remove(1), SetOp::Remove(1)]
        );
    }

    #[test]
    fn test_model_detects_divergence() {
        // Sanity check of the harness itself: a model check over a sequence
        // that exercises every op kind passes for the correct implementation.
        let ops = [
            SetOp::Insert(1),
            SetOp::Contains(1),
            SetOp::Remove(1),
            SetOp::Contains(1),
        ];
        assert_eq!(run_set_model(&ops), Ok(()));
    }
}

// ============================================================================
// The same properties with proptest (cargo test --features testing-extras)
// ============================================================================

#[cfg(all(test, feature = "testing-extras"))]
mod proptests {
    use super::*;
    use proptest::prelude::*;
    use proptest::test_runner::{Config as PtConfig, TestError, TestRunner};

    prop_compose! {
        /// Custom strategy: a few runs, each up to 300 long.
        fn runny_bytes()(runs in prop::collection::vec((any::<u8>(), 1usize..300), 0..6)) -> Vec<u8> {
            runs.into_iter().flat_map(|(b, n)| std::iter::repeat_n(b, n)).collect()
        }
    }

    fn set_op() -> impl Strategy<Value = SetOp> {
        prop_oneof![
            (0u8..16).prop_map(SetOp::Insert),
            (0u8..16).prop_map(SetOp::Remove),
            (0u8..16).prop_map(SetOp::Contains),
        ]
    }

    proptest! {
        #[test]
        fn pt_rle_roundtrip(input in runny_bytes()) {
            prop_assert_eq!(rle_decode(&rle_encode(&input)), input);
        }

        #[test]
        fn pt_rle_roundtrip_arbitrary_bytes(input in prop::collection::vec(any::<u8>(), 0..512)) {
            prop_assert_eq!(rle_decode(&rle_encode(&input)), input);
        }

        #[test]
        fn pt_rle_canonical(input in runny_bytes()) {
            prop_assert!(rle_is_canonical(&rle_encode(&input)));
        }

        #[test]
        fn pt_sorted_set_model(ops in prop::collection::vec(set_op(), 0..64)) {
            prop_assert_eq!(run_set_model(&ops), Ok(()));
        }
    }

    #[test]
    fn pt_runner_api_catches_buggy_encoder() {
        // Drive the runner by hand to assert that a property FAILS.
        let mut runner = TestRunner::new(PtConfig {
            failure_persistence: None,
            ..PtConfig::default()
        });
        let result = runner.run(&runny_bytes(), |input| {
            prop_assert_eq!(rle_decode(&rle_encode_u8_counts_buggy(&input)), input);
            Ok(())
        });
        match result {
            Err(TestError::Fail(_, minimal)) => {
                assert!(minimal.len() >= 256);
                assert_ne!(rle_decode(&rle_encode_u8_counts_buggy(&minimal)), minimal);
            }
            other => panic!("expected a shrunk failure, got {other:?}"),
        }
    }
}
