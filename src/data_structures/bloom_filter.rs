//! # Bloom Filter Implementation
//!
//! A space-efficient probabilistic data structure that checks for set membership.
//!
//! **Replaces Crates:** `bloomfilter`, `fastbloom`
//!
//! **Real-world Usage:**
//! - Databases (Cassandra, Postgres) to avoid disk lookups for non-existent rows.
//! - CDNs (Akamai) to avoid caching "one-hit wonders".
//! - Chromium (safe browsing) to check against malicious URL lists.
//!
//! **Why build it yourself?**
//! Understanding the math behind false positive rates and optimal sizing (m bits, k hashes)
//! is crucial for system design. You'll also learn about double hashing to simulate k hash functions.

use std::collections::hash_map::DefaultHasher;
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::{self, Read, Write};
use std::marker::PhantomData;
use std::path::Path;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure: Bit Array (Vec<u64> used as a bitset)
//
// [0, 0, 1, 0, 1, ..., 0]  (m bits)
//
// Operations:
// - Add(item): Hash item k times, set bits at indices h_i % m to 1.
// - Contains(item): Hash item k times, check if all bits at indices h_i % m are 1.
//
// Invariants:
// 1. False negatives are impossible (if it says "no", it's definitely no).
// 2. False positives are possible (if it says "yes", it might be no).
// 3. Items cannot be removed (standard Bloom Filter).
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ Add           │ O(k)        │ O(m) bits   │
// │ Contains      │ O(k)        │ O(m) bits   │
// └───────────────┴─────────────┴─────────────┘
// where k is number of hash functions, m is bit array size.

/// A probabilistic data structure for set membership.
pub struct BloomFilter<T: ?Sized> {
    /// The bit array.
    bit_vec: Vec<u64>,
    /// Number of bits in the filter (m).
    bit_count: u64,
    /// Number of hash functions (k).
    hash_count: u32,
    /// Phantom data to hold the type T.
    _marker: PhantomData<T>,
}

impl<T: ?Sized + Hash> BloomFilter<T> {
    /// Creates a new Bloom Filter optimized for `expected_items` and `false_positive_rate`.
    ///
    /// # Arguments
    /// * `expected_items` - The number of items you expect to insert (n).
    /// * `false_positive_rate` - The desired false positive rate (p) (e.g., 0.01 for 1%).
    pub fn new(expected_items: usize, false_positive_rate: f64) -> Self {
        // RUST INSIGHT:
        // Optimal m = -(n * ln(p)) / (ln(2)^2)
        // Optimal k = (m / n) * ln(2)

        let n = expected_items as f64;
        let p = false_positive_rate;
        let ln2 = std::f64::consts::LN_2;

        let m = -(n * p.ln()) / (ln2 * ln2);
        let k = (m / n) * ln2;

        let bit_count = m.ceil() as u64;
        let hash_count = k.ceil() as u32;

        // Ensure at least 1 bit and 1 hash function
        let bit_count = bit_count.max(1);
        let hash_count = hash_count.max(1);

        // We use u64 for the bit vector, so we need (bit_count + 63) / 64 u64s.
        let vec_size = bit_count.div_ceil(64) as usize;

        Self {
            bit_vec: vec![0; vec_size],
            bit_count,
            hash_count,
            _marker: PhantomData,
        }
    }

    /// Adds an item to the Bloom Filter.
    pub fn add(&mut self, item: &T) {
        let (h1, h2) = self.get_hash_pair(item);

        for i in 0..self.hash_count {
            // Double hashing: h_i = (h1 + i * h2) % m
            // Wrapping add to allow overflow (standard behavior)
            let index = h1.wrapping_add((i as u64).wrapping_mul(h2)) % self.bit_count;
            self.set_bit(index);
        }
    }

    /// Checks if an item might be in the Bloom Filter.
    /// Returns `true` if the item is *probably* present, `false` if it is *definitely* not.
    pub fn contains(&self, item: &T) -> bool {
        let (h1, h2) = self.get_hash_pair(item);

        for i in 0..self.hash_count {
            let index = h1.wrapping_add((i as u64).wrapping_mul(h2)) % self.bit_count;
            if !self.get_bit(index) {
                return false;
            }
        }
        true
    }

    /// Helper to set a bit at the given index.
    fn set_bit(&mut self, index: u64) {
        let vec_idx = (index / 64) as usize;
        let bit_idx = (index % 64) as usize;
        // GOTCHA: Ensure we don't go out of bounds if logic is wrong, though index % bit_count protects us.
        if vec_idx < self.bit_vec.len() {
            self.bit_vec[vec_idx] |= 1 << bit_idx;
        }
    }

    /// Helper to get a bit at the given index.
    fn get_bit(&self, index: u64) -> bool {
        let vec_idx = (index / 64) as usize;
        let bit_idx = (index % 64) as usize;
        if vec_idx < self.bit_vec.len() {
            (self.bit_vec[vec_idx] & (1 << bit_idx)) != 0
        } else {
            false
        }
    }

    /// Generates two 64-bit hashes using double hashing simulation.
    ///
    /// We use `DefaultHasher` which is not cryptographically secure and can vary across Rust versions,
    /// but is sufficient for this educational implementation.
    /// In production, use SipHash (which DefaultHasher often wraps) or Murmur3 explicitly.
    fn get_hash_pair(&self, item: &T) -> (u64, u64) {
        let mut hasher1 = DefaultHasher::new();
        item.hash(&mut hasher1);
        let h1 = hasher1.finish();

        let _hasher2 = DefaultHasher::new();
        // Hash the item again. To get a different hash, we could try to hash something else too,
        // but DefaultHasher doesn't take a seed.
        // PRODUCTION NOTE: A real implementation would use a hasher that supports seeding,
        // or run a different hash algorithm.
        // Here, we simulate a second hash by hashing the first hash bit-flipped or rotated.
        // Actually, just hashing (item, 0) and (item, 1) is better if T allows it, but T is generic.
        //
        // Workaround: We'll use the h1 to mix into the second hasher state differently if possible.
        // Since we can't easily modify the input without cloning, we rely on a trick:
        // We use a different hasher instance and hope internal state randomization (if any) or
        // just accept that we might need a better strategy.
        //
        // Wait, standard `DefaultHasher` is deterministic per process execution.
        // To get a second independent hash for *any* T, we typically need to hash T with a different seed.
        // Since `Hash` trait only exposes `hash<H: Hasher>(&self, state: &mut H)`, we can only control the Hasher.
        // But `DefaultHasher` doesn't let us set a seed.
        //
        // Alternative: Use a custom Hasher that wraps `DefaultHasher` and injects a seed byte first?
        // No, `Hash` calls `state.write...`.
        //
        // Let's implement a simple FNV-1a hasher for the second hash to ensure independence.

        let h2 = {
            let mut hasher = Fnv1aHasher::new(0xcbf29ce484222325); // FNV offset basis
            item.hash(&mut hasher);
            hasher.finish()
        };

        (h1, h2)
    }

    /// Saves the Bloom Filter to a file.
    /// Format: [bit_count (8 bytes)] [hash_count (4 bytes)] [vec_len (8 bytes)] [bit_vec (8 * vec_len bytes)]
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        let mut file = File::create(path)?;
        file.write_all(&self.bit_count.to_le_bytes())?;
        file.write_all(&self.hash_count.to_le_bytes())?;
        file.write_all(&(self.bit_vec.len() as u64).to_le_bytes())?;
        for word in &self.bit_vec {
            file.write_all(&word.to_le_bytes())?;
        }
        Ok(())
    }

    /// Loads a Bloom Filter from a file.
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let mut file = File::open(path)?;

        let mut buffer_u64 = [0u8; 8];
        file.read_exact(&mut buffer_u64)?;
        let bit_count = u64::from_le_bytes(buffer_u64);

        let mut buffer_u32 = [0u8; 4];
        file.read_exact(&mut buffer_u32)?;
        let hash_count = u32::from_le_bytes(buffer_u32);

        file.read_exact(&mut buffer_u64)?;
        let vec_len = u64::from_le_bytes(buffer_u64);

        let mut bit_vec = Vec::with_capacity(vec_len as usize);
        for _ in 0..vec_len {
            file.read_exact(&mut buffer_u64)?;
            bit_vec.push(u64::from_le_bytes(buffer_u64));
        }

        Ok(Self {
            bit_vec,
            bit_count,
            hash_count,
            _marker: PhantomData,
        })
    }
}

// Minimal FNV-1a Hasher for the second hash function.
struct Fnv1aHasher {
    state: u64,
}

impl Fnv1aHasher {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }
}

impl Hasher for Fnv1aHasher {
    fn finish(&self) -> u64 {
        self.state
    }

    fn write(&mut self, bytes: &[u8]) {
        let prime = 1099511628211;
        for byte in bytes {
            self.state ^= *byte as u64;
            self.state = self.state.wrapping_mul(prime);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_add_contains() {
        let mut bf = BloomFilter::new(100, 0.01);
        bf.add("hello");
        bf.add("world");

        assert!(bf.contains("hello"));
        assert!(bf.contains("world"));
        // "foo" might be a false positive, but unlikely with 1% rate and empty filter
        // However, we can't assert !contains("foo") with 100% certainty in a probabilistic test,
        // but for a small test it's usually fine.
        assert!(!bf.contains("foo"));
    }

    #[test]
    fn test_false_positive_rate() {
        // Stress test to check if the FPR is roughly correct.
        let n = 1000;
        let p = 0.05; // 5%
        let mut bf = BloomFilter::new(n, p);

        // Add 0..1000
        for i in 0..n {
            bf.add(&i.to_string());
        }

        // Check 0..1000 are present (no false negatives)
        for i in 0..n {
            assert!(bf.contains(&i.to_string()));
        }

        // Check 1000..2000 (should be absent)
        let mut false_positives = 0;
        let trials = 1000;
        for i in n..(n + trials) {
            if bf.contains(&i.to_string()) {
                false_positives += 1;
            }
        }

        let actual_rate = false_positives as f64 / trials as f64;
        println!("Expected FPR: {}, Actual FPR: {}", p, actual_rate);

        // Allow some variance, but it shouldn't be way off (e.g. > 2*p)
        assert!(
            actual_rate < p * 2.0 + 0.01,
            "FPR too high: {}",
            actual_rate
        );
    }

    #[test]
    fn test_small_filter() {
        // Edge case: small capacity
        let mut bf = BloomFilter::new(1, 0.5);
        bf.add("a");
        assert!(bf.contains("a"));
    }

    #[test]
    fn test_save_load() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = format!("test_bloom_filter_{}.bin", now);

        let mut bf = BloomFilter::new(100, 0.01);
        bf.add("persist");
        bf.save_to_file(&path).unwrap();

        let loaded_bf: BloomFilter<String> = BloomFilter::load_from_file(&path).unwrap();
        assert!(loaded_bf.contains(&"persist".to_string()));
        // "missing" should be false with high probability (99%)
        // but technically could be true. We trust it won't be for this specific case.
        assert!(!loaded_bf.contains(&"missing".to_string()));

        std::fs::remove_file(path).unwrap();
    }
}
