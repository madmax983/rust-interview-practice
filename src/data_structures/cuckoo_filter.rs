//! # Cuckoo Filter Implementation
//!
//! A probabilistic data structure that supports adding, checking, and *deleting* items.
//! It offers higher space efficiency than Bloom Filters for low false positive rates and supports deletion.
//!
//! **Replaces Crates:** `cuckoofilter`, `vacu`
//!
//! **Real-world Usage:**
//! - Distributed databases (LSM-tree optimizations) to replace Bloom Filters where deletion is needed.
//! - Network routers for high-speed IP lookups and packet classification.
//!
//! **Why build it yourself?**
//! You'll learn about "cuckoo hashing" (using multiple hash functions and relocating items on collision),
//! fingerprinting, and how to manage partial-key collisions. It's a great example of trading computation (relocation) for space.

use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

// =========================================================================================
// FNV-1a Hasher Implementation (Deterministic)
// =========================================================================================
// We implement a custom Hasher to ensure deterministic behavior across process runs
// and to guarantee that `alt_index` calculation is consistent. Standard `DefaultHasher`
// uses a random seed per process, which would break Cuckoo Filter logic if used blindly
// (specifically `alt_index` must be the same for a given fingerprint regardless of when/where it's calculated).

pub struct FnvHasher {
    state: u64,
}

impl FnvHasher {
    const OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;

    #[must_use] 
    pub const fn new() -> Self {
        Self {
            state: Self::OFFSET_BASIS,
        }
    }
}

impl Hasher for FnvHasher {
    fn finish(&self) -> u64 {
        self.state
    }

    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.state ^= u64::from(byte);
            self.state = self.state.wrapping_mul(Self::PRIME);
        }
    }
}

impl Default for FnvHasher {
    fn default() -> Self {
        Self::new()
    }
}

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure:
// - A table of Buckets.
// - Each Bucket holds fixed number of Fingerprints (e.g., 4).
// - Fingerprint is a short hash (e.g., 16 bits) of the item.
//
// Operations:
// - Insert(x):
//   1. Compute f = fingerprint(x).
//   2. Compute i1 = hash(x).
//   3. Compute i2 = i1 ^ hash(f).
//   4. If bucket[i1] or bucket[i2] has space, put f there.
//   5. Else, pick a random entry from i1 or i2, swap f with it, and re-insert the victim (kick).
//   6. Repeat up to MaxKicks.
//
// - Contains(x):
//   1. Compute f, i1, i2.
//   2. Check if f is in bucket[i1] or bucket[i2].
//
// - Delete(x):
//   1. Compute f, i1, i2.
//   2. Remove f from bucket[i1] or bucket[i2] if found.
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Average     │ Worst Case  │
// ├───────────────┼─────────────┼─────────────┤
// │ Insert        │ O(1)        │ O(MaxKicks) │
// │ Contains      │ O(1)        │ O(1)        │
// │ Delete        │ O(1)        │ O(1)        │
// │ Space         │ O(n)        │ O(n)        │
// └───────────────┴─────────────┴─────────────┘
//
// Invariants:
// 1. Table size is a power of 2 (to simplify index calculation via XOR).
// 2. Fingerprints are non-zero (0 is used as empty sentinel, though `Option` handles this safely in Rust).

// RUST INSIGHT:
// Defining a trait allows us to swap this probabilistic filter with others (like BloomFilter)
// in a system without changing the consuming code.
pub trait Filter<T: ?Sized> {
    fn insert(&mut self, item: &T) -> bool;
    fn contains(&self, item: &T) -> bool;
    fn delete(&mut self, item: &T) -> bool;
}

const BUCKET_SIZE: usize = 4;
const MAX_KICKS: usize = 500;

type Fingerprint = u16; // 16-bit fingerprint (good for ~0.003% false positive rate)

#[derive(Clone, Copy, Debug, PartialEq)]
struct Bucket {
    entries: [Option<Fingerprint>; BUCKET_SIZE],
}

impl Bucket {
    const fn new() -> Self {
        Self {
            entries: [None; BUCKET_SIZE],
        }
    }

    fn insert(&mut self, fp: Fingerprint) -> bool {
        for entry in &mut self.entries {
            if entry.is_none() {
                *entry = Some(fp);
                return true;
            }
        }
        false
    }

    fn remove(&mut self, fp: Fingerprint) -> bool {
        for entry in &mut self.entries {
            if let Some(existing) = entry
                && *existing == fp
            {
                *entry = None;
                return true;
            }
        }
        false
    }

    fn contains(&self, fp: Fingerprint) -> bool {
        for entry in &self.entries {
            if let Some(existing) = entry
                && *existing == fp
            {
                return true;
            }
        }
        false
    }

    #[allow(dead_code)]
    fn is_full(&self) -> bool {
        self.entries.iter().all(std::option::Option::is_some)
    }
}

pub struct CuckooFilter<T: ?Sized> {
    buckets: Vec<Bucket>,
    len: usize,
    rng: XorShift,
    _marker: PhantomData<T>,
}

impl<T: ?Sized + Hash> CuckooFilter<T> {
    /// Creates a new Cuckoo Filter with capacity for at least `capacity` items.
    /// Actual capacity will be rounded up to the next power of 2 of buckets.
    #[must_use] 
    pub fn new(capacity: usize) -> Self {
        // Target 95% load factor roughly.
        // capacity / 4 = num_buckets needed (since 4 slots per bucket).
        // We round up to power of 2.
        let mut num_buckets = capacity.div_ceil(BUCKET_SIZE);
        if num_buckets == 0 {
            num_buckets = 1;
        }
        num_buckets = num_buckets.next_power_of_two();

        Self {
            buckets: vec![Bucket::new(); num_buckets],
            len: 0,
            rng: XorShift::new(12345),
            _marker: PhantomData,
        }
    }

    /// Returns the number of items currently in the filter.
    #[must_use] 
    pub const fn len(&self) -> usize {
        self.len
    }

    #[must_use] 
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Adds an item to the filter. Returns true on success, false if the filter is too full.
    pub fn insert(&mut self, item: &T) -> bool {
        let (mut f, i1) = self.generate_fingerprint_and_index(item);
        let i2 = self.alt_index(i1, f);

        // RUST INSIGHT:
        // We access buckets by index rather than holding references to avoid fighting the borrow checker,
        // which would complain if we tried to hold mutable references to two different buckets simultaneously
        // without disjoint capture logic.

        // 1. Try insert in i1
        if self.buckets[i1].insert(f) {
            self.len += 1;
            return true;
        }

        // 2. Try insert in i2
        if self.buckets[i2].insert(f) {
            self.len += 1;
            return true;
        }

        // 3. Kick out victims
        let mut curr_index = if self.rng.next_bool() { i1 } else { i2 };

        for _ in 0..MAX_KICKS {
            // Pick a random slot in the bucket to swap
            let slot = (self.rng.next_u32() as usize) % BUCKET_SIZE;

            // Swap f with the entry in the slot
            // GOTCHA: We must ensure we don't accidentally drop the old fingerprint; it must be re-inserted.
            // We know the bucket is full (otherwise we would have inserted), so unwrapping is safe logic-wise.
            // But let's be safe.
            if let Some(old_fp) = self.buckets[curr_index].entries[slot] {
                self.buckets[curr_index].entries[slot] = Some(f);
                f = old_fp;

                // Move to alternate location
                curr_index = self.alt_index(curr_index, f);

                // Try to insert there
                if self.buckets[curr_index].insert(f) {
                    self.len += 1;
                    return true;
                }
                // If full, we loop and kick again
            } else {
                // Should not happen if logic is correct
                self.buckets[curr_index].entries[slot] = Some(f);
                self.len += 1;
                return true;
            }
        }

        // Filter is full
        false
    }

    /// Checks if the item is in the filter.
    pub fn contains(&self, item: &T) -> bool {
        let (f, i1) = self.generate_fingerprint_and_index(item);
        let i2 = self.alt_index(i1, f);

        self.buckets[i1].contains(f) || self.buckets[i2].contains(f)
    }

    /// Removes an item from the filter.
    pub fn delete(&mut self, item: &T) -> bool {
        let (f, i1) = self.generate_fingerprint_and_index(item);
        let i2 = self.alt_index(i1, f);

        if self.buckets[i1].remove(f) {
            self.len -= 1;
            return true;
        }
        if self.buckets[i2].remove(f) {
            self.len -= 1;
            return true;
        }
        false
    }

    // --- Helpers ---

    fn generate_fingerprint_and_index(&self, item: &T) -> (Fingerprint, usize) {
        let mut hasher = FnvHasher::new();
        item.hash(&mut hasher);
        let hash = hasher.finish();

        // Fingerprint: use high bits or low bits. We'll use low 16 bits.
        // Ensure fingerprint is not 0 (optional, but 0 is often sentinel).
        let mut fp = (hash & 0xFFFF) as Fingerprint;
        if fp == 0 {
            fp = 1;
        }

        // Index: hash modulo buckets.
        // Since buckets len is power of 2, we can use mask.
        // But hash >> 16 to use different bits than fingerprint is safer for independence.
        let index_hash = hash >> 16;
        let index = (index_hash as usize) & (self.buckets.len() - 1);

        (fp, index)
    }

    fn alt_index(&self, index: usize, fp: Fingerprint) -> usize {
        // i2 = i1 ^ hash(f)
        let mut hasher = FnvHasher::new();
        fp.hash(&mut hasher);
        let hash_fp = hasher.finish();

        // We need to match the bit-width of the index.
        let index_mask = self.buckets.len() - 1;

        // Note: The hash of the fingerprint is much larger than the index space.
        // We take the relevant bits.
        // Crucial: The transformation must be reversible: alt(alt(i, f), f) == i
        // (i ^ h) ^ h = i. Correct.

        // We use the same bits of hash_fp as we did for index.
        let hash_component = (hash_fp as usize) & index_mask;

        index ^ hash_component
    }
}

// =========================================================================================
// Helper: PRNG (XorShift)
// =========================================================================================

struct XorShift {
    state: u32,
}

impl XorShift {
    const fn new(seed: u32) -> Self {
        Self {
            state: if seed == 0 { 12345 } else { seed },
        }
    }

    const fn next_u32(&mut self) -> u32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        x
    }

    const fn next_bool(&mut self) -> bool {
        self.next_u32().is_multiple_of(2)
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `cuckoofilter`: Simd-optimized, highly tuned Cuckoo Filter.
// - `bloomfilter`: Standard Bloom Filter (no delete).
//
// Missing vs. Production:
// - **SIMD**: Production implementations use SIMD to check all 4 bucket slots in parallel.
// - **Fingerprint resizing**: Support for dynamic sizing or semi-sorting buckets to save bits.
// - **Serialization**: Now possible due to deterministic hashing, but `serde` impl not included here.
//
// Next Steps:
// 1. Implement `iter()` (hard because we only store fingerprints, not original items).
// 2. Add `resize()` (requires rehashing all items, which is impossible without storing original items - Cuckoo filters generally can't grow easily).

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_ops() {
        let mut cf = CuckooFilter::new(100);

        assert!(cf.insert("hello"));
        assert!(cf.insert("world"));

        assert!(cf.contains("hello"));
        assert!(cf.contains("world"));
        assert!(!cf.contains("foo"));

        assert!(cf.delete("hello"));
        assert!(!cf.contains("hello"));
        assert!(cf.contains("world"));
    }

    #[test]
    fn test_full_capacity_behavior() {
        // Small filter: 4 buckets * 4 slots = 16 items max.
        let mut cf = CuckooFilter::new(16);

        let mut inserted = 0;
        // Try to insert more than capacity
        for i in 0..100 {
            if cf.insert(&i) {
                inserted += 1;
            }
        }

        println!("Inserted {} items into capacity 16", inserted);
        assert!(inserted >= 15); // Should fit close to max
        assert!(inserted <= 16); // Hard limit
    }

    #[test]
    fn test_false_positives() {
        // It's probabilistic, but fingerprints are 16-bit, so collision is 1/65536 per bucket check.
        // With 2 buckets, roughly 2/65536.
        let mut cf = CuckooFilter::new(1000);
        for i in 0..100 {
            cf.insert(&i);
        }

        let mut fp_count = 0;
        for i in 1000..2000 {
            if cf.contains(&i) {
                fp_count += 1;
            }
        }

        assert!(fp_count < 5, "Too many false positives: {}", fp_count);
    }

    #[test]
    fn test_delete_non_existent() {
        let mut cf = CuckooFilter::<i32>::new(100);
        assert!(!cf.delete(&1));
        cf.insert(&1);
        assert!(cf.delete(&1));
        assert!(!cf.delete(&1));
    }

    #[test]
    fn test_determinism() {
        let mut cf1 = CuckooFilter::new(100);
        let mut cf2 = CuckooFilter::new(100);

        let items = vec!["a", "b", "c", "d", "e", "f", "g", "h"];

        for item in &items {
            cf1.insert(item);
            cf2.insert(item);
        }

        for item in &items {
            assert!(cf1.contains(item));
            assert!(cf2.contains(item));
        }

        // Also verify indices are same (indirectly via kick behavior if we fill it up)
        // With same capacity and same items and same RNG seed (12345), structure must be identical.
        // We can't inspect internal structure easily, but behavior is consistent.
    }
}
