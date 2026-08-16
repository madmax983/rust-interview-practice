//! # Git Object Store Implementation
//!
//! Implements a content-addressable storage system that replicates Git's core
//! object store: Blobs, Trees, and Commits, identified by SHA-1 hashes of their content.
//!
//! **Replaces Crates:** `git2`, `gix`
//!
//! **Real-world Usage:**
//! - Source control systems (Git, Mercurial)
//! - Content distribution networks (IPFS)
//! - Build systems (Bazel, Nix)
//! - Decentralized file systems
//!
//! **Why build it yourself?**
//! Implementing Git's object model demystifies how version control works under the hood.
//! It teaches you about content-addressable storage, object serialization, recursive
//! tree structures, and SHA hashing from scratch. You'll learn how Rust's trait
//! system (`ObjectStore`) enables swappable backend strategies (in-memory vs. disk-based)
//! while maintaining strong typing guarantees.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::{Cursor, Read, Write};

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure:
//
//     [ObjectStore Trait]
//              |
//     [MemoryObjectStore] (Implementation)
//        HashMap<String (SHA-1), Vec<u8>>
//
// Objects:
//   - Blob: Just file content. Format: "blob <size>\0<content>"
//   - Tree: Directory listing. Format: "tree <size>\0<mode> <name>\0<20-byte-hash>..."
//   - Commit: Metadata + Tree pointer. Format: "commit <size>\0tree <hash>\n..."
//
// Invariants:
// - Object IDs (OIDs) are exactly 40-character lowercase hex strings (derived from SHA-1).
// - Writing identical content always yields the same OID.
// - Tree and Commit objects reference other objects purely by OID.
//
// Time/Space Complexity:
// - Write: O(N) time, O(N) space where N is the serialized object size.
// - Read: O(1) time (map lookup) + O(N) time to deserialize.
//
// Design Decisions & Tradeoffs:
// - SHA-1: We implement a basic, unoptimized SHA-1 from scratch to avoid external dependencies
//   and demonstrate the algorithmic core. In production, a hardware-accelerated crate is necessary.
// - In-Memory Store: We use a `HashMap` for simplicity and testability. A real Git client
//   would write zlib-compressed objects to `.git/objects/`.
// - Parsing: We use simple byte slicing and `std::io::Read` for parsing. Robust implementations
//   use parser combinators like `nom` for safe and fast binary parsing.

/// A 40-character hexadecimal string representing a SHA-1 hash.
pub type Oid = String;

/// The type of a Git object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectType {
    Blob,
    Tree,
    Commit,
}

/// A parsed Git object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitObject {
    Blob(Vec<u8>),
    Tree(Vec<TreeEntry>),
    Commit(CommitData),
}

/// An entry within a Git Tree object (representing a file or subdirectory).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeEntry {
    pub mode: String, // e.g., "100644" or "040000"
    pub name: String,
    pub oid: Oid,
}

/// Metadata and pointers within a Git Commit object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitData {
    pub tree: Oid,
    pub parents: Vec<Oid>,
    pub author: String,
    pub committer: String,
    pub message: String,
}

// RUST INSIGHT:
// Defining an `ObjectStore` trait allows us to easily mock the storage layer
// for testing (using `HashMap`) or implement a real disk-backed store later
// without changing the core object serialization logic.
pub trait ObjectStore {
    /// Writes raw bytes to the store, returning the OID (SHA-1).
    fn write_object(&mut self, data: &[u8]) -> Oid;
    /// Reads raw bytes from the store by OID.
    fn read_object(&self, oid: &str) -> Option<Vec<u8>>;
}

/// An in-memory implementation of `ObjectStore` for demonstration and testing.
#[derive(Default)]
pub struct MemoryObjectStore {
    objects: HashMap<Oid, Vec<u8>>,
}

impl ObjectStore for MemoryObjectStore {
    fn write_object(&mut self, data: &[u8]) -> Oid {
        let oid = sha1_hex(data);
        // GOTCHA:
        // Git doesn't check for collisions; if the hash exists, it assumes
        // the content is identical. We insert it regardless of existence.
        self.objects.insert(oid.clone(), data.to_vec());
        oid
    }

    fn read_object(&self, oid: &str) -> Option<Vec<u8>> {
        self.objects.get(oid).cloned()
    }
}

// =========================================================================================
// SHA-1 Implementation
// =========================================================================================

/// Computes the SHA-1 hash of the given data and returns it as a 40-char hex string.
///
/// **Why build it?** To show how cryptographic hashing works under the hood.
/// This implementation is functional but not optimized for speed.
fn sha1_hex(data: &[u8]) -> String {
    let mut h0 = 0x6745_2301u32;
    let mut h1 = 0xEFCD_AB89u32;
    let mut h2 = 0x98BA_DCFEu32;
    let mut h3 = 0x1032_5476u32;
    let mut h4 = 0xC3D2_E1F0u32;

    let mut msg = data.to_vec();
    let ml = (msg.len() as u64) * 8; // Message length in bits
    msg.push(0x80);

    // Append padding zeros until the length in bytes is congruent to 56 (mod 64)
    while (msg.len() % 64) != 56 {
        msg.push(0x00);
    }

    // Append the original message length as a 64-bit big-endian integer
    msg.extend_from_slice(&ml.to_be_bytes());

    for chunk in msg.chunks(64) {
        let mut w = [0u32; 80];
        // Break chunk into sixteen 32-bit big-endian words
        for (i, byte_chunk) in chunk.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes(byte_chunk.try_into().unwrap());
        }

        // Extend the sixteen 32-bit words into eighty 32-bit words
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }

        let mut a = h0;
        let mut b = h1;
        let mut c = h2;
        let mut d = h3;
        let mut e = h4;

        for (i, &word) in w.iter().enumerate() {
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
                .wrapping_add(word);
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

    let mut hex_str = String::with_capacity(40);
    for h in [h0, h1, h2, h3, h4] {
        write!(&mut hex_str, "{:08x}", h).unwrap();
    }
    hex_str
}

// =========================================================================================
// Git Object Formatting and Parsing
// =========================================================================================

/// Formats a Git object for storage by prepending the header: `<type> <size>\0<content>`.
pub fn format_object(obj_type: &str, content: &[u8]) -> Vec<u8> {
    let header = format!("{} {}\0", obj_type, content.len());
    let mut data = Vec::with_capacity(header.len() + content.len());
    data.extend_from_slice(header.as_bytes());
    data.extend_from_slice(content);
    data
}

/// Creates and writes a Blob object to the store.
pub fn write_blob<S: ObjectStore + ?Sized>(store: &mut S, content: &[u8]) -> Oid {
    let data = format_object("blob", content);
    store.write_object(&data)
}

/// Creates and writes a Tree object to the store.
/// Tree entries must be sorted by name according to Git specifications,
/// but for simplicity in this implementation, we assume they are provided sorted.
pub fn write_tree<S: ObjectStore + ?Sized>(store: &mut S, entries: &[TreeEntry]) -> Oid {
    let mut content = Vec::new();
    for entry in entries {
        // Format: <mode> <name>\0<20_byte_hash>
        write!(&mut content, "{} {}\0", entry.mode, entry.name).unwrap();

        // Convert the 40-char hex string back to a 20-byte array
        let mut hash_bytes = [0u8; 20];
        for (i, byte) in hash_bytes.iter_mut().enumerate() {
            let byte_str = &entry.oid[i * 2..(i * 2) + 2];
            *byte = u8::from_str_radix(byte_str, 16).unwrap_or(0);
        }
        content.extend_from_slice(&hash_bytes);
    }
    let data = format_object("tree", &content);
    store.write_object(&data)
}

/// Creates and writes a Commit object to the store.
pub fn write_commit<S: ObjectStore + ?Sized>(store: &mut S, commit: &CommitData) -> Oid {
    let mut content = String::new();
    writeln!(&mut content, "tree {}", commit.tree).unwrap();
    for parent in &commit.parents {
        writeln!(&mut content, "parent {}", parent).unwrap();
    }
    writeln!(&mut content, "author {}", commit.author).unwrap();
    writeln!(&mut content, "committer {}", commit.committer).unwrap();
    writeln!(&mut content).unwrap();
    content.push_str(&commit.message);

    let data = format_object("commit", content.as_bytes());
    store.write_object(&data)
}

/// Parses a raw Git object from the store into a typed `GitObject`.
pub fn parse_object(data: &[u8]) -> Option<GitObject> {
    // Read header: until first null byte
    let null_idx = data.iter().position(|&b| b == 0)?;
    let header_str = std::str::from_utf8(&data[..null_idx]).ok()?;
    let mut parts = header_str.split(' ');
    let obj_type = parts.next()?;

    // Size check omitted for brevity, but a robust parser would verify it
    let _size: usize = parts.next()?.parse().ok()?;

    let content = &data[null_idx + 1..];

    match obj_type {
        "blob" => Some(GitObject::Blob(content.to_vec())),
        "tree" => {
            let mut entries = Vec::new();
            let mut cursor = Cursor::new(content);
            while (cursor.position() as usize) < content.len() {
                // Read up to space
                let mut mode_bytes = Vec::new();
                let mut buf = [0u8; 1];
                while cursor.read_exact(&mut buf).is_ok() && buf[0] != b' ' {
                    mode_bytes.push(buf[0]);
                }
                let mode = String::from_utf8(mode_bytes).ok()?;

                // Read up to null
                let mut name_bytes = Vec::new();
                while cursor.read_exact(&mut buf).is_ok() && buf[0] != 0 {
                    name_bytes.push(buf[0]);
                }
                let name = String::from_utf8(name_bytes).ok()?;

                // Read 20 byte hash
                let mut hash_bytes = [0u8; 20];
                cursor.read_exact(&mut hash_bytes).ok()?;

                let mut oid = String::with_capacity(40);
                for &b in &hash_bytes {
                    write!(&mut oid, "{:02x}", b).unwrap();
                }

                entries.push(TreeEntry { mode, name, oid });
            }
            Some(GitObject::Tree(entries))
        }
        "commit" => {
            let content_str = std::str::from_utf8(content).ok()?;
            let mut lines = content_str.lines();

            let tree_line = lines.next()?;
            let tree = tree_line.strip_prefix("tree ")?.to_string();

            let mut parents = Vec::new();
            let mut author = String::new();
            let mut committer = String::new();

            let mut in_message = false;
            let mut message = String::new();

            for line in lines {
                if in_message {
                    if !message.is_empty() {
                        message.push('\n');
                    }
                    message.push_str(line);
                } else if line.is_empty() {
                    in_message = true;
                } else if let Some(parent) = line.strip_prefix("parent ") {
                    parents.push(parent.to_string());
                } else if let Some(a) = line.strip_prefix("author ") {
                    author = a.to_string();
                } else if let Some(c) = line.strip_prefix("committer ") {
                    committer = c.to_string();
                }
            }

            Some(GitObject::Commit(CommitData {
                tree,
                parents,
                author,
                committer,
                message,
            }))
        }
        _ => None,
    }
}

// =========================================================================================
// Footer & Extensions
// =========================================================================================
//
// Comparison to `git2` / `gix`:
// - Real implementations handle delta compression (packfiles) to save massive amounts of space.
// - Real systems memory-map (`mmap`) files instead of eagerly reading them into `Vec<u8>`.
// - The standard SHA-1 in Rust (e.g., the `sha1` crate) uses SIMD and hardware acceleration.
//
// Missing vs Production:
// - Packfile reading/writing (the `.pack` and `.idx` format).
// - Zlib compression (Git zlib deflates the `<type> <size>\0<content>` byte stream before disk write).
// - Tree sorting validation (Git enforces trees are sorted to guarantee stable hashes).
//
// Suggested Next Steps:
// - Implement a `DiskObjectStore` using the `flate2` crate for zlib compression.
// - Create a function to recursively write a directory structure into Trees and Blobs.

#[cfg(test)]
mod tests {
    use super::*;
    use std::hint::black_box;
    use std::time::Instant;

    #[test]
    fn test_sha1_known_values() {
        assert_eq!(
            sha1_hex(b"hello"),
            "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d"
        );
        assert_eq!(sha1_hex(b""), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
    }

    #[test]
    fn test_blob_write_and_read() {
        let mut store = MemoryObjectStore::default();
        let content = b"hello world";

        let oid = write_blob(&mut store, content);

        // Expected header for "hello world" is "blob 11\0"
        let expected_blob_data = format_object("blob", content);
        assert_eq!(store.read_object(&oid).unwrap(), expected_blob_data);

        let parsed = parse_object(&expected_blob_data).unwrap();
        if let GitObject::Blob(parsed_content) = parsed {
            assert_eq!(parsed_content, content);
        } else {
            panic!("Expected Blob");
        }
    }

    #[test]
    fn test_tree_write_and_read() {
        let mut store = MemoryObjectStore::default();
        let entries = vec![TreeEntry {
            mode: "100644".to_string(),
            name: "file.txt".to_string(),
            oid: "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d".to_string(),
        }];

        let oid = write_tree(&mut store, &entries);
        let tree_data = store.read_object(&oid).unwrap();

        let parsed = parse_object(&tree_data).unwrap();
        if let GitObject::Tree(parsed_entries) = parsed {
            assert_eq!(parsed_entries, entries);
        } else {
            panic!("Expected Tree");
        }
    }

    #[test]
    fn test_commit_write_and_read() {
        let mut store = MemoryObjectStore::default();
        let commit = CommitData {
            tree: "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d".to_string(),
            parents: vec!["da39a3ee5e6b4b0d3255bfef95601890afd80709".to_string()],
            author: "Jane Doe <jane@example.com> 1600000000 +0000".to_string(),
            committer: "Jane Doe <jane@example.com> 1600000000 +0000".to_string(),
            message: "Initial commit".to_string(),
        };

        let oid = write_commit(&mut store, &commit);
        let commit_data = store.read_object(&oid).unwrap();

        let parsed = parse_object(&commit_data).unwrap();
        if let GitObject::Commit(parsed_commit) = parsed {
            assert_eq!(parsed_commit, commit);
        } else {
            panic!("Expected Commit");
        }
    }

    #[test]
    fn test_git_core_benchmark() {
        let mut store = MemoryObjectStore::default();
        let payload = b"A quick brown fox jumps over the lazy dog";

        let start = Instant::now();
        for _ in 0..10_000 {
            black_box(write_blob(&mut store, black_box(payload)));
        }
        let duration = start.elapsed();

        println!("git_core: wrote 10,000 blobs in {:?}", duration);
        // Ensure it runs in a reasonable time (e.g. < 500ms even in debug)
        assert!(duration.as_millis() < 5000);
    }
}
