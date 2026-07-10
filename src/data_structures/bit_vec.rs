//! # Dynamic Bit Vector (`BitSet`)
//!
//! Implements a dynamically sized bit vector (or bitset) where individual bits
//! are packed efficiently into underlying `usize` words.
//!
//! **Replaces Crates:** `bitvec`, `fixedbitset`
//!
//! **Real-world Usage:**
//! - Entity Component Systems (ECS) (tracking which components an entity has).
//! - Bloom filters (storing the hashed bit array).
//! - Graph algorithms (tracking visited nodes efficiently).
//! - Chess engines (bitboards).
//!
//! **Why build it yourself?**
//! Standard `Vec<bool>` in Rust wastes memory because each `bool` takes an entire byte (8 bits).
//! By packing 64 bits into a single `usize` (on 64-bit architectures), you reduce memory usage
//! by 8x and drastically improve CPU cache utilization. Building this teaches you bitwise
//! operations (`<<`, `&`, `|`), alignment, and chunked storage patterns.
//!
//! # Architecture
//!
//! ```text
//! BitVec {
//!     words: Vec<usize>,
//!     len: usize
//! }
//!
//! Index 65 mapping:
//! word_index = 65 / 64 = 1
//! bit_index  = 65 % 64 = 1
//!
//! words[0] [................................................................] (bits 0-63)
//! words[1] [..............................................................1.] (bits 64-127)
//! ```
//!
//! **Invariants:**
//! - `len` represents the number of logical bits, not the capacity of the `words` vector.
//! - Any bits beyond `len` in the last word must be zeroed out (crucial for bitwise operations like `count_ones`).
//!
//! **Complexity:**
//! - `push`: O(1) amortized
//! - `get`: O(1)
//! - `set`: O(1)
//! - Memory footprint: `ceil(len / 64) * 8` bytes

const BITS_PER_WORD: usize = std::mem::size_of::<usize>() * 8;

/// Core operations for a dynamically sized bitset.
pub trait BitSet {
    /// Pushes a boolean value to the end of the bit vector.
    fn push(&mut self, value: bool);
    /// Retrieves the bit at the given index.
    fn get(&self, index: usize) -> Option<bool>;
    /// Sets the bit at the given index to the specified value.
    fn set(&mut self, index: usize, value: bool) -> bool;
    /// Counts the total number of `1` (true) bits.
    fn count_ones(&self) -> usize;
    /// Returns the number of bits stored.
    fn len(&self) -> usize;
    /// Returns `true` if the bit vector is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// A dynamically sized bit vector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BitVec {
    /// Internal storage using CPU-native word sizes for efficient bitwise operations.
    words: Vec<usize>,
    /// Logical number of bits stored.
    len: usize,
}

impl BitSet for BitVec {
    fn push(&mut self, value: bool) {
        let word_index = self.len / BITS_PER_WORD;
        let bit_index = self.len % BITS_PER_WORD;

        if word_index == self.words.len() {
            self.words.push(0);
        }

        // RUST INSIGHT:
        // We use bitwise OR (`|`) to set a bit, and bitwise AND with NOT (`& !`) to clear it.
        // However, since newly allocated words are zeroed out, we only need to set it if `value` is true.
        if value {
            self.words[word_index] |= 1 << bit_index;
        } else {
            self.words[word_index] &= !(1 << bit_index);
        }

        self.len += 1;
    }

    fn get(&self, index: usize) -> Option<bool> {
        if index >= self.len {
            return None;
        }

        let word_index = index / BITS_PER_WORD;
        let bit_index = index % BITS_PER_WORD;

        // Extract the bit by shifting it down to the 0th position and masking it.
        let bit = (self.words[word_index] >> bit_index) & 1;
        Some(bit != 0)
    }

    fn set(&mut self, index: usize, value: bool) -> bool {
        if index >= self.len {
            return false;
        }

        let word_index = index / BITS_PER_WORD;
        let bit_index = index % BITS_PER_WORD;

        if value {
            self.words[word_index] |= 1 << bit_index;
        } else {
            self.words[word_index] &= !(1 << bit_index);
        }

        true
    }

    fn count_ones(&self) -> usize {
        // PRODUCTION NOTE:
        // By maintaining the invariant that unused bits in the final word are always zero,
        // we can simply map over the words and sum `count_ones()` without masking the last word.
        self.words.iter().map(|w| w.count_ones() as usize).sum()
    }

    fn len(&self) -> usize {
        self.len
    }
}

impl BitVec {
    /// Creates a new, empty bit vector.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            words: Vec::new(),
            len: 0,
        }
    }

    /// Creates a bit vector with at least the specified capacity (in bits).
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        let words_capacity = capacity.div_ceil(BITS_PER_WORD);
        Self {
            words: Vec::with_capacity(words_capacity),
            len: 0,
        }
    }
}

// =========================================================================================
// Benchmarking
// =========================================================================================
// Benchmark `BitVec` operations using `criterion`.
// - Measure `push` speed vs `Vec<bool>::push`.
// - Measure memory utilization for large arrays (e.g., 10 million bits).
//
// Example `criterion` benchmark:
// ```rust,ignore
// b.iter(|| {
//     let mut bv = BitVec::with_capacity(1000);
//     for _ in 0..1000 {
//         bv.push(black_box(true));
//     }
// })
// ```

// =========================================================================================
// Footer
// =========================================================================================
//
// **Comparison to Canonical Crate (`bitvec`):**
// The `bitvec` crate provides macro support (`bitvec![1, 0, 1]`), extensive bitwise operations
// between multiple vectors (`&`, `|`, `^`), and allows selecting the underlying storage type
// (e.g., `u8`, `u32`, `usize`) and ordering (LSB vs MSB).
//
// **What's Missing vs. Production:**
// 1. **Bitwise operations:** Missing implementations for `BitAnd`, `BitOr`, `BitXor`.
// 2. **Iterator support:** Missing an iterator to cleanly yield each bit or the indices of set bits.
// 3. **Macros:** No macro `bitvec!` for ergonomic creation.
//
// **Suggested Next Steps:**
// - Implement `std::ops::BitAnd` to intersect two bit vectors.
// - Implement an iterator `iter_ones()` that returns the index of every bit that is set to 1.

impl Default for BitVec {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_and_get() {
        let mut bv = BitVec::new();
        assert_eq!(bv.len(), 0);

        bv.push(true);
        bv.push(false);
        bv.push(true);

        assert_eq!(bv.len(), 3);
        assert_eq!(bv.get(0), Some(true));
        assert_eq!(bv.get(1), Some(false));
        assert_eq!(bv.get(2), Some(true));
        assert_eq!(bv.get(3), None);
    }

    #[test]
    fn test_set() {
        let mut bv = BitVec::new();
        bv.push(false);
        bv.push(false);

        assert!(bv.set(1, true));
        assert_eq!(bv.get(1), Some(true));

        assert!(!bv.set(5, true)); // Out of bounds
    }

    #[test]
    fn test_cross_word_boundary() {
        let mut bv = BitVec::new();

        // Push enough bits to cross into the second word
        for _ in 0..65 {
            bv.push(false);
        }

        assert_eq!(bv.len(), 65);
        assert_eq!(bv.words.len(), 2);

        bv.set(64, true); // Set the first bit of the second word

        assert_eq!(bv.get(63), Some(false));
        assert_eq!(bv.get(64), Some(true));
    }

    #[test]
    fn test_count_ones() {
        let mut bv = BitVec::new();
        for i in 0..100 {
            bv.push(i % 2 == 0); // true for even, false for odd
        }

        assert_eq!(bv.count_ones(), 50);

        bv.set(0, false);
        assert_eq!(bv.count_ones(), 49);
    }
}
