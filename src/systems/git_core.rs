//! # Git Object Store Implementation
//!
//! Implements a from-scratch Git object system, covering parsing and serialization of
//! Blob, Tree, and Commit objects, as well as addressing them via SHA-1 hashes.
//!
//! **Replaces Crates:** `git2` (partial object handling), `gix` (gitoxide partial)
//!
//! **Real-world Usage:**
//! - Git clients and servers
//! - Distributed version control systems
//! - Code hosting platforms (GitHub, GitLab, Bitbucket)
//!
//! **Why build it yourself?**
//! Implementing Git's core object model teaches you about content-addressable storage.
//! You will learn how Git represents versions simply as snapshots of trees and blobs,
//! and how it strings them together with commits. It's an excellent exercise in parsing
//! custom binary formats (like tree objects) and understanding Merkle trees.

use crate::cryptography::sha256::Sha256;
use std::collections::BTreeMap;
use std::fmt;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Object Storage Model:
// Git is fundamentally a content-addressable filesystem. Every object is identified
// by the hash (traditionally SHA-1, but we use our SHA-256 for demonstration) of its
// contents.
//
// Objects have a standard header format: `<type> <size>\0<content>`
//
// ┌────────────────┐
// │ Commit Object  │
// │ - tree hash    │
// │ - parent hash  │
// │ - author       │
// │ - message      │
// └────────┬───────┘
//          │
//          ▼
// ┌────────────────┐
// │ Tree Object    │
// │ - mode name \0 │──► Blob (File)
// │   hash         │
// │ - mode name \0 │──► Tree (Directory)
// │   hash         │
// └────────────────┘
//
// Invariants:
// 1. The object ID (OID) is always derived exclusively from the exact bytes
//    of the serialized object (including the header).
// 2. Tree objects must sort their entries by name to ensure stable hashes.
// 3. Blobs contain only raw file data with a header.
//
// Complexity:
// ┌───────────┬──────────────┬────────┐
// │ Operation │ Time         │ Space  │
// ├───────────┼──────────────┼────────┤
// │ Serialize │ O(N)         │ O(N)   │
// │ Hash      │ O(N)         │ O(1)   │
// └───────────┴──────────────┴────────┘
// (where N is the size of the object data)
//
// RUST INSIGHT:
// Using enums (`GitObject`) allows us to represent all Git object types safely.
// `BTreeMap` ensures our Tree entries are always sorted, preventing accidental
// non-deterministic hashes based on insertion order.

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ObjectId(pub [u8; 32]); // We mock SHA-1 with SHA-256 for this implementation

impl fmt::Display for ObjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in &self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl ObjectId {
    #[must_use]
    pub const fn new(hash: [u8; 32]) -> Self {
        Self(hash)
    }

    /// Helper to create from a string (for testing)
    #[must_use]
    pub fn from_hex(hex: &str) -> Option<Self> {
        if hex.len() != 64 {
            return None;
        }
        let mut bytes = [0u8; 32];
        for i in 0..32 {
            bytes[i] = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok()?;
        }
        Some(Self(bytes))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitObject {
    Blob(Vec<u8>),
    Tree(BTreeMap<String, TreeEntry>),
    Commit {
        tree: ObjectId,
        parents: Vec<ObjectId>,
        author: String,
        committer: String,
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeEntry {
    pub mode: String,
    pub oid: ObjectId,
}

impl GitObject {
    /// Serializes the Git object into its raw byte representation, including the header.
    #[must_use]
    pub fn serialize(&self) -> Vec<u8> {
        let (obj_type, content) = match self {
            // RUST INSIGHT: Cloning the blob data here causes an allocation.
            // A more optimized implementation would stream the bytes directly to a `Write` trait.
            Self::Blob(data) => ("blob", data.clone()),
            Self::Tree(entries) => ("tree", serialize_tree(entries)),
            Self::Commit {
                tree,
                parents,
                author,
                committer,
                message,
            } => (
                "commit",
                serialize_commit(tree, parents, author, committer, message),
            ),
        };

        let mut data = Vec::with_capacity(obj_type.len() + 1 + 20 + content.len());
        // Header format: `<type> <size>\0`
        // GOTCHA: Git requires the size in bytes of the content *before* compression
        // in its header string, not the number of characters.
        data.extend_from_slice(obj_type.as_bytes());
        data.push(b' ');
        data.extend_from_slice(content.len().to_string().as_bytes());
        data.push(0);
        data.extend_from_slice(&content);
        data
    }

    /// Computes the Object ID (hash) of this object.
    #[must_use]
    pub fn id(&self) -> ObjectId {
        let data = self.serialize();
        // PRODUCTION NOTE: In a real Git implementation, this would be SHA-1.
        // We use our existing SHA-256 for ease.
        let mut hasher = Sha256::new();
        hasher.update(&data);
        ObjectId(hasher.finalize())
    }
}

fn serialize_tree(entries: &BTreeMap<String, TreeEntry>) -> Vec<u8> {
    let mut data = Vec::new();
    for (name, entry) in entries {
        // Format: `<mode> <name>\0<20-byte/32-byte hash>`
        data.extend_from_slice(entry.mode.as_bytes());
        data.push(b' ');
        data.extend_from_slice(name.as_bytes());
        data.push(0);
        data.extend_from_slice(&entry.oid.0);
    }
    data
}

fn serialize_commit(
    tree: &ObjectId,
    parents: &[ObjectId],
    author: &str,
    committer: &str,
    message: &str,
) -> Vec<u8> {
    let mut data = Vec::new();

    // Tree
    data.extend_from_slice(b"tree ");
    data.extend_from_slice(tree.to_string().as_bytes());
    data.push(b'\n');

    // Parents
    for parent in parents {
        data.extend_from_slice(b"parent ");
        data.extend_from_slice(parent.to_string().as_bytes());
        data.push(b'\n');
    }

    // Author/Committer
    data.extend_from_slice(b"author ");
    data.extend_from_slice(author.as_bytes());
    data.push(b'\n');

    data.extend_from_slice(b"committer ");
    data.extend_from_slice(committer.as_bytes());
    data.push(b'\n');

    // Message
    data.push(b'\n');
    data.extend_from_slice(message.as_bytes());
    data.push(b'\n'); // Commits typically end with a newline

    data
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `git2`: Provides bindings to `libgit2` (C). Full-featured, heavily optimized, but requires linking C code.
// - `gix`: Pure Rust Git implementation. Very fast and comprehensive.
//
// Missing vs. Production:
// - **SHA-1**: Git uses SHA-1 (and is transitioning to SHA-256). We just use our SHA-256 for ease.
// - **Deflate Compression**: Real Git objects are zlib compressed on disk. We just generate raw bytes.
// - **Packfiles**: Git packs many objects into `.pack` files with delta compression to save space. We only implement loose objects.
// - **Parsing**: We haven't implemented `GitObject::parse(bytes)`, which is required to read existing repositories.
//
// Benchmarking Note:
// To benchmark serialization and hashing, use `criterion`. Generate synthetic trees and
// blobs of various sizes (1KB to 100MB), measure `GitObject::serialize()` speed, and
// wrap the outputs in `std::hint::black_box()` to prevent compiler optimization.
//
// Next Steps:
// 1. Implement `GitObject::parse` for each object type.
// 2. Add zlib compression/decompression.
// 3. Build a simple `git cat-file -p` command-line utility.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blob_serialization() {
        let blob = GitObject::Blob(b"hello world".to_vec());
        let data = blob.serialize();

        let expected_header = b"blob 11\0";
        assert_eq!(&data[0..expected_header.len()], expected_header);
        assert_eq!(&data[expected_header.len()..], b"hello world");
    }

    #[test]
    fn test_tree_serialization() {
        let mut entries = BTreeMap::new();
        let oid =
            ObjectId::from_hex("1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef")
                .unwrap();
        entries.insert(
            "hello.txt".to_string(),
            TreeEntry {
                mode: "100644".to_string(),
                oid: oid.clone(),
            },
        );

        let tree = GitObject::Tree(entries);
        let data = tree.serialize();

        // Check header
        let expected_header = b"tree 49\0"; // 6(mode) + 1(space) + 9(hello.txt) + 1(nul) + 32(hash) = 49.
        assert_eq!(&data[0..expected_header.len()], expected_header);

        // Check content
        let content = &data[expected_header.len()..];
        assert_eq!(&content[0..17], b"100644 hello.txt\0");
        assert_eq!(&content[17..], oid.0.as_slice());
    }

    #[test]
    fn test_commit_serialization() {
        let tree_oid =
            ObjectId::from_hex("1111111111111111111111111111111111111111111111111111111111111111")
                .unwrap();
        let parent_oid =
            ObjectId::from_hex("2222222222222222222222222222222222222222222222222222222222222222")
                .unwrap();

        let commit = GitObject::Commit {
            tree: tree_oid,
            parents: vec![parent_oid],
            author: "Author Name <author@example.com> 1600000000 +0000".to_string(),
            committer: "Committer Name <committer@example.com> 1600000000 +0000".to_string(),
            message: "Initial commit".to_string(),
        };

        let data = commit.serialize();

        // Find the null byte separator
        let null_pos = data.iter().position(|&b| b == 0).unwrap();
        let header = String::from_utf8(data[0..null_pos].to_vec()).unwrap();
        assert!(header.starts_with("commit "));

        let content = String::from_utf8(data[null_pos + 1..].to_vec()).unwrap();
        assert!(
            content.contains(
                "tree 1111111111111111111111111111111111111111111111111111111111111111\n"
            )
        );
        assert!(
            content.contains(
                "parent 2222222222222222222222222222222222222222222222222222222222222222\n"
            )
        );
        assert!(content.contains("author Author Name <author@example.com>"));
        assert!(content.contains("\n\nInitial commit\n"));
    }

    #[test]
    fn test_hash_consistency() {
        let blob1 = GitObject::Blob(b"hello world".to_vec());
        let blob2 = GitObject::Blob(b"hello world".to_vec());
        let blob3 = GitObject::Blob(b"hello Rust".to_vec());

        assert_eq!(blob1.id(), blob2.id());
        assert_ne!(blob1.id(), blob3.id());
    }
}
