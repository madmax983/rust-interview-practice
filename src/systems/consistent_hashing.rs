//! # Consistent Hashing Ring Implementation
//!
//! Implements a consistent hashing ring with virtual nodes (replicas) to distribute keys across a dynamic set of nodes.
//! This implementation minimizes key remapping when nodes are added or removed, a critical property for distributed systems.
//!
//! **Replaces Crates:** `consistent_hash`, `hashring`
//!
//! **Real-world Usage:**
//! - Distributed Caches (Memcached, Redis Cluster)
//! - Distributed Databases (Cassandra, DynamoDB, Riak)
//! - Load Balancers (HAProxy, Nginx)
//!
//! **Why build it yourself?**
//! Consistent hashing is the backbone of horizontal scalability. Implementing it teaches you:
//! 1. How mapping keys to a continuous ring (0..2^64) minimizes reshuffling when nodes leave/join (only `k/N` keys move).
//! 2. The importance of virtual nodes (vnodes) to mitigate data skew and balance load.
//! 3. How to use an ordered map (`BTreeMap`) for efficient range queries (`O(log N)`) to find the "next" node.

use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure:
//
//      Ring: BTreeMap<u64, Node>
//      (Ordered by Hash Value)
//
//      Hash Space (0 .. 2^64 - 1)
//      Using Virtual Nodes (Replicas = 3):
//
//      Node A:  [Hash(A-0), Hash(A-1), Hash(A-2)]
//      Node B:  [Hash(B-0), Hash(B-1), Hash(B-2)]
//
//      Ring Layout:
//      ┌───────────┐      ┌───────────┐      ┌───────────┐
//      │ Hash(A-0) │ ───▶ │ Hash(B-1) │ ───▶ │ Hash(A-2) │ ...
//      └───────────┘      └───────────┘      └───────────┘
//            ▲                  ▲                  ▲
//            │                  │                  │
//         Key K1             Key K2             Key K3
//
//      Lookup Logic:
//      - Hash(Key) -> H
//      - Find first node with Hash >= H
//      - If not found (end of ring), wrap around to first node.
//
// Invariants:
// 1. The ring is always sorted by hash value (guaranteed by BTreeMap).
// 2. Each physical node is represented by `replicas` virtual nodes in the ring.
// 3. If the ring is not empty, `get_node` always returns a node.
//
// Complexity:
// ┌─────────────┬──────────────┬────────────┐
// │ Operation   │ Time         │ Space      │
// ├─────────────┼──────────────┼────────────┤
// │ add_node    │ O(R * log V) │ O(R)       │
// │ remove_node │ O(R * log V) │ O(1)       │
// │ get_node    │ O(log V)     │ O(1)       │
// └─────────────┴──────────────┴────────────┘
// Where R = replicas, V = total virtual nodes (N * R).
//
// Design Decisions:
// - **Backing Store**: `BTreeMap` is chosen over a sorted `Vec` because insertion/deletion is `O(log V)` vs `O(V)`.
//   While a sorted `Vec` + binary search is cache-friendlier for lookups, consistent hashing rings change (nodes flap),
//   so efficient updates are important.
// - **Virtual Nodes**: Essential for uniform distribution. Without them, a few nodes can skew the ring.
// - **Hash Function**: Uses `DefaultHasher` for simplicity. In production, use a stable hasher like Murmur3 or xxHash.

/// A Consistent Hashing Ring.
#[derive(Debug, Clone)]
pub struct ConsistentHashRing<T> {
    ring: BTreeMap<u64, T>,
    replicas: usize,
}

impl<T: Hash + Clone + Eq> ConsistentHashRing<T> {
    /// Creates a new empty Consistent Hash Ring with the specified number of replicas (virtual nodes) per physical node.
    ///
    /// # Arguments
    /// * `replicas` - The number of virtual nodes to create for each physical node.
    ///   Higher values (e.g., 100-200) improve load balancing but increase memory usage and lookup time slightly.
    pub fn new(replicas: usize) -> Self {
        assert!(replicas > 0, "Replicas must be at least 1");
        Self {
            ring: BTreeMap::new(),
            replicas,
        }
    }

    /// Adds a node to the ring.
    ///
    /// Creates `replicas` virtual nodes for the given physical node.
    pub fn add_node(&mut self, node: T) {
        for i in 0..self.replicas {
            let hash = self.calculate_hash(&node, i);
            self.ring.insert(hash, node.clone());
        }
    }

    /// Removes a node from the ring.
    ///
    /// Removes all `replicas` virtual nodes associated with the physical node.
    pub fn remove_node(&mut self, node: &T) {
        for i in 0..self.replicas {
            let hash = self.calculate_hash(node, i);
            // Verify that the node at this hash is indeed the one we want to remove
            // (Strictly speaking, with a good hash, collisions are negligible, but good to be safe)
            // GOTCHA: BTreeMap::remove returns the value. We can check it.
            // RUST INSIGHT: We can't use `remove` if we just want to check, but here we want to remove.
            // However, we must ensure we don't remove a DIFFERENT node that collided (extremely rare).
            // A production system might store a separate Map<Node, Vec<u64>> to track virtual node positions explicitly.
            // Here, we re-hash to find positions.
            if let Some(existing_node) = self.ring.get(&hash) {
                if existing_node == node {
                    self.ring.remove(&hash);
                }
            }
        }
    }

    /// Returns the node responsible for the given key.
    ///
    /// returns `None` if the ring is empty.
    pub fn get_node<K: Hash + ?Sized>(&self, key: &K) -> Option<&T> {
        if self.ring.is_empty() {
            return None;
        }

        let hash = self.calculate_key_hash(key);

        // RUST INSIGHT: `range` returns an iterator over entries with keys in the given range.
        // We want the first entry with key >= hash.
        // `(hash..)` is a RangeFrom.
        let mut iterator = self.ring.range(hash..);

        if let Some((_, node)) = iterator.next() {
            Some(node)
        } else {
            // Wrap around to the beginning of the ring
            self.ring.iter().next().map(|(_, node)| node)
        }
    }

    /// Returns the number of virtual nodes in the ring.
    pub fn len(&self) -> usize {
        self.ring.len()
    }

    /// Returns true if the ring is empty.
    pub fn is_empty(&self) -> bool {
        self.ring.is_empty()
    }

    // GOTCHA: `DefaultHasher` is not stable across Rust releases or process restarts.
    // For a distributed system where the mapping must be consistent across different processes/machines,
    // you MUST use a stable hash algorithm (e.g., Murmur3, xxHash, FNV).
    // This implementation uses `DefaultHasher` for educational purposes and lack of external crate dependencies.
    fn calculate_hash(&self, node: &T, replica_index: usize) -> u64 {
        let mut hasher = DefaultHasher::new();
        node.hash(&mut hasher);
        replica_index.hash(&mut hasher);
        hasher.finish()
    }

    fn calculate_key_hash<K: Hash + ?Sized>(&self, key: &K) -> u64 {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        hasher.finish()
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `consistent_hash`: Often provides a similar API.
// - `hashring`: Optimized for specific use cases.
//
// Missing vs. Production:
// - **Stable Hashing**: As noted, `DefaultHasher` is random-seeded per process. Production needs stable hashing.
// - **Weighted Nodes**: This implementation assumes all nodes have equal capacity (same `replicas`).
//   Production systems often allow assigning weights (more vnodes) to powerful servers.
// - **Thread Safety**: This structure is not thread-safe. Wrap in `Arc<RwLock<...>>` for concurrent access.
//
// Next Steps:
// 1. Implement `WeightedNode` support.
// 2. Switch to a stable hasher (e.g., FNV1a implementation since we can't add crates).
// 3. Add `get_nodes(key, n)` to return primary + backups for replication.

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
    struct ServerNode(String);

    #[test]
    fn test_empty_ring() {
        let ring: ConsistentHashRing<ServerNode> = ConsistentHashRing::new(3);
        assert!(ring.get_node("any_key").is_none());
    }

    #[test]
    fn test_single_node() {
        let mut ring = ConsistentHashRing::new(3);
        let node = ServerNode("server-1".to_string());
        ring.add_node(node.clone());

        assert_eq!(ring.get_node("key1"), Some(&node));
        assert_eq!(ring.get_node("key2"), Some(&node));
    }

    #[test]
    fn test_multiple_nodes_distribution() {
        let mut ring = ConsistentHashRing::new(10); // More replicas for better distribution
        let node1 = ServerNode("server-1".to_string());
        let node2 = ServerNode("server-2".to_string());
        let node3 = ServerNode("server-3".to_string());

        ring.add_node(node1.clone());
        ring.add_node(node2.clone());
        ring.add_node(node3.clone());

        let mut counts = std::collections::HashMap::new();
        for i in 0..1000 {
            let key = format!("key-{}", i);
            let node = ring.get_node(&key).unwrap();
            *counts.entry(node.clone()).or_insert(0) += 1;
        }

        // With 3 nodes and 10 replicas, distribution should be roughly balanced.
        // It won't be perfect, but all should have some keys.
        assert_eq!(counts.len(), 3);
        for (node, count) in counts {
            println!("Node {:?} has {} keys", node, count);
            assert!(count > 0);
        }
    }

    #[test]
    fn test_add_remove_node() {
        let mut ring = ConsistentHashRing::new(3);
        let node1 = ServerNode("server-1".to_string());
        let node2 = ServerNode("server-2".to_string());

        ring.add_node(node1.clone());
        ring.add_node(node2.clone());

        let key = "test-key";
        let _initial_node = ring.get_node(key).unwrap().clone();

        // Remove the node that owns the key (if it's node1, remove node1; if node2, remove node2)
        // Actually, let's just remove node1 and see.
        ring.remove_node(&node1);

        let new_node = ring.get_node(key).unwrap();
        assert_eq!(new_node, &node2); // Must be node2 since node1 is gone

        ring.remove_node(&node2);
        assert!(ring.get_node(key).is_none());
    }

    #[test]
    fn test_consistency() {
        // Same keys should map to same nodes given same ring state
        let mut ring = ConsistentHashRing::new(5);
        let node1 = ServerNode("server-1".to_string());
        let node2 = ServerNode("server-2".to_string());
        ring.add_node(node1);
        ring.add_node(node2);

        let key = "consistent-key";
        let node_a = ring.get_node(key).unwrap();
        let node_b = ring.get_node(key).unwrap();

        assert_eq!(node_a, node_b);
    }
}
