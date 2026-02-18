//! # SHA-256 (Simplified)
//!
//! # Header
//!
//! *   **Problem Name**: SHA-256 Hashing Algorithm
//! *   **Difficulty**: Hard (Bitwise Arithmetic)
//! *   **Link**: <https://en.wikipedia.org/wiki/SHA-2>
//! *   **Why this matters in Rust**: Understanding hash functions is crucial for cryptography, data integrity, and blockchain.
//!
//! # Architecture
//!
//! SHA-256 (Secure Hash Algorithm 256-bit) operates on 512-bit message blocks.
//!
//! **Structure:**
//! 1.  **Padding**: Append `1` bit, then `0` bits, then the 64-bit message length (in bits) to make the total length a multiple of 512 bits.
//! 2.  **Message Schedule (W)**: Expand the 16 words (32-bit) of the block into 64 words.
//! 3.  **Compression Function**: Update the 8 state variables (a-h) using bitwise operations (Ch, Maj, Sigma, sigma) over 64 rounds.
//! 4.  **Final Hash**: Concatenate the final state variables.
//!
//! **Complexity:**
//!
//! | Operation | Time | Space |
//! | :--- | :--- | :--- |
//! | Hash | O(N) | O(1) |
//!
//! where N is the message length in blocks. Space is constant (state variables).
//!
//! # Educational Note
//!
//! **WARNING**: This is an educational implementation. It is **not constant-time** and **not side-channel resistant**.
//! For production, use the `sha2` crate which uses SIMD optimizations and hardware instructions (SHA-NI).

/// Educational SHA-256 Hasher
pub struct Sha256 {
    state: [u32; 8],
    data: Vec<u8>,
    len: u64,
}

impl Sha256 {
    // Initial hash values (first 32 bits of the fractional parts of the square roots of the first 8 primes)
    const H0: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
        0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
    ];

    // Round constants (first 32 bits of the fractional parts of the cube roots of the first 64 primes)
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
        0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
        0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
        0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
        0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];

    #[must_use]
    pub fn new() -> Self {
        Self {
            state: Self::H0,
            data: Vec::new(),
            len: 0,
        }
    }

    /// Update the hash with new data.
    pub fn update(&mut self, data: &[u8]) {
        self.data.extend_from_slice(data);
        self.len += data.len() as u64;

        // Process full 64-byte blocks
        while self.data.len() >= 64 {
            let block: Vec<u8> = self.data.drain(..64).collect();
            self.process_block(&block);
        }
    }

    /// Finalize the hash and return the 32-byte digest.
    pub fn finalize(mut self) -> [u8; 32] {
        // Padding
        let bit_len = self.len * 8;

        // Append '1' bit (0x80 byte)
        self.data.push(0x80);

        // Append '0' bits until length % 64 == 56
        while (self.data.len() % 64) != 56 {
            self.data.push(0x00);
        }

        // Append length as 64-bit big-endian integer
        self.data.extend_from_slice(&bit_len.to_be_bytes());

        // Process final block(s)
        // Since we padded to multiple of 64, we can process normally
        while !self.data.is_empty() {
            let block: Vec<u8> = self.data.drain(..64).collect();
            self.process_block(&block);
        }

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
