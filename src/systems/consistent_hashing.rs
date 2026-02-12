//! # Consistent Hashing Ring Implementation
//!
//! A distributed system primitive for mapping keys to nodes with minimal churn when nodes are added or removed.
//!
//! **Replaces Crates:** `consistent_hash`, `hashring`
//!
//! **Real-world Usage:**
//! - DynamoDB (partitioning data across nodes).
//! - Cassandra (token ring).
//! - Memcached clients (distributing keys across cache servers).
//!
//! **Why build it yourself?**
//! You'll learn how "virtual nodes" smooth out data distribution and how the ring topology
//! handles node failures gracefully (remapping only k/N keys).

use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure: Ring (Sorted Map)
//
// Ring: [Hash1 -> NodeA, Hash2 -> NodeB, Hash3 -> NodeA, ...]
//
// Key Mapping:
// 1. Hash(key) -> H_k
// 2. Find first Node entry with Hash >= H_k
// 3. If no such entry, wrap around to first entry.
//
// Virtual Nodes:
// To ensure even distribution, each physical node is mapped to multiple points (replicas) on the ring.
//
// Invariants:
// 1. Ring is always sorted by hash.
// 2. get_node is deterministic for a given ring state.
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ Add Node      │ O(V * log N)│ O(N * V)    │
// │ Remove Node   │ O(V * log N)│ O(N * V)    │
// │ Get Node      │ O(log (N*V))│ O(1)        │
// └───────────────┴─────────────┴─────────────┘
// where N is number of physical nodes, V is virtual nodes per physical node.

/// A Consistent Hashing Ring.
pub struct ConsistentHashRing {
    /// Map from Hash -> Node Identifier.
    /// We use BTreeMap to keep hashes sorted and allow efficient range queries.
    ring: BTreeMap<u64, String>,
    /// Number of virtual nodes (replicas) per physical node.
    virtual_nodes: usize,
}

impl ConsistentHashRing {
    /// Creates a new empty Consistent Hash Ring.
    ///
    /// # Arguments
    /// * `virtual_nodes` - Number of points on the ring each node is responsible for.
    ///                     Higher values provide better distribution balance but increase memory/lookup cost.
    ///                     (e.g., 100-200 is common in production).
    pub fn new(virtual_nodes: usize) -> Self {
        Self {
            ring: BTreeMap::new(),
            virtual_nodes,
        }
    }

    /// Adds a physical node to the ring.
    pub fn add_node(&mut self, node_id: &str) {
        for i in 0..self.virtual_nodes {
            let key = format!("{}:{}", node_id, i);
            let hash = self.hash_key(&key);
            self.ring.insert(hash, node_id.to_string());
        }
    }

    /// Removes a physical node from the ring.
    pub fn remove_node(&mut self, node_id: &str) {
        // RUST INSIGHT:
        // Removing items from a BTreeMap while iterating is tricky.
        // We collect keys to remove first.
        // Alternatively, we could construct the keys we know we added.
        for i in 0..self.virtual_nodes {
            let key = format!("{}:{}", node_id, i);
            let hash = self.hash_key(&key);
            // We only remove if the value matches, in case of hash collision (unlikely but possible)
            // GOTCHA: Cannot remove while holding a reference from get()
            let should_remove = self.ring.get(&hash).map_or(false, |val| val == node_id);

            if should_remove {
                self.ring.remove(&hash);
            }
        }
    }

    /// Returns the node responsible for the given key.
    pub fn get_node(&self, key: &str) -> Option<&String> {
        if self.ring.is_empty() {
            return None;
        }

        let hash = self.hash_key(key);

        // RUST INSIGHT:
        // BTreeMap::range gives us an iterator over entries with keys >= hash.
        // The first item is our target.
        // If the iterator is empty, it means we wrapped around the ring, so we take the first item in the map.

        let mut range = self.ring.range(hash..);
        range.next().map(|(_, node)| node).or_else(|| {
            // Wrap around to the start
            self.ring.values().next()
        })
    }

    /// Helper to hash a key using DefaultHasher.
    fn hash_key(&self, key: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        hasher.finish()
    }

    /// Returns the number of physical nodes (calculated).
    /// Note: This is O(N*V), used mainly for testing/introspection.
    pub fn node_count(&self) -> usize {
        // Collect unique values
        let mut nodes = std::collections::HashSet::new();
        for node in self.ring.values() {
            nodes.insert(node);
        }
        nodes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_ring() {
        let mut ring = ConsistentHashRing::new(10);
        ring.add_node("node1");
        ring.add_node("node2");
        ring.add_node("node3");

        assert_eq!(ring.node_count(), 3);

        let key = "my_awesome_key";
        let node = ring.get_node(key);
        assert!(node.is_some());
        let node_id = node.unwrap();
        assert!(["node1", "node2", "node3"].contains(&node_id.as_str()));
    }

    #[test]
    fn test_node_removal() {
        let mut ring = ConsistentHashRing::new(5);
        ring.add_node("A");
        ring.add_node("B");

        let key = "user_123";
        let initial_node = ring.get_node(key).unwrap().clone();

        // If we remove the other node, the key should stay or move to the remaining one.
        // If we remove the responsible node, it must move.

        let other_node = if initial_node == "A" { "B" } else { "A" };
        ring.remove_node(other_node);

        let new_node = ring.get_node(key).unwrap();
        assert_eq!(new_node, &initial_node); // Should stick if we removed the *other* node

        // Now remove the responsible node
        ring.remove_node(&initial_node);
        // Ring is empty
        assert!(ring.get_node(key).is_none());
    }

    #[test]
    fn test_distribution() {
        // With enough virtual nodes, distribution should be somewhat even.
        let mut ring = ConsistentHashRing::new(100);
        ring.add_node("A");
        ring.add_node("B");
        ring.add_node("C");

        let mut counts = std::collections::HashMap::new();
        counts.insert("A", 0);
        counts.insert("B", 0);
        counts.insert("C", 0);

        for i in 0..1000 {
            let key = format!("key_{}", i);
            let node = ring.get_node(&key).unwrap();
            *counts.get_mut(node.as_str()).unwrap() += 1;
        }

        println!("Distribution: {:?}", counts);

        // Check that no node is completely starving (basic sanity check)
        for (_, count) in counts {
            assert!(count > 200); // Ideally around 333, but variance allows > 200
        }
    }

    #[test]
    fn test_monotonicity() {
        // When a node is added, keys should only move TO the new node, never between old nodes.
        let mut ring = ConsistentHashRing::new(50);
        ring.add_node("A");
        ring.add_node("B");

        let mut assignments = std::collections::HashMap::new();
        for i in 0..100 {
            let key = format!("key_{}", i);
            assignments.insert(key, ring.get_node(&format!("key_{}", i)).unwrap().clone());
        }

        ring.add_node("C");

        for (key, old_node) in assignments {
            let new_node = ring.get_node(&key).unwrap();
            if new_node != &old_node {
                // If it moved, it MUST have moved to C
                assert_eq!(new_node, "C", "Key moved from {} to {} (not C)", old_node, new_node);
            }
        }
    }
}
