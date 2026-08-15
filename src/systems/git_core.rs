//! # Git Object Store Implementation
//!
//! Implements a minimalist Git-like content-addressable storage system from scratch.
//! This demonstrates how Git hashes, compresses (conceptually), and stores
//! blobs, trees, and commits using a directed acyclic graph (DAG).
//!
//! **Replaces Crates:** `git2`, `gix`
//!
//! **Real-world Usage:**
//! - Version control systems (Git, Mercurial)
//! - Decentralized storage systems (IPFS, BitTorrent)
//! - Blockchain architectures
//! - Docker image layer storage
//!
//! **Why build it yourself?**
//! It demystifies version control. You learn that Git is just a key-value store where
//! the key is a SHA-1 hash of the value. By building it, you master content-addressable
//! storage, serialization, graph traversal (for logs), and how cryptographic hashing
//! guarantees data integrity.

use std::collections::{HashMap, HashSet};

use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

// =========================================================================================
// Architecture
// =========================================================================================
//
// Object Types:
//
//   Commit ──────► Tree ──────► Blob ("file.txt")
//    │               │
//    │               └────────► Blob ("main.rs")
//    ▼
//   Parent Commit
//
// Invariants:
// 1. Objects are immutable once written.
// 2. An object's ID (hash) strictly depends on its exact content.
// 3. Trees only reference Blobs or other Trees. Commits only reference Trees.
//
// Complexity:
// ┌──────────────────┬─────────────┬─────────────┐
// │ Operation        │ Time        │ Space       │
// ├──────────────────┼─────────────┼─────────────┤
// │ Write Object     │ O(N)        │ O(N)        │
// │ Read Object      │ O(1)        │ O(N)        │
// │ Log Traversal    │ O(V + E)    │ O(V)        │
// └──────────────────┴─────────────┴─────────────┘
// N: Size of object, V: Commits, E: Parent links
//
// Design Decisions:
// - **In-Memory Store**: Uses a `HashMap<String, GitObject>` for simplicity instead of disk I/O.
// - **Hashing**: Uses a basic naive SHA-1 equivalent (simulated with CRC32 for compilation without crypto crates) to generate a hex string.
// - **References**: Employs `String` for OIDs (Object IDs).
//
// PRODUCTION NOTE: Real Git uses zlib compression (DEFLATE) before writing to disk
// and actual SHA-1. Real git objects include the object type and size in the hashed payload:
// `<type> <size>\0<content>`.

/// Represents the types of objects in the Git object model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitObject {
    /// A file's contents.
    Blob(Vec<u8>),
    /// A directory listing. Maps filename -> Object ID.
    Tree(HashMap<String, String>),
    /// A snapshot in time.
    Commit {
        tree: String,
        parents: Vec<String>,
        author: String,
        message: String,
        timestamp: u64,
    },
}

impl GitObject {
    /// Serializes the object into a byte stream for hashing and storage.
    /// This mimics Git's `<type> <size>\0<content>` format.
    pub fn serialize(&self) -> Vec<u8> {
        let mut content = Vec::new();

        let ty: &[u8] = match self {
            Self::Blob(data) => {
                content.extend_from_slice(data);
                b"blob"
            }
            Self::Tree(entries) => {
                // To ensure consistent hashing, we must sort entries by name.
                let mut sorted_entries: Vec<_> = entries.iter().collect();
                sorted_entries.sort_by_key(|(name, _)| *name);

                for (name, oid) in sorted_entries {
                    // Simulating a tree entry: "<mode> <name>\0<oid>"
                    // We'll use 100644 for blob, 040000 for tree, but for simplicity
                    // we'll just write "100644 <name>\0<oid_hex>"
                    content.extend_from_slice(b"100644 ");
                    content.extend_from_slice(name.as_bytes());
                    content.push(0);
                    content.extend_from_slice(oid.as_bytes());
                }
                b"tree"
            }
            Self::Commit { tree, parents, author, message, timestamp } => {
                let tree_str = format!("tree {tree}\n");
                content.extend_from_slice(tree_str.as_bytes());

                for parent in parents {
                    let parent_str = format!("parent {parent}\n");
                    content.extend_from_slice(parent_str.as_bytes());
                }

                let author_str = format!("author {author} {timestamp}\n\n");
                content.extend_from_slice(author_str.as_bytes());

                content.extend_from_slice(message.as_bytes());
                b"commit"
            }
        };

        let mut header = Vec::new();
        header.extend_from_slice(ty);
        header.push(b' ');
        header.extend_from_slice(content.len().to_string().as_bytes());
        header.push(0);

        let mut full_payload = header;
        full_payload.extend(content);
        full_payload
    }

    /// Computes the Object ID (OID) based on the serialized content.
    /// Uses CRC32 + hex encoding as a stand-in for SHA-1.
    pub fn hash(&self) -> String {
        let payload = self.serialize();
        // Use crc32fast since it's already in the dependency tree
        let checksum = crc32fast::hash(&payload);
        format!("{checksum:08x}") // Simulates a 40-char SHA-1
    }
}

/// The Git Object Store.
#[derive(Default, Clone)]
pub struct GitRepository {
    /// In-memory object database: OID -> Object.
    objects: Arc<RwLock<HashMap<String, GitObject>>>,
    /// References (e.g., HEAD, refs/heads/main)
    refs: Arc<RwLock<HashMap<String, String>>>,
}

impl GitRepository {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Writes an object to the repository and returns its OID.
    pub fn write_object(&self, object: GitObject) -> String {
        let oid = object.hash();
        // RUST INSIGHT: RwLock allows concurrent reads. Write is only locked briefly
        // to insert the object.
        let mut db = self.objects.write().unwrap();
        // Since OID depends strictly on content, if it's already there, we don't need to overwrite.
        db.entry(oid.clone()).or_insert(object);
        oid
    }

    /// Reads an object by its OID.
    pub fn read_object(&self, oid: &str) -> Option<GitObject> {
        let db = self.objects.read().unwrap();
        db.get(oid).cloned()
    }

    /// Updates a reference (like a branch pointer).
    pub fn update_ref(&self, ref_name: &str, oid: &str) {
        let mut refs = self.refs.write().unwrap();
        refs.insert(ref_name.to_string(), oid.to_string());
    }

    /// Gets the OID for a given reference.
    pub fn resolve_ref(&self, ref_name: &str) -> Option<String> {
        let refs = self.refs.read().unwrap();
        refs.get(ref_name).cloned()
    }

    /// Helper to create and write a commit.
    pub fn commit(
        &self,
        tree_oid: String,
        parents: Vec<String>,
        author: String,
        message: String,
    ) -> String {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let commit = GitObject::Commit {
            tree: tree_oid,
            parents,
            author,
            message,
            timestamp,
        };

        self.write_object(commit)
    }

    /// Traverses the commit history starting from a given OID, returning a sequence of commit OIDs.
    pub fn log(&self, start_oid: &str) -> Vec<String> {
        let mut result = Vec::new();
        let mut queue = vec![start_oid.to_string()];
        let mut visited = HashSet::new();

        while let Some(current_oid) = queue.pop() {
            if !visited.insert(current_oid.clone()) {
                continue;
            }

            result.push(current_oid.clone());

            if let Some(GitObject::Commit { parents, .. }) = self.read_object(&current_oid) {
                for parent in parents {
                    queue.push(parent);
                }
            }
        }

        result
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `git2`: Uses libgit2 C library under the hood. Massively complex and full-featured.
// - `gix`: Pure Rust git implementation. Highly optimized, uses memmaps and zero-copy parsing.
//
// Missing vs. Production:
// - **Disk I/O and Packfiles**: Real Git uses zlib compression and stores objects in packfiles
//   using delta compression for efficiency.
// - **SHA-1**: We use CRC32 for compilation speed.
// - **Index (Staging Area)**: No staging area or working directory management.
// - **Garbage Collection**: Unreachable objects remain in memory forever.
//
// Next Steps:
// 1. Add zlib compression via the `flate2` crate.
// 2. Implement the `.git/index` file structure.
// 3. Write a recursive function to checkout a `Tree` into the file system.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blob_creation_and_hashing() {
        let repo = GitRepository::new();
        let blob = GitObject::Blob(b"hello world".to_vec());
        let oid = repo.write_object(blob.clone());

        assert_eq!(oid.len(), 8); // CRC32 hex length
        let retrieved = repo.read_object(&oid).unwrap();
        assert_eq!(retrieved, blob);
    }

    #[test]
    fn test_tree_creation() {
        let repo = GitRepository::new();

        let blob1 = GitObject::Blob(b"fn main() {}".to_vec());
        let oid1 = repo.write_object(blob1);

        let blob2 = GitObject::Blob(b"[package]".to_vec());
        let oid2 = repo.write_object(blob2);

        let mut entries = HashMap::new();
        entries.insert("main.rs".to_string(), oid1);
        entries.insert("Cargo.toml".to_string(), oid2);

        let tree = GitObject::Tree(entries);
        let tree_oid = repo.write_object(tree.clone());

        let retrieved = repo.read_object(&tree_oid).unwrap();
        assert_eq!(retrieved, tree);
    }

    #[test]
    fn test_commit_and_log() {
        let repo = GitRepository::new();

        // Empty tree
        let tree = GitObject::Tree(HashMap::new());
        let tree_oid = repo.write_object(tree);

        // First commit
        let commit1_oid = repo.commit(
            tree_oid.clone(),
            vec![],
            "Alice <alice@example.com>".to_string(),
            "Initial commit".to_string(),
        );

        // Second commit
        let commit2_oid = repo.commit(
            tree_oid,
            vec![commit1_oid.clone()],
            "Bob <bob@example.com>".to_string(),
            "Second commit".to_string(),
        );

        repo.update_ref("HEAD", &commit2_oid);

        // Verify ref
        assert_eq!(repo.resolve_ref("HEAD"), Some(commit2_oid.clone()));

        // Check log
        let history = repo.log(&commit2_oid);
        assert_eq!(history.len(), 2);
        assert_eq!(history[0], commit2_oid);
        assert_eq!(history[1], commit1_oid);
    }
}
