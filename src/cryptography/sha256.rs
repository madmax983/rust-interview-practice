//! # SHA-256 (Secure Hash Algorithm)
//!
//! Implements the SHA-256 cryptographic hashing algorithm from scratch.
//! This implementation processes input streams in 64-byte blocks and produces a 256-bit (32-byte) digest.
//!
//! **Replaces Crates:** `sha2`, `ring`, `crypto-hash`
//!
//! **Real-world Usage:**
//! - Digital signatures (TLS, SSL, SSH)
//! - Blockchain verification (Bitcoin proof-of-work)
//! - Password hashing (though bcrypt/Argon2 are preferred, PBKDF2-HMAC-SHA256 is common)
//! - Data integrity verification (checksums for file downloads)
//!
//! **Why build it yourself?**
//! Implementing SHA-256 demystifies how cryptographic primitives work under the hood.
//! It teaches you about bitwise operations (rotations, XORs), wrapping arithmetic in Rust,
//! padding rules, and how data can be securely compressed into a fixed-length digest.
//! You learn the mechanics of the Merkle-Damgård construction.
//!
//! // =========================================================================================
//! // Architecture
//! // =========================================================================================
//!
//! Data Structure / Flow:
//! ```text
//!
//!      [Input Message]
//!             |
//!             v
//!      [Padding (1 bit, 0s, length)]
//!             |
//!             v
//!      [512-bit Blocks (64 bytes)]
//!             |
//!             v
//!    +--------------------------+
//!    | Message Schedule W (64)  |
//!    +--------------------------+
//!             |
//!             v
//!    +--------------------------+     +----------------+
//!    | Compression Function     |◄----┤ Previous State |
//!    | (64 rounds of bit math)  |     | (a,b,c,d,e,f,g)|
//!    +--------------------------+     +----------------+
//!             |
//!             v
//!      [Final 256-bit Digest]
//! ```
//!
//! Invariants:
//! 1. The input message must be padded such that its length is a multiple of 64 bytes.
//! 2. The padding always includes a `1` bit (0x80) followed by `0` bits, ending with a 64-bit integer representing the original message length in bits.
//! 3. The state variables (`a` through `h`) are initialized to standard constants (fractional parts of square roots of prime numbers).
//!
//! Complexity:
//! +---------------┬-------------┬-------------+
//! | Operation     | Time        | Space       |
//! ├---------------┼-------------┼-------------┤
//! | update        | O(N)        | O(1)        |
//! | finalize      | O(1)        | O(1)        |
//! +---------------┴-------------┴-------------+
//! N = length of the input data block being processed. Space is strictly bounded to the 64-byte buffer and 8 state variables.
//!
//! Design Decisions:
//! - **Buffer Strategy**: A fixed `[u8; 64]` buffer is used to batch input bytes, avoiding dynamic `Vec` allocations during streaming updates.
//!   - *Tradeoff*: Requires manual tracking of `buffer_len`.
//!   - *Alternative*: Allocating a `Vec` and processing it, which introduces O(N) heap allocations and copies.
//! - **Wrapping Arithmetic**: Extensively uses `.wrapping_add()` to prevent Rust from panicking on debug builds when `u32` overflows.

/// Educational SHA-256 Hasher
pub struct Sha256 {
    state: [u32; 8],
    buffer: [u8; 64],
    buffer_len: usize,
    len: u64,
}

impl Sha256 {
    // Initial hash values (first 32 bits of the fractional parts of the square roots of the first 8 primes)
    const H0: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];

    // Round constants (first 32 bits of the fractional parts of the cube roots of the first 64 primes)
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];

    #[must_use]
    pub fn new() -> Self {
        Self {
            state: Self::H0,
            buffer: [0; 64],
            buffer_len: 0,
            len: 0,
        }
    }

    /// Update the hash with new data.
    pub fn update(&mut self, mut data: &[u8]) {
        self.len += data.len() as u64;

        // If we have data in the buffer, try to fill it
        if self.buffer_len > 0 {
            let space = 64 - self.buffer_len;
            if data.len() >= space {
                // Buffer will be filled
                self.buffer[self.buffer_len..64].copy_from_slice(&data[..space]);

                // Rust requires block to live long enough, but here we can just pass a copy or array reference.
                // However, process_block takes &mut self, so we need to copy the buffer out.
                let block = self.buffer;
                self.process_block(&block);

                data = &data[space..];
                self.buffer_len = 0;
            } else {
                // Buffer won't be filled, just append and return
                self.buffer[self.buffer_len..self.buffer_len + data.len()].copy_from_slice(data);
                self.buffer_len += data.len();
                return;
            }
        }

        // Process full 64-byte blocks directly from the input slice
        while data.len() >= 64 {
            let (block, rest) = data.split_at(64);
            self.process_block(block);
            data = rest;
        }

        // Store any remaining bytes in the buffer
        if !data.is_empty() {
            self.buffer[..data.len()].copy_from_slice(data);
            self.buffer_len = data.len();
        }
    }

    /// Finalize the hash and return the 32-byte digest.
    pub fn finalize(mut self) -> [u8; 32] {
        let bit_len = self.len * 8;

        // Append '1' bit (0x80 byte)
        self.buffer[self.buffer_len] = 0x80;
        self.buffer_len += 1;

        // If not enough room for the 8-byte length, pad and process block
        if self.buffer_len > 56 {
            self.buffer[self.buffer_len..64].fill(0x00);
            let block = self.buffer;
            self.process_block(&block);
            self.buffer_len = 0;
        }

        // Pad with '0' bits up to the last 8 bytes
        self.buffer[self.buffer_len..56].fill(0x00);

        // Append length as 64-bit big-endian integer
        self.buffer[56..64].copy_from_slice(&bit_len.to_be_bytes());

        // Process final block
        let block = self.buffer;
        self.process_block(&block);

        // Convert state to bytes (big-endian)
        let mut result = [0u8; 32];
        for (i, &word) in self.state.iter().enumerate() {
            let bytes = word.to_be_bytes();
            result[i * 4..i * 4 + 4].copy_from_slice(&bytes);
        }

        result
    }

    fn process_block(&mut self, block: &[u8]) {
        debug_assert_eq!(block.len(), 64);

        let mut w = [0u32; 64];

        // 1. Prepare message schedule W
        for (i, chunk) in block.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        }

        // RUST INSIGHT: Wrapping arithmetic is essential here. Rust panics on overflow in debug mode.
        // We use the `impl_wrapping_ops` logic implicitly via explicit operators or `wrapping_add`.
        // However, standard `+` is checked. We must use `wrapping_add`.

        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        // 2. Initialize working variables
        let mut a = self.state[0];
        let mut b = self.state[1];
        let mut c = self.state[2];
        let mut d = self.state[3];
        let mut e = self.state[4];
        let mut f = self.state[5];
        let mut g = self.state[6];
        let mut h = self.state[7];

        // 3. Main loop
        for (ki, wi) in Self::K.iter().zip(w.iter()) {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(*ki)
                .wrapping_add(*wi);

            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        // 4. Update state
        self.state[0] = self.state[0].wrapping_add(a);
        self.state[1] = self.state[1].wrapping_add(b);
        self.state[2] = self.state[2].wrapping_add(c);
        self.state[3] = self.state[3].wrapping_add(d);
        self.state[4] = self.state[4].wrapping_add(e);
        self.state[5] = self.state[5].wrapping_add(f);
        self.state[6] = self.state[6].wrapping_add(g);
        self.state[7] = self.state[7].wrapping_add(h);
    }
}

impl Default for Sha256 {
    fn default() -> Self {
        Self::new()
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `sha2`: The standard rust implementation. It utilizes highly optimized architecture-specific assembly instructions (like Intel SHA-NI or ARM Cryptography Extensions) when available, gracefully falling back to standard SIMD or scalar routines.
//
// Missing vs. Production:
// - **Hardware Acceleration**: We only implement the scalar, software fallback version.
// - **Constant-Time Execution**: Educational versions aren't typically scrutinized for side-channels or cache-timing attacks, though SHA-256 naturally avoids data-dependent branching.
//
// Next Steps:
// 1. Explore implementing `portable_simd` (or `core::arch`) optimizations.
// 2. Extend to SHA-512, which uses the same logical structure but with 64-bit words.

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to convert bytes to hex string
    fn bytes_to_hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }

    #[test]
    fn test_empty_string() {
        let mut hasher = Sha256::new();
        hasher.update(b"");
        let hash = hasher.finalize();
        assert_eq!(
            bytes_to_hex(&hash),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn test_abc() {
        let mut hasher = Sha256::new();
        hasher.update(b"abc");
        let hash = hasher.finalize();
        assert_eq!(
            bytes_to_hex(&hash),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn test_multiple_updates() {
        let mut hasher = Sha256::new();
        hasher.update(b"a");
        hasher.update(b"b");
        hasher.update(b"c");
        let hash = hasher.finalize();
        assert_eq!(
            bytes_to_hex(&hash),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn test_long_message() {
        let mut hasher = Sha256::new();
        // 1 million 'a's
        let data = vec![b'a'; 1_000_000];
        // We can't update all at once if we wanted to stream, but our update handles chunks.
        // However, let's feed it in one go to test chunking logic.
        hasher.update(&data);
        let hash = hasher.finalize();
        assert_eq!(
            bytes_to_hex(&hash),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
    }
}
