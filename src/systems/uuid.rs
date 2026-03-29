//! # UUID Implementation
//!
//! Implements a Version 4 (Random) Universally Unique Identifier (UUID) generator from scratch.
//! This handles the 128-bit structure, bitwise manipulation for version and variant flags,
//! and standard hex string formatting.
//!
//! **Replaces Crates:** `uuid`
//!
//! **Real-world Usage:**
//! - Primary keys in distributed databases (Cassandra, Postgres).
//! - Trace IDs in distributed tracing systems.
//! - Idempotency keys in API requests.
//!
//! **Why build it yourself?**
//! It demystifies the magic "random string" you see everywhere. You'll learn how the 128 bits
//! are structured, why standard UUIDs aren't perfectly random (due to reserved version/variant bits),
//! and how to manipulate individual bits using bitwise operators (`&`, `|`, `!`) safely in Rust.

use std::collections::hash_map::RandomState;
use std::fmt;
use std::hash::{BuildHasher, Hasher};
use std::time::{SystemTime, UNIX_EPOCH};

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure:
//
//      128-bit array (16 bytes)
//      [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]
//
// String Representation (8-4-4-4-12 format):
//      xxxxxxxx-xxxx-Mxxx-Nxxx-xxxxxxxxxxxx
//
// Invariants:
// 1. **Version (M):** The 4 most significant bits of the 7th byte (byte[6]) must be `0100` (Version 4).
// 2. **Variant (N):** The 2 to 3 most significant bits of the 9th byte (byte[8]) must be `10` (Variant 1).
// 3. Formatted strings must match the `xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx` pattern.
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ Generate v4   │ O(1)        │ O(1)        │
// │ Format to Hex │ O(1)        │ O(1)        │
// │ Parse String  │ O(1)        │ O(1)        │
// └───────────────┴─────────────┴─────────────┘
//
// Design Decisions:
// - **Randomness:** We use a custom, simple SplitMix64 pseudo-random number generator (PRNG) seeded by the system
//   time and Rust's `RandomState` to avoid external dependencies. A production implementation
//   would use the OS's cryptographically secure PRNG (`/dev/urandom` or `BCryptGenRandom`).
// - **Storage:** We use `[u8; 16]` for O(1) size and stack allocation instead of a `String`.

/// A Universally Unique Identifier (UUID).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Uuid([u8; 16]);

/// Trait defining the behavior of a UUID Generator.
/// RUST INSIGHT: Using traits for generation allows us to mock the generator in tests
/// to ensure specific outputs or test collision handling.
pub trait UuidGenerator {
    /// Generates a Version 4 (Random) UUID.
    fn generate_v4() -> Uuid;
}

/// A simple, non-cryptographic PRNG to generate random bytes for our UUID without external crates.
struct Prng {
    state: u64,
}

impl Prng {
    fn new() -> Self {
        // Seed the PRNG with current time and RandomState
        let mut state = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        let rs = RandomState::new();
        let mut hasher = rs.build_hasher();
        hasher.write_u64(state);
        state ^= hasher.finish();

        // Ensure state is never 0
        if state == 0 {
            state = 1;
        }

        Self { state }
    }

    /// SplitMix64 algorithm for fast, decent statistical quality pseudo-randomness.
    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        for chunk in dest.chunks_mut(8) {
            let val = self.next_u64();
            let bytes = val.to_le_bytes();
            let len = chunk.len();
            chunk.copy_from_slice(&bytes[..len]);
        }
    }
}

/// Default generator utilizing our internal PRNG.
pub struct DefaultUuidGenerator;

impl UuidGenerator for DefaultUuidGenerator {
    fn generate_v4() -> Uuid {
        let mut prng = Prng::new();
        let mut bytes = [0u8; 16];
        prng.fill_bytes(&mut bytes);

        // GOTCHA: We must overwrite bits to comply with RFC 4122 for UUIDv4.
        // Failing to do so creates invalid UUIDs that might be rejected by databases.

        // PRODUCTION NOTE: In a real-world scenario, instantiating a PRNG on every
        // call to generate_v4() is slow. A production implementation would typically
        // store the PRNG state in a `thread_local!` to avoid lock contention while
        // maintaining high throughput.

        // Set version to 4 (0100_xxxx) in the 7th byte.
        // We clear the top 4 bits with `& 0x0F` and set the 0x40 (0100) bits with `| 0x40`.
        bytes[6] = (bytes[6] & 0x0F) | 0x40;

        // Set variant to 1 (10xx_xxxx) in the 9th byte.
        // We clear the top 2 bits with `& 0x3F` and set the 0x80 (1000) bits with `| 0x80`.
        bytes[8] = (bytes[8] & 0x3F) | 0x80;

        Uuid(bytes)
    }
}

impl Default for Uuid {
    fn default() -> Self {
        Self::new_v4()
    }
}

impl Uuid {
    /// Creates a UUIDv4 using the default generator.
    #[must_use]
    pub fn new_v4() -> Self {
        DefaultUuidGenerator::generate_v4()
    }

    /// Returns the raw 16-byte array.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }

    /// Parses a UUID from a string slice (e.g., "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx").
    #[must_use]
    pub fn parse_str(s: &str) -> Option<Self> {
        if s.len() != 36 {
            return None;
        }

        let mut bytes = [0u8; 16];
        let mut byte_idx = 0;
        let mut char_idx = 0;

        let s_bytes = s.as_bytes();

        while char_idx < 36 {
            // Check hyphens at specific positions
            if char_idx == 8 || char_idx == 13 || char_idx == 18 || char_idx == 23 {
                if s_bytes[char_idx] != b'-' {
                    return None;
                }
                char_idx += 1;
                continue;
            }

            // Parse two hex characters
            if char_idx + 1 >= 36 {
                return None;
            }

            let high = hex_val(s_bytes[char_idx])?;
            let low = hex_val(s_bytes[char_idx + 1])?;

            bytes[byte_idx] = (high << 4) | low;
            byte_idx += 1;
            char_idx += 2;
        }

        if byte_idx != 16 {
            return None;
        }

        Some(Self(bytes))
    }
}

/// Helper to parse a single hex character into its integer value.
const fn hex_val(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

// RUST INSIGHT: Implementing `fmt::Display` automatically provides `.to_string()`
// and allows UUIDs to be formatted nicely with `format!("{}", uuid)`.
impl fmt::Display for Uuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let b = self.0;
        // Format strings in Rust are incredibly powerful. We format bytes as zero-padded
        // 2-character lowercase hex values.
        write!(
            f,
            "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            b[0], b[1], b[2], b[3],
            b[4], b[5],
            b[6], b[7],
            b[8], b[9],
            b[10], b[11], b[12], b[13], b[14], b[15]
        )
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `uuid`: The official Rust UUID crate is comprehensive. It supports versions 1, 3, 4, 5, 6, 7, and 8.
//   It utilizes `getrandom` or `rand` crates for cryptographically secure randomness, ensuring
//   no predictability or collisions across distributed nodes. It also highly optimizes string parsing/formatting
//   using SIMD or unrolled bit-manipulation where possible.
//
// Missing vs. Production:
// - **Cryptographic Randomness**: Our PRNG is fast but completely predictable if the seed (time) is known.
//   A real implementation MUST use cryptographically secure sources.
// - **Other Versions**: We only implement UUIDv4 (Random). Modern systems increasingly use UUIDv7
//   for timestamp-ordered, database-friendly indexing.
//
// Next Steps:
// 1. Implement UUIDv7 using Unix epoch milliseconds and monotonicity checks.
// 2. Add an optimized `const` parsing function to build UUIDs at compile time.
//
// Benchmarking Note:
// Use `criterion` to benchmark `Uuid::new_v4()`. Compare it against the official `uuid` crate.
// Also benchmark `Uuid::parse_str()` and `uuid.to_string()` to see the impact of our manual parsing
// vs. the SIMD-optimized routines in the official crate.

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_uuid_v4_generation() {
        let uuid = Uuid::new_v4();
        let bytes = uuid.as_bytes();

        // Check Version 4 (0100)
        assert_eq!(bytes[6] >> 4, 4);

        // Check Variant 1 (10)
        assert_eq!(bytes[8] >> 6, 2);
    }

    #[test]
    fn test_uuid_formatting() {
        let mut bytes = [0u8; 16];
        // Populate with some sequential bytes for deterministic testing
        for i in 0..16 {
            bytes[i] = i as u8;
        }
        let uuid = Uuid(bytes);

        let s = uuid.to_string();
        assert_eq!(s, "00010203-0405-0607-0809-0a0b0c0d0e0f");
    }

    #[test]
    fn test_uuid_parsing() {
        let valid_str = "f47ac10b-58cc-4372-a567-0e02b2c3d479";
        let uuid = Uuid::parse_str(valid_str).expect("Failed to parse valid UUID");

        assert_eq!(uuid.to_string(), valid_str);

        // Test parsing failures
        assert!(Uuid::parse_str("f47ac10b-58cc-4372-a567-0e02b2c3d47").is_none()); // Too short
        assert!(Uuid::parse_str("z47ac10b-58cc-4372-a567-0e02b2c3d479").is_none()); // Invalid hex
        assert!(Uuid::parse_str("f47ac10b+58cc-4372-a567-0e02b2c3d479").is_none()); // Invalid separator
    }

    #[test]
    fn test_uuid_randomness_and_uniqueness() {
        // Generate a lot of UUIDs and ensure no duplicates
        let mut seen = HashSet::new();
        for _ in 0..10_000 {
            let uuid = Uuid::new_v4();
            assert!(seen.insert(uuid), "Duplicate UUID generated!");
        }
    }
}
