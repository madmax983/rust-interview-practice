//! # Git Object Store
//!
//! Implements a Git-compatible content-addressable object store from scratch.
//! This includes object serialization (Blob, Tree, Commit) and a raw SHA-1 implementation.
//!
//! **Replaces Crates:** `git2`, `gix`
//!
//! **Real-world Usage:**
//! - Version control systems (Git, Mercurial)
//! - Content distribution networks and build systems (Bazel, Nix)
//!
//! **Why build it yourself?**
//! Understanding Git's object model demystifies how version control works.
//! You learn about content-addressable storage, Merkle trees, and how complex
//! history is built from simple immutable objects (Blobs, Trees, Commits).
//! You also get hands-on experience with hashing algorithms (SHA-1).
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────┐     ┌──────────────┐     ┌──────────────┐
//! │    Commit    │────▶│     Tree     │────▶│     Blob     │
//! │ - tree       │     │ - mode       │     │ - data       │
//! │ - parents    │     │ - name       │     └──────────────┘
//! │ - author     │     │ - hash       │
//! │ - message    │     └──────────────┘
//! └──────────────┘
//! ```
//!
//! **Invariants:**
//! 1. All objects are serialized with a header `<type> <size>\0` followed by content.
//! 2. SHA-1 hashes are 160-bit (20 byte) identifiers computed over the exact serialized object.
//! 3. Tree objects store hashes in raw 20-byte binary, whereas Commits store them as 40-character hex strings.
//!
//! **Complexity:**
//! - Serialize: O(N) where N is the content length.
//! - SHA-1 Hash: O(N) where N is the serialized length.
//!
//! **Tradeoffs:**
//! - We store the whole object in memory (`Vec<u8>`). A production implementation might stream large blobs.
//!
//! # Comparison to Canonical Crates
//! - `git2`: Uses `libgit2` (C bindings), extremely fast and compliant.
//! - `gix`: Pure Rust Git implementation, highly optimized and safe.
//!
//! **Missing vs Production:**
//! - Deflate compression (zlib) is omitted here for educational clarity (normally objects are zlib compressed on disk).
//! - Streaming APIs for huge files.
//! - Packfile support (Git packs objects together to save space).
//!
//! **Suggested next steps / extensions:**
//! - Implement `flate2` for zlib compression of objects.
//! - Support reading/writing Packfiles (`.pack` and `.idx`).
//!
//! **Benchmarking Note:** Use `criterion` to benchmark the SHA-1 hasher against `sha1` crate and measure the throughput of `GitObject::serialize`.

use std::fmt::Write;

/// A simple SHA-1 hasher implemented from scratch for educational purposes.
///
#[allow(clippy::many_single_char_names)]
/// # Panics
/// Panics if the internal `write!` macro fails when converting hex string,
/// which in practice should never happen.
#[must_use]
pub fn sha1(data: &[u8]) -> [u8; 20] {
    // RUST INSIGHT:
    // Rust's fixed-size arrays `[T; N]` and wrapping arithmetic methods
    // `wrapping_add` make implementing crypto primitives much safer than C,
    // avoiding undefined behavior on integer overflow.
    let mut h0: u32 = 0x6745_2301;
    let mut h1: u32 = 0xEFCD_AB89;
    let mut h2: u32 = 0x98BA_DCFE;
    let mut h3: u32 = 0x1032_5476;
    let mut h4: u32 = 0xC3D2_E1F0;

    let mut message = data.to_vec();
    let original_byte_len = message.len() as u64;
    let original_bit_len = original_byte_len * 8;

    message.push(0x80);
    while (message.len() % 64) != 56 {
        message.push(0x00);
    }

    message.extend_from_slice(&original_bit_len.to_be_bytes());

    // GOTCHA: chunking must be exact. Since we padded to exactly a multiple of 64 bytes,
    // `chunks_exact` is appropriate here and avoids branching on uneven chunks.
    for chunk in message.chunks_exact(64) {
        let mut w = [0u32; 80];
        for (i, byte_chunk) in chunk.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes([byte_chunk[0], byte_chunk[1], byte_chunk[2], byte_chunk[3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }

        let mut a = h0;
        let mut b = h1;
        let mut c = h2;
        let mut d = h3;
        let mut e = h4;

        for (i, &wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5A82_7999),
                20..=39 => (b ^ c ^ d, 0x6ED9_EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1B_BCDC),
                60..=79 => (b ^ c ^ d, 0xCA62_C1D6),
                _ => unreachable!(),
            };

            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(wi);
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

    let mut result = [0u8; 20];
    result[0..4].copy_from_slice(&h0.to_be_bytes());
    result[4..8].copy_from_slice(&h1.to_be_bytes());
    result[8..12].copy_from_slice(&h2.to_be_bytes());
    result[12..16].copy_from_slice(&h3.to_be_bytes());
    result[16..20].copy_from_slice(&h4.to_be_bytes());

    result
}

/// Helper to convert a 20-byte hash array to a 40-character hex string.
#[must_use]
pub fn hash_to_hex(hash: &[u8; 20]) -> String {
    let mut s = String::with_capacity(40);
    for byte in hash {
        write!(&mut s, "{byte:02x}").expect("Writing to a String should never fail");
    }
    s
}

/// Represents a core Git object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitObject {
    /// A blob stores file data.
    Blob(Vec<u8>),
    /// A tree stores directory structure (list of mode, name, and SHA-1 hashes).
    Tree(Vec<TreeEntry>),
    /// A commit stores a snapshot tree, parents, author info, and message.
    Commit(CommitData),
}

/// Represents an entry inside a Git Tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeEntry {
    pub mode: String,
    pub name: String,
    pub hash: [u8; 20],
}

/// Represents the data within a Git Commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitData {
    pub tree: String,
    pub parents: Vec<String>,
    pub author: String,
    pub committer: String,
    pub message: String,
}

impl GitObject {
    /// Serializes the Git object to bytes, including its header `<type> <size>\0`.
    #[must_use]
    pub fn serialize(&self) -> Vec<u8> {
        let mut content = Vec::new();

        // RUST INSIGHT:
        // Explicitly typing the match arm returns to a byte slice `&[u8]` allows us to return byte string literals of different lengths (e.g. b"tree" vs b"commit") without a compiler error.
        let ty: &[u8] = match self {
            Self::Blob(data) => {
                content.extend_from_slice(data);
                b"blob"
            }
            Self::Tree(entries) => {
                for entry in entries {
                    content.extend_from_slice(entry.mode.as_bytes());
                    content.push(b' ');
                    content.extend_from_slice(entry.name.as_bytes());
                    content.push(0);
                    content.extend_from_slice(&entry.hash);
                }
                b"tree"
            }
            Self::Commit(commit) => {
                content.extend_from_slice(b"tree ");
                content.extend_from_slice(commit.tree.as_bytes());
                content.push(b'\n');

                for parent in &commit.parents {
                    content.extend_from_slice(b"parent ");
                    content.extend_from_slice(parent.as_bytes());
                    content.push(b'\n');
                }

                content.extend_from_slice(b"author ");
                content.extend_from_slice(commit.author.as_bytes());
                content.push(b'\n');

                content.extend_from_slice(b"committer ");
                content.extend_from_slice(commit.committer.as_bytes());
                content.push(b'\n');
                content.push(b'\n');

                content.extend_from_slice(commit.message.as_bytes());
                b"commit"
            }
        };

        let mut output = Vec::new();
        output.extend_from_slice(ty);
        output.push(b' ');
        output.extend_from_slice(content.len().to_string().as_bytes());
        output.push(0);
        output.extend_from_slice(&content);
        output
    }

    /// Computes the SHA-1 hash of the serialized object.
    #[must_use]
    pub fn hash(&self) -> [u8; 20] {
        sha1(&self.serialize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha1_basic() {
        // "hello world" hash: 2aae6c35c94fcfb415dbe95f408b9ce91ee846ed
        let hash = sha1(b"hello world");
        let hex = hash_to_hex(&hash);
        assert_eq!(hex, "2aae6c35c94fcfb415dbe95f408b9ce91ee846ed");
    }

    #[test]
    fn test_sha1_empty() {
        // "" hash: da39a3ee5e6b4b0d3255bfef95601890afd80709
        let hash = sha1(b"");
        let hex = hash_to_hex(&hash);
        assert_eq!(hex, "da39a3ee5e6b4b0d3255bfef95601890afd80709");
    }

    #[test]
    fn test_blob_serialize() {
        let blob = GitObject::Blob(b"hello".to_vec());
        let serialized = blob.serialize();
        // blob 5\0hello
        let mut expected = b"blob 5\0".to_vec();
        expected.extend_from_slice(b"hello");
        assert_eq!(serialized, expected);

        let hash = blob.hash();
        let hex = hash_to_hex(&hash);
        // hash of "blob 5\0hello" is b6fc4c620b67d95f953a5c1c1230aaab5db5a1b0
        assert_eq!(hex, "b6fc4c620b67d95f953a5c1c1230aaab5db5a1b0");
    }

    #[test]
    fn test_tree_serialize() {
        let entry = TreeEntry {
            mode: "100644".to_string(),
            name: "hello.txt".to_string(),
            hash: sha1(b"blob 5\0hello"),
        };
        let tree = GitObject::Tree(vec![entry]);
        let serialized = tree.serialize();

        assert!(serialized.starts_with(b"tree "));
        // verify content structure
        let hash = tree.hash();
        assert_eq!(hash.len(), 20);
    }

    #[test]
    fn test_commit_serialize() {
        let commit = GitObject::Commit(CommitData {
            tree: "b6fc4c620b67d95f953a5c1c1230aaab5db5a1b0".to_string(),
            parents: vec!["da39a3ee5e6b4b0d3255bfef95601890afd80709".to_string()],
            author: "Author Name <author@example.com> 1629810452 +0000".to_string(),
            committer: "Committer Name <committer@example.com> 1629810452 +0000".to_string(),
            message: "Initial commit".to_string(),
        });

        let serialized = commit.serialize();
        assert!(serialized.starts_with(b"commit "));

        let content = std::str::from_utf8(&serialized).unwrap();
        assert!(content.contains("tree b6fc4c620b67d95f953a5c1c1230aaab5db5a1b0\n"));
        assert!(content.contains("parent da39a3ee5e6b4b0d3255bfef95601890afd80709\n"));
        assert!(content.contains("\n\nInitial commit"));
    }
}
