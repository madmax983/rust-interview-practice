//! # UUID Implementation
//!
//! A foundational implementation of a UUID (Universally Unique Identifier) Version 4 generator.
//! Version 4 UUIDs are completely random-based.
//!
//! **Replaces Crates:** `uuid`
//!
//! **Real-world Usage:**
//! - Database primary keys.
//! - Trace IDs in distributed tracing.
//! - Session identifiers in web applications.
//!
//! **Why build it yourself?**
//! While it's easy to just call `uuid::Uuid::new_v4()`, building it teaches you about
//! bitwise manipulation, formatting hex strings, and how standard UUID formats
//! actually embed "version" and "variant" metadata into what appears to be pure noise.

use std::fmt;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure:
//
//      UUID (128 bits / 16 bytes)
//      Displayed as: xxxxxxxx-xxxx-Mxxx-Nxxx-xxxxxxxxxxxx
//      (M = Version, N = Variant)
//
// Bit Layout:
// - **time_low**: 32 bits (bytes 0-3)
// - **time_mid**: 16 bits (bytes 4-5)
// - **time_hi_and_version**: 16 bits (bytes 6-7). The 4 most significant bits hold the version (4).
// - **clock_seq_hi_and_reserved**: 8 bits (byte 8). The 2-3 most significant bits hold the variant.
// - **clock_seq_low**: 8 bits (byte 9)
// - **node**: 48 bits (bytes 10-15)
//
// Invariants:
// 1. A v4 UUID must have the 4 most significant bits of byte 6 set to `0100` (Version 4).
// 2. A v4 UUID must have the 2 most significant bits of byte 8 set to `10` (Variant 1).
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ Generate V4   │ O(1)        │ O(1)        │
// │ Format        │ O(1)        │ O(1)        │
// └───────────────┴─────────────┴─────────────┘
//
// Design Decisions:
// - **Randomness**: We implement a very simple PRNG (XorShift64) internally for demonstration.
//   - *Tradeoff*: Cryptographically insecure.
//   - *Production*: A real implementation uses a secure RNG like `getrandom` or `rand`.
// - **Representation**: We store the UUID as a simple `[u8; 16]`.

/// Represents a 128-bit Universally Unique Identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Uuid {
    bytes: [u8; 16],
}

impl Uuid {
    /// Creates a new random-based Version 4 UUID.
    #[must_use]
    pub fn new_v4() -> Self {
        // RUST INSIGHT: We initialize a stack array.
        let mut bytes = [0u8; 16];

        // 1. Fill with random data
        // For demonstration, we use a simple thread-local XorShift PRNG.
        // In production, `getrandom::getrandom(&mut bytes)` should be used.
        fill_random(&mut bytes);

        // 2. Set the Version (4)
        // GOTCHA: We must clear the top 4 bits with `& 0x0F` before setting them
        // with `| 0x40`. `0x40` is `01000000` in binary.
        bytes[6] = (bytes[6] & 0x0F) | 0x40;

        // 3. Set the Variant (1)
        // Clear the top 2 bits with `& 0x3F` and set them with `| 0x80`.
        // `0x3F` is `00111111`, `0x80` is `10000000`.
        bytes[8] = (bytes[8] & 0x3F) | 0x80;

        Self { bytes }
    }

    /// Returns the raw bytes of the UUID.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.bytes
    }
}

// =========================================================================================
// Formatting
// =========================================================================================

impl fmt::Display for Uuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Format as xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
        let b = &self.bytes;

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

impl Default for Uuid {
    fn default() -> Self {
        Self::new_v4()
    }
}

// =========================================================================================
// Helper: Simple RNG (Xorshift64)
// =========================================================================================
// This is used to avoid pulling in the `rand` crate for the sake of a zero-dependency example.
// It is NOT cryptographically secure.

use std::cell::Cell;
use std::time::{SystemTime, UNIX_EPOCH};

thread_local! {
    static RNG_STATE: Cell<u64> = {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let seed = now.as_secs() ^ now.subsec_nanos() as u64;
        Cell::new(if seed == 0 { 0xCAFEBABE } else { seed })
    };
}

fn xorshift64() -> u64 {
    RNG_STATE.with(|state| {
        let mut x = state.get();
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        state.set(x);
        x
    })
}

fn fill_random(bytes: &mut [u8; 16]) {
    let r1 = xorshift64();
    let r2 = xorshift64();

    bytes[0..8].copy_from_slice(&r1.to_ne_bytes());
    bytes[8..16].copy_from_slice(&r2.to_ne_bytes());
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `uuid`: The official crate provides full support for v1, v3, v4, v5, and v7.
//   It utilizes secure RNGs, supports parsing from strings, and optimizes formatting.
//
// Missing vs. Production:
// - **Parsing**: We don't have a `FromStr` implementation to parse a UUID back from a string.
// - **Security**: Our `XorShift` PRNG means these UUIDs are predictable. Production v4 UUIDs
//   must be generated from a CSPRNG (Cryptographically Secure Pseudo-Random Number Generator).
//
// Next Steps:
// 1. Implement `FromStr` to parse `"xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"`.
// 2. Implement UUID v7 (Time-ordered).
//
// Benchmarking Note:
// Use `criterion` to benchmark the throughput of UUID generation.
// Compare the performance of random data filling (`fill_random`) versus string formatting.
// Example: `std::hint::black_box(Uuid::new_v4().to_string())` to measure total allocation and generation cost.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uuid_v4_generation() {
        let uuid1 = Uuid::new_v4();
        let uuid2 = Uuid::new_v4();

        // Should not be equal (extremely high probability even with simple PRNG)
        assert_ne!(uuid1.as_bytes(), uuid2.as_bytes());
    }

    #[test]
    fn test_uuid_v4_version_bits() {
        let uuid = Uuid::new_v4();
        let bytes = uuid.as_bytes();

        // Check Version (byte 6, top 4 bits should be 0100)
        assert_eq!(bytes[6] >> 4, 4);
    }

    #[test]
    fn test_uuid_v4_variant_bits() {
        let uuid = Uuid::new_v4();
        let bytes = uuid.as_bytes();

        // Check Variant (byte 8, top 2 bits should be 10)
        assert_eq!(bytes[8] >> 6, 2);
    }

    #[test]
    fn test_uuid_formatting() {
        // Manually construct a known UUID
        let mut bytes = [0u8; 16];
        for (i, b) in bytes.iter_mut().enumerate() {
            *b = i as u8;
        }

        // Apply v4 bits to make it a technically valid layout (even though not random)
        bytes[6] = (bytes[6] & 0x0F) | 0x40;
        bytes[8] = (bytes[8] & 0x3F) | 0x80;

        let uuid = Uuid { bytes };
        let formatted = uuid.to_string();

        assert_eq!(formatted.len(), 36);
        assert_eq!(formatted, "00010203-0405-4607-8809-0a0b0c0d0e0f");
    }
}
