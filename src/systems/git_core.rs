//! # Git Object Store
//!
//! Implements a minimal Git content-addressable object store from scratch.
//!
//! **Replaces Crates:** `git2`, `gix`
//!
//! **Real-world Usage:**
//! - Underpins version control systems (Git itself).
//! - Content-addressable storage systems (IPFS).
//! - Docker image layer storage.
//!
//! **Why build it yourself?**
//! Building a Git object store teaches you how Git uses a simple key-value store
//! based on SHA-1 hashes to build a directed acyclic graph (DAG) of history.
//! It forces you to understand the exact serialization formats of blobs, trees,
//! and commits, and how content dictates identity.

use std::collections::HashMap;
use std::fmt::Write;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure:
//
//     [Commit Object] ───► [Tree Object] ───► [Blob Object]
//     (Metadata)           (Directory)        (File Data)
//          │                     │
//          ▼                     ▼
//     [Parent Commit]      [Tree Object] ───► [Blob Object]
//
// Git objects are stored as bytes, keyed by their SHA-1 hash.
//
// Format: `<type> <size>\0<content>`
//
// Blob:   File content
// Tree:   Directory listing (mode, name, sha1)
// Commit: Metadata (tree, parent, author, message)
//
// Invariants:
// 1. Two identical files yield the same SHA-1 and are stored exactly once.
// 2. Modifying a file changes its blob hash, which bubbles up to change the tree and commit hashes.
// 3. Object immutability: once written, an object's content is never modified.
//
// Complexity:
// ┌─────────────┬─────────────┬─────────────┐
// │ Operation   │ Time        │ Space       │
// ├─────────────┼─────────────┼─────────────┤
// │ write_object│ O(N)        │ O(N)        │
// │ read_object │ O(1)        │ O(N)        │
// └─────────────┴─────────────┴─────────────┘
// N is the length of the data to be hashed and stored.
//
// Design Decisions:
// - **SHA-1 Implementation**: Custom, minimal SHA-1 to demonstrate the core hashing logic without dependencies.
// - **Storage Backend**: In-memory `HashMap` keyed by hash.
//   - *Alternative*: File system backend using `.git/objects/` structure, which provides persistence.
// - **Compression**: Uncompressed for clarity.
//   - *Tradeoff*: Uses more memory/storage, but allows easier inspection of raw object contents.

/// Custom SHA-1 Hash from scratch.
#[must_use]
pub fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h0 = 0x6745_2301_u32;
    let mut h1 = 0xEFCD_AB89_u32;
    let mut h2 = 0x98BA_DCFE_u32;
    let mut h3 = 0x1032_5476_u32;
    let mut h4 = 0xC3D2_E1F0_u32;

    let mut message = data.to_vec();
    let ml = (message.len() as u64) * 8;
    message.push(0x80);
    while (message.len() % 64) != 56 {
        message.push(0);
    }
    message.extend_from_slice(&ml.to_be_bytes());

    for chunk in message.chunks(64) {
        let mut w = [0_u32; 80];
        // RUST INSIGHT: Use of chunks allows safe slicing without index checks inside the loop.
        for (i, block) in chunk.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes([block[0], block[1], block[2], block[3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }

        let mut a = h0;
        let mut b = h1;
        let mut c = h2;
        let mut d = h3;
        let mut e = h4;

        for i in 0..80 {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5A82_7999),
                20..=39 => (b ^ c ^ d, 0x6ED9_EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1B_BCDC),
                _ => (b ^ c ^ d, 0xCA62_C1D6),
            };

            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(w[i]);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }

        h0 = h0.wrapping_add(a);
        h1 = h1.wrapping_add(b);
        h2 = h2.wrapping_add(c);
        h3 = h3.wrapping_add(d);
        h4 = h4.wrapping_add(e);
    }

    let mut out = [0_u8; 20];
    out[0..4].copy_from_slice(&h0.to_be_bytes());
    out[4..8].copy_from_slice(&h1.to_be_bytes());
    out[8..12].copy_from_slice(&h2.to_be_bytes());
    out[12..16].copy_from_slice(&h3.to_be_bytes());
    out[16..20].copy_from_slice(&h4.to_be_bytes());
    out
}

/// Formats a 20-byte SHA-1 hash as a hex string.
#[must_use]
pub fn hash_to_hex(hash: &[u8; 20]) -> String {
    let mut s = String::with_capacity(40);
    for &b in hash {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// Represents a Git Object Type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitObject {
    Blob(Vec<u8>),
    Tree(Vec<TreeEntry>),
    Commit {
        tree_hash: String,
        parent_hash: Option<String>,
        author: String,
        committer: String,
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeEntry {
    pub mode: String,
    pub name: String,
    pub hash: [u8; 20],
}

impl GitObject {
    /// Serializes the Git object into its raw byte representation.
    #[must_use]
    pub fn serialize(&self) -> Vec<u8> {
        match self {
            Self::Blob(data) => {
                let mut buf = format!("blob {}\0", data.len()).into_bytes();
                buf.extend_from_slice(data);
                buf
            }
            Self::Tree(entries) => {
                let mut content = Vec::new();
                for entry in entries {
                    // Format: `<mode> <name>\0<20_byte_hash>`
                    content.extend_from_slice(format!("{} {}\0", entry.mode, entry.name).as_bytes());
                    content.extend_from_slice(&entry.hash);
                }
                let mut buf = format!("tree {}\0", content.len()).into_bytes();
                buf.extend_from_slice(&content);
                buf
            }
            Self::Commit {
                tree_hash,
                parent_hash,
                author,
                committer,
                message,
            } => {
                let mut content = String::new();
                content.push_str(&format!("tree {tree_hash}\n"));
                if let Some(parent) = parent_hash {
                    content.push_str(&format!("parent {parent}\n"));
                }
                content.push_str(&format!("author {author}\n"));
                content.push_str(&format!("committer {committer}\n\n"));
                content.push_str(message);

                let mut buf = format!("commit {}\0", content.len()).into_bytes();
                buf.extend_from_slice(content.as_bytes());
                buf
            }
        }
    }
}

/// Trait representing a content-addressable object store.
///
/// Defining this trait allows swapping out the storage backend (e.g., from an in-memory
/// mock store for testing to a file-system backed store for production).
pub trait ObjectStore {
    /// Stores a Git object and returns its SHA-1 hash.
    fn write_object(&mut self, obj: &GitObject) -> [u8; 20];

    /// Retrieves a raw object by its SHA-1 hash.
    fn read_object(&self, hash: &[u8; 20]) -> Option<&Vec<u8>>;
}

/// A simplified Git Object Store.
#[derive(Default)]
pub struct GitStore {
    // PRODUCTION NOTE: In production, objects are compressed via zlib and stored on disk in `.git/objects/`.
    // We store them uncompressed in memory for educational clarity.
    objects: HashMap<[u8; 20], Vec<u8>>,
}

impl GitStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl ObjectStore for GitStore {
    fn write_object(&mut self, obj: &GitObject) -> [u8; 20] {
        let data = obj.serialize();
        let hash = sha1(&data);
        // GOTCHA: Git relies on hash collisions being cryptographically unfeasible.
        // If two different files produce the same hash, Git will silently corrupt the newer file
        // by keeping the old blob (SHA-1 collision vulnerability).
        self.objects.insert(hash, data);
        hash
    }

    fn read_object(&self, hash: &[u8; 20]) -> Option<&Vec<u8>> {
        self.objects.get(hash)
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `git2`: Native bindings to libgit2, robust and production-ready, handles networking, refs, merges, and packs.
// - `gix`: Pure Rust Git implementation. More modular and performant than libgit2 in some areas.
//
// Missing vs. Production:
// - Zlib compression (Git compresses all objects).
// - Packfiles (Git packs multiple objects into a single delta-compressed file to save space).
// - Reference parsing (e.g., resolving `HEAD` or branches).
// - Deserialization (parsing raw bytes back into `GitObject`).
//
// Next Steps:
// 1. Add zlib compression using a deflate implementation.
// 2. Implement parsing raw bytes back into `GitObject`.
// 3. Write a tree traversal algorithm to recursively recreate a working directory from a commit hash.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha1_known_value() {
        let hash = sha1(b"hello world");
        assert_eq!(hash_to_hex(&hash), "2aae6c35c94fcfb415dbe95f408b9ce91ee846ed");
    }

    #[test]
    fn test_blob_serialization() {
        let blob = GitObject::Blob(b"hello".to_vec());
        let mut store = GitStore::new();
        let hash = store.write_object(&blob);

        let hex = hash_to_hex(&hash);
        // "blob 5\0hello"
        // echo -n -e "blob 5\0hello" | sha1sum
        // b6fc4c620b67d95f953a5c1c1230aaab5db5a1b0
        assert_eq!(hex, "b6fc4c620b67d95f953a5c1c1230aaab5db5a1b0");

        let raw = store.read_object(&hash).unwrap();
        assert_eq!(raw, b"blob 5\0hello");
    }

    #[test]
    fn test_tree_serialization() {
        let blob_hash = sha1(b"blob 5\0hello");
        let tree = GitObject::Tree(vec![TreeEntry {
            mode: "100644".to_string(),
            name: "hello.txt".to_string(),
            hash: blob_hash,
        }]);

        let mut store = GitStore::new();
        let hash = store.write_object(&tree);
        let raw = store.read_object(&hash).unwrap();
        assert!(raw.starts_with(b"tree "));
    }

    #[test]
    fn test_commit_serialization() {
        let commit = GitObject::Commit {
            tree_hash: "a".repeat(40),
            parent_hash: None,
            author: "Alice <alice@example.com> 1600000000 +0000".to_string(),
            committer: "Bob <bob@example.com> 1600000000 +0000".to_string(),
            message: "Initial commit".to_string(),
        };

        let mut store = GitStore::new();
        let hash = store.write_object(&commit);
        let raw = store.read_object(&hash).unwrap();

        let content = String::from_utf8(raw.clone()).unwrap();
        assert!(content.contains("tree aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
        assert!(!content.contains("parent"));
        assert!(content.contains("author Alice"));
        assert!(content.contains("\n\nInitial commit"));
    }
}
