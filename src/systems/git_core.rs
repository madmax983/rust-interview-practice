//! # Git Object Store
//!
//! ## What this implements and what it replaces
//! This implements a simplified version of Git's content-addressable object store.
//! It handles the serialization of Git objects (Blobs, Trees, and Commits) and hashing.
//! This replaces the core object serialization logic found in crates like `git2` or `gix` (gitoxide).
//!
//! ## Real-world systems that use this
//! Git itself! This is the foundational model for how version control systems store snapshots of files
//! (Blobs), directories (Trees), and history (Commits) in a deduplicated, cryptographically verified way.
//!
//! ## Why build it yourself?
//! Understanding the Git object model transforms Git from a "magic box of commands" into a predictable
//! data structure (a Directed Acyclic Graph of immutable objects). Building it reveals how content addressing
//! naturally deduplicates data and guarantees integrity.
//!
//! ## Architecture
//! ```text
//! Commit (metadata + pointer to Root Tree)
//!   |
//!   v
//! Tree (directory listing)
//!   |
//!   |---> Blob (file content)
//!   |
//!   |---> Tree (subdirectory)
//!           |
//!           v
//!         Blob (file content)
//! ```
//!
//! ### Invariants
//! - The Hash (SHA-1) of an object must be derived strictly from its serialized byte format.
//! - Objects are immutable once created.
//!
//! ### Time / Space Complexity
//! - **Hashing / Serialization**: Time `O(N)`, Space `O(N)` where `N` is the size of the object data.
//!
//! ### Design Decisions & Tradeoffs
//! - **SHA-1**: Git uses SHA-1 (and is transitioning to SHA-256). For simplicity, we mock the SHA-1 hash with a dummy hash function or `sha1` crate if available. Here we implement a very naive, toy hashing function to keep it dependency-free, returning a mock 40-char hex string just for structural demonstration.
//! - **Compression**: Real Git compresses objects using zlib before storing them on disk. We omit zlib compression to focus on the object serialization format.

use std::fmt;

/// Represents a 40-character Git Hash (mocked).
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct GitHash(pub String);

impl fmt::Debug for GitHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A simplified representation of a Git Object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitObject {
    Blob(Vec<u8>),
    Tree(Vec<TreeEntry>),
    Commit {
        tree_hash: GitHash,
        parent_hashes: Vec<GitHash>,
        author: String,
        message: String,
    },
}

/// An entry in a Git Tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeEntry {
    pub mode: String,
    pub name: String,
    pub hash: GitHash,
}

impl GitObject {
    /// Serializes the object into the format Git uses for hashing and storage.
    /// Format: `<type> <content-length>\0<content>`
    ///
    /// # Panics
    /// Panics if the type is not valid UTF-8.
    #[must_use]
    pub fn serialize(&self) -> Vec<u8> {
        let mut content = Vec::new();

        // RUST INSIGHT: We dynamically coerce the array to a slice (`&[u8]`)
        // to satisfy E0308 because match arms returning byte strings of different lengths
        // (`b"blob"`, `b"commit"`) have different underlying types (e.g., `&[u8; 4]`, `&[u8; 6]`).
        let ty: &[u8] = match self {
            Self::Blob(_) => b"blob",
            Self::Tree(_) => b"tree",
            Self::Commit { .. } => b"commit",
        };

        match self {
            Self::Blob(data) => {
                content.extend_from_slice(data);
            }
            Self::Tree(entries) => {
                for entry in entries {
                    // format: [mode] [name]\0[hash bytes]
                    let entry_header = format!("{} {}\0", entry.mode, entry.name);
                    content.extend_from_slice(entry_header.as_bytes());
                    // Real git stores the 20-byte raw hash here. We use the hex string bytes for simplicity.
                    content.extend_from_slice(entry.hash.0.as_bytes());
                }
            }
            Self::Commit {
                tree_hash,
                parent_hashes,
                author,
                message,
            } => {
                use std::fmt::Write;
                let mut commit_str = String::new();
                let _ = writeln!(commit_str, "tree {}", tree_hash.0);
                for p in parent_hashes {
                    let _ = writeln!(commit_str, "parent {}", p.0);
                }
                let _ = writeln!(commit_str, "author {author}");
                let _ = writeln!(commit_str, "committer {author}"); // simplified
                commit_str.push('\n');
                commit_str.push_str(message);
                content.extend_from_slice(commit_str.as_bytes());
            }
        }

        let mut header =
            format!("{} {}\0", std::str::from_utf8(ty).unwrap(), content.len()).into_bytes();
        header.extend(content);
        header
    }

    /// Calculates the "hash" of this object based on its serialized form.
    ///
    /// // UNSAFE JUSTIFICATION: No unsafe used.
    /// // PRODUCTION NOTE: A real implementation would use a robust SHA-1 (or SHA-256) implementation here.
    #[must_use]
    pub fn calculate_hash(&self) -> GitHash {
        let serialized = self.serialize();
        // Very fake hash for demonstration! Just sums bytes.
        let mut sum: u32 = 0;
        for &b in &serialized {
            sum = sum.wrapping_add(u32::from(b));
        }
        GitHash(format!("{sum:040x}"))
    }
}

/// ## Footer
///
/// ### Comparison to Canonical Crates
/// Crates like `gix` (gitoxide) provide extremely optimized, zero-copy, highly concurrent
/// parsers and serializers for Git objects, supporting memory-mapped packs, zlib decompression,
/// and exact SHA-1 semantics.
///
/// ### Missing vs Production
/// - Real SHA-1 / SHA-256 implementation.
/// - zlib compression.
/// - Packfile support (Git packs many objects together into compressed deltas).
///
/// ### Benchmarking Notes
/// The `serialize()` and `calculate_hash()` functions can be benchmarked using `criterion` by
/// passing large `GitObject::Blob` instances containing multimegabyte dummy file data to measure
/// serialization throughput and hashing speed.
///
/// ### Suggested Next Steps
/// - Implement a basic `ObjectDatabase` struct that writes these serialized, compressed bytes to `.git/objects/...`.
/// - Parse Git objects back into the `GitObject` enum.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blob_serialization() {
        let blob = GitObject::Blob(b"hello world".to_vec());
        let serialized = blob.serialize();
        let expected = b"blob 11\0hello world";
        assert_eq!(serialized, expected);
    }

    #[test]
    fn test_commit_serialization() {
        let commit = GitObject::Commit {
            tree_hash: GitHash("tree123".to_string()),
            parent_hashes: vec![],
            author: "Alice".to_string(),
            message: "initial commit".to_string(),
        };
        let serialized = commit.serialize();

        let content_str = "tree tree123\nauthor Alice\ncommitter Alice\n\ninitial commit";
        let header_str = format!("commit {}\0", content_str.len());

        let mut expected = header_str.into_bytes();
        expected.extend_from_slice(content_str.as_bytes());

        assert_eq!(serialized, expected);
    }

    #[test]
    fn test_hashing_determinism() {
        let blob1 = GitObject::Blob(b"hello".to_vec());
        let blob2 = GitObject::Blob(b"hello".to_vec());
        assert_eq!(blob1.calculate_hash(), blob2.calculate_hash());
    }
}
