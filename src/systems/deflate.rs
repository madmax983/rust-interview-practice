//! # DEFLATE Compression Algorithm (Educational)
//!
//! Implements a simplified, educational version of the DEFLATE compression algorithm from scratch.
//! DEFLATE is the core algorithm behind gzip, zlib, and PNG images, combining LZ77 sliding window
//! compression with Huffman coding.
//!
//! **Replaces Crates:** `flate2`, `miniz_oxide`
//!
//! **Real-world Usage:**
//! - HTTP payload compression (`Content-Encoding: gzip/deflate`).
//! - Asset bundling and compression in games and web development.
//! - Image compression (PNG format).
//!
//! **Why build it yourself?**
//! Implementing DEFLATE demystifies how "zipping" a file actually works. You'll learn
//! how sliding window compression (LZ77) eliminates redundant data over space, and how
//! Huffman coding eliminates redundancy over symbol frequency. It also forces you to handle
//! bit-level IO operations in Rust, crossing standard byte boundaries.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure Diagram (DEFLATE Pipeline):
//
//  [Raw Bytes]  ──►  LZ77 Pass  ──►  [Tokens: Literals & Matches]
//                                              │
//                                              ▼
//                     Huffman Pass  ◄──  [Frequency Analysis]
//                          │
//                          ▼
//                  [Bit-Packed Stream]
//
// Invariants:
// 1. The LZ77 sliding window does not reference data beyond its size.
// 2. Matches must have a minimum length to be worth encoding.
// 3. Huffman trees must satisfy the prefix property (no code is a prefix of another).
// 4. Bit padding must be applied at the end of the stream to align to byte boundaries.
//
// Complexity:
// ┌──────────────────┬──────────────────┬─────────────────┐
// │ Operation        │ Time             │ Space           │
// ├──────────────────┼──────────────────┼─────────────────┤
// │ Compress (LZ77)  │ O(N * W)         │ O(N)            │
// │ Huffman Build    │ O(A log A)       │ O(A)            │
// │ Decompress       │ O(N)             │ O(N)            │
// └──────────────────┴──────────────────┴─────────────────┘
// * N = input size, W = window size, A = alphabet size (258 in our case)
//
// Design Decisions & Tradeoffs vs RFC 1951 (Real DEFLATE):
// - **LZ77 Search**: Uses a naive backward search within the window limit.
//   - *Production Note*: Real implementations use hash chains or suffix trees to find matches in O(1)/O(log N).
// - **Huffman Coding**: Uses a single dynamic Huffman tree for literals, matches, and EOF.
//   - *Production Note*: RFC 1951 uses two complex trees (one for Literal/Length, one for Distance)
//     and encodes lengths/distances using base tables + extra bits. We simply emit raw bits
//     for length and distance after a Match token for educational clarity.
// - **Bit IO**: Simple BitWriter/BitReader accumulating bits in a u64 buffer.

const MATCH_MARKER: u16 = 256;
const EOF_MARKER: u16 = 257;
const MIN_MATCH_LEN: usize = 3;
const MAX_MATCH_LEN: usize = 255 + MIN_MATCH_LEN; // Fit length in 8 bits
const WINDOW_SIZE: usize = 32768;

/// A token emitted by the LZ77 compression pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lz77Token {
    Literal(u8),
    Match { length: u16, distance: u16 },
}

/// A bit writer for packing variable-length codes into bytes.
struct BitWriter {
    bytes: Vec<u8>,
    accumulator: u64,
    bits_in_acc: u8,
}

impl BitWriter {
    fn new() -> Self {
        Self {
            bytes: Vec::new(),
            accumulator: 0,
            bits_in_acc: 0,
        }
    }

    /// Writes the lower `num_bits` of `bits` into the buffer.
    fn write_bits(&mut self, bits: u64, num_bits: u8) {
        // RUST INSIGHT: Bitwise operations in Rust are strict.
        // `accumulator` is u64, preventing overflow for up to 56 bits of buffering.
        self.accumulator |= (bits & ((1 << num_bits) - 1)) << self.bits_in_acc;
        self.bits_in_acc += num_bits;

        while self.bits_in_acc >= 8 {
            self.bytes.push((self.accumulator & 0xFF) as u8);
            self.accumulator >>= 8;
            self.bits_in_acc -= 8;
        }
    }

    /// Flushes any remaining bits, padding with 0s to the next byte boundary.
    fn flush(mut self) -> Vec<u8> {
        if self.bits_in_acc > 0 {
            self.bytes.push((self.accumulator & 0xFF) as u8);
        }
        self.bytes
    }
}

/// A bit reader for extracting variable-length codes from bytes.
struct BitReader<'a> {
    bytes: &'a [u8],
    byte_idx: usize,
    accumulator: u64,
    bits_in_acc: u8,
}

impl<'a> BitReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            byte_idx: 0,
            accumulator: 0,
            bits_in_acc: 0,
        }
    }

    /// Reads `num_bits` bits from the buffer. Returns None if EOF.
    fn read_bits(&mut self, num_bits: u8) -> Option<u64> {
        while self.bits_in_acc < num_bits {
            if self.byte_idx >= self.bytes.len() {
                return None; // Not enough bits
            }
            self.accumulator |= (self.bytes[self.byte_idx] as u64) << self.bits_in_acc;
            self.bits_in_acc += 8;
            self.byte_idx += 1;
        }

        let mask = (1 << num_bits) - 1;
        let res = self.accumulator & mask;
        self.accumulator >>= num_bits;
        self.bits_in_acc -= num_bits;
        Some(res)
    }

    /// Reads a single bit.
    fn read_bit(&mut self) -> Option<bool> {
        self.read_bits(1).map(|b| b == 1)
    }
}

/// A node in the Huffman Tree.
#[derive(Debug, Clone, Eq, PartialEq)]
struct FreqNode {
    freq: usize,
    id: usize, // Tie-breaker for stable sorting
    symbol: Option<u16>,
    left: Option<Box<FreqNode>>,
    right: Option<Box<FreqNode>>,
}

// Implement custom Ord to make FreqNode a Min-Heap element based on frequency.
impl Ord for FreqNode {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Reverse because BinaryHeap is a Max-Heap, and we want smallest frequencies first.
        other
            .freq
            .cmp(&self.freq)
            .then_with(|| other.id.cmp(&self.id))
    }
}

impl PartialOrd for FreqNode {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// A generic trait for data compression algorithms.
/// This allows swappable strategies (e.g., Deflate, LZW, Snappy) without changing consumer code.
pub trait Compressor {
    /// Compresses byte data into a specific format.
    fn compress(data: &[u8]) -> Vec<u8>;

    /// Decompresses data compressed by this algorithm.
    fn decompress(data: &[u8]) -> Result<Vec<u8>, &'static str>;
}

/// Simplified DEFLATE compressor.
pub struct Deflate;

impl Compressor for Deflate {
    /// Compresses byte data into our custom DEFLATE format.
    fn compress(data: &[u8]) -> Vec<u8> {
        if data.is_empty() {
            return Vec::new();
        }

        // 1. LZ77 Pass
        let tokens = Self::lz77_compress(data);

        // 2. Frequency Analysis
        let mut freqs = HashMap::new();
        for &t in &tokens {
            match t {
                Lz77Token::Literal(b) => *freqs.entry(b as u16).or_insert(0) += 1,
                Lz77Token::Match { .. } => *freqs.entry(MATCH_MARKER).or_insert(0) += 1,
            }
        }
        *freqs.entry(EOF_MARKER).or_insert(0) += 1; // Ensure EOF is in the tree

        // 3. Build Huffman Tree
        let tree = Self::build_huffman_tree(&freqs).expect("Must have at least one symbol");

        // 4. Generate Prefix Codes
        let mut codes = HashMap::new();
        Self::generate_codes(&tree, 0, 0, &mut codes);

        // 5. Serialize
        let mut writer = BitWriter::new();

        // Write the tree structure so the decoder can rebuild it.
        Self::serialize_tree(&tree, &mut writer);

        // Write tokens
        for t in tokens {
            match t {
                Lz77Token::Literal(b) => {
                    let (code, bits) = codes[&(b as u16)];
                    writer.write_bits(code, bits);
                }
                Lz77Token::Match { length, distance } => {
                    let (code, bits) = codes[&MATCH_MARKER];
                    writer.write_bits(code, bits);

                    // GOTCHA: We write length as 8 bits and distance as 16 bits *uncompressed*.
                    // This deviates from real DEFLATE which uses a second Huffman tree for distances
                    // and base values + extra bits.
                    writer.write_bits((length - MIN_MATCH_LEN as u16) as u64, 8);
                    writer.write_bits(distance as u64, 16);
                }
            }
        }

        // Write EOF
        let (code, bits) = codes[&EOF_MARKER];
        writer.write_bits(code, bits);

        writer.flush()
    }

    /// Decompresses data compressed by our custom DEFLATE format.
    fn decompress(data: &[u8]) -> Result<Vec<u8>, &'static str> {
        if data.is_empty() {
            return Ok(Vec::new());
        }

        let mut reader = BitReader::new(data);

        // 1. Read Huffman Tree
        let tree = Self::deserialize_tree(&mut reader).ok_or("Failed to read Huffman tree")?;

        let mut out = Vec::new();

        // 2. Decode Stream
        loop {
            // Walk tree to find next symbol
            let mut current = &tree;
            loop {
                if let Some(symbol) = current.symbol {
                    if symbol == EOF_MARKER {
                        return Ok(out);
                    } else if symbol == MATCH_MARKER {
                        // Read length and distance
                        let len_encoded = reader
                            .read_bits(8)
                            .ok_or("Unexpected end of stream reading match length")?;
                        let distance = reader
                            .read_bits(16)
                            .ok_or("Unexpected end of stream reading match distance")?;

                        let length = len_encoded as u16 + MIN_MATCH_LEN as u16;

                        // Copy from window
                        // RUST INSIGHT: We must be careful to handle overlapping copies properly!
                        // If `distance` < `length`, we copy bytes we just appended.
                        let start_idx = out
                            .len()
                            .checked_sub(distance as usize)
                            .ok_or("Invalid match distance")?;
                        for i in 0..length {
                            let b = out[start_idx + i as usize];
                            out.push(b);
                        }
                    } else {
                        // Literal
                        out.push(symbol as u8);
                    }
                    break;
                } else {
                    let bit = reader
                        .read_bit()
                        .ok_or("Unexpected end of stream navigating tree")?;
                    if bit {
                        current = current.right.as_ref().unwrap();
                    } else {
                        current = current.left.as_ref().unwrap();
                    }
                }
            }
        }
    }
}

impl Deflate {}

impl Deflate {
    /// Naive LZ77 sliding window compression.
    fn lz77_compress(data: &[u8]) -> Vec<Lz77Token> {
        // ⚡ BOLT OPTIMIZATION: Avoid intermediate reallocations by pre-allocating tokens based on typical compression ratios.
        // Even a conservative estimate (e.g. data.len() / 4) reduces reallocations significantly on large inputs.
        let mut tokens = Vec::with_capacity(data.len() / 4);
        let mut i = 0;

        while i < data.len() {
            let mut best_match = None;
            let mut best_len = 0;

            let window_start = i.saturating_sub(WINDOW_SIZE);

            // Search backwards in the window for the longest match.
            for j in window_start..i {
                let mut len = 0;
                while len < MAX_MATCH_LEN && i + len < data.len() && data[j + len] == data[i + len]
                {
                    len += 1;
                }

                if len >= MIN_MATCH_LEN && len > best_len {
                    best_len = len;
                    // distance is how far back from the current position the match starts
                    best_match = Some((i - j) as u16);
                }
            }

            if let Some(dist) = best_match {
                tokens.push(Lz77Token::Match {
                    length: best_len as u16,
                    distance: dist,
                });
                i += best_len;
            } else {
                tokens.push(Lz77Token::Literal(data[i]));
                i += 1;
            }
        }
        tokens
    }

    fn build_huffman_tree(freqs: &HashMap<u16, usize>) -> Option<Box<FreqNode>> {
        let mut heap = BinaryHeap::new();
        let mut id_gen = 0;

        for (&sym, &freq) in freqs {
            heap.push(FreqNode {
                freq,
                id: id_gen,
                symbol: Some(sym),
                left: None,
                right: None,
            });
            id_gen += 1;
        }

        if heap.is_empty() {
            return None;
        }

        while heap.len() > 1 {
            let left = heap.pop().unwrap();
            let right = heap.pop().unwrap();

            heap.push(FreqNode {
                freq: left.freq + right.freq,
                id: id_gen,
                symbol: None,
                left: Some(Box::new(left)),
                right: Some(Box::new(right)),
            });
            id_gen += 1;
        }

        Some(Box::new(heap.pop().unwrap()))
    }

    fn generate_codes(node: &FreqNode, path: u64, depth: u8, codes: &mut HashMap<u16, (u64, u8)>) {
        if let Some(sym) = node.symbol {
            codes.insert(sym, (path, depth));
        } else {
            if let Some(ref l) = node.left {
                // Left branch is bit 0
                Self::generate_codes(l, path, depth + 1, codes);
            }
            if let Some(ref r) = node.right {
                // Right branch is bit 1
                Self::generate_codes(r, path | (1 << depth), depth + 1, codes);
            }
        }
    }

    fn serialize_tree(node: &FreqNode, writer: &mut BitWriter) {
        if let Some(sym) = node.symbol {
            writer.write_bits(1, 1); // 1 indicates Leaf
            writer.write_bits(sym as u64, 9); // 9 bits handles 0-257
        } else {
            writer.write_bits(0, 1); // 0 indicates Internal node
            if let Some(ref l) = node.left {
                Self::serialize_tree(l, writer);
            }
            if let Some(ref r) = node.right {
                Self::serialize_tree(r, writer);
            }
        }
    }

    fn deserialize_tree(reader: &mut BitReader) -> Option<Box<FreqNode>> {
        let is_leaf = reader.read_bit()?;
        if is_leaf {
            let sym = reader.read_bits(9)? as u16;
            Some(Box::new(FreqNode {
                freq: 0,
                id: 0,
                symbol: Some(sym),
                left: None,
                right: None,
            }))
        } else {
            let left = Self::deserialize_tree(reader)?;
            let right = Self::deserialize_tree(reader)?;
            Some(Box::new(FreqNode {
                freq: 0,
                id: 0,
                symbol: None,
                left: Some(left),
                right: Some(right),
            }))
        }
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `flate2`: Uses an optimized C or Rust backend (`miniz_oxide`) to compress. It strictly
//   adheres to RFC 1951, utilizing dynamic lengths/distances base tables, multiple blocks,
//   and highly optimized suffix arrays or hash chains for O(1) LZ77 matching.
//
// What's missing vs. production:
// - **RFC 1951 Compliance**: This output cannot be decompressed by standard gzip. We simplified
//   the bitstream format to focus purely on the algorithmic concepts.
// - **Performance**: O(N * W) LZ77 search is too slow for large files.
// - **Block Segmentation**: Real DEFLATE splits data into blocks, building new Huffman trees
//   when data characteristics change.
// - **Length/Distance Huffman Trees**: Real DEFLATE encodes distances using a secondary Huffman tree.
//
// Suggested next steps:
// 1. Implement a Hash Chain for LZ77 matching to achieve O(1) searches.
// 2. Separate Lengths and Distances into their own respective tables (RFC 1951 standard).

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lz77_basic_match() {
        // "abracadabra"
        let data = b"abracadabra";
        let tokens = Deflate::lz77_compress(data);

        _ = 0;
        let mut match_count = 0;

        for t in &tokens {
            match t {
                Lz77Token::Literal(_) => (),
                Lz77Token::Match { .. } => match_count += 1,
            }
        }
        // At least one match is expected since "abra" repeats.
        assert!(match_count > 0);
    }

    #[test]
    fn test_bit_writer_reader() {
        let mut writer = BitWriter::new();
        writer.write_bits(0b101, 3);
        writer.write_bits(0b1100, 4);
        writer.write_bits(0b1, 1);
        let bytes = writer.flush();

        let mut reader = BitReader::new(&bytes);
        assert_eq!(reader.read_bits(3), Some(0b101));
        assert_eq!(reader.read_bits(4), Some(0b1100));
        assert_eq!(reader.read_bits(1), Some(0b1));
        assert_eq!(reader.read_bits(1), None);
    }

    #[test]
    fn test_deflate_compress_decompress_empty() {
        let data = b"";
        let compressed = Deflate::compress(data);
        let decompressed = Deflate::decompress(&compressed).unwrap();
        assert_eq!(data, decompressed.as_slice());
    }

    #[test]
    fn test_deflate_compress_decompress_no_matches() {
        let data = b"abcdefg"; // All unique, no matches
        let compressed = Deflate::compress(data);
        let decompressed = Deflate::decompress(&compressed).unwrap();
        assert_eq!(data, decompressed.as_slice());
    }

    #[test]
    fn test_deflate_compress_decompress_highly_compressible() {
        let data = vec![b'A'; 1000]; // 1000 'A's, highly compressible
        let compressed = Deflate::compress(&data);

        // Should be much smaller than 1000 bytes
        assert!(compressed.len() < 100);

        let decompressed = Deflate::decompress(&compressed).unwrap();
        assert_eq!(data, decompressed);
    }

    #[test]
    fn test_deflate_compress_decompress_mixed() {
        let data = b"The quick brown fox jumps over the lazy dog. The quick brown fox jumps again!";
        let compressed = Deflate::compress(data);
        let decompressed = Deflate::decompress(&compressed).unwrap();
        assert_eq!(data, decompressed.as_slice());
    }
}

// Benchmarking note
// To benchmark this implementation, you would typically use the `criterion` crate.
// Create a `benches/deflate_bench.rs` file, generate various payloads (e.g., highly compressible,
// random noise, English text), and benchmark both compression and decompression times.
// Use `std::hint::black_box` to prevent the compiler from optimizing away the calls.
