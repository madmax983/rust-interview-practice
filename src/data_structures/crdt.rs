//! # Conflict-Free Replicated Data Types (CRDTs)
//!
//! Implements foundational CRDT primitives to achieve strong eventual consistency in distributed systems.
//!
//! **Replaces Crates:** `automerge`
//!
//! **Real-world Usage:**
//! - Collaborative text editing (Google Docs, Figma).
//! - Distributed databases (Riak, Cassandra).
//! - Local-first software and offline-capable mobile apps.
//!
//! **Why build it yourself?**
//! Implementing CRDTs teaches you how to decouple state changes from their delivery order over a network.
//! By relying on mathematical properties—specifically that operations or state merges must be Commutative,
//! Associative, and Idempotent—you can safely merge state from multiple peers without coordination or a central server.

// =========================================================================================
// Architecture
// =========================================================================================
//
// Types implemented:
// 1. LWW-Register (Last-Writer-Wins Register)
// 2. OR-Set (Observed-Remove Set)
//
// Invariants:
// - LWW-Register: The state with the highest timestamp always wins during a merge.
//   If timestamps are equal, a secondary mechanism (like node ID or value comparison) breaks the tie.
// - OR-Set: An element is in the set if its add-set is not empty and its add-set is not a strict subset of its remove-set.
//
// Complexity:
// ┌───────────────────────────┬──────────────┬──────────────┐
// │ Operation (OR-Set)        │ Time         │ Space        │
// ├───────────────────────────┼──────────────┼──────────────┤
// │ Add                       │ O(1)         │ O(N)         │
// │ Remove                    │ O(1)*        │ O(N)         │
// │ Merge                     │ O(N)         │ O(N)         │
// └───────────────────────────┴──────────────┴──────────────┘
// * Amortized.
//
// Design Decisions:
// - We use state-based CRDTs (CvRDTs) rather than operation-based (CmRDTs) for simplicity in the merge logic.
// - `uuid::Uuid` or a simple unique string can be used for unique operation tags in OR-Set.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::hash::Hash;

/// A trait representing a State-based CRDT (`CvRDT`).
pub trait Crdt {
    /// Merges another state into this state.
    /// The merge operation must be Commutative, Associative, and Idempotent.
    fn merge(&mut self, other: Self);
}

// =========================================================================================
// LWW-Register
// =========================================================================================

/// A Last-Writer-Wins Register.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LwwRegister<T> {
    pub value: T,
    pub timestamp: u64,
    pub node_id: String, // Used to break ties deterministically
}

impl<T> LwwRegister<T> {
    pub const fn new(value: T, timestamp: u64, node_id: String) -> Self {
        Self {
            value,
            timestamp,
            node_id,
        }
    }
}

impl<T: Clone + Eq> Crdt for LwwRegister<T> {
    fn merge(&mut self, other: Self) {
        // RUST INSIGHT:
        // Using `std::cmp::Ordering` makes the intention of comparing timestamps exhaustive and safe.
        // The compiler ensures we handle all three possibilities (Greater, Equal, Less).
        match other.timestamp.cmp(&self.timestamp) {
            Ordering::Greater => {
                self.value = other.value;
                self.timestamp = other.timestamp;
                self.node_id = other.node_id;
            }
            Ordering::Equal => {
                // GOTCHA:
                // Tie-breaking must be strictly deterministic across all nodes.
                // Using a String comparison for `node_id` achieves this consistently.
                if other.node_id > self.node_id {
                    self.value = other.value;
                    self.timestamp = other.timestamp;
                    self.node_id = other.node_id;
                }
            }
            Ordering::Less => {} // Ignore older updates
        }
    }
}

// =========================================================================================
// OR-Set
// =========================================================================================

/// An Observed-Remove Set.
/// Allows elements to be added and removed. When an element is added, it is tagged with a unique token.
/// Removing an element removes all observed tokens for that element.
#[derive(Debug, Clone, Default)]
pub struct OrSet<T> {
    // Maps an element to the set of unique add tokens.
    adds: HashMap<T, HashSet<String>>,
    // Maps an element to the set of unique remove tokens.
    removes: HashMap<T, HashSet<String>>,
}

impl<T: Clone + Eq + Hash> OrSet<T> {
    #[must_use] 
    pub fn new() -> Self {
        Self {
            adds: HashMap::new(),
            removes: HashMap::new(),
        }
    }

    /// Adds an element to the set with a unique tag.
    pub fn add(&mut self, element: T, tag: String) {
        self.adds.entry(element).or_default().insert(tag);
    }

    /// Removes an element from the set by observing all currently known tags.
    pub fn remove(&mut self, element: T) {
        // PRODUCTION NOTE:
        // A production OR-Set optimizes this by not storing an unbounded set of remove tags forever (tombstones).
        // It relies on garbage collection mechanisms to eventually clean up elements that all nodes agree are removed.
        if let Some(added_tags) = self.adds.get(&element) {
            let removed_tags = self.removes.entry(element).or_default();
            for tag in added_tags {
                removed_tags.insert(tag.clone());
            }
        }
    }

    /// Checks if the element is currently in the set.
    pub fn contains(&self, element: &T) -> bool {
        let added_tags = self.adds.get(element);
        let removed_tags = self.removes.get(element);

        match (added_tags, removed_tags) {
            (Some(adds), Some(rems)) => !adds.is_subset(rems),
            (Some(_), None) => true,
            _ => false,
        }
    }

    /// Returns an iterator over all elements currently in the set.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.adds.keys().filter(move |k| self.contains(k))
    }
}

impl<T: Clone + Eq + Hash> Crdt for OrSet<T> {
    fn merge(&mut self, other: Self) {
        // Merge additions
        for (element, tags) in other.adds {
            self.adds.entry(element).or_default().extend(tags);
        }

        // Merge removals
        for (element, tags) in other.removes {
            self.removes.entry(element).or_default().extend(tags);
        }
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to canonical `automerge`:
// - `automerge` implements complex sequence and JSON-like CRDTs (e.g., RGA for text, Map CRDTs).
// - `automerge` optimizes the data representation heavily for transmission, using a custom binary format and delta state changes.
// - This implementation uses unoptimized HashMaps and full-state merges, which is inefficient over the wire for large data structures.
//
// What's missing vs. production:
// - Delta-state CRDTs: Transmitting only what changed instead of the whole state.
// - Garbage collection (tombstone removal): The OR-Set grows unbounded because it keeps track of removed tags forever.
// - Sequence CRDTs (like RGA or Logoot) needed for text editing.
//
// Suggested Next Steps:
// - Implement a sequence CRDT for collaborative text editing.
// - Implement delta-state merges for the `OrSet`.
// - Implement an optimized garbage collection strategy for removed tombstones.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lww_register_merge() {
        let mut r1 = LwwRegister::new("A", 1, "node1".into());
        let r2 = LwwRegister::new("B", 2, "node2".into());

        r1.merge(r2);
        assert_eq!(r1.value, "B");

        // Tie-breaker
        let mut r3 = LwwRegister::new("C", 2, "node3".into());
        let r4 = LwwRegister::new("D", 2, "node1".into());

        r3.merge(r4);
        assert_eq!(r3.value, "C"); // node3 > node1, so r3 keeps its value
    }

    #[test]
    fn test_orset_add_remove() {
        let mut set = OrSet::new();
        set.add("A", "tag1".into());
        assert!(set.contains(&"A"));

        set.remove("A");
        assert!(!set.contains(&"A"));

        // Adding again with a new tag
        set.add("A", "tag2".into());
        assert!(set.contains(&"A"));
    }

    // =========================================================================================
    // Benchmarking Note
    // =========================================================================================
    // To benchmark the OR-Set merge performance:
    // ```rust
    // use std::time::Instant;
    // use std::hint::black_box;
    //
    // fn bench_merge() {
    //     let mut node1 = OrSet::new();
    //     let mut node2 = OrSet::new();
    //     for i in 0..10_000 {
    //         node1.add(i, format!("tag_1_{}", i));
    //         node2.add(i, format!("tag_2_{}", i));
    //     }
    //
    //     let start = Instant::now();
    //     node1.merge(black_box(node2));
    //     println!("Merge time: {:?}", start.elapsed());
    // }
    // ```

    #[test]
    fn test_orset_merge() {
        let mut node1 = OrSet::new();
        let mut node2 = OrSet::new();

        node1.add("A", "tag1".into());
        node2.add("B", "tag2".into());

        node1.merge(node2.clone());
        assert!(node1.contains(&"A"));
        assert!(node1.contains(&"B"));

        // Concurrent remove and add
        node1.remove("A"); // Removes "A" from node1's perspective
        node2.add("A", "tag3".into()); // Node2 concurrently adds "A" with a new tag

        node1.merge(node2);
        // "A" should be present because node2 added it with "tag3" which node1 hasn't observed removal of
        assert!(node1.contains(&"A"));
    }
}
