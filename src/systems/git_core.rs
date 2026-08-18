//! # Git Object Store Implementation
//!
//! Implements a simplified, educational version of a Git-compatible content-addressable object store.
//! It handles the core concepts of `Blob`, `Tree`, and `Commit` objects, including serialization,
//! SHA-1 hashing basics (using an educational placeholder or built-in hasher), and formatting.
//!
//! **Replaces Crates:** `git2`, `gix` (gitoxide)
//!
//! **Real-world Usage:**
//! - Version control systems (Git itself).
//! - Distributed file systems and immutable databases (IPFS, Nix store).
//! - Merkle tree-based data structures (Blockchains).
//!
//! **Why build it yourself?**
//! Understanding how Git builds a directed acyclic graph (DAG) out of simple content-addressed
//! byte arrays demystifies "magic" Git operations. You'll learn how blobs represent file content,
//! trees represent directories, and commits represent snapshots in time. It also forces you to handle
//! precise byte serialization (like null-byte separation) which is critical in binary protocols.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure Diagram (Git Object Graph):
//
//   [Commit Object] ---> [Tree Object] ---> [Blob Object (File a.txt)]
//         |                     |
//         v                     +---------> [Tree Object (Folder sub/)]
//   [Parent Commit]                               |
//                                                 v
//                                          [Blob Object (File b.txt)]
//
// Invariants:
// 1. Every object is identified by a hash of its serialized content (Header + Body).
// 2. The Header format is `<type> <size>\0`.
// 3. Tree objects contain sorted entries (mode, name, sha1).
// 4. Content is immutable; changing anything changes the hash (cascading up to the commit).
//
// Complexity:
// ┌────────────────┬─────────────────────┬──────────────────┐
// │ Operation      │ Time                │ Space            │
// ├────────────────┼─────────────────────┼──────────────────┤
// │ Serialize Blob │ O(N) where N=size   │ O(N)             │
// │ Serialize Tree │ O(E log E + N)      │ O(N)             │
// │ Hash Object    │ O(N)                │ O(1) beyond body │
// └────────────────┴─────────────────────┴──────────────────┘
//
// Design Decisions:
// - **Hashing**: Uses standard `DefaultHasher` for educational simplicity instead of full SHA-1.
//   - *Tradeoff*: Not genuinely compatible with real Git (which requires strict SHA-1).
//   - *Alternative*: Implementing a full SHA-1 would be too verbose for this demonstration,
//     but the *structure* of hashing the exact byte format is identical.
// - **Enums vs Traits**: Using an Enum `GitObject` allows a heterogeneous store.

/// Represents the 40-character hex string (20 bytes) of a hash.
/// For this educational model, we just use a u64 hex formatted to string.
pub type HashString = String;

/// Helper function to compute our mock "SHA-1".
fn compute_hash(data: &[u8]) -> HashString {
    let mut hasher = DefaultHasher::new();
    data.hash(&mut hasher);
    let hash_val = hasher.finish();
    // In real git, this is a 40-char hex of a 20-byte SHA1.
    // We pad a u64 to 16 chars, then append 0s to make it 40 for realism.
    format!("{:016x}000000000000000000000000", hash_val)
}

/// Core trait for Git objects.
pub trait GitObjectExt {
    /// Gets the git object type string (e.g., "blob", "tree", "commit").
    fn object_type(&self) -> &'static str;

    /// Serializes the object body (without the header).
    fn serialize_body(&self) -> Vec<u8>;

    /// Serializes the full object (header + body).
    fn serialize(&self) -> Vec<u8> {
        let body = self.serialize_body();
        let header = format!("{} {}\0", self.object_type(), body.len());

        // ⚡ BOLT OPTIMIZATION: Instead of `let mut full = ...; full.extend(...)`,
        // allocate exactly what is needed to avoid reallocations.
        let mut full = Vec::with_capacity(header.len() + body.len());
        full.extend_from_slice(header.as_bytes());
        full.extend_from_slice(&body);
        full
    }

    /// Computes the object ID (hash of the serialized object).
    fn id(&self) -> HashString {
        compute_hash(&self.serialize())
    }
}

/// A Blob object represents file content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Blob {
    pub content: Vec<u8>,
}

impl Blob {
    #[must_use]
    pub const fn new(content: Vec<u8>) -> Self {
        Self { content }
    }
}

impl GitObjectExt for Blob {
    fn object_type(&self) -> &'static str {
        "blob"
    }

    fn serialize_body(&self) -> Vec<u8> {
        // RUST INSIGHT: Cloning a Vec can be expensive, but required here for the API.
        // A more advanced implementation would use `Cow<[u8]>` or references.
        self.content.clone()
    }
}

/// A mode for a tree entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileMode {
    Regular,
    Executable,
    Symlink,
    Directory,
}

impl FileMode {
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Regular => "100644",
            Self::Executable => "100755",
            Self::Symlink => "120000",
            Self::Directory => "40000", // git trees omit the leading 0 in 040000
        }
    }
}

/// An entry in a Tree object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeEntry {
    pub mode: FileMode,
    pub name: String,
    pub sha1: HashString,
}

/// A Tree object represents a directory (a list of tree entries).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tree {
    pub entries: Vec<TreeEntry>,
}

impl Tree {
    #[must_use]
    pub const fn new(entries: Vec<TreeEntry>) -> Self {
        Self { entries }
    }
}

impl GitObjectExt for Tree {
    fn object_type(&self) -> &'static str {
        "tree"
    }

    fn serialize_body(&self) -> Vec<u8> {
        let mut body = Vec::new();
        // GOTCHA: Git requires tree entries to be sorted by name.
        let mut sorted_entries = self.entries.clone();
        sorted_entries.sort_by(|a, b| a.name.cmp(&b.name));

        for entry in sorted_entries {
            // Format: `<mode> <name>\0<20_byte_sha1>`
            // Note: In real git, the sha1 is raw bytes, not hex string.
            // We use the hex string bytes here for simplicity in our mock.
            let entry_str = format!("{} {}\0", entry.mode.as_str(), entry.name);
            body.extend_from_slice(entry_str.as_bytes());

            // For a real Git implementation, we would parse the hex back to 20 raw bytes.
            // Here we just write the string bytes.
            body.extend_from_slice(entry.sha1.as_bytes());
        }
        body
    }
}

/// A Commit object represents a snapshot in time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commit {
    pub tree: HashString,
    pub parents: Vec<HashString>,
    pub author: String,
    pub committer: String,
    pub message: String,
}

impl GitObjectExt for Commit {
    fn object_type(&self) -> &'static str {
        "commit"
    }

    fn serialize_body(&self) -> Vec<u8> {
        let mut body = String::new();
        body.push_str(&format!("tree {}\n", self.tree));

        for parent in &self.parents {
            body.push_str(&format!("parent {}\n", parent));
        }

        body.push_str(&format!("author {}\n", self.author));
        body.push_str(&format!("committer {}\n", self.committer));
        body.push('\n');
        body.push_str(&self.message);

        body.into_bytes()
    }
}

/// An enum representing any Git object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitObject {
    Blob(Blob),
    Tree(Tree),
    Commit(Commit),
}

impl GitObjectExt for GitObject {
    fn object_type(&self) -> &'static str {
        match self {
            Self::Blob(b) => b.object_type(),
            Self::Tree(t) => t.object_type(),
            Self::Commit(c) => c.object_type(),
        }
    }

    fn serialize_body(&self) -> Vec<u8> {
        match self {
            Self::Blob(b) => b.serialize_body(),
            Self::Tree(t) => t.serialize_body(),
            Self::Commit(c) => c.serialize_body(),
        }
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `git2`: A robust C-binding to `libgit2` that handles everything natively.
// - `gix`: A pure Rust implementation of Git, highly optimized and feature-complete.
//
// Missing vs. Production:
// - **Hash Algorithm**: We use `DefaultHasher` -> hex string instead of real SHA-1.
// - **Compression**: Real git zlib-deflates objects before writing them to disk.
// - **Packfiles**: Real git uses packfiles (delta compression) to save space. We only simulate loose objects.
// - **Strict Parsing**: We only implement serialization, not parsing loose objects from disk.
//
// Next Steps:
// 1. Implement actual SHA-1 hashing (`sha1` crate or custom).
// 2. Add zlib compression/decompression for writing to `.git/objects/...`.
// 3. Implement parsing objects back from bytes.
//
// Benchmarking:
// To benchmark serialization performance, use `criterion` to measure `Blob::serialize()` and `Tree::serialize()`
// with varying sizes and counts of tree entries, taking care to `black_box` the inputs to avoid optimizer elision.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blob_serialization() {
        let blob = Blob::new(b"hello world".to_vec());
        assert_eq!(blob.object_type(), "blob");

        let body = blob.serialize_body();
        assert_eq!(body, b"hello world");

        let full = blob.serialize();
        assert!(full.starts_with(b"blob 11\0"));
        assert!(full.ends_with(b"hello world"));
    }

    #[test]
    fn test_tree_serialization() {
        let entry1 = TreeEntry {
            mode: FileMode::Regular,
            name: "a.txt".to_string(),
            sha1: "1234567890123456789012345678901234567890".to_string(),
        };
        let entry2 = TreeEntry {
            mode: FileMode::Directory,
            name: "dir".to_string(),
            sha1: "0987654321098765432109876543210987654321".to_string(),
        };

        let tree = Tree::new(vec![entry2, entry1]); // Out of order

        let full = tree.serialize();
        let full_str = String::from_utf8_lossy(&full);

        assert!(full_str.starts_with("tree "));
        // Should be sorted by name: a.txt before dir
        let pos_a = full_str.find("a.txt").unwrap();
        let pos_dir = full_str.find("dir").unwrap();
        assert!(pos_a < pos_dir);
    }

    #[test]
    fn test_commit_serialization() {
        let commit = Commit {
            tree: "tree_hash".to_string(),
            parents: vec!["parent_hash".to_string()],
            author: "Author Name <author@example.com> 1620000000 +0000".to_string(),
            committer: "Committer Name <committer@example.com> 1620000000 +0000".to_string(),
            message: "Initial commit\n".to_string(),
        };

        let full = commit.serialize();
        let full_str = String::from_utf8_lossy(&full);

        assert!(full_str.starts_with("commit "));
        assert!(full_str.contains("tree tree_hash\n"));
        assert!(full_str.contains("parent parent_hash\n"));
        assert!(full_str.contains("\n\nInitial commit\n"));
    }

    #[test]
    fn test_object_id_stability() {
        let blob1 = Blob::new(b"same content".to_vec());
        let blob2 = Blob::new(b"same content".to_vec());
        let blob3 = Blob::new(b"diff content".to_vec());

        assert_eq!(blob1.id(), blob2.id());
        assert_ne!(blob1.id(), blob3.id());
    }
}
