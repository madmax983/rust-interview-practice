//! # Git Object Store Implementation
//!
//! Implements a minimal Git Object Store from scratch.
//! It handles content-addressable storage, object serialization (Blob, Tree, Commit), and SHA hashing.
//!
//! **Replaces Crates:** `git2`, `gix`
//!
//! **Real-world Usage:**
//! - Core component of the Git version control system.
//! - Used in deployment pipelines, code hosting platforms (GitHub, GitLab), and continuous integration.
//! - Foundational to content-addressable file systems (IPFS).
//!
//! **Why build it yourself?**
//! Building a Git object store teaches you about content-addressable storage, where the ID of a file
//! is derived entirely from its contents. You'll learn how Git uses zlib compression, how it
//! constructs the header for blobs, trees, and commits, and how to compute SHA-1 hashes manually.
//! This demystifies the magic of how Git tracks changes efficiently without storing full copies of every file on every commit.

use std::collections::BTreeMap;
use sha1::{Sha1, Digest};

// RUST INSIGHT: We use `sha1::Sha1` here for computing hashes. In a fully from-scratch implementation,
// we could write our own SHA-1 implementation, but that's a cryptographic exercise rather than a systems one.
// We focus on the structure and serialization of the Git objects.

// =========================================================================================
// Architecture
// =========================================================================================
//
// Flow:
//
//      File Content
//          │
//          ▼
//   [Blob Serialization]  ──►  [Compute SHA-1 Hash]
//          │                           │
//          ▼                           ▼
//   [Zlib Compression] ◄──────── [Hash as Object ID]
//          │
//          ▼
//   [Write to `.git/objects/XX/XXXXX`]
//
//
// Invariants:
// 1. The SHA-1 hash is computed over the uncompressed header AND content.
// 2. The header format is `<type> <content_size>\0`.
// 3. Tree objects store entries sorted by name to ensure consistent hashing.
// 4. Commits point to a single tree and zero or more parent commits.
//
// Complexity:
// ┌────────────────┬────────────────┬────────────────┐
// │ Operation      │ Time           │ Space          │
// ├────────────────┼────────────────┼────────────────┤
// │ Compute Hash   │ O(Size)        │ O(Size)        │
// │ Serialize Obj  │ O(Size)        │ O(Size)        │
// │ Parse Tree     │ O(Entries * L) │ O(Entries * L) │
// └────────────────┴────────────────┴────────────────┘
// Where L is the average length of the filename.
//
// Design Decisions:
// - **In-Memory Objects**: We represent Blob, Tree, and Commit as Rust structs.
//   - *Tradeoff*: Loads entire objects into memory.
//   - *Alternative*: Streaming parsing and serialization for very large objects.
// - **Zlib Compression**: We skip zlib compression in this educational implementation to focus on
//   the structural formatting and hashing. A real Git store *must* zlib compress objects before writing.

/// A Git Object ID (SHA-1 hash, 20 bytes).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectId(pub [u8; 20]);

impl std::fmt::Debug for ObjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for byte in &self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl std::fmt::Display for ObjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for byte in &self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl ObjectId {
    /// Creates an `ObjectId` from a 40-character hex string.
    pub fn from_hex(hex: &str) -> Option<Self> {
        if hex.len() != 40 {
            return None;
        }
        let mut bytes = [0u8; 20];
        for i in 0..20 {
            let byte_str = &hex[i * 2..i * 2 + 2];
            bytes[i] = u8::from_str_radix(byte_str, 16).ok()?;
        }
        Some(Self(bytes))
    }
}

/// A trait defining the behavior of a serializable Git object.
/// RUST INSIGHT: Defining this trait allows for a swappable architecture, letting us seamlessly
/// mock objects, implement new object types (like Tags), or swap out the serialization backend.
pub trait GitSerializable {
    /// Serializes the object into its raw Git byte format (header + content).
    fn serialize(&self) -> Vec<u8>;

    /// Returns the type string used in the Git header.
    fn kind(&self) -> &'static str;

    /// Computes the SHA-1 hash of the serialized object.
    fn compute_hash(&self) -> ObjectId {
        let serialized = self.serialize();
        let mut hasher = Sha1::new();
        hasher.update(&serialized);
        let result = hasher.finalize();
        let mut hash = [0u8; 20];
        hash.copy_from_slice(&result);
        ObjectId(hash)
    }
}

/// Represents a Git Object (Blob, Tree, Commit).
pub enum GitObject {
    Blob(Blob),
    Tree(Tree),
    Commit(Commit),
}

impl GitSerializable for GitObject {
    fn serialize(&self) -> Vec<u8> {
        match self {
            Self::Blob(blob) => blob.serialize(),
            Self::Tree(tree) => tree.serialize(),
            Self::Commit(commit) => commit.serialize(),
        }
    }

    fn kind(&self) -> &'static str {
        match self {
            Self::Blob(_) => "blob",
            Self::Tree(_) => "tree",
            Self::Commit(_) => "commit",
        }
    }
}

/// A Blob object, representing file contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Blob {
    pub content: Vec<u8>,
}

impl Blob {
    pub fn new(content: Vec<u8>) -> Self {
        Self { content }
    }

    fn serialize(&self) -> Vec<u8> {
        let header = format!("blob {}\0", self.content.len());
        let mut buf = Vec::with_capacity(header.len() + self.content.len());
        buf.extend_from_slice(header.as_bytes());
        buf.extend_from_slice(&self.content);
        buf
    }
}

/// Represents an entry in a Git Tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeEntry {
    /// Mode (e.g., "100644" for regular file, "40000" for directory).
    pub mode: String,
    /// Name of the file or directory.
    pub name: String,
    /// SHA-1 hash of the referenced object.
    pub oid: ObjectId,
}

/// A Tree object, representing a directory structure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tree {
    /// Entries in the tree. Git requires these to be sorted by name.
    /// We use a BTreeMap to maintain this sorted order naturally.
    pub entries: BTreeMap<String, TreeEntry>,
}

impl Tree {
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    pub fn add_entry(&mut self, mode: &str, name: &str, oid: ObjectId) {
        self.entries.insert(
            name.to_string(),
            TreeEntry {
                mode: mode.to_string(),
                name: name.to_string(),
                oid,
            },
        );
    }

    fn serialize(&self) -> Vec<u8> {
        let mut content = Vec::new();
        for (_, entry) in &self.entries {
            // Format: `<mode> <name>\0<20_byte_sha1>`
            let entry_header = format!("{} {}\0", entry.mode, entry.name);
            content.extend_from_slice(entry_header.as_bytes());
            content.extend_from_slice(&entry.oid.0);
        }

        let header = format!("tree {}\0", content.len());
        let mut buf = Vec::with_capacity(header.len() + content.len());
        buf.extend_from_slice(header.as_bytes());
        buf.extend_from_slice(&content);
        buf
    }
}

/// A Commit object, representing a snapshot in history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commit {
    pub tree: ObjectId,
    pub parents: Vec<ObjectId>,
    pub author: String,
    pub committer: String,
    pub message: String,
}

impl Commit {
    fn serialize(&self) -> Vec<u8> {
        let mut content = String::new();
        content.push_str(&format!("tree {}\n", self.tree));
        for parent in &self.parents {
            content.push_str(&format!("parent {}\n", parent));
        }
        content.push_str(&format!("author {}\n", self.author));
        content.push_str(&format!("committer {}\n", self.committer));
        content.push_str("\n");
        content.push_str(&self.message);

        let header = format!("commit {}\0", content.len());
        let mut buf = Vec::with_capacity(header.len() + content.len());
        buf.extend_from_slice(header.as_bytes());
        buf.extend_from_slice(content.as_bytes());
        buf
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `git2`: Uses libgit2 (C) under the hood. Extremely comprehensive but has C dependencies.
// - `gix` (Gitoxide): A pure Rust implementation of Git. Highly optimized, concurrent, and robust.
//
// Missing vs. Production:
// - **Zlib Compression**: Production Git zlib-compresses all objects. We omitted it for clarity.
// - **Packfiles**: Real Git uses packfiles (delta compression) to save space. We only implement loose objects.
// - **Parsing**: We haven't implemented parsers to read these objects back from raw bytes.
//
// Next Steps:
// 1. Add `flate2` and implement zlib compression/decompression.
// 2. Implement parsing logic to read objects from disk.
// 3. Add packfile support.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blob_serialization() {
        let blob = Blob::new(b"hello world\n".to_vec());
        let obj = GitObject::Blob(blob);
        let serialized = obj.serialize();

        let expected_header = b"blob 12\0";
        assert!(serialized.starts_with(expected_header));
        assert_eq!(&serialized[expected_header.len()..], b"hello world\n");
    }

    #[test]
    fn test_blob_hash() {
        // Echoing "hello world" and hashing in Git:
        // $ echo "hello world" | git hash-object --stdin
        // 3b18e512dba79e4c8300dd08aeb37f8e728b8dad
        let blob = Blob::new(b"hello world\n".to_vec());
        let obj = GitObject::Blob(blob);
        let hash = obj.compute_hash();
        assert_eq!(hash.to_string(), "3b18e512dba79e4c8300dd08aeb37f8e728b8dad");
    }

    #[test]
    fn test_tree_serialization_and_hash() {
        let mut tree = Tree::new();
        // Add a dummy entry
        let oid = ObjectId::from_hex("3b18e512dba79e4c8300dd08aeb37f8e728b8dad").unwrap();
        tree.add_entry("100644", "test.txt", oid);

        let obj = GitObject::Tree(tree);
        let serialized = obj.serialize();

        let header = b"tree 36\0"; // "100644 test.txt\0" (16) + 20 bytes hash = 36
        assert!(serialized.starts_with(header));
    }

    #[test]
    fn test_commit_serialization() {
        let tree_oid = ObjectId::from_hex("0000000000000000000000000000000000000000").unwrap();
        let commit = Commit {
            tree: tree_oid,
            parents: vec![],
            author: "Author <author@example.com> 1620000000 +0000".to_string(),
            committer: "Committer <committer@example.com> 1620000000 +0000".to_string(),
            message: "Initial commit\n".to_string(),
        };

        let obj = GitObject::Commit(commit);
        let serialized = obj.serialize();

        let s = String::from_utf8_lossy(&serialized);
        assert!(s.contains("tree 0000000000000000000000000000000000000000\n"));
        assert!(s.contains("author Author <author@example.com> 1620000000 +0000\n"));
        assert!(s.contains("\n\nInitial commit\n"));
    }
}
