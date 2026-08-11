//! # Git Core Implementation (Git Object Store)
//!
//! **Replaces Crates:** `git2`, `gix`
//!
//! **Real-world systems:** Git itself, GitLab Gitaly server, custom source control systems.
//!
//! **Why build it yourself?** To demystify Git. It's often viewed as magic, but at its core, it's just a
//! content-addressable filesystem. Building the object store teaches you how SHA-1 hashing, zlib compression,
//! and simple text formats combine to create a robust version control system.
//!
//! ## Architecture
//!
//! Git stores all data as objects. The key is the SHA-1 hash of the object's contents, and the value is the
//! compressed contents. There are three main types of objects we implement:
//!
//! 1. **Blob**: Stores file data. (Format: `blob <size>\0<data>`)
//! 2. **Tree**: Stores directory structure, linking names to blobs or other trees. (Format: `tree <size>\0<entries>`)
//! 3. **Commit**: Stores metadata (author, message) and points to a root tree and parent commits.
//!
//! ```text
//! +-------------+      +-------------+      +-------------+
//! | Commit Obj  |----->| Tree Obj    |----->| Blob Obj    |
//! | (hash: abc) |      | (hash: def) |      | (hash: 123) |
//! +-------------+      +-------------+      +-------------+
//!                         |
//!                         +--> +-------------+
//!                              | Tree Obj    |
//!                              | (hash: 456) |
//!                              +-------------+
//! ```
//!
//! ### Invariants
//! * The SHA-1 hash must be calculated over the *uncompressed* data, including the `<type> <size>\0` header.
//! * Objects must be stored zlib-compressed on disk.
//! * Tree entries must be sorted by name to ensure consistent hashing for identical directory structures.
//!
//! ### Time/Space Complexity
//! | Operation | Time Complexity | Space Complexity |
//! |-----------|-----------------|------------------|
//! | Hash calc | O(N) where N=size | O(N)             |
//! | Read/Write| O(N)            | O(N)             |
//!
//! ## Footer
//!
//! **Comparison to `git2` / `gix` crate:** The canonical crates are highly optimized C bindings or pure Rust
//! implementations supporting the full Git protocol, packfiles, networking, and references. Our implementation
//! focuses solely on the core object database (loose objects).
//!
//! **Missing from production:**
//! - Packfile support (crucial for performance and saving space).
//! - Delta compression.
//! - Network protocols (fetch/push).
//! - Index (staging area) management.
//! - Refs management (`.git/refs/heads/master`).
//!
//! **Next steps:**
//! - Implement reading and writing to the Git Index (`.git/index`).
//! - Add support for parsing and creating Refs.

use std::fmt;
use std::io;

// Simulating SHA-1 hashing. In a real implementation, you'd use a crate like `sha1`.
// We'll build a very simplistic, non-cryptographic mock hash for educational purposes here,
// but output it as a 40-character hex string just like Git.
// UNSAFE JUSTIFICATION: No unsafe used. We use a simple hash function for demonstration.
fn mock_sha1(data: &[u8]) -> String {
    let mut hash: u32 = 0x811c9dc5;
    for &b in data {
        hash ^= b as u32;
        hash = hash.wrapping_mul(0x01000193);
    }
    // Repeat to pad to 40 hex chars for realism
    format!(
        "{:08x}{:08x}{:08x}{:08x}{:08x}",
        hash,
        hash.wrapping_add(1),
        hash.wrapping_add(2),
        hash.wrapping_add(3),
        hash.wrapping_add(4)
    )
}

// Mocking zlib compression. In reality, you'd use `flate2`.
fn mock_compress(data: &[u8]) -> Vec<u8> {
    // Just wrap in a magic header for our tests
    let mut out = vec![0x78, 0x9c];
    out.extend_from_slice(data);
    out
}

fn mock_decompress(data: &[u8]) -> io::Result<Vec<u8>> {
    if data.len() < 2 || data[0] != 0x78 || data[1] != 0x9c {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid zlib header",
        ));
    }
    Ok(data[2..].to_vec())
}

/// Represents a 40-character Git SHA-1 hash.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Oid(pub String);

impl fmt::Display for Oid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The core Git object types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitObject {
    Blob(Vec<u8>),
    Tree(Vec<TreeEntry>),
    Commit {
        tree: Oid,
        parents: Vec<Oid>,
        author: String,
        committer: String,
        message: String,
    },
}

impl GitObject {
    /// Returns the type string used in the object header.
    fn type_str(&self) -> &'static str {
        match self {
            GitObject::Blob(_) => "blob",
            GitObject::Tree(_) => "tree",
            GitObject::Commit { .. } => "commit",
        }
    }

    /// Serializes the object into its uncompressed raw byte format (without the header).
    fn serialize_data(&self) -> Vec<u8> {
        match self {
            GitObject::Blob(data) => data.clone(),
            GitObject::Tree(entries) => {
                let mut out = Vec::new();
                for entry in entries {
                    // format: <mode> <name>\0<20-byte raw sha1>
                    // For simplicity in this mock, we use the 40-char hex string as bytes since we mocked SHA-1.
                    // Real git stores the 20 raw bytes.
                    out.extend_from_slice(entry.mode.as_bytes());
                    out.push(b' ');
                    out.extend_from_slice(entry.name.as_bytes());
                    out.push(0);
                    out.extend_from_slice(entry.oid.0.as_bytes());
                }
                out
            }
            GitObject::Commit {
                tree,
                parents,
                author,
                committer,
                message,
            } => {
                let mut out = Vec::new();
                out.extend_from_slice(format!("tree {}\n", tree.0).as_bytes());
                for p in parents {
                    out.extend_from_slice(format!("parent {}\n", p.0).as_bytes());
                }
                out.extend_from_slice(format!("author {}\n", author).as_bytes());
                out.extend_from_slice(format!("committer {}\n", committer).as_bytes());
                out.extend_from_slice(b"\n");
                out.extend_from_slice(message.as_bytes());
                out
            }
        }
    }

    /// Deserializes raw object data (excluding the header) into a GitObject.
    fn deserialize_data(kind: &str, data: &[u8]) -> io::Result<Self> {
        match kind {
            "blob" => Ok(GitObject::Blob(data.to_vec())),
            "tree" => {
                // Parsing tree entries is tricky because of the null byte separator and raw hash.
                let mut entries = Vec::new();
                let mut i = 0;
                while i < data.len() {
                    let space = data[i..].iter().position(|&b| b == b' ').unwrap_or(0) + i;
                    let mode = String::from_utf8_lossy(&data[i..space]).into_owned();
                    i = space + 1;

                    let null = data[i..].iter().position(|&b| b == 0).unwrap_or(0) + i;
                    let name = String::from_utf8_lossy(&data[i..null]).into_owned();
                    i = null + 1;

                    // In our mock, the hash is 40 bytes. Real git is 20 bytes.
                    if i + 40 > data.len() {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "Truncated tree entry",
                        ));
                    }
                    let hash = String::from_utf8_lossy(&data[i..i + 40]).into_owned();
                    i += 40;

                    entries.push(TreeEntry {
                        mode,
                        name,
                        oid: Oid(hash),
                    });
                }
                Ok(GitObject::Tree(entries))
            }
            "commit" => {
                let text = String::from_utf8_lossy(data);
                let mut lines = text.split('\n');

                let tree_line = lines.next().unwrap_or("");
                let tree = Oid(tree_line.strip_prefix("tree ").unwrap_or("").to_string());

                let mut parents = Vec::new();
                let mut author = String::new();
                let mut committer = String::new();

                for line in &mut lines {
                    if line.is_empty() {
                        break;
                    }
                    if let Some(p) = line.strip_prefix("parent ") {
                        parents.push(Oid(p.to_string()));
                    } else if let Some(a) = line.strip_prefix("author ") {
                        author = a.to_string();
                    } else if let Some(c) = line.strip_prefix("committer ") {
                        committer = c.to_string();
                    }
                }

                let message = lines.collect::<Vec<_>>().join("\n");

                Ok(GitObject::Commit {
                    tree,
                    parents,
                    author,
                    committer,
                    message,
                })
            }
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Unknown object type",
            )),
        }
    }
}

/// An entry within a Tree object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeEntry {
    pub mode: String, // e.g., "100644" for normal file, "040000" for directory
    pub name: String,
    pub oid: Oid,
}

/// A simple, in-memory Git Object Database.
/// In reality, this would read/write files to `.git/objects/`.
pub struct ObjectDatabase {
    // Map of SHA-1 hash to zlib-compressed object data
    store: std::collections::HashMap<String, Vec<u8>>,
}

impl ObjectDatabase {
    pub fn new() -> Self {
        Self {
            store: std::collections::HashMap::new(),
        }
    }

    /// Writes an object to the database, returning its SHA-1 hash.
    pub fn write_object(&mut self, obj: &GitObject) -> io::Result<Oid> {
        let kind = obj.type_str();
        let data = obj.serialize_data();

        // Construct the Git object format: "<type> <size>\0<data>"
        let header = format!("{} {}\0", kind, data.len());
        let mut full_data = header.into_bytes();
        full_data.extend_from_slice(&data);

        // Hash the uncompressed full data
        let hash = mock_sha1(&full_data);

        // Compress and store
        // PRODUCTION NOTE: Real git stores in `.git/objects/xx/yyyy...` based on the hash.
        let compressed = mock_compress(&full_data);
        self.store.insert(hash.clone(), compressed);

        Ok(Oid(hash))
    }

    /// Reads an object from the database by its SHA-1 hash.
    pub fn read_object(&self, oid: &Oid) -> io::Result<GitObject> {
        let compressed = self
            .store
            .get(&oid.0)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Object not found"))?;

        let full_data = mock_decompress(compressed)?;

        // Find the null byte separating header from data
        let null_pos = full_data.iter().position(|&b| b == 0).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Missing null byte in object header",
            )
        })?;

        let header = String::from_utf8_lossy(&full_data[..null_pos]);
        let mut parts = header.split(' ');
        let kind = parts.next().unwrap_or("");
        let size: usize = parts.next().unwrap_or("0").parse().unwrap_or(0);

        let data = &full_data[null_pos + 1..];
        if data.len() != size {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Object size mismatch",
            ));
        }

        GitObject::deserialize_data(kind, data)
    }
}

impl Default for ObjectDatabase {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blob_roundtrip() {
        let mut db = ObjectDatabase::new();
        let blob = GitObject::Blob(b"Hello, Git!".to_vec());

        let oid = db.write_object(&blob).unwrap();
        let read_back = db.read_object(&oid).unwrap();

        assert_eq!(blob, read_back);
    }

    #[test]
    fn test_tree_roundtrip() {
        let mut db = ObjectDatabase::new();

        let blob_oid = db.write_object(&GitObject::Blob(b"code".to_vec())).unwrap();

        let tree = GitObject::Tree(vec![TreeEntry {
            mode: "100644".to_string(),
            name: "main.rs".to_string(),
            oid: blob_oid,
        }]);

        let tree_oid = db.write_object(&tree).unwrap();
        let read_back = db.read_object(&tree_oid).unwrap();

        assert_eq!(tree, read_back);
    }

    #[test]
    fn test_commit_roundtrip() {
        let mut db = ObjectDatabase::new();

        let tree_oid = db.write_object(&GitObject::Tree(vec![])).unwrap();
        let parent_oid = Oid("dummy_parent_hash123".to_string());

        let commit = GitObject::Commit {
            tree: tree_oid,
            parents: vec![parent_oid],
            author: "Linus Torvalds <torvalds@linux-foundation.org> 1112911993 -0700".to_string(),
            committer: "Linus Torvalds <torvalds@linux-foundation.org> 1112911993 -0700"
                .to_string(),
            message: "Initial commit".to_string(),
        };

        let commit_oid = db.write_object(&commit).unwrap();
        let read_back = db.read_object(&commit_oid).unwrap();

        assert_eq!(commit, read_back);
    }

    #[test]
    fn test_missing_object() {
        let db = ObjectDatabase::new();
        let res = db.read_object(&Oid("nonexistent".to_string()));
        assert!(res.is_err());
    }
}
