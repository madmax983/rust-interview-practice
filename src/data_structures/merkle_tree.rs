//! Merkle Tree implementation.
//!
//! # Header
//!
//! *   **Problem Name**: Merkle Tree
//! *   **Difficulty**: Hard
//! *   **Link**: <https://en.wikipedia.org/wiki/Merkle_tree>
//! *   **Why this matters in Rust**: Fundamental to distributed systems (Git, BitTorrent, Blockchains) for efficient data verification.
//!
//! # Architecture
//!
//! A Merkle Tree is a binary tree where every leaf node is labeled with the cryptographic hash of a data block,
//! and every non-leaf node is labeled with the cryptographic hash of the labels of its child nodes.
//!
//! **Diagram:**
//!
//! ```text
//!          Root Hash (H1234)
//!         /                 \
//!     H12 (H1+H2)         H34 (H3+H4)
//!     /      \            /      \
//!   H1        H2        H3        H4
//! (Data1)   (Data2)   (Data3)   (Data4)
//! ```
//!
//! **Invariants:**
//! *   The root hash uniquely identifies the entire dataset.
//! *   Changing any single data bit changes the root hash.
//! *   A proof consists of the sibling hashes along the path from the leaf to the root.
//!
//! **Complexity:**
//!
//! | Operation | Time | Space |
//! | :--- | :--- | :--- |
//! | Build | O(N) | O(N) |
//! | Generate Proof | O(log N) | O(log N) |
//! | Verify Proof | O(log N) | O(1) |

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// A Merkle Tree implementation using `DefaultHasher`.
///
/// **WARNING**: This implementation uses `DefaultHasher` which is not cryptographically secure
/// and can vary across Rust versions. In production, use a stable cryptographic hash like SHA-256.
// RUST INSIGHT: We use a generic type `T` that implements `Hash`. This allows us to store any hashable data.
// We store the tree layers for proof generation.
#[derive(Debug, Clone)]
pub struct MerkleTree<T: Hash> {
    layers: Vec<Vec<u64>>,
    data: Vec<T>, // Keep original data to verify leaves if needed.
}

impl<T: Hash + Clone> MerkleTree<T> {
    /// Constructs a Merkle Tree from a list of items.
    // GOTCHA: If the number of items is odd, the last item is duplicated to balance the tree level.
    pub fn new(data: Vec<T>) -> Self {
        if data.is_empty() {
            return MerkleTree {
                layers: vec![],
                data,
            };
        }

        let mut current_layer: Vec<u64> = data.iter().map(|item| Self::hash_item(item)).collect();
        let mut layers = vec![current_layer.clone()];

        while current_layer.len() > 1 {
            // ⚡ BOLT OPTIMIZATION: Use `Vec::with_capacity` to eliminate intermediate heap allocations
            // when building the next layer, saving O(N) reallocations where N is the current layer size.
            let mut next_layer = Vec::with_capacity((current_layer.len() + 1) / 2);
            for chunk in current_layer.chunks(2) {
                let left = chunk[0];
                let right = if chunk.len() > 1 { chunk[1] } else { chunk[0] }; // Duplicate if odd
                next_layer.push(Self::hash_pair(left, right));
            }
            layers.push(next_layer.clone());
            current_layer = next_layer;
        }

        MerkleTree { layers, data }
    }

    /// Returns the root hash of the tree.
    pub fn root(&self) -> Option<u64> {
        self.layers.last().and_then(|layer| layer.first().copied())
    }

    /// Generates a Merkle Proof for the item at the given index.
    /// The proof consists of a list of sibling hashes needed to recompute the root.
    pub fn generate_proof(&self, index: usize) -> Option<Vec<u64>> {
        if index >= self.data.len() {
            return None;
        }

        // ⚡ BOLT OPTIMIZATION: Use `Vec::with_capacity` to eliminate intermediate heap allocations
        // when generating the Merkle proof, saving `layers.len() - 1` potential reallocations.
        let mut proof = Vec::with_capacity(self.layers.len().saturating_sub(1));
        let mut current_index = index;

        // Iterate through all layers except the root (which is the last layer)
        for layer in self.layers.iter().take(self.layers.len() - 1) {
            let is_left_child = current_index.is_multiple_of(2);
            let sibling_index = if is_left_child {
                current_index + 1
            } else {
                current_index - 1
            };

            // If sibling index is out of bounds (odd number of nodes at this level),
            // the node was duplicated with itself, so the sibling is itself (at current_index).
            let sibling_hash = if sibling_index < layer.len() {
                layer[sibling_index]
            } else {
                layer[current_index]
            };

            proof.push(sibling_hash);
            current_index /= 2;
        }

        Some(proof)
    }

    /// Verifies a Merkle Proof.
    /// Returns true if the proof is valid for the given item, index, and root.
    // RUST INSIGHT: This is a static method that mimics a light client verification.
    // It does not require the full tree, only the root hash.
    pub fn verify(root: u64, item: &T, proof: &[u64], mut index: usize) -> bool {
        let mut current_hash = Self::hash_item(item);

        for &sibling_hash in proof {
            let is_left_child = index.is_multiple_of(2);
            if is_left_child {
                current_hash = Self::hash_pair(current_hash, sibling_hash);
            } else {
                current_hash = Self::hash_pair(sibling_hash, current_hash);
            }
            index /= 2;
        }

        current_hash == root
    }

    // Helper to hash a single item
    fn hash_item(item: &T) -> u64 {
        let mut hasher = DefaultHasher::new();
        item.hash(&mut hasher);
        hasher.finish()
    }

    // Helper to hash a pair of hashes
    fn hash_pair(left: u64, right: u64) -> u64 {
        let mut hasher = DefaultHasher::new();
        left.hash(&mut hasher);
        right.hash(&mut hasher);
        hasher.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merkle_tree_construction() {
        let data = vec!["a", "b", "c", "d"];
        let tree = MerkleTree::new(data.clone());

        // 4 leaves -> 2 intermediate -> 1 root.
        // Layers should be:
        // 0: [H(a), H(b), H(c), H(d)] (len 4)
        // 1: [H(Ha+Hb), H(Hc+Hd)]     (len 2)
        // 2: [Root]                   (len 1)
        assert_eq!(tree.layers.len(), 3);
        assert_eq!(tree.layers[0].len(), 4);
        assert_eq!(tree.layers[1].len(), 2);
        assert_eq!(tree.layers[2].len(), 1);

        assert!(tree.root().is_some());
    }

    #[test]
    fn test_merkle_tree_odd_items() {
        let data = vec!["a", "b", "c"];
        let tree = MerkleTree::new(data.clone());

        // 3 leaves. 'c' is duplicated to make 'cc'.
        // Layer 0: [H(a), H(b), H(c)] (len 3)
        // Layer 1: [H(Ha+Hb), H(Hc+Hc)] (len 2)
        // Layer 2: [Root] (len 1)
        assert_eq!(tree.layers.len(), 3);
        assert_eq!(tree.layers[0].len(), 3);
        assert_eq!(tree.layers[1].len(), 2);
        assert_eq!(tree.layers[2].len(), 1);
    }

    #[test]
    fn test_proof_verification() {
        let data = vec!["block1", "block2", "block3", "block4", "block5"];
        let tree = MerkleTree::new(data.clone());
        let root = tree.root().unwrap();

        for (i, item) in data.iter().enumerate() {
            let proof = tree.generate_proof(i).unwrap();
            assert!(
                MerkleTree::verify(root, item, &proof, i),
                "Failed to verify item {} at index {}",
                item,
                i
            );
        }
    }

    #[test]
    fn test_tampered_data() {
        let data = vec!["valid"];
        let tree = MerkleTree::new(data.clone());
        let root = tree.root().unwrap();
        let proof = tree.generate_proof(0).unwrap();

        assert!(!MerkleTree::verify(root, &"tampered", &proof, 0));
    }

    #[test]
    fn test_empty_tree() {
        let data: Vec<&str> = vec![];
        let tree = MerkleTree::new(data);
        assert!(tree.root().is_none());
    }
}

// Footer
//
// *   **Comparison**: Production crates like `rs_merkle` or `merkle_light` offer more hashing algorithm choices and optimized storage.
// *   **Missing features**: Sparse Merkle Trees, sorted leaves (for non-inclusion proofs), different hash functions support.
