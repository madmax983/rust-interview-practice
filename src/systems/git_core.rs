//! # Git Object Store Implementation
//!
//! Implements a Git-compatible content-addressable object store from scratch.
//!
//! **Replaces Crates:** `git2`, `gix`
//!
//! **Real-world Usage:**
//! - Version control systems (Git, Mercurial)
//! - Decentralized file systems (IPFS)
//! - Build systems caching (Bazel, Nix)
//!
//! **Why build it yourself?**
//! Understanding Git's object model demystifies how a complex DAG of history is represented
//! using simple building blocks. You'll learn about content-addressable storage, where the ID
//! of an object is uniquely determined by its content via cryptographic hashing. This enforces
//! immutability and makes deduplication trivial. You'll also confront binary serialization for
//! `Tree` objects versus textual formats for `Commit`s.

use std::collections::HashMap;
use std::fmt;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure:
//
//      Memory Store (HashMap)
//         │
//         ▼
//    [ SHA-1 Hash ]  ───────►  [ Zlib Compressed Object ] (mocked here as raw bytes for clarity)
//
// Object Types:
// 1. Blob:   `blob <size>\0<content>`
// 2. Tree:   `tree <size>\0[mode name\0<20-byte-hash>]...`
// 3. Commit: `commit <size>\0tree <hash>\nparent <hash>\nauthor <...>\ncommitter <...>\n\n<msg>`
//
// Invariants:
// 1. Every object is identified by the SHA-1 hash of its exact serialized representation.
// 2. Objects are immutable once stored.
// 3. A tree's hash changes if any of its children change (Merkle tree property).
//
// Complexity:
// ┌───────────┬────────┬────────┐
// │ Operation │ Time   │ Space  │
// ├───────────┼────────┼────────┤
// │ write_obj │ O(N)   │ O(N)   │ N = object size
// │ read_obj  │ O(1)   │ O(N)   │ In-memory hash lookup
// └───────────┴────────┴────────┘

/// A 20-byte SHA-1 hash identifier.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjectId(pub [u8; 20]);

impl fmt::Display for ObjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0.iter() {
            write!(f, "{:02x}", byte)?;
        }
        Ok(())
    }
}

impl fmt::Debug for ObjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ObjectId({})", self)
    }
}

/// The core types of Git objects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Object {
    Blob(Vec<u8>),
    Tree(Vec<TreeEntry>),
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
    pub name: String,
    pub id: ObjectId,
}

impl Object {
    /// Serializes the object into the standard Git format.
    ///
    /// Note: In production Git, the result is Zlib compressed before writing to disk.
    pub fn serialize(&self) -> Vec<u8> {
        let mut content = Vec::new();
        match self {
            Object::Blob(data) => {
                content.extend_from_slice(data);
            }
            Object::Tree(entries) => {
                for entry in entries {
                    // entry format: `<mode> <name>\0<20-byte-id>`
                    content.extend_from_slice(entry.mode.as_bytes());
                    content.push(b' ');
                    content.extend_from_slice(entry.name.as_bytes());
                    content.push(0);
                    content.extend_from_slice(&entry.id.0);
                }
            }
            Object::Commit {
                tree,
                parents,
                author,
                committer,
                message,
            } => {
                // commit format is purely textual until the message
                content.extend_from_slice(format!("tree {}\n", tree).as_bytes());
                for parent in parents {
                    content.extend_from_slice(format!("parent {}\n", parent).as_bytes());
                }
                content.extend_from_slice(format!("author {}\n", author).as_bytes());
                content.extend_from_slice(format!("committer {}\n", committer).as_bytes());
                content.extend_from_slice(b"\n");
                content.extend_from_slice(message.as_bytes());
            }
        }

        let ty: &[u8] = match self {
            Object::Blob(_) => b"blob",
            Object::Tree(_) => b"tree",
            Object::Commit { .. } => b"commit",
        };

        // Header: `<type> <size>\0<content>`
        let mut header = Vec::new();
        header.extend_from_slice(ty);
        header.push(b' ');
        header.extend_from_slice(content.len().to_string().as_bytes());
        header.push(0);

        let mut out = header;
        out.extend_from_slice(&content);
        out
    }

    /// Deserializes an object from the standard Git format (uncompressed).
    pub fn deserialize(data: &[u8]) -> Option<Self> {
        let null_idx = data.iter().position(|&b| b == 0)?;
        let header_str = std::str::from_utf8(&data[..null_idx]).ok()?;
        let mut parts = header_str.split(' ');
        let ty = parts.next()?;
        let _size: usize = parts.next()?.parse().ok()?;

        let content = &data[null_idx + 1..];

        match ty {
            "blob" => Some(Object::Blob(content.to_vec())),
            "tree" => {
                let mut entries = Vec::new();
                let mut i = 0;
                while i < content.len() {
                    let sp = content[i..].iter().position(|&b| b == b' ')?;
                    let mode = std::str::from_utf8(&content[i..i + sp]).ok()?.to_string();
                    i += sp + 1;

                    let nul = content[i..].iter().position(|&b| b == 0)?;
                    let name = std::str::from_utf8(&content[i..i + nul]).ok()?.to_string();
                    i += nul + 1;

                    if i + 20 > content.len() {
                        return None;
                    }
                    let mut id_bytes = [0; 20];
                    id_bytes.copy_from_slice(&content[i..i + 20]);
                    let id = ObjectId(id_bytes);
                    i += 20;

                    entries.push(TreeEntry { mode, name, id });
                }
                Some(Object::Tree(entries))
            }
            "commit" => {
                let content_str = std::str::from_utf8(content).ok()?;
                let (headers, message) = content_str.split_once("\n\n")?;

                let mut tree = None;
                let mut parents = Vec::new();
                let mut author = String::new();
                let mut committer = String::new();

                for line in headers.lines() {
                    if let Some(t) = line.strip_prefix("tree ") {
                        tree = Some(Self::parse_hex_id(t)?);
                    } else if let Some(p) = line.strip_prefix("parent ") {
                        parents.push(Self::parse_hex_id(p)?);
                    } else if let Some(a) = line.strip_prefix("author ") {
                        author = a.to_string();
                    } else if let Some(c) = line.strip_prefix("committer ") {
                        committer = c.to_string();
                    }
                }

                Some(Object::Commit {
                    tree: tree?,
                    parents,
                    author,
                    committer,
                    message: message.to_string(),
                })
            }
            _ => None,
        }
    }

    fn parse_hex_id(hex: &str) -> Option<ObjectId> {
        if hex.len() != 40 {
            return None;
        }
        let mut bytes = [0; 20];
        for i in 0..20 {
            bytes[i] = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok()?;
        }
        Some(ObjectId(bytes))
    }
}

/// A rudimentary SHA-1 implementation for computing Git object hashes.
///
/// // RUST INSIGHT:
/// Implementing cryptographic algorithms from scratch highlights Rust's robust handling of integer
/// overflows (using `wrapping_add`) and strict typing, which prevents subtle casting bugs common in C.
pub fn sha1(data: &[u8]) -> ObjectId {
    let mut h = [
        0x67452301u32,
        0xEFCDAB89u32,
        0x98BADCFEu32,
        0x10325476u32,
        0xC3D2E1F0u32,
    ];

    let mut message = data.to_vec();
    let original_len_bits = (message.len() as u64) * 8;
    message.push(0x80);
    while (message.len() % 64) != 56 {
        message.push(0x00);
    }
    message.extend_from_slice(&original_len_bits.to_be_bytes());

    for chunk in message.chunks_exact(64) {
        let mut w = [0u32; 80];
        for (i, block) in chunk.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes([block[0], block[1], block[2], block[3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }

        let mut a = h[0];
        let mut b = h[1];
        let mut c = h[2];
        let mut d = h[3];
        let mut e = h[4];

        for (i, &wj) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5A827999),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };

            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(wj);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }

    let mut out = [0u8; 20];
    for (i, &val) in h.iter().enumerate() {
        let bytes = val.to_be_bytes();
        out[i * 4..i * 4 + 4].copy_from_slice(&bytes);
    }
    ObjectId(out)
}

/// An in-memory Git object store.
#[derive(Default)]
pub struct ObjectStore {
    objects: HashMap<ObjectId, Vec<u8>>,
}

impl ObjectStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Stores an object and returns its SHA-1 hash.
    pub fn write_object(&mut self, obj: &Object) -> ObjectId {
        let serialized = obj.serialize();
        let hash = sha1(&serialized);
        self.objects.insert(hash, serialized);
        hash
    }

    /// Retrieves an object by its SHA-1 hash.
    pub fn read_object(&self, id: &ObjectId) -> Option<Object> {
        let data = self.objects.get(id)?;
        Object::deserialize(data)
    }
}

// =========================================================================================
// Footer
// =========================================================================================
// Comparison to canonical crates:
// - `git2` uses libgit2 under the hood, dealing with loose objects, packfiles (delta compression),
//   and the git configuration/ref systems.
// - `gix` is a pure-Rust implementation of git, heavily optimized and modular.
//
// What's missing:
// - Zlib compression: Real Git uses zlib for all stored objects.
// - Packfiles: Git doesn't store every object individually (loose objects); it packs them together
//   and uses delta compression to save space.
// - Reference parsing (e.g., resolving `HEAD` or branches).
// - Tree iteration/walking abstractions.

#[cfg(test)]
mod tests {
    use super::*;
    use std::hint::black_box;
    use std::time::Instant;

    #[test]
    fn test_blob_serialization() {
        let blob = Object::Blob(b"hello world".to_vec());
        let serialized = blob.serialize();
        assert!(serialized.starts_with(b"blob 11\0"));
        assert!(serialized.ends_with(b"hello world"));

        let deserialized = Object::deserialize(&serialized).unwrap();
        assert_eq!(blob, deserialized);
    }

    #[test]
    fn test_sha1_hash() {
        // Known SHA-1 hash for "hello world" (note: git adds the `blob 11\0` header)
        let blob = Object::Blob(b"hello world".to_vec());
        let serialized = blob.serialize();
        let hash = sha1(&serialized);
        let hash_hex = format!("{}", hash);
        assert_eq!(hash_hex, "95d09f2b10159347eece71399a7e2e907ea3df4f");
    }

    #[test]
    fn test_tree_serialization() {
        let id1 = sha1(b"test1");
        let id2 = sha1(b"test2");
        let tree = Object::Tree(vec![
            TreeEntry {
                mode: "100644".to_string(),
                name: "file1.txt".to_string(),
                id: id1,
            },
            TreeEntry {
                mode: "040000".to_string(),
                name: "dir".to_string(),
                id: id2,
            },
        ]);

        let serialized = tree.serialize();
        let deserialized = Object::deserialize(&serialized).unwrap();
        assert_eq!(tree, deserialized);
    }

    #[test]
    fn test_commit_serialization() {
        let tree_id = sha1(b"tree content");
        let parent_id = sha1(b"parent content");
        let commit = Object::Commit {
            tree: tree_id,
            parents: vec![parent_id],
            author: "Alice <alice@example.com> 1234567890 +0000".to_string(),
            committer: "Bob <bob@example.com> 1234567890 +0000".to_string(),
            message: "Initial commit".to_string(),
        };

        let serialized = commit.serialize();
        let deserialized = Object::deserialize(&serialized).unwrap();
        assert_eq!(commit, deserialized);
    }

    #[test]
    fn test_object_store() {
        let mut store = ObjectStore::new();

        let blob = Object::Blob(b"rust programming".to_vec());
        let blob_id = store.write_object(&blob);

        let tree = Object::Tree(vec![TreeEntry {
            mode: "100644".to_string(),
            name: "main.rs".to_string(),
            id: blob_id,
        }]);
        let tree_id = store.write_object(&tree);

        assert_eq!(store.read_object(&blob_id), Some(blob));
        assert_eq!(store.read_object(&tree_id), Some(tree));
    }

    #[test]
    fn benchmark_sha1() {
        let data = vec![0u8; 1024 * 1024]; // 1MB buffer
        let start = Instant::now();
        let hash = sha1(black_box(&data));
        let elapsed = start.elapsed();
        // Benchmark note: Pure Rust naive SHA-1 is slower than optimized assembly (like in `sha1` crate),
        // but typically processes 1MB in a few milliseconds on modern hardware.
        assert!(elapsed.as_secs() < 1, "Hash took too long: {:?}", hash);
    }
}
