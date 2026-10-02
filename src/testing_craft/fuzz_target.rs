//! # Fuzz Targets
//!
//! A fuzzer feeds a function millions of mutated byte strings and watches for
//! panics, overflows, hangs, and broken assertions. Parsers of untrusted bytes
//! are the classic target: every length field is an attacker-controlled number.
//!
//! This module provides:
//!
//! - A binary frame format with a **hardened parser** ([`parse`]) and encoder ([`encode`]).
//! - A **deliberately fragile parser** ([`parse_fragile_buggy`]) that indexes slices
//!   with lengths read from the input.
//! - **Fuzz entry points** ([`fuzz_parse`], [`fuzz_structured`]) — plain functions
//!   called both by the `cargo-fuzz` harness in `fuzz/fuzz_targets/` and by unit tests.
//! - A small **feedback-guided mutation fuzzer** ([`MiniFuzzer`]) that runs on
//!   stable inside `cargo test`, so the targets get exercised in CI without nightly.
//!
//! ## Wire format
//!
//! ```text
//! +-------+---------+-------+-------------+---------------------------------+--------+
//! | "TF"  | version | flags | field_count | field_count x (tag, len, value) | crc32  |
//! | 2 B   | u8 = 1  | u8    | u16 BE      | u8, u32 BE, len bytes           | u32 BE |
//! +-------+---------+-------+-------------+---------------------------------+--------+
//! ```
//!
//! The CRC covers every byte before it.
//!
//! ## Fuzzing lessons encoded here
//!
//! 1. **Never trust a length**: every read goes through [`Reader::take`], which uses
//!    `checked_add` and `slice::get`, so no input can panic or overflow.
//! 2. **Checksums blind mutation fuzzers**: a random bit flip almost always breaks the
//!    CRC, so the fuzzer never gets past it. Targets parse with
//!    [`ParseOptions::FUZZING`] (CRC check off) to reach the deep code.
//! 3. **Assert more than "no panic"**: the round-trip oracle
//!    `parse(encode(parse(x))) == parse(x)` turns the fuzzer into a correctness tester.
//! 4. **Structure-aware fuzzing**: [`fuzz_structured`] interprets raw bytes as
//!    *choices* for building a valid [`Frame`], so every execution is a deep one.
//!
//! ```
//! use rust_interview_practice::testing_craft::fuzz_target::{encode, parse, Field, Frame};
//!
//! let frame = Frame { flags: 1, fields: vec![Field { tag: 7, value: b"hi".to_vec() }] };
//! let bytes = encode(&frame).unwrap();
//! assert_eq!(parse(&bytes), Ok(frame));
//! assert!(parse(&bytes[..bytes.len() - 1]).is_err()); // truncated: error, not panic
//! ```

use std::collections::HashSet;
use std::fmt;
use std::panic::{self, AssertUnwindSafe};

use super::sim_rng::SimRng;

/// Frame magic bytes.
pub const MAGIC: [u8; 2] = *b"TF";
/// The only supported version.
pub const VERSION: u8 = 1;
/// Maximum fields per frame (bounds memory per frame).
pub const MAX_FIELDS: usize = 64;
/// Maximum bytes per field value.
pub const MAX_FIELD_LEN: usize = 64 * 1024;

const HEADER_LEN: usize = 2 + 1 + 1 + 2;
const FIELD_HEADER_LEN: usize = 1 + 4;
const CHECKSUM_LEN: usize = 4;

/// One tagged field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    /// Application-defined tag.
    pub tag: u8,
    /// Opaque value bytes.
    pub value: Vec<u8>,
}

/// A decoded frame.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Frame {
    /// Application-defined flag bits.
    pub flags: u8,
    /// Fields in wire order.
    pub fields: Vec<Field>,
}

/// Every way a frame can be rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameError {
    /// Input ended before `needed` more bytes could be read at `offset`.
    Truncated {
        /// Byte offset of the failed read.
        offset: usize,
        /// Bytes the read wanted.
        needed: usize,
    },
    /// The first two bytes are not [`MAGIC`].
    BadMagic,
    /// The version byte is not [`VERSION`].
    UnsupportedVersion(u8),
    /// `field_count` exceeds [`MAX_FIELDS`].
    TooManyFields(usize),
    /// A field length exceeds [`MAX_FIELD_LEN`].
    FieldTooLarge(usize),
    /// Bytes remain after the checksum.
    TrailingBytes(usize),
    /// The stored CRC does not match the computed one.
    ChecksumMismatch {
        /// CRC stored in the frame.
        stored: u32,
        /// CRC computed over the frame bytes.
        computed: u32,
    },
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated { offset, needed } => {
                write!(f, "truncated: needed {needed} bytes at offset {offset}")
            }
            Self::BadMagic => write!(f, "bad magic"),
            Self::UnsupportedVersion(v) => write!(f, "unsupported version {v}"),
            Self::TooManyFields(n) => write!(f, "too many fields: {n} > {MAX_FIELDS}"),
            Self::FieldTooLarge(n) => write!(f, "field too large: {n} > {MAX_FIELD_LEN}"),
            Self::TrailingBytes(n) => write!(f, "{n} trailing bytes after checksum"),
            Self::ChecksumMismatch { stored, computed } => {
                write!(
                    f,
                    "checksum mismatch: stored {stored:#010x}, computed {computed:#010x}"
                )
            }
        }
    }
}

impl std::error::Error for FrameError {}

/// Parser knobs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseOptions {
    /// Verify the trailing CRC. Turn off only for fuzzing.
    pub verify_checksum: bool,
}

impl ParseOptions {
    /// Production settings.
    pub const STRICT: Self = Self {
        verify_checksum: true,
    };
    /// Fuzzing settings: skip the CRC so mutations reach the field parser.
    pub const FUZZING: Self = Self {
        verify_checksum: false,
    };
}

/// A bounds-checked cursor. Every read is `checked_add` + `slice::get`, so
/// hostile lengths become errors, never panics or overflows.
#[derive(Debug)]
pub struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    /// Starts reading at offset 0.
    #[must_use]
    pub const fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    /// Current offset.
    #[must_use]
    pub const fn position(&self) -> usize {
        self.pos
    }

    /// Bytes not yet consumed.
    #[must_use]
    pub const fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }

    /// Consumes exactly `n` bytes.
    ///
    /// # Errors
    ///
    /// [`FrameError::Truncated`] if fewer than `n` bytes remain.
    pub fn take(&mut self, n: usize) -> Result<&'a [u8], FrameError> {
        let truncated = FrameError::Truncated {
            offset: self.pos,
            needed: n,
        };
        let end = self.pos.checked_add(n).ok_or(truncated)?;
        let bytes = self.data.get(self.pos..end).ok_or(truncated)?;
        self.pos = end;
        Ok(bytes)
    }

    /// Consumes exactly `N` bytes into an array.
    ///
    /// # Errors
    ///
    /// [`FrameError::Truncated`] if fewer than `N` bytes remain.
    pub fn array<const N: usize>(&mut self) -> Result<[u8; N], FrameError> {
        let mut out = [0u8; N];
        out.copy_from_slice(self.take(N)?);
        Ok(out)
    }

    /// Reads one byte.
    ///
    /// # Errors
    ///
    /// [`FrameError::Truncated`] at end of input.
    pub fn u8(&mut self) -> Result<u8, FrameError> {
        Ok(self.array::<1>()?[0])
    }

    /// Reads a big-endian `u16`.
    ///
    /// # Errors
    ///
    /// [`FrameError::Truncated`] if fewer than 2 bytes remain.
    pub fn u16_be(&mut self) -> Result<u16, FrameError> {
        Ok(u16::from_be_bytes(self.array()?))
    }

    /// Reads a big-endian `u32`.
    ///
    /// # Errors
    ///
    /// [`FrameError::Truncated`] if fewer than 4 bytes remain.
    pub fn u32_be(&mut self) -> Result<u32, FrameError> {
        Ok(u32::from_be_bytes(self.array()?))
    }
}

/// Parses a frame with production settings ([`ParseOptions::STRICT`]).
///
/// # Errors
///
/// Any [`FrameError`]; never panics, for any input.
pub fn parse(data: &[u8]) -> Result<Frame, FrameError> {
    parse_with(data, ParseOptions::STRICT)
}

/// Parses a frame.
///
/// Time: O(n). Space: O(n) - field values are copied out.
///
/// # Errors
///
/// Any [`FrameError`]; never panics, for any input.
pub fn parse_with(data: &[u8], options: ParseOptions) -> Result<Frame, FrameError> {
    let mut r = Reader::new(data);
    if r.array::<2>()? != MAGIC {
        return Err(FrameError::BadMagic);
    }
    let version = r.u8()?;
    if version != VERSION {
        return Err(FrameError::UnsupportedVersion(version));
    }
    let flags = r.u8()?;
    let field_count = usize::from(r.u16_be()?);
    if field_count > MAX_FIELDS {
        return Err(FrameError::TooManyFields(field_count));
    }

    // Do NOT `Vec::with_capacity(len)` from an untrusted length without a cap;
    // here `field_count` is already bounded by MAX_FIELDS.
    let mut fields = Vec::with_capacity(field_count);
    for _ in 0..field_count {
        let tag = r.u8()?;
        let len = usize::try_from(r.u32_be()?).unwrap_or(usize::MAX);
        if len > MAX_FIELD_LEN {
            return Err(FrameError::FieldTooLarge(len));
        }
        let value = r.take(len)?.to_vec();
        fields.push(Field { tag, value });
    }

    let body_len = r.position();
    let stored = r.u32_be()?;
    if r.remaining() != 0 {
        return Err(FrameError::TrailingBytes(r.remaining()));
    }
    if options.verify_checksum {
        let computed = crc32fast::hash(&data[..body_len]);
        if stored != computed {
            return Err(FrameError::ChecksumMismatch { stored, computed });
        }
    }
    Ok(Frame { flags, fields })
}

/// Encodes a frame.
///
/// # Errors
///
/// [`FrameError::TooManyFields`] / [`FrameError::FieldTooLarge`] if the frame
/// exceeds the format limits (so `encode` never produces bytes `parse` rejects).
pub fn encode(frame: &Frame) -> Result<Vec<u8>, FrameError> {
    let count = u16::try_from(frame.fields.len())
        .ok()
        .filter(|&n| usize::from(n) <= MAX_FIELDS)
        .ok_or(FrameError::TooManyFields(frame.fields.len()))?;
    let payload: usize = frame
        .fields
        .iter()
        .map(|f| FIELD_HEADER_LEN + f.value.len())
        .sum();
    let mut out = Vec::with_capacity(HEADER_LEN + payload + CHECKSUM_LEN);
    out.extend_from_slice(&MAGIC);
    out.push(VERSION);
    out.push(frame.flags);
    out.extend_from_slice(&count.to_be_bytes());
    for field in &frame.fields {
        let len = u32::try_from(field.value.len())
            .ok()
            .filter(|&n| n as usize <= MAX_FIELD_LEN)
            .ok_or(FrameError::FieldTooLarge(field.value.len()))?;
        out.push(field.tag);
        out.extend_from_slice(&len.to_be_bytes());
        out.extend_from_slice(&field.value);
    }
    let crc = crc32fast::hash(&out);
    out.extend_from_slice(&crc.to_be_bytes());
    Ok(out)
}

/// FRAGILE on purpose: the "first draft" parser.
///
/// It slices with lengths taken
/// straight from the input, so a truncated or lying frame panics. The mini
/// fuzzer finds a crashing input for it within a few hundred executions.
///
/// # Panics
///
/// On truncated input or any length field that points past the end of `data`.
#[must_use]
pub fn parse_fragile_buggy(data: &[u8]) -> Option<Frame> {
    if data[0..2] != MAGIC || data[2] != VERSION {
        return None;
    }
    let flags = data[3];
    let field_count = usize::from(u16::from_be_bytes([data[4], data[5]]));
    let mut pos = HEADER_LEN;
    let mut fields = Vec::new();
    for _ in 0..field_count {
        let tag = data[pos];
        let len = u32::from_be_bytes([data[pos + 1], data[pos + 2], data[pos + 3], data[pos + 4]])
            as usize;
        fields.push(Field {
            tag,
            value: data[pos + 5..pos + 5 + len].to_vec(),
        });
        pos += 5 + len;
    }
    Some(Frame { flags, fields })
}

// ============================================================================
// Fuzz entry points (called by fuzz/fuzz_targets/*.rs and by unit tests)
// ============================================================================

/// Coarse "coverage" signal for [`MiniFuzzer`]: which outcome the parser
/// reached and how deep it got. Real fuzzers use edge coverage from compiler
/// instrumentation; this stands in for it on stable.
#[must_use]
pub fn parse_feature(data: &[u8]) -> u64 {
    match parse_with(data, ParseOptions::FUZZING) {
        Ok(frame) => 1000 + frame.fields.len() as u64,
        Err(FrameError::Truncated { offset, .. }) => 100 + offset.min(64) as u64,
        Err(FrameError::BadMagic) => 1,
        Err(FrameError::UnsupportedVersion(_)) => 2,
        Err(FrameError::TooManyFields(_)) => 3,
        Err(FrameError::FieldTooLarge(_)) => 4,
        Err(FrameError::TrailingBytes(_)) => 5,
        Err(FrameError::ChecksumMismatch { .. }) => 6,
    }
}

/// Fuzz target: arbitrary bytes. Must not panic, and every accepted frame must
/// survive an encode/parse round trip unchanged.
///
/// # Panics
///
/// Only when an oracle is violated — that is a bug report, which is the point.
pub fn fuzz_parse(data: &[u8]) {
    let _ = parse(data); // strict path: exercises the CRC check
    if let Ok(frame) = parse_with(data, ParseOptions::FUZZING) {
        let bytes = encode(&frame).expect("a parsed frame is within limits");
        assert_eq!(
            parse(&bytes).as_ref(),
            Ok(&frame),
            "round trip changed the frame"
        );
        assert_eq!(bytes.len(), data.len(), "encoding is not canonical");
    }
}

/// Reads fuzzer bytes as *decisions*, the idea behind the `arbitrary` crate.
/// Running out of bytes yields zeros, so every input maps to some value.
#[derive(Debug)]
pub struct ByteSource<'a> {
    data: &'a [u8],
}

impl<'a> ByteSource<'a> {
    /// Wraps fuzzer input.
    #[must_use]
    pub const fn new(data: &'a [u8]) -> Self {
        Self { data }
    }

    /// Next decision byte (0 once exhausted).
    pub const fn byte(&mut self) -> u8 {
        match self.data.split_first() {
            Some((&b, rest)) => {
                self.data = rest;
                b
            }
            None => 0,
        }
    }

    /// A value in `0..=max`.
    pub fn up_to(&mut self, max: usize) -> usize {
        let raw = usize::from(u16::from_be_bytes([self.byte(), self.byte()]));
        raw % (max + 1)
    }

    /// Up to `max` bytes taken directly from the input.
    pub fn bytes(&mut self, max: usize) -> Vec<u8> {
        let n = self.up_to(max).min(self.data.len());
        let (head, rest) = self.data.split_at(n);
        self.data = rest;
        head.to_vec()
    }
}

/// Builds a valid [`Frame`] from fuzzer decisions.
#[must_use]
pub fn frame_from_bytes(data: &[u8]) -> Frame {
    let mut src = ByteSource::new(data);
    let flags = src.byte();
    let count = src.up_to(8);
    let fields = (0..count)
        .map(|_| Field {
            tag: src.byte(),
            value: src.bytes(256),
        })
        .collect();
    Frame { flags, fields }
}

/// Fuzz target: structure-aware. Every execution builds a *valid* frame, so
/// the fuzzer spends its time on the encoder/decoder, not on the magic check.
///
/// # Panics
///
/// Only when the round-trip oracle is violated.
pub fn fuzz_structured(data: &[u8]) {
    let frame = frame_from_bytes(data);
    let bytes = encode(&frame).expect("generated frames are within limits");
    assert_eq!(parse(&bytes), Ok(frame), "structured round trip failed");
}

/// A handful of valid frames to seed the fuzzer.
///
/// # Panics
///
/// Never: the seed frames are within the format limits.
#[must_use]
pub fn seed_corpus() -> Vec<Vec<u8>> {
    let frames = [
        Frame::default(),
        Frame {
            flags: 0x80,
            fields: vec![Field {
                tag: 1,
                value: b"hello".to_vec(),
            }],
        },
        Frame {
            flags: 3,
            fields: vec![
                Field {
                    tag: 2,
                    value: vec![],
                },
                Field {
                    tag: 9,
                    value: vec![0xFF; 16],
                },
            ],
        },
    ];
    frames
        .iter()
        .map(|f| encode(f).expect("seed frames are within limits"))
        .collect()
}

// ============================================================================
// MiniFuzzer: feedback-guided mutation fuzzing on stable Rust
// ============================================================================

/// A panic found by the fuzzer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Crash {
    /// The input that panicked.
    pub input: Vec<u8>,
    /// The panic payload, if it was a string.
    pub message: String,
}

/// Summary of a fuzzing campaign.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FuzzReport {
    /// Target executions performed.
    pub executions: usize,
    /// Inputs kept because they reached a new feature.
    pub corpus_size: usize,
    /// Distinct features observed.
    pub features: usize,
    /// First crash, if any (the campaign stops there).
    pub crash: Option<Crash>,
}

/// Mutation-based fuzzer with a corpus and feature feedback, in the spirit of
/// libFuzzer/AFL, small enough to type from memory.
#[derive(Debug)]
pub struct MiniFuzzer {
    rng: SimRng,
    corpus: Vec<Vec<u8>>,
    seen: HashSet<u64>,
    max_len: usize,
}

const INTERESTING: [u8; 8] = [0x00, 0x01, 0x7F, 0x80, 0xFF, 0x40, 0x10, 0x20];

impl MiniFuzzer {
    /// Creates a fuzzer with a deterministic seed and an initial corpus.
    #[must_use]
    pub fn new(seed: u64, corpus: Vec<Vec<u8>>) -> Self {
        let corpus = if corpus.is_empty() {
            vec![Vec::new()]
        } else {
            corpus
        };
        Self {
            rng: SimRng::new(seed),
            corpus,
            seen: HashSet::new(),
            max_len: 4096,
        }
    }

    /// Produces a mutated copy of a corpus entry (1-4 stacked mutations).
    pub fn mutate(&mut self, input: &[u8]) -> Vec<u8> {
        let mut out = input.to_vec();
        let rounds = 1 + self.rng.below(4);
        for _ in 0..rounds {
            let len = out.len();
            match self.rng.below(8) {
                0 if len > 0 => {
                    let i = self.rng.below(len);
                    out[i] ^= 1 << self.rng.below(8);
                }
                1 if len > 0 => {
                    let i = self.rng.below(len);
                    out[i] = self.rng.byte();
                }
                2 if len > 0 => {
                    let i = self.rng.below(len);
                    out[i] = *self.rng.pick(&INTERESTING).unwrap_or(&0);
                }
                3 => {
                    let i = self.rng.below(len + 1);
                    let b = self.rng.byte();
                    out.insert(i, b);
                }
                4 if len > 0 => {
                    out.remove(self.rng.below(len));
                }
                5 if len > 0 => {
                    out.truncate(self.rng.below(len));
                }
                6 if len > 1 => {
                    let start = self.rng.below(len);
                    let end = start + self.rng.below(len - start) + 1;
                    let chunk = out[start..end].to_vec();
                    let at = self.rng.below(out.len() + 1);
                    out.splice(at..at, chunk);
                }
                7 => {
                    let other = self.rng.below(self.corpus.len());
                    let donor = &self.corpus[other];
                    let cut = self.rng.below(out.len() + 1);
                    let from = self.rng.below(donor.len() + 1);
                    out.truncate(cut);
                    out.extend_from_slice(&donor[from..]);
                }
                _ => {}
            }
        }
        out.truncate(self.max_len);
        out
    }

    /// Runs `iterations` executions. `feature` provides feedback (inputs that
    /// reach a new feature join the corpus); `target` is the code under test,
    /// and any panic in it is reported as a [`Crash`].
    pub fn run<F, T>(&mut self, iterations: usize, feature: F, target: T) -> FuzzReport
    where
        F: Fn(&[u8]) -> u64,
        T: Fn(&[u8]),
    {
        for entry in &self.corpus {
            self.seen.insert(feature(entry));
        }
        for executions in 1..=iterations {
            let parent_index = self.rng.below(self.corpus.len());
            let parent = self.corpus[parent_index].clone();
            let input = self.mutate(&parent);

            if let Err(payload) = panic::catch_unwind(AssertUnwindSafe(|| target(&input))) {
                let message = payload
                    .downcast_ref::<&str>()
                    .map(ToString::to_string)
                    .or_else(|| payload.downcast_ref::<String>().cloned())
                    .unwrap_or_default();
                return self.report(executions, Some(Crash { input, message }));
            }
            if self.seen.insert(feature(&input)) {
                self.corpus.push(input);
            }
        }
        self.report(iterations, None)
    }

    fn report(&self, executions: usize, crash: Option<Crash>) -> FuzzReport {
        FuzzReport {
            executions,
            corpus_size: self.corpus.len(),
            features: self.seen.len(),
            crash,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Frame {
        Frame {
            flags: 0b101,
            fields: vec![
                Field {
                    tag: 1,
                    value: b"alpha".to_vec(),
                },
                Field {
                    tag: 2,
                    value: vec![],
                },
            ],
        }
    }

    #[test]
    fn test_roundtrip_example() {
        let bytes = encode(&sample()).unwrap();
        assert_eq!(parse(&bytes), Ok(sample()));
    }

    #[test]
    fn test_every_prefix_is_an_error_not_a_panic() {
        let bytes = encode(&sample()).unwrap();
        for cut in 0..bytes.len() {
            assert!(parse(&bytes[..cut]).is_err(), "prefix {cut} accepted");
        }
    }

    #[test]
    fn test_rejections() {
        let good = encode(&sample()).unwrap();

        let mut bad_magic = good.clone();
        bad_magic[0] = b'X';
        assert_eq!(parse(&bad_magic), Err(FrameError::BadMagic));

        let mut bad_version = good.clone();
        bad_version[2] = 9;
        assert_eq!(parse(&bad_version), Err(FrameError::UnsupportedVersion(9)));

        let mut trailing = good.clone();
        trailing.push(0);
        assert_eq!(parse(&trailing), Err(FrameError::TrailingBytes(1)));

        let mut flipped = good;
        flipped[HEADER_LEN + FIELD_HEADER_LEN] ^= 0xFF; // corrupt a value byte
        assert!(matches!(
            parse(&flipped),
            Err(FrameError::ChecksumMismatch { .. })
        ));
        assert!(parse_with(&flipped, ParseOptions::FUZZING).is_ok());
    }

    #[test]
    fn test_hostile_lengths() {
        // Field claims u32::MAX bytes: must be rejected without allocating.
        let mut data = vec![b'T', b'F', VERSION, 0, 0, 1, 7];
        data.extend_from_slice(&u32::MAX.to_be_bytes());
        assert!(matches!(parse(&data), Err(FrameError::FieldTooLarge(_))));

        // field_count over the cap.
        let data = [b'T', b'F', VERSION, 0, 0xFF, 0xFF];
        assert_eq!(parse(&data), Err(FrameError::TooManyFields(0xFFFF)));

        // Reader::take with an overflowing length.
        let mut r = Reader::new(&[1, 2, 3]);
        r.take(1).unwrap();
        assert!(r.take(usize::MAX).is_err());
        assert_eq!(r.remaining(), 2);
    }

    #[test]
    fn test_encode_enforces_limits() {
        let too_many = Frame {
            flags: 0,
            fields: vec![
                Field {
                    tag: 0,
                    value: vec![]
                };
                MAX_FIELDS + 1
            ],
        };
        assert_eq!(
            encode(&too_many),
            Err(FrameError::TooManyFields(MAX_FIELDS + 1))
        );
        let too_big = Frame {
            flags: 0,
            fields: vec![Field {
                tag: 0,
                value: vec![0; MAX_FIELD_LEN + 1],
            }],
        };
        assert_eq!(
            encode(&too_big),
            Err(FrameError::FieldTooLarge(MAX_FIELD_LEN + 1))
        );
    }

    #[test]
    fn test_error_display() {
        let msg = FrameError::Truncated {
            offset: 3,
            needed: 4,
        }
        .to_string();
        assert!(msg.contains("offset 3"));
        assert!(FrameError::BadMagic.to_string().contains("magic"));
        assert!(
            FrameError::ChecksumMismatch {
                stored: 1,
                computed: 2
            }
            .to_string()
            .contains("0x00000001")
        );
    }

    #[test]
    fn test_fragile_parser_handles_well_formed_input() {
        let bytes = encode(&sample()).unwrap();
        assert_eq!(parse_fragile_buggy(&bytes), Some(sample()));
    }

    #[test]
    fn test_seed_corpus_passes_both_targets() {
        for seed in seed_corpus() {
            fuzz_parse(&seed);
            fuzz_structured(&seed);
        }
        fuzz_parse(&[]);
        fuzz_structured(&[]);
    }

    #[test]
    fn test_structured_source_is_total() {
        // Any bytes, including none, produce a valid frame.
        assert_eq!(frame_from_bytes(&[]), Frame::default());
        let mut rng = SimRng::new(1);
        for _ in 0..200 {
            let len = rng.below(600);
            let frame = frame_from_bytes(&rng.bytes(len));
            assert!(frame.fields.len() <= 8);
            assert!(encode(&frame).is_ok());
        }
    }

    #[test]
    fn fuzz_hardened_parser_finds_no_crash() {
        let mut fuzzer = MiniFuzzer::new(0xF022, seed_corpus());
        let report = fuzzer.run(20_000, parse_feature, fuzz_parse);
        assert_eq!(report.crash, None);
        assert_eq!(report.executions, 20_000);
        // Feedback grew the corpus beyond the seeds.
        assert!(report.corpus_size > seed_corpus().len(), "{report:?}");
    }

    #[test]
    fn fuzz_structured_target_finds_no_crash() {
        let mut fuzzer = MiniFuzzer::new(0x5EED, vec![vec![0; 32]]);
        let report = fuzzer.run(
            5_000,
            |d| frame_from_bytes(d).fields.len() as u64,
            fuzz_structured,
        );
        assert_eq!(report.crash, None);
    }

    #[test]
    fn fuzz_fragile_parser_is_crashed() {
        let mut fuzzer = MiniFuzzer::new(0xBAD, seed_corpus());
        let report = fuzzer.run(20_000, parse_feature, |d| {
            let _ = parse_fragile_buggy(d);
        });
        let crash = report.crash.expect("the fragile parser must crash");
        // Regression pattern: replay the crasher against the hardened parser.
        fuzz_parse(&crash.input);
        assert_ne!(crash.message, "");
    }

    #[test]
    fn test_fuzzer_is_deterministic() {
        let run = || MiniFuzzer::new(42, seed_corpus()).run(2_000, parse_feature, fuzz_parse);
        assert_eq!(run(), run());
    }

    #[test]
    fn test_mutate_respects_max_len_and_empty_corpus() {
        let mut fuzzer = MiniFuzzer::new(3, Vec::new());
        for _ in 0..500 {
            let out = fuzzer.mutate(&[1, 2, 3]);
            assert!(out.len() <= 4096);
        }
        let report = fuzzer.run(100, |d| d.len() as u64, |_| {});
        assert_eq!(report.crash, None);
    }
}
