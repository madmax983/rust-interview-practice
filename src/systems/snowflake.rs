//! # Snowflake ID Generator Implementation
//!
//! Generates unique, distributed, k-ordered 64-bit integers.
//!
//! **Replaces Crates:** `rs-snowflake`, `sonyflake`, `uuid` (for specific use-cases)
//!
//! **Real-world Usage:**
//! - Twitter (original creator for tweets).
//! - Discord (message IDs, user IDs).
//! - Instagram (custom sharding IDs).
//! - Distributed databases (primary keys).
//!
//! **Why build it yourself?**
//! Understanding Snowflake IDs demystifies how massive scale systems generate unique keys
//! without a central bottleneck (like a single database auto-incrementing column).
//! It teaches you about bitwise operations, epoch offsets, handling clock drift (NTP sync issues),
//! and concurrency control (Mutex vs Atomics) for sequence generation.

// Word truncation is intentional when packing millisecond timestamps into 64-bit IDs.
#![allow(clippy::cast_possible_truncation)]

use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

// =========================================================================================
// Architecture
// =========================================================================================
//
// 64-bit ID Layout:
//
//  1 bit  | 41 bits                                 | 10 bits    | 12 bits
// ┌───────┼─────────────────────────────────────────┼────────────┼──────────────┐
// │ Sign  │ Timestamp (Milliseconds since Epoch)    │ Node ID    │ Sequence     │
// │ (0)   │ (Provides ~69 years of IDs)             │ (Up to     │ (Up to 4096  │
// │       │                                         │  1024 nodes│  IDs per ms) │
// └───────┴─────────────────────────────────────────┴────────────┴──────────────┘
//
// Invariants:
// 1. IDs are unique across the cluster as long as Node IDs are unique.
// 2. IDs are roughly sortable by time (k-ordered) because the timestamp is the most significant part.
// 3. The generator must handle clock moving backwards (NTP adjustments) by pausing or throwing an error.
// 4. If sequence space (4096) is exhausted in a single millisecond, it must block until the next millisecond.
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ Generate ID   │ O(1)*       │ O(1)        │
// └───────────────┴─────────────┴─────────────┘
// * Time is O(1) mostly, but O(N) where N is the sleep duration if the clock moves backward or sequence overflows.
//
// Design Decisions:
// - **Concurrency**: `Mutex` for state.
//   - *Alternative*: `AtomicU64` storing a packed `(timestamp, sequence)` state to make it lock-free.
//     However, `SystemTime::now()` isn't lock-free itself usually, and handling clock rollback is trickier with pure atomics.
//     A `Mutex` is simple, correct, and sufficient for 4 million IDs/sec/node.
// - **Epoch**: Custom epoch (e.g., Twitter's 2010-11-04) allows squeezing more years into the 41 bits.

const NODE_ID_BITS: u64 = 10;
const SEQUENCE_BITS: u64 = 12;

const MAX_NODE_ID: u64 = (1 << NODE_ID_BITS) - 1; // 1023
const MAX_SEQUENCE: u64 = (1 << SEQUENCE_BITS) - 1; // 4095

const NODE_ID_SHIFT: u64 = SEQUENCE_BITS;
const TIMESTAMP_SHIFT: u64 = SEQUENCE_BITS + NODE_ID_BITS;

/// A trait for generating unique distributed IDs.
/// Allows swapping implementations (e.g., Snowflake vs `UUIDv7`) seamlessly.
pub trait IdGenerator: Send + Sync {
    type Id;

    /// Generates the next unique ID.
    fn generate(&self) -> Self::Id;
}

/// State protected by a Mutex.
struct SnowflakeState {
    last_timestamp: u64,
    sequence: u64,
}

/// A Thread-safe Snowflake ID Generator.
pub struct Snowflake {
    epoch: u64,
    node_id: u64,
    state: Arc<Mutex<SnowflakeState>>,
}

// RUST INSIGHT: Deriving common traits (Debug, Clone) where appropriate.
// Since `state` is wrapped in `Arc<Mutex>`, cloning the generator provides another handle
// to the same logical generator, which is highly useful in multi-threaded contexts like web servers.
impl Clone for Snowflake {
    fn clone(&self) -> Self {
        Self {
            epoch: self.epoch,
            node_id: self.node_id,
            state: Arc::clone(&self.state),
        }
    }
}

impl Snowflake {
    /// Creates a new Snowflake ID Generator.
    ///
    /// # Arguments
    /// * `epoch` - The custom epoch in milliseconds. IDs are generated relative to this time.
    /// * `node_id` - The unique identifier for this machine/process (0-1023).
    ///
    /// # Panics
    /// Panics if `node_id` is greater than 1023.
    #[must_use]
    pub fn new(epoch: u64, node_id: u64) -> Self {
        assert!(
            node_id <= MAX_NODE_ID,
            "Node ID must be between 0 and {MAX_NODE_ID}"
        );

        Self {
            epoch,
            node_id,
            state: Arc::new(Mutex::new(SnowflakeState {
                last_timestamp: 0,
                sequence: 0,
            })),
        }
    }
}

impl IdGenerator for Snowflake {
    type Id = u64;

    /// Generates the next unique Snowflake ID.
    ///
    /// Blocks if the sequence for the current millisecond is exhausted,
    /// or if the system clock moved backwards (until it catches up).
    #[allow(clippy::missing_panics_doc)]
    fn generate(&self) -> Self::Id {
        let mut state = self.state.lock().unwrap();

        let mut current_timestamp = Self::current_time_ms();

        // GOTCHA: Clock moved backwards (NTP drift).
        // If we generate an ID now, it might collide with an ID generated in the "future".
        // Production systems typically either return an error, or spin/sleep until the clock catches up.
        // We spin/sleep here.
        if current_timestamp < state.last_timestamp {
            // Spin until the clock catches up to the last timestamp.
            while current_timestamp < state.last_timestamp {
                // Yield to avoid busy loop burning CPU 100%
                std::thread::yield_now();
                current_timestamp = Self::current_time_ms();
            }
        }

        if current_timestamp == state.last_timestamp {
            // Same millisecond, increment sequence
            state.sequence = (state.sequence + 1) & MAX_SEQUENCE;

            // Sequence overflowed (hit 4096)
            if state.sequence == 0 {
                // Wait for the next millisecond
                current_timestamp = Self::wait_next_millis(current_timestamp);
            }
        } else {
            // New millisecond, reset sequence
            state.sequence = 0;
        }

        state.last_timestamp = current_timestamp;

        // Construct the 64-bit ID
        // RUST INSIGHT: Safe bitwise operations. Rust strictly enforces types,
        // so all components must be u64.
        let timestamp_offset = current_timestamp - self.epoch;

        (timestamp_offset << TIMESTAMP_SHIFT) | (self.node_id << NODE_ID_SHIFT) | state.sequence
    }
}

impl Snowflake {
    /// Helper to get current time in milliseconds.
    fn current_time_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards before UNIX_EPOCH")
            .as_millis() as u64
    }

    /// Blocks until the clock ticks to the next millisecond.
    fn wait_next_millis(last_timestamp: u64) -> u64 {
        let mut current = Self::current_time_ms();
        while current <= last_timestamp {
            std::thread::yield_now();
            current = Self::current_time_ms();
        }
        current
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `sonyflake`: Uses a slightly different bit layout (39 bits time in 10ms units, 8 bits sequence, 16 bits machine ID)
//   which gives it 174 years of lifetime but fewer IDs per ms.
// - `uuid`: UUIDv4 is 128-bit random, huge string, not sortable. UUIDv7 is sortable, but 128-bit. Snowflake is 64-bit (fits in a SQL BIGINT).
//
// Missing vs. Production:
// - **Error Handling for Clock Drift**: Busy-waiting on clock drift is dangerous if the clock jumped back hours.
//   Production versions return an `Err(ClockMovedBackwards)` to let the application handle it.
// - **Node ID Provisioning**: We assume Node ID is passed in. Production systems often use ZooKeeper/Etcd
//   or read MAC addresses/container IPs to dynamically assign Node IDs safely.
//
// Next Steps:
// 1. Return `Result<u64, Error>` for clock drift.
// 2. Add an iterator implementation for stream generation.
//
// Benchmarking Note:
// Use `criterion` to benchmark `generate()` across 1, 4, and 16 threads to observe lock contention on the Mutex.
// Since `generate()` acquires a lock, highly concurrent systems might see degraded performance.
// Compare this against an `AtomicU64`-based lock-free implementation.

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::thread;

    // Use a recent epoch for testing (e.g., 2024-01-01)
    const TEST_EPOCH: u64 = 1_704_067_200_000;

    #[test]
    fn test_unique_generation() {
        let generator = Snowflake::new(TEST_EPOCH, 1);
        let mut ids = HashSet::new();

        // Generate 10k IDs rapidly.
        // This will span multiple milliseconds and test the sequence logic.
        for _ in 0..10_000 {
            let id = generator.generate();
            assert!(ids.insert(id), "Duplicate ID generated: {id}");
        }
    }

    #[test]
    fn test_k_ordered() {
        let generator = Snowflake::new(TEST_EPOCH, 1);
        let id1 = generator.generate();

        // Force a clock tick
        thread::sleep(std::time::Duration::from_millis(2));

        let id2 = generator.generate();

        assert!(id1 < id2, "IDs are not time-ordered");
    }

    #[test]
    fn test_concurrent_generation() {
        let generator = Snowflake::new(TEST_EPOCH, 1);
        let mut handles = vec![];

        // 4 threads, generating 10k IDs each
        for _ in 0..4 {
            let gen_clone = generator.clone();
            handles.push(thread::spawn(move || {
                let mut local_ids = Vec::with_capacity(10_000);
                for _ in 0..10_000 {
                    local_ids.push(gen_clone.generate());
                }
                local_ids
            }));
        }

        let mut all_ids = HashSet::new();
        for handle in handles {
            let local_ids = handle.join().unwrap();
            for id in local_ids {
                assert!(all_ids.insert(id), "Duplicate ID generated across threads");
            }
        }

        assert_eq!(all_ids.len(), 40_000);
    }

    #[test]
    #[should_panic(expected = "Node ID must be between 0 and 1023")]
    fn test_invalid_node_id() {
        let _ = Snowflake::new(TEST_EPOCH, 2048);
    }

    #[test]
    fn test_id_structure() {
        let generator = Snowflake::new(TEST_EPOCH, 512); // Node ID = 512
        let id = generator.generate();

        // Extract Node ID (shift right by sequence bits, then mask)
        let extracted_node_id = (id >> SEQUENCE_BITS) & MAX_NODE_ID;
        assert_eq!(extracted_node_id, 512);
    }
}
