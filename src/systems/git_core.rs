//! # Git Object Store
//!
//! Implements a minimal Git core mimicking object storage.
//!
//! **Replaces Crates:** `git2`, `gix`
//!
//! **Real-world Usage:**
//! - Version control systems (Git)
//! - Content-addressable storage systems
//! - Decentralized file systems (IPFS)
//!
//! **Why build it yourself?**
//! Implementing a Git object store demystifies how Git tracks content, trees, and commits.
//! It teaches you about content-addressable storage, serialization of Git objects,
//! and how SHA-1 hashing is used to identify data uniquely.
//!
//! # Architecture
//!
//! **Flow:**
//! ```text
//! [ Content ] -> [ Git Object ] -> [ Serialize ] -> [ SHA-1 Hash ] -> [ Storage (e.g., .git/objects) ]
//! ```
//!
//! **Invariants:**
//! 1. All objects are hashed based on their serialized format: `<type> <size>\0<content>`.
//! 2. The SHA-1 hash is unique to the exact content of the object.
//! 3. Trees contain sorted entries for stable hashing.
//!
//! **Complexity:**
//! ┌───────────────┬────────────┬─────────────┐
//! │ Operation     │ Time       │ Space       │
//! ├───────────────┼────────────┼─────────────┤
//! │ Hash/Serialize│ O(N)       │ O(N)        │
//! │ Store/Load    │ O(N)       │ O(N)        │
//! └───────────────┴────────────┴─────────────┘
//! * N = length of the content.
//!
//! **Design Decisions & Tradeoffs:**
//! - **Storage:** We use an in-memory `HashMap` to simulate `.git/objects`.
//! - **Hashing:** We implement a basic SHA-1 hasher from scratch for educational purposes.
//! - **Compression:** Real Git uses zlib compression (deflate). We omit compression for clarity, but it would wrap the serialized object.
//!
//! **Comparison to canonical crates:**
//! - Production crates like `gix` use memory-mapped files and delta compression for performance.

use std::collections::HashMap;
use std::fmt::Write;

// =========================================================================================
// SHA-1 Implementation (Minimal)
// =========================================================================================

/// A minimal SHA-1 implementation.
pub struct Sha1 {
    h: [u32; 5],
}

impl Default for Sha1 {
    fn default() -> Self {
        Self::new()
    }
}

impl Sha1 {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            h: [
                0x6745_2301,
                0xEFCD_AB89,
                0x98BA_DCFE,
                0x1032_5476,
                0xC3D2_E1F0,
            ],
        }
    }

    /// Hashes the given data and returns the 40-character hex string.
    #[must_use]
    pub fn hash(data: &[u8]) -> String {
        let mut sha1 = Self::new();
        sha1.update(data);
        sha1.finalize()
    }

    fn update(&mut self, data: &[u8]) {
        let mut padded = Vec::with_capacity(data.len() + 64);
        padded.extend_from_slice(data);
        padded.push(0x80);
        let bit_len = (data.len() as u64) * 8;
        while (padded.len() % 64) != 56 {
            padded.push(0);
        }
        padded.extend_from_slice(&bit_len.to_be_bytes());

        for chunk in padded.chunks_exact(64) {
            let mut w = [0_u32; 80];
            for (i, block) in chunk.chunks_exact(4).enumerate() {
                w[i] = u32::from_be_bytes([block[0], block[1], block[2], block[3]]);
            }
            for i in 16..80 {
                w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
            }

            let mut a = self.h[0];
            let mut b = self.h[1];
            let mut c = self.h[2];
            let mut d = self.h[3];
            let mut e = self.h[4];

            for (i, &w_i) in w.iter().enumerate() {
                let (f, k) = match i {
                    0..=19 => ((b & c) | (!b & d), 0x5A82_7999),
                    20..=39 => (b ^ c ^ d, 0x6ED9_EBA1),
                    40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1B_BCDC),
                    _ => (b ^ c ^ d, 0xCA62_C1D6),
                };
                let temp = a
                    .rotate_left(5)
                    .wrapping_add(f)
                    .wrapping_add(e)
                    .wrapping_add(k)
                    .wrapping_add(w_i);
                e = d;
                d = c;
                c = b.rotate_left(30);
                b = a;
                a = temp;
            }

            self.h[0] = self.h[0].wrapping_add(a);
            self.h[1] = self.h[1].wrapping_add(b);
            self.h[2] = self.h[2].wrapping_add(c);
            self.h[3] = self.h[3].wrapping_add(d);
            self.h[4] = self.h[4].wrapping_add(e);
        }
    }

    fn finalize(self) -> String {
        let mut out = String::with_capacity(40);
        for val in self.h {
            let _ = write!(out, "{val:08x}");
        }
        out
    }
}

// =========================================================================================
// Git Object Types
// =========================================================================================

/// A Git object representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitObject {
    /// A Blob object containing raw file data.
    Blob(Vec<u8>),
    /// A Tree object representing a directory.
    Tree(Vec<TreeEntry>),
    /// A Commit object linking a tree to a history and author.
    Commit {
        tree: String,
        parent: Option<String>,
        author: String,
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeEntry {
    pub mode: String,
    pub name: String,
    pub hash: String,
}

impl GitObject {
    /// Serializes the Git object into its raw byte format.
    ///
    /// Git objects are prefixed with their type and size, followed by a null byte.
    #[must_use]
    pub fn serialize(&self) -> Vec<u8> {
        let (ty, content): (&[u8], Vec<u8>) = match self {
            Self::Blob(data) => {
                let ty: &[u8] = b"blob";
                (ty, data.clone())
            }
            Self::Tree(entries) => {
                let ty: &[u8] = b"tree";
                let mut data = Vec::new();
                for entry in entries {
                    data.extend_from_slice(entry.mode.as_bytes());
                    data.push(b' ');
                    data.extend_from_slice(entry.name.as_bytes());
                    data.push(0);
                    // Decode hex hash to binary
                    if entry.hash.len() == 40 {
                        for i in (0..40).step_by(2) {
                            if let Ok(byte) = u8::from_str_radix(&entry.hash[i..i + 2], 16) {
                                data.push(byte);
                            }
                        }
                    }
                }
                (ty, data)
            }
            Self::Commit {
                tree,
                parent,
                author,
                message,
            } => {
                let ty: &[u8] = b"commit";
                let mut data = String::new();
                data.push_str(&format!("tree {tree}\n"));
                if let Some(p) = parent {
                    data.push_str(&format!("parent {p}\n"));
                }
                data.push_str(&format!("author {author}\n"));
                data.push_str(&format!("committer {author}\n\n"));
                data.push_str(message);
                (ty, data.into_bytes())
            }
        };

        let mut out = Vec::new();
        out.extend_from_slice(ty);
        out.push(b' ');
        out.extend_from_slice(content.len().to_string().as_bytes());
        out.push(0);
        out.extend_from_slice(&content);
        out
    }

    /// Deserializes a Git object from its raw byte format.
    #[must_use]
    pub fn deserialize(data: &[u8]) -> Option<Self> {
        let null_pos = data.iter().position(|&b| b == 0)?;
        let header = std::str::from_utf8(&data[..null_pos]).ok()?;
        let mut parts = header.split(' ');
        let ty = parts.next()?;
        let _size = parts.next()?;

        let content = &data[null_pos + 1..];

        match ty {
            "blob" => Some(Self::Blob(content.to_vec())),
            "tree" => {
                let mut entries = Vec::new();
                let mut i = 0;
                while i < content.len() {
                    let space = i + content[i..].iter().position(|&b| b == b' ')?;
                    let mode = std::str::from_utf8(&content[i..space]).ok()?.to_string();
                    let null = space + 1 + content[space + 1..].iter().position(|&b| b == 0)?;
                    let name = std::str::from_utf8(&content[space + 1..null])
                        .ok()?
                        .to_string();
                    let hash_bytes = &content[null + 1..null + 21];
                    let mut hash = String::with_capacity(40);
                    for b in hash_bytes {
                        let _ = write!(hash, "{b:02x}");
                    }
                    entries.push(TreeEntry { mode, name, hash });
                    i = null + 21;
                }
                Some(Self::Tree(entries))
            }
            "commit" => {
                let content_str = std::str::from_utf8(content).ok()?;
                let mut tree = String::new();
                let mut parent = None;
                let mut author = String::new();
                let mut message = String::new();

                let mut lines = content_str.lines();
                while let Some(line) = lines.next() {
                    if line.is_empty() {
                        message = lines.collect::<Vec<_>>().join("\n");
                        break;
                    }
                    if let Some(t) = line.strip_prefix("tree ") {
                        tree = t.to_string();
                    } else if let Some(p) = line.strip_prefix("parent ") {
                        parent = Some(p.to_string());
                    } else if let Some(a) = line.strip_prefix("author ") {
                        author = a.to_string();
                    }
                }
                Some(Self::Commit {
                    tree,
                    parent,
                    author,
                    message,
                })
            }
            _ => None,
        }
    }
}

// =========================================================================================
// Git Object Store
// =========================================================================================

/// Trait defining a generic content-addressable storage backend.
/// Real Git would have an implementation backed by `.git/objects` and packfiles.
pub trait ObjectStore {
    /// Stores a Git object and returns its SHA-1 hash.
    fn store(&mut self, obj: &GitObject) -> String;

    /// Retrieves and parses a Git object by its SHA-1 hash.
    fn load(&self, hash: &str) -> Option<GitObject>;
}

/// An in-memory Git Object Store mimicking `.git/objects`.
#[derive(Default)]
pub struct GitObjectStore {
    // RUST INSIGHT: We use `HashMap` for simplicity. A production Git store would use
    // memory-mapped files and a more complex indexing structure for packfiles.
    objects: HashMap<String, Vec<u8>>,
}

impl GitObjectStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl ObjectStore for GitObjectStore {
    fn store(&mut self, obj: &GitObject) -> String {
        let serialized = obj.serialize();
        // RUST INSIGHT: We compute the SHA-1 hash natively.
        // GOTCHA: We hash the serialized representation, not just the content.
        let hash = Sha1::hash(&serialized);
        self.objects.insert(hash.clone(), serialized);
        hash
    }

    fn load(&self, hash: &str) -> Option<GitObject> {
        // PRODUCTION NOTE: Production `git` avoids loading the full file into memory
        // if possible, instead streaming the blobs.
        let serialized = self.objects.get(hash)?;
        GitObject::deserialize(serialized)
    }
}

// =========================================================================================
// Tests
// =========================================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha1_hashing() {
        assert_eq!(
            Sha1::hash(b"hello world"),
            "2aae6c35c94fcfb415dbe95f408b9ce91ee846ed"
        );
        assert_eq!(Sha1::hash(b""), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
    }

    #[test]
    fn test_blob_serialization() {
        let blob = GitObject::Blob(b"hello world".to_vec());
        let mut store = GitObjectStore::new();
        let hash = store.store(&blob);

        // Expected hash for blob "hello world" in Git
        // "blob 11\0hello world"
        assert_eq!(hash, "95d09f2b10159347eece71399a7e2e907ea3df4f");

        let loaded = store.load(&hash).unwrap();
        assert_eq!(blob, loaded);
    }

    #[test]
    fn test_tree_serialization() {
        let tree = GitObject::Tree(vec![TreeEntry {
            mode: "100644".to_string(),
            name: "hello.txt".to_string(),
            hash: "95d09f2b10159347eece71399a7e2e907ea3df4f".to_string(),
        }]);
        let mut store = GitObjectStore::new();
        let hash = store.store(&tree);

        let loaded = store.load(&hash).unwrap();
        assert_eq!(tree, loaded);
    }

    #[test]
    fn test_commit_serialization() {
        let commit = GitObject::Commit {
            tree: "tree_hash".to_string(),
            parent: None,
            author: "Author Name <author@example.com> 1234567890 +0000".to_string(),
            message: "Initial commit".to_string(),
        };
        let mut store = GitObjectStore::new();
        let hash = store.store(&commit);

        let loaded = store.load(&hash).unwrap();
        assert_eq!(commit, loaded);
    }

    #[test]
    fn test_sha1_benchmark() {
        use std::hint::black_box;
        use std::time::Instant;

        let data = vec![0u8; 1024 * 1024]; // 1MB buffer
        let start = Instant::now();
        let hash = Sha1::hash(black_box(&data));
        let duration = start.elapsed();

        // This validates our basic Sha1 speed (educational benchmark).
        assert_eq!(hash, "3b71f43ff30f4b15b5cd85dd9e95ebc7e84eb5a3");
        println!("Hashed 1MB in {:?}", duration);
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// **Comparison to canonical crates:**
// - The `git2` and `gix` crates offer fully spec-compliant object resolution, including
//   Delta compression handling within Packfiles, whereas this only handles loose objects.
//
// **What's missing vs. production:**
// - **Zlib Compression**: We store plain text, but Git zlib-compresses all objects.
// - **Packfiles & Deltas**: Git stores deltas (diffs) to save massive space; we only have raw snapshots.
// - **Streaming I/O**: Real implementations memory-map files and avoid allocating full blobs into memory when hashing.
//
// **Suggested next steps:**
// - Implement Zlib Deflate compression/decompression for realistic `.git/objects` writing.
// - Add parsing for `.pack` and `.idx` files.
