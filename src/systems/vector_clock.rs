//! # Vector Clock Implementation
//!
//! A logical clock mechanism for capturing causality in distributed systems.
//! It allows detecting whether events are causally related (happened-before) or concurrent.
//!
//! **Replaces Crates:** `vector-clock`, `crdts` (partially)
//!
//! **Real-world Usage:**
//! - Amazon `DynamoDB` (detecting conflicting versions of an object).
//! - Riak (dotted version vectors).
//! - Collaborative editing (CRDTs).
//!
//! **Why build it yourself?**
//! Understanding Vector Clocks is key to grasping distributed consistency.
//! You learn that "time" in a distributed system is a partial order, not a total order.
//! It forces you to think about concurrency not as "simultaneous" but as "independent".

use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure:
// - A Map of `NodeId -> Counter`.
// - Represents the state of knowledge a node has about the system.
// - `V[i] = k` means node `i` has performed `k` events.
//
// Operations:
// - Increment: `V[my_id] += 1`. Represents a local event.
// - Merge: `V[j] = max(V[j], incoming_V[j])` for all `j`. Represents receiving a message.
// - Compare:
//   - A < B iff for all i, A[i] <= B[i] AND there exists j such that A[j] < B[j].
//   - A > B iff B < A.
//   - A == B iff for all i, A[i] == B[i].
//   - A || B (concurrent) otherwise (some elements larger, some smaller).
//
// Invariants:
// 1. Counters are monotonically increasing.
// 2. A node always knows its own latest counter value.
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ Increment     │ O(1)        │ O(N)        │
// │ Merge         │ O(N)        │ O(N)        │
// │ Compare       │ O(N)        │ O(1)        │
// └───────────────┴─────────────┴─────────────┘
// where N is the number of nodes in the vector.

// NOTE: `PartialEq` is implemented manually (see below) rather than derived. The derived
// impl compares the raw `clock` HashMap and `my_id` exactly, which is incoherent with
// `partial_cmp` (e.g. `{A:0}` and `{B:0}` compare `Some(Equal)` under causality but are
// `!=` structurally). We make equality mean "causally equal".
#[derive(Clone, Debug)]
pub struct VectorClock {
    /// Map of Node ID to logical timestamp.
    /// Using `BTreeMap` would allow cheaper comparison (sorted keys), but `HashMap` is O(1) access.
    /// For small N, iteration overhead dominates anyway.
    /// We use `HashMap` for O(1) increment.
    clock: HashMap<String, u64>,
    my_id: String,
}

impl VectorClock {
    /// Creates a new Vector Clock for a specific node.
    pub fn new(node_id: impl Into<String>) -> Self {
        let node_id = node_id.into();
        let mut clock = HashMap::new();
        clock.insert(node_id.clone(), 0);
        Self {
            clock,
            my_id: node_id,
        }
    }

    /// Increments the logical clock for the local node.
    /// Should be called before sending a message or recording an internal event.
    pub fn increment(&mut self) {
        // RUST INSIGHT: `entry` API avoids double lookup.
        // We know `my_id` is in the map from `new`, but robust code handles missing key just in case.
        *self.clock.entry(self.my_id.clone()).or_insert(0) += 1;
    }

    /// Merges another vector clock into this one.
    /// Should be called when receiving a message carrying a vector clock.
    /// Takes the element-wise maximum.
    pub fn merge(&mut self, other: &Self) {
        for (node, &count) in &other.clock {
            let my_count = self.clock.entry(node.clone()).or_insert(0);
            if count > *my_count {
                *my_count = count;
            }
        }
    }

    /// Returns the logical timestamp for a specific node.
    #[must_use]
    pub fn get(&self, node_id: &str) -> u64 {
        *self.clock.get(node_id).unwrap_or(&0)
    }

    /// Determines the causal relationship between two vector clocks.
    /// Returns:
    /// - `Some(Ordering::Equal)` if they are identical.
    /// - `Some(Ordering::Less)` if `self` happened before `other`.
    /// - `Some(Ordering::Greater)` if `other` happened before `self`.
    /// - `None` if they are concurrent.
    #[must_use]
    pub fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let mut self_has_greater = false;
        let mut other_has_greater = false;

        // Iterate over union of keys
        // We can iterate self, then check other for missing keys in self.
        // Or simpler: iterate all keys from both.

        // Optimization: Use a HashSet to track visited keys if N is large.
        // For small N, we can just iterate self, then other.

        // Check all keys in self
        for (node, &self_val) in &self.clock {
            let other_val = other.get(node);
            if self_val > other_val {
                self_has_greater = true;
            } else if self_val < other_val {
                other_has_greater = true;
            }
        }

        // Check keys in other that are NOT in self
        for (node, &other_val) in &other.clock {
            if !self.clock.contains_key(node) {
                // self_val is 0
                if 0 < other_val {
                    other_has_greater = true;
                }
                // if other_val == 0, equal, no flag change
            }
        }

        match (self_has_greater, other_has_greater) {
            (false, false) => Some(Ordering::Equal),
            (true, false) => Some(Ordering::Greater),
            (false, true) => Some(Ordering::Less),
            (true, true) => None, // Concurrent
        }
    }
}

// RUST INSIGHT: Implementing `PartialOrd` allows using `<`, `>`, `<=` operators.
// Note that `PartialOrd` returns `Option<Ordering>`, fitting our causality semantics perfectly.
impl PartialOrd for VectorClock {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.partial_cmp(other)
    }
}

// RUST INSIGHT: Equality must be coherent with `PartialOrd`. Two vector clocks are equal
// iff every node's count matches, treating a missing node as 0 (so `{A:0}`, `{B:0}`, and
// `{}` are all equal). This is exactly `partial_cmp(...) == Some(Ordering::Equal)`, which
// also ignores `my_id` — the identity of the observing node is not part of causal state.
impl PartialEq for VectorClock {
    fn eq(&self, other: &Self) -> bool {
        self.partial_cmp(other) == Some(Ordering::Equal)
    }
}

impl fmt::Display for VectorClock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{{")?;
        let mut sorted_keys: Vec<_> = self.clock.keys().collect();
        sorted_keys.sort();
        for (i, key) in sorted_keys.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}:{}", key, self.clock.get(*key).unwrap())?;
        }
        write!(f, "}}")
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `vector-clock`: Generic implementation, similar to this.
// - `crdts`: Includes VClock as part of a larger suite of Conflict-free Replicated Data Types.
//
// Missing vs. Production:
// - **Pruning**: In long-running systems, the vector grows with the number of nodes (including dead ones).
//   Production systems need a mechanism to prune old nodes (e.g., using a timestamp or explicit leave).
// - **Compactness**: Sending map strings over wire is inefficient. Production uses integer IDs or compression.
//
// Next Steps:
// 1. Implement Version Vectors (store only exceptions/dots).
// 2. Add pruning for stale nodes.
// 3. Implement serialization (Serde).

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector_clock_basic() {
        let mut vc = VectorClock::new("A");
        assert_eq!(vc.get("A"), 0);

        vc.increment();
        assert_eq!(vc.get("A"), 1);
        assert_eq!(vc.get("B"), 0);
    }

    #[test]
    fn test_merge() {
        let mut vc1 = VectorClock::new("A");
        vc1.increment(); // {A:1}

        let mut vc2 = VectorClock::new("B");
        vc2.increment(); // {B:1}
        vc2.merge(&vc1); // {A:1, B:1}

        assert_eq!(vc2.get("A"), 1);
        assert_eq!(vc2.get("B"), 1);

        vc1.increment(); // {A:2}
        vc2.merge(&vc1); // {A:2, B:1}
        assert_eq!(vc2.get("A"), 2);
    }

    #[test]
    fn test_causality() {
        let mut a = VectorClock::new("A");
        let mut b = VectorClock::new("B");

        // Initial state: {A:0}, {B:0}. Technically equal if we assume default 0.
        // Our PartialOrd implementation handles missing keys as 0.
        assert_eq!(a.partial_cmp(&b), Some(Ordering::Equal));

        a.increment(); // {A:1}
        // A > B because {A:1} > {A:0} and {B:0} == {B:0}
        assert_eq!(a.partial_cmp(&b), Some(Ordering::Greater));
        assert_eq!(b.partial_cmp(&a), Some(Ordering::Less));

        b.increment(); // {B:1}
        // Now a={A:1}, b={B:1}.
        // A has A:1 > B's A:0.
        // B has B:1 > A's B:0.
        // Concurrent.
        assert_eq!(a.partial_cmp(&b), None);
    }

    #[test]
    fn test_eq_coherent_with_partial_cmp() {
        // Regression: derived PartialEq compared the raw HashMap + my_id, so `{A:0}` and
        // `{B:0}` were `!=` even though partial_cmp reports Some(Equal). Equality must now
        // agree with causal ordering (missing keys treated as 0, my_id ignored).
        let a = VectorClock::new("A"); // {A:0}
        let b = VectorClock::new("B"); // {B:0}

        assert_eq!(a.partial_cmp(&b), Some(Ordering::Equal));
        assert_eq!(a, b);
        assert!(!(a != b));

        // A clock that has learned about a zero-count node is still equal.
        let mut c = VectorClock::new("C"); // {C:0}
        c.merge(&a); // {C:0, A:0}
        assert_eq!(a, c);
        assert_eq!(b, c);

        // Divergence breaks equality.
        let mut d = VectorClock::new("A");
        d.increment(); // {A:1}
        assert_ne!(a, d);
    }

    #[test]
    fn test_merge_and_causality() {
        let mut a = VectorClock::new("A");
        let mut b = VectorClock::new("B");

        a.increment(); // A: {A:1}
        b.merge(&a); // B: {A:1, B:0}
        b.increment(); // B: {A:1, B:1}

        // B should be strictly greater than A
        // B has A:1 (==), B:1 (> A's B:0)
        assert_eq!(b.partial_cmp(&a), Some(Ordering::Greater));
        assert_eq!(a.partial_cmp(&b), Some(Ordering::Less));
    }
}
