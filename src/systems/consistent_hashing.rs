//! # Consistent Hashing Ring
//!
//! A distributed hashing scheme that operates independently of the number of servers or objects in a distributed hash table.
//! It minimizes the reorganization of keys when nodes are added or removed.
//!
//! Replaces: `consistent_hash`, `hashring`
//! Used in: DynamoDB, Cassandra, Discord (sharding), Memcached clients
//! Why build it: To understand how to distribute data evenly across a dynamic set of nodes
//! while minimizing movement (rebalancing) when the topology changes.
//!
//! ## Architecture
//!
//! The ring is represented as a sorted map of hash values to nodes.
//! Virtual nodes (replicas) are used to improve load balancing.
//!
//! ```text
//!       0
//!    /     \
//!  N1       N2
//! |         |
//! N3        N1 (virtual)
//!  \       /
//!    (Max)
//! ```
//!
//! ### Invariants
//! 1. The ring is always sorted by hash key.
//! 2. Requests are routed to the first node with a hash greater than or equal to the request's hash.
//! 3. If no such node exists (wrap around), route to the first node in the ring.
//!
//! ### Complexity
//!
//! | Operation   | Time            | Space    |
//! |-------------|-----------------|----------|
//! | Add Node    | O(V * log N)    | O(N * V) |
//! | Remove Node | O(V * log N)    | O(N * V) |
//! | Get Node    | O(log N)        | O(N * V) |
//!
//! Where $N$ is the number of physical nodes and $V$ is the number of virtual nodes per physical node.
//!
//! ### Design Decisions
//!
//! - **BTreeMap**: Used for the ring to allow $O(\log N)$ lookups (via `range`).
//! - **Virtual Nodes**: Essential for load balancing. Without them, the distribution of keys
//!   can be very skewed. We suffix the node key with a counter to generate virtual node hashes.
//! - **DefaultHasher**: Used for simplicity. In production, a stable hasher like Murmur3 is preferred.

use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::borrow::Borrow;

/// A Consistent Hash Ring for distributing keys across nodes.
#[derive(Debug, Clone)]
pub struct ConsistentHashRing<T> {
    /// The ring mapping hash values to nodes.
    ring: BTreeMap<u64, T>,
    /// Number of virtual nodes per physical node.
    virtual_nodes: usize,
}

impl<T> ConsistentHashRing<T>
where
    T: Hash + Clone,
{
    /// Creates a new empty `ConsistentHashRing` with the specified number of virtual nodes.
    pub fn new(virtual_nodes: usize) -> Self {
        assert!(virtual_nodes > 0, "Virtual nodes must be greater than 0");
        Self {
            ring: BTreeMap::new(),
            virtual_nodes,
        }
    }

    /// Adds a node to the ring.
    ///
    /// Generates `virtual_nodes` replicas for the given node and places them on the ring.
    pub fn add_node(&mut self, node: T) {
        for i in 0..self.virtual_nodes {
            let key = self.hash_node(&node, i);
            self.ring.insert(key, node.clone());
        }
    }

    /// Removes a node from the ring.
    ///
    /// Removes all virtual replicas of the node.
    pub fn remove_node(&mut self, node: &T) {
        for i in 0..self.virtual_nodes {
            let key = self.hash_node(node, i);
            // RUST INSIGHT: BTreeMap::remove returns the value.
            // In a production system, we might want to verify that the value being removed
            // is indeed the node we expect, to handle hash collisions, although rare with u64.
            // Here, we trust the hash.
            self.ring.remove(&key);
        }
    }

    /// Returns the node responsible for the given key.
    ///
    /// Uses consistent hashing to find the first node with a hash greater than or equal
    /// to the key's hash. If none exists, wraps around to the first node.
    pub fn get_node<K: Hash + ?Sized>(&self, key: &K) -> Option<&T> {
        if self.ring.is_empty() {
            return None;
        }

        let hash = self.hash_key(key);

        // RUST INSIGHT: BTreeMap::range allows us to find the first entry with key >= hash.
        // This effectively implements the "clockwise" search on the ring.
        // We look for the smallest key in the ring that is >= `hash`.
        let node = self.ring.range(hash..).next().map(|(_, node)| node);

        // If we found a node, return it.
        // If not (we reached the end of the ring), wrap around to the first node.
        node.or_else(|| self.ring.values().next())
    }

    /// Helper to hash a node with a virtual node index.
    fn hash_node(&self, node: &T, index: usize) -> u64 {
        let mut hasher = DefaultHasher::new();
        node.hash(&mut hasher);
        // GOTCHA: We must differentiate virtual nodes.
        // Hashing the index ensures distinct spots on the ring.
        index.hash(&mut hasher);
        hasher.finish()
    }

    /// Helper to hash a key.
    fn hash_key<K: Hash + ?Sized>(&self, key: &K) -> u64 {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        hasher.finish()
    }

    /// Returns the number of physical nodes (approximate if collisions occurred, but exact for distinct T).
    /// Actually, this returns the total number of points on the ring (physical * virtual).
    pub fn len(&self) -> usize {
        self.ring.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ring.is_empty()
    }
}

// Footer:
// Comparison to canonical crates:
// - `consistent_hash`: Similar API, often allows custom hashers.
// - `hashring`: More optimized for specific use cases.
//
// Missing vs Production:
// - No support for weighting (assigning more virtual nodes to powerful servers).
// - No support for custom hashers (generic BuildHasher).
// - No "replication strategy" (getting N distinct nodes for a key, not just 1).

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ring_basic() {
        let mut ring = ConsistentHashRing::new(10);
        ring.add_node("node1".to_string());
        ring.add_node("node2".to_string());
        ring.add_node("node3".to_string());

        assert_eq!(ring.len(), 30); // 3 nodes * 10 virtual nodes

        let key = "my_key";
        let node = ring.get_node(key);
        assert!(node.is_some());

        let node_val = node.unwrap();
        assert!(["node1", "node2", "node3"].contains(&node_val.as_str()));
    }

    #[test]
    fn test_add_remove() {
        let mut ring = ConsistentHashRing::new(3);
        ring.add_node("A");

        assert_eq!(ring.get_node("key1"), Some(&"A"));

        ring.add_node("B");
        // Keys might map to A or B now

        ring.remove_node(&"A");
        // All keys should map to B
        assert_eq!(ring.get_node("key1"), Some(&"B"));
        assert_eq!(ring.get_node("key2"), Some(&"B"));

        assert_eq!(ring.len(), 3); // 1 node * 3 virtual nodes
    }

    #[test]
    fn test_distribution() {
        // This is a probabilistic test, so we just check it doesn't crash
        // and assigns to available nodes.
        let mut ring = ConsistentHashRing::new(50);
        for i in 0..5 {
            ring.add_node(i);
        }

        let mut counts = std::collections::HashMap::new();
        for i in 0..1000 {
            let key = format!("key_{}", i);
            let node = *ring.get_node(&key).unwrap();
            *counts.entry(node).or_insert(0) += 1;
        }

        assert_eq!(counts.len(), 5);
        for i in 0..5 {
            assert!(counts.contains_key(&i));
        }
    }

    #[test]
    fn test_empty_ring() {
        let ring: ConsistentHashRing<String> = ConsistentHashRing::new(10);
        assert!(ring.get_node("key").is_none());
    }
}
