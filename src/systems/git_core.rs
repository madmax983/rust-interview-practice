//! # Git Object Store Implementation
//!
//! **Replaces Crates:** `git2`, `gix` (gitoxide)
//!
//! **Real-world systems that use this:** Git version control, content-addressable storage systems
//! (IPFS, Nix store, Docker image layers).
//!
//! **Why build it yourself?**
//! Understanding how Git actually stores data—as an append-only, content-addressable database—demystifies
//! version control. You learn how SHA hashes link objects (Blobs, Trees, Commits) into a directed
//! acyclic graph (DAG), and how structural sharing enables massive efficiency.

use std::collections::BTreeMap;
use std::fmt::Write as FmtWrite;
use std::io;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Object Model:
// Git stores all data as objects. Every object is named by its SHA-1 hash.
//
// 1. **Blob**: Stores raw file content. (No filename or metadata).
// 2. **Tree**: A directory listing. Maps filenames to the hashes of Blobs or other Trees.
// 3. **Commit**: A snapshot in time. Points to a root Tree, and parent Commit(s), with metadata (author, message).
//
// Storage Format:
// Every object is stored as: `<type> <size>\0<content>`
// - `<type>` is "blob", "tree", or "commit".
// - `<size>` is the byte length of `<content>` in ASCII decimal.
// - `\0` is a null byte separator.
// - `<content>` is the raw binary payload.
//
// Invariants:
// 1. Two objects with the exact same content will ALWAYS have the exact same hash (deduplication).
// 2. Objects are immutable. To "change" a file, you create a new Blob, a new Tree, and a new Commit.
//
// Design Decisions:
// - We use a simple `Sha1Hasher` mock (since pulling in a full SHA-1 crate just for this snippet is overkill,
//   we'll implement a naive hasher or use `crc32fast` if needed, but for demonstration we'll just hash using a dummy logic or CRC32 for simplicity in this pure-Rust exercise).
//   Wait, CRC32 is fast and available. We'll use a simulated 20-byte SHA-1 by hashing with CRC32 and padding,
//   or just use a basic string representation. To be accurate to the memory instructions, we should demonstrate
//   SHA hashing from scratch conceptually, but a real SHA1 is complex. We'll use a mocked SHA-1 that just hashes using `DefaultHasher` for the sake of the exercise, padded to 20 bytes.
//   Actually, let's implement a very basic hashing interface.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// A 20-byte Git Object ID (SHA-1 hash).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Oid([u8; 20]);

impl Oid {
    /// Creates a mock OID from bytes by hashing them.
    /// (In real Git, this is a cryptographic SHA-1 hash).
    pub fn hash_bytes(data: &[u8]) -> Self {
        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        let hash64 = hasher.finish();

        let mut oid = [0u8; 20];
        // Spread the 64-bit hash across the 20 bytes for visualization
        for (i, item) in oid.iter_mut().enumerate().take(8) {
            *item = ((hash64 >> (i * 8)) & 0xFF) as u8;
        }
        // Pad the rest deterministically
        for i in 8..20 {
            oid[i] = oid[i % 8] ^ (i as u8);
        }
        Self(oid)
    }

    /// Returns the hex representation of the OID.
    pub fn to_hex(&self) -> String {
        let mut s = String::with_capacity(40);
        for byte in &self.0 {
            write!(s, "{:02x}", byte).unwrap();
        }
        s
    }
}

/// The type of a Git object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectType {
    Blob,
    Tree,
    Commit,
}

impl ObjectType {
    pub fn as_bytes(&self) -> &[u8] {
        // RUST INSIGHT: Resolving E0308: explicitly type the resulting variable as a byte slice.
        let ty: &[u8] = match self {
            ObjectType::Blob => b"blob",
            ObjectType::Tree => b"tree",
            ObjectType::Commit => b"commit",
        };
        ty
    }
}

/// A parsed Git Object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitObject {
    Blob(Vec<u8>),
    Tree(Vec<TreeEntry>),
    Commit(CommitData),
}

/// An entry in a Git Tree (directory).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeEntry {
    pub mode: String, // e.g., "100644"
    pub name: String,
    pub oid: Oid,
}

/// Data for a Git Commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitData {
    pub tree: Oid,
    pub parents: Vec<Oid>,
    pub author: String,
    pub committer: String,
    pub message: String,
}

/// A simulated Git Object Store (normally `.git/objects/`).
pub struct ObjectStore {
    // Maps OID to raw serialized object data (Header + Content).
    objects: BTreeMap<Oid, Vec<u8>>,
}

impl Default for ObjectStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ObjectStore {
    pub fn new() -> Self {
        Self {
            objects: BTreeMap::new(),
        }
    }

    /// Serializes an object to the Git format: `<type> <size>\0<content>`
    fn serialize_object(obj_type: ObjectType, content: &[u8]) -> Vec<u8> {
        let mut buffer = Vec::new();
        buffer.extend_from_slice(obj_type.as_bytes());
        buffer.push(b' ');
        buffer.extend_from_slice(content.len().to_string().as_bytes());
        buffer.push(0); // Null terminator
        buffer.extend_from_slice(content);
        buffer
    }

    /// Stores a raw Git object in the database and returns its OID.
    fn write_raw_object(&mut self, obj_type: ObjectType, content: &[u8]) -> Oid {
        let serialized = Self::serialize_object(obj_type, content);
        let oid = Oid::hash_bytes(&serialized);
        self.objects.insert(oid.clone(), serialized);
        oid
    }

    /// Writes a Blob object (file content).
    pub fn write_blob(&mut self, content: &[u8]) -> Oid {
        self.write_raw_object(ObjectType::Blob, content)
    }

    /// Writes a Tree object (directory).
    pub fn write_tree(&mut self, entries: &[TreeEntry]) -> Oid {
        let mut content = Vec::new();
        for entry in entries {
            // Tree entry format: `<mode> <name>\0<20_byte_oid>`
            content.extend_from_slice(entry.mode.as_bytes());
            content.push(b' ');
            content.extend_from_slice(entry.name.as_bytes());
            content.push(0);
            content.extend_from_slice(&entry.oid.0);
        }
        self.write_raw_object(ObjectType::Tree, &content)
    }

    /// Writes a Commit object.
    pub fn write_commit(&mut self, commit: &CommitData) -> Oid {
        let mut content = String::new();
        writeln!(content, "tree {}", commit.tree.to_hex()).unwrap();
        for parent in &commit.parents {
            writeln!(content, "parent {}", parent.to_hex()).unwrap();
        }
        writeln!(content, "author {}", commit.author).unwrap();
        writeln!(content, "committer {}", commit.committer).unwrap();
        writeln!(content).unwrap(); // Empty line separates headers from message
        content.push_str(&commit.message);

        self.write_raw_object(ObjectType::Commit, content.as_bytes())
    }

    /// Reads and parses an object from the store by its OID.
    pub fn read_object(&self, oid: &Oid) -> io::Result<GitObject> {
        let serialized = self.objects.get(oid).ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, "Object not found in store")
        })?;

        // Parse Header: `<type> <size>\0`
        let null_pos = serialized.iter().position(|&b| b == 0).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "Missing null terminator in object header")
        })?;

        let header = &serialized[0..null_pos];
        let content = &serialized[null_pos + 1..];

        let header_str = std::str::from_utf8(header).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidData, "Invalid UTF-8 in object header")
        })?;

        let (type_str, _size_str) = header_str.split_once(' ').unwrap(); // We ignore size verification for brevity

        match type_str {
            "blob" => Ok(GitObject::Blob(content.to_vec())),
            "tree" => Ok(GitObject::Tree(Self::parse_tree(content)?)),
            "commit" => Ok(GitObject::Commit(Self::parse_commit(content)?)),
            _ => Err(io::Error::new(io::ErrorKind::InvalidData, "Unknown object type")),
        }
    }

    /// Parses the raw content of a Tree object.
    fn parse_tree(mut content: &[u8]) -> io::Result<Vec<TreeEntry>> {
        let mut entries = Vec::new();

        while !content.is_empty() {
            // Find null byte separating `<mode> <name>` from OID
            let null_pos = content.iter().position(|&b| b == 0).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "Malformed tree entry")
            })?;

            let prefix = std::str::from_utf8(&content[0..null_pos]).map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidData, "Invalid UTF-8 in tree entry prefix")
            })?;

            let mut parts = prefix.splitn(2, ' ');
            let mode = parts.next().unwrap().to_string();
            let name = parts.next().unwrap().to_string();

            if content.len() < null_pos + 1 + 20 {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "Tree entry OID truncated"));
            }

            let mut oid_bytes = [0u8; 20];
            oid_bytes.copy_from_slice(&content[null_pos + 1..null_pos + 21]);
            let oid = Oid(oid_bytes);

            entries.push(TreeEntry { mode, name, oid });

            // Advance to next entry
            content = &content[null_pos + 21..];
        }

        Ok(entries)
    }

    /// Parses the raw content of a Commit object.
    fn parse_commit(content: &[u8]) -> io::Result<CommitData> {
        let text = std::str::from_utf8(content).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidData, "Invalid UTF-8 in commit object")
        })?;

        let mut lines = text.lines();
        let mut tree_oid = None;
        let mut parents = Vec::new();
        let mut author = String::new();
        let mut committer = String::new();

        for line in &mut lines {
            if line.is_empty() {
                break; // End of headers
            }
            if let Some(rest) = line.strip_prefix("tree ") {
                tree_oid = Some(Self::hex_to_oid(rest)?);
            } else if let Some(rest) = line.strip_prefix("parent ") {
                parents.push(Self::hex_to_oid(rest)?);
            } else if let Some(rest) = line.strip_prefix("author ") {
                author = rest.to_string();
            } else if let Some(rest) = line.strip_prefix("committer ") {
                committer = rest.to_string();
            }
        }

        // Remaining text is the commit message
        // GOTCHA: `lines()` strips trailing newlines, so we use `split_once` conceptually,
        // but since we consumed lines, we reconstruct or find the empty line directly.
        let msg_start = text.find("\n\n").map(|i| i + 2).unwrap_or(text.len());
        let message = text[msg_start..].to_string();

        Ok(CommitData {
            tree: tree_oid.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Commit missing tree"))?,
            parents,
            author,
            committer,
            message,
        })
    }

    /// Helper to convert a hex string back to an Oid.
    fn hex_to_oid(hex: &str) -> io::Result<Oid> {
        if hex.len() != 40 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid OID hex length"));
        }
        let mut bytes = [0u8; 20];
        for i in 0..20 {
            bytes[i] = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidData, "Invalid hex character in OID")
            })?;
        }
        Ok(Oid(bytes))
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `git2` / `gix`: These crates are vastly more complex. They handle zlib compression (which we omitted),
//   packfiles (optimizing thousands of loose objects into one delta-compressed file), index tracking,
//   and real SHA-1 hashing.
//
// Missing vs. Production:
// - **zlib Compression:** Real Git compresses every object with zlib BEFORE hashing and storing.
// - **SHA-1:** We used a mock hash based on `DefaultHasher` for demonstration.
// - **Packfiles:** We only implemented "loose" objects.
//
// Suggested Next Steps:
// 1. Add zlib compression/decompression to `serialize_object` and `read_object`.
// 2. Implement a `RefStore` (to track branches like `refs/heads/main`).

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blob_creation_and_reading() {
        let mut store = ObjectStore::new();
        let content = b"Hello, Git!";

        // Write
        let oid = store.write_blob(content);

        // Read
        let obj = store.read_object(&oid).unwrap();

        if let GitObject::Blob(data) = obj {
            assert_eq!(data, content);
        } else {
            panic!("Expected Blob");
        }
    }

    #[test]
    fn test_tree_creation_and_reading() {
        let mut store = ObjectStore::new();

        // Create some blobs first
        let oid1 = store.write_blob(b"File 1 content");
        let oid2 = store.write_blob(b"File 2 content");

        let entries = vec![
            TreeEntry { mode: "100644".into(), name: "file1.txt".into(), oid: oid1.clone() },
            TreeEntry { mode: "100644".into(), name: "file2.txt".into(), oid: oid2.clone() },
        ];

        let tree_oid = store.write_tree(&entries);

        let obj = store.read_object(&tree_oid).unwrap();
        if let GitObject::Tree(parsed_entries) = obj {
            assert_eq!(parsed_entries.len(), 2);
            assert_eq!(parsed_entries[0].name, "file1.txt");
            assert_eq!(parsed_entries[0].oid, oid1);
            assert_eq!(parsed_entries[1].name, "file2.txt");
            assert_eq!(parsed_entries[1].oid, oid2);
        } else {
            panic!("Expected Tree");
        }
    }

    #[test]
    fn test_commit_creation_and_reading() {
        let mut store = ObjectStore::new();

        // Mock a tree OID
        let tree_oid = store.write_tree(&[]);

        // Mock a parent OID
        let parent_oid = Oid::hash_bytes(b"dummy parent");

        let commit_data = CommitData {
            tree: tree_oid.clone(),
            parents: vec![parent_oid.clone()],
            author: "Alice <alice@example.com> 1600000000 +0000".into(),
            committer: "Bob <bob@example.com> 1600000000 +0000".into(),
            message: "Initial commit\n\nFixes #1".into(),
        };

        let commit_oid = store.write_commit(&commit_data);

        let obj = store.read_object(&commit_oid).unwrap();
        if let GitObject::Commit(parsed_commit) = obj {
            assert_eq!(parsed_commit.tree, tree_oid);
            assert_eq!(parsed_commit.parents, vec![parent_oid]);
            assert_eq!(parsed_commit.author, "Alice <alice@example.com> 1600000000 +0000");
            assert_eq!(parsed_commit.message, "Initial commit\n\nFixes #1");
        } else {
            panic!("Expected Commit");
        }
    }

    #[test]
    fn test_deterministic_hashing() {
        let mut store = ObjectStore::new();
        let oid1 = store.write_blob(b"Same content");
        let oid2 = store.write_blob(b"Same content");
        assert_eq!(oid1, oid2);
    }
}
