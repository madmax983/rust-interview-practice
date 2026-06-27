//! # RESP (REdis Serialization Protocol) Parser
//!
//! Implements a parser and serializer for RESP2/RESP3, the underlying protocol used by Redis.
//!
//! **Replaces Crates:** `redis` (protocol parsing logic)
//!
//! **Real-world Usage:**
//! - Redis clients and servers.
//! - Message brokers using Redis as a backend.
//! - Custom cache servers wanting to provide a Redis-compatible interface.
//!
//! **Why build it yourself?**
//! Text-based, stream-oriented protocols like RESP are everywhere (HTTP/1.1, Memcached, Redis).
//! Building this teaches you how to handle network streams where messages might be fragmented,
//! how to parse prefix-length arrays recursively, and how to represent heterogeneous data (strings,
//! integers, nested arrays) in a unified Rust enum safely without excessive memory copying.

// =========================================================================================
// Architecture
// =========================================================================================
//
// Protocol Format:
// All components are terminated by \r\n (CRLF).
//
// + : Simple String (e.g., +OK\r\n)
// - : Error         (e.g., -Error message\r\n)
// : : Integer       (e.g., :1000\r\n)
// $ : Bulk String   (e.g., $6\r\nfoobar\r\n). Null string is $-1\r\n
// * : Array         (e.g., *2\r\n$3\r\nfoo\r\n$3\r\nbar\r\n). Null array is *-1\r\n
//
// Data Structure:
// `RespValue` enum recursively represents these types.
//
// Invariants:
// 1. Parsing must be capable of failing gracefully if the buffer doesn't contain a full message yet (Incomplete).
// 2. Output serialization must always strictly terminate lines with \r\n.
//
// Complexity:
// ┌───────────────┬────────┬────────┐
// │ Operation     │ Time   │ Space  │
// ├───────────────┼────────┼────────┤
// │ Parse         │ O(N)   │ O(N)   │
// │ Serialize     │ O(N)   │ O(N)   │
// └───────────────┴────────┴────────┘
// N is the length of the byte stream.
//
// Design Decisions:
// - **Allocation**: We allocate `String` and `Vec` for simplicity. A high-performance, zero-allocation
//   parser would yield `&[u8]` references tied to the lifetime of the input buffer.
// - **Incomplete Data**: We return a custom error `Incomplete` to allow the network loop to read more
//   data and try again, standard for non-blocking I/O.

use std::str;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum RespValue {
    SimpleString(String),
    Error(String),
    Integer(i64),
    BulkString(Option<Vec<u8>>),   // None represents Null bulk string
    Array(Option<Vec<RespValue>>), // None represents Null array
}

#[derive(Debug, PartialEq, Eq)]
pub enum RespError {
    Incomplete,
    InvalidProtocol(String),
}

impl RespValue {
    /// Serializes the value into a RESP byte stream.
    pub fn serialize(&self, buf: &mut Vec<u8>) {
        // RUST INSIGHT: Matching over enums is exhaustive and fast.
        // We match recursively here to serialize complex structures like Arrays.
        match self {
            RespValue::SimpleString(s) => {
                buf.push(b'+');
                buf.extend_from_slice(s.as_bytes());
                buf.extend_from_slice(b"\r\n");
            }
            RespValue::Error(err) => {
                buf.push(b'-');
                buf.extend_from_slice(err.as_bytes());
                buf.extend_from_slice(b"\r\n");
            }
            RespValue::Integer(i) => {
                buf.push(b':');
                // ⚡ BOLT OPTIMIZATION: Use `write!` directly to `Vec<u8>` to avoid intermediate `String` allocation overhead.
                use std::io::Write;
                write!(buf, "{}", i).unwrap();
                buf.extend_from_slice(b"\r\n");
            }
            RespValue::BulkString(Some(data)) => {
                buf.push(b'$');
                // ⚡ BOLT OPTIMIZATION: Use `write!` directly to `Vec<u8>` to avoid intermediate `String` allocation overhead.
                use std::io::Write;
                write!(buf, "{}", data.len()).unwrap();
                buf.extend_from_slice(b"\r\n");
                buf.extend_from_slice(data);
                buf.extend_from_slice(b"\r\n");
            }
            RespValue::BulkString(None) => {
                buf.extend_from_slice(b"$-1\r\n");
            }
            RespValue::Array(Some(arr)) => {
                buf.push(b'*');
                // ⚡ BOLT OPTIMIZATION: Use `write!` directly to `Vec<u8>` to avoid intermediate `String` allocation overhead.
                use std::io::Write;
                write!(buf, "{}", arr.len()).unwrap();
                buf.extend_from_slice(b"\r\n");
                for item in arr {
                    item.serialize(buf);
                }
            }
            RespValue::Array(None) => {
                buf.extend_from_slice(b"*-1\r\n");
            }
        }
    }

    /// Attempts to parse a RESP value from the given buffer.
    /// Returns `Ok((value, bytes_consumed))` on success.
    pub fn parse(buf: &[u8]) -> Result<(Self, usize), RespError> {
        // GOTCHA: Always handle incomplete or empty buffers gracefully without panicking.
        if buf.is_empty() {
            return Err(RespError::Incomplete);
        }

        match buf[0] {
            b'+' => Self::parse_simple_string(buf),
            b'-' => Self::parse_error(buf),
            b':' => Self::parse_integer(buf),
            b'$' => Self::parse_bulk_string(buf),
            b'*' => Self::parse_array(buf),
            _ => Err(RespError::InvalidProtocol(format!(
                "Unknown data type byte: {}",
                buf[0]
            ))),
        }
    }

    /// Finds the index of the next \r\n. Returns `Ok((line_content, total_bytes_to_skip))`.
    fn read_line(buf: &[u8]) -> Result<(&[u8], usize), RespError> {
        for i in 0..buf.len() - 1 {
            if buf[i] == b'\r' && buf[i + 1] == b'\n' {
                return Ok((&buf[..i], i + 2));
            }
        }
        Err(RespError::Incomplete)
    }

    fn parse_simple_string(buf: &[u8]) -> Result<(Self, usize), RespError> {
        let (line, consumed) = Self::read_line(&buf[1..])?;
        let s = str::from_utf8(line)
            .map_err(|_| RespError::InvalidProtocol("Invalid UTF-8 in Simple String".into()))?
            .to_string();
        Ok((RespValue::SimpleString(s), consumed + 1))
    }

    fn parse_error(buf: &[u8]) -> Result<(Self, usize), RespError> {
        let (line, consumed) = Self::read_line(&buf[1..])?;
        let s = str::from_utf8(line)
            .map_err(|_| RespError::InvalidProtocol("Invalid UTF-8 in Error".into()))?
            .to_string();
        Ok((RespValue::Error(s), consumed + 1))
    }

    fn parse_integer(buf: &[u8]) -> Result<(Self, usize), RespError> {
        let (line, consumed) = Self::read_line(&buf[1..])?;
        // PRODUCTION NOTE: In a high performance implementation, parsing integers directly
        // from ASCII bytes without building a UTF-8 string is faster.
        let s = str::from_utf8(line)
            .map_err(|_| RespError::InvalidProtocol("Invalid UTF-8 in Integer".into()))?;
        let val = s
            .parse::<i64>()
            .map_err(|_| RespError::InvalidProtocol("Failed to parse integer".into()))?;
        Ok((RespValue::Integer(val), consumed + 1))
    }

    fn parse_bulk_string(buf: &[u8]) -> Result<(Self, usize), RespError> {
        let (line, consumed) = Self::read_line(&buf[1..])?;
        let s = str::from_utf8(line).map_err(|_| {
            RespError::InvalidProtocol("Invalid UTF-8 in Bulk String length".into())
        })?;
        let len = s
            .parse::<i64>()
            .map_err(|_| RespError::InvalidProtocol("Failed to parse bulk string length".into()))?;

        if len == -1 {
            return Ok((RespValue::BulkString(None), consumed + 1));
        }

        if len < 0 {
            return Err(RespError::InvalidProtocol(
                "Negative bulk string length (other than -1)".into(),
            ));
        }

        let len = len as usize;
        let data_start = 1 + consumed;
        let data_end = data_start + len;

        // We need data + \r\n
        if buf.len() < data_end + 2 {
            return Err(RespError::Incomplete);
        }

        if buf[data_end] != b'\r' || buf[data_end + 1] != b'\n' {
            return Err(RespError::InvalidProtocol(
                "Bulk string data not terminated by CRLF".into(),
            ));
        }

        let data = buf[data_start..data_end].to_vec();
        Ok((RespValue::BulkString(Some(data)), data_end + 2))
    }

    fn parse_array(buf: &[u8]) -> Result<(Self, usize), RespError> {
        let (line, consumed) = Self::read_line(&buf[1..])?;
        let s = str::from_utf8(line)
            .map_err(|_| RespError::InvalidProtocol("Invalid UTF-8 in Array length".into()))?;
        let len = s
            .parse::<i64>()
            .map_err(|_| RespError::InvalidProtocol("Failed to parse array length".into()))?;

        if len == -1 {
            return Ok((RespValue::Array(None), consumed + 1));
        }

        if len < 0 {
            return Err(RespError::InvalidProtocol(
                "Negative array length (other than -1)".into(),
            ));
        }

        let mut offset = 1 + consumed;
        let mut elements = Vec::with_capacity(len as usize);

        for _ in 0..len {
            let (val, read) = Self::parse(&buf[offset..])?;
            elements.push(val);
            offset += read;
        }

        Ok((RespValue::Array(Some(elements)), offset))
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `redis`: The official rust-redis crate implements this protocol alongside a connection manager,
//   connection pooling, cluster support, and async I/O.
//
// Missing vs. Production:
// - **Zero-Copy**: A real parser uses `&'a [u8]` or `bytes::Bytes` to avoid allocating new `Vec`s
//   for every bulk string.
// - **Streaming Reader**: Real implementations wrap a TCP socket and manage a ring buffer to handle
//   `Incomplete` reads efficiently without copying data backwards.
//
// Next Steps:
// 1. Refactor `RespValue` to use `&[u8]` (borrowed strings/bytes) to eliminate allocations.
// 2. Build a simple Redis clone server listening on TCP and executing `GET`/`SET` commands.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_simple_string() {
        let val = RespValue::SimpleString("OK".to_string());
        let mut buf = Vec::new();
        val.serialize(&mut buf);
        assert_eq!(buf, b"+OK\r\n");
    }

    #[test]
    fn test_parse_simple_string() {
        let (val, read) = RespValue::parse(b"+OK\r\n").unwrap();
        assert_eq!(val, RespValue::SimpleString("OK".to_string()));
        assert_eq!(read, 5);
    }

    #[test]
    fn test_serialize_integer() {
        let val = RespValue::Integer(1000);
        let mut buf = Vec::new();
        val.serialize(&mut buf);
        assert_eq!(buf, b":1000\r\n");
    }

    #[test]
    fn test_parse_integer() {
        let (val, read) = RespValue::parse(b":1000\r\n").unwrap();
        assert_eq!(val, RespValue::Integer(1000));
        assert_eq!(read, 7);
    }

    #[test]
    fn test_serialize_bulk_string() {
        let val = RespValue::BulkString(Some(b"foobar".to_vec()));
        let mut buf = Vec::new();
        val.serialize(&mut buf);
        assert_eq!(buf, b"$6\r\nfoobar\r\n");
    }

    #[test]
    fn test_parse_bulk_string() {
        let (val, read) = RespValue::parse(b"$6\r\nfoobar\r\n").unwrap();
        assert_eq!(val, RespValue::BulkString(Some(b"foobar".to_vec())));
        assert_eq!(read, 12);
    }

    #[test]
    fn test_parse_null_bulk_string() {
        let (val, read) = RespValue::parse(b"$-1\r\n").unwrap();
        assert_eq!(val, RespValue::BulkString(None));
        assert_eq!(read, 5);
    }

    #[test]
    fn test_serialize_array() {
        let val = RespValue::Array(Some(vec![
            RespValue::BulkString(Some(b"foo".to_vec())),
            RespValue::BulkString(Some(b"bar".to_vec())),
        ]));
        let mut buf = Vec::new();
        val.serialize(&mut buf);
        assert_eq!(buf, b"*2\r\n$3\r\nfoo\r\n$3\r\nbar\r\n");
    }

    #[test]
    fn test_parse_array() {
        let (val, read) = RespValue::parse(b"*2\r\n$3\r\nfoo\r\n$3\r\nbar\r\n").unwrap();
        assert_eq!(
            val,
            RespValue::Array(Some(vec![
                RespValue::BulkString(Some(b"foo".to_vec())),
                RespValue::BulkString(Some(b"bar".to_vec())),
            ]))
        );
        assert_eq!(read, 22);
    }

    #[test]
    fn test_parse_incomplete() {
        // Missing CRLF
        let res = RespValue::parse(b"+OK");
        assert_eq!(res, Err(RespError::Incomplete));

        // Missing bulk string data
        let res = RespValue::parse(b"$6\r\nfoo");
        assert_eq!(res, Err(RespError::Incomplete));

        // Missing array elements
        let res = RespValue::parse(b"*2\r\n$3\r\nfoo\r\n");
        assert_eq!(res, Err(RespError::Incomplete));
    }
}

// Benchmarking Note:
// To benchmark `RespValue::parse` vs the `redis` crate parser, use Criterion.
// Provide a long, complex RESP array buffer and benchmark parsing it.
// Ensure you wrap inputs and outputs in `std::hint::black_box`.
// Example:
// ```rust
// pub fn criterion_benchmark(c: &mut Criterion) {
//     let data = b"*2\r\n$3\r\nfoo\r\n$3\r\nbar\r\n";
//     c.bench_function("resp_parse_array", |b| b.iter(|| {
//         std::hint::black_box(RespValue::parse(std::hint::black_box(data)).unwrap());
//     }));
// }
// ```
