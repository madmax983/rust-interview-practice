//! # WebSocket Protocol Implementation
//!
//! Implements the WebSocket Protocol (RFC 6455) from scratch.
//! Includes the opening handshake, frame framing/unframing, masking, and a minimal SHA-1 implementation.
//!
//! **Replaces Crates:** `tungstenite`, `tokio-tungstenite`, `websocket`
//!
//! **Real-world Usage:**
//! - Real-time chat applications.
//! - Live dashboard updates.
//! - Multiplayer games.
//!
//! **Why build it yourself?**
//! `WebSockets` bridge the gap between HTTP (request/response) and raw TCP (streams).
//! Implementing it teaches you about protocol upgrading, binary framing (handling bits, variable-length fields),
//! and masking requirements designed to prevent proxy cache poisoning.

use crate::serialization::base64;
use std::convert::TryInto;
use std::io::{self, BufRead, BufReader, Read, Write};

// =========================================================================================
// Architecture
// =========================================================================================
//
// Frame Format (RFC 6455):
//  0                   1                   2                   3
//  0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
// +-+-+-+-+-------+-+-------------+-------------------------------+
// |F|R|R|R| opcode|M| Payload len |    Extended payload length    |
// |I|S|S|S|  (4)  |A|     (7)     |             (16/64)           |
// |N|V|V|V|       |S|             |   (if payload len==126/127)   |
// | |1|2|3|       |K|             |                               |
// +-+-+-+-+-------+-+-------------+ - - - - - - - - - - - - - - - +
// |     Extended payload length continued, if payload len == 127  |
// + - - - - - - - - - - - - - - - +-------------------------------+
// |                               |Masking-key, if MASK set to 1  |
// +-------------------------------+-------------------------------+
// | Masking-key (continued)       |          Payload Data         |
// +-------------------------------- - - - - - - - - - - - - - - - +
// :                     Payload Data continued ...                :
// + - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - +
//
// Invariants:
// 1. Client-to-Server frames MUST be masked.
// 2. Server-to-Client frames MUST NOT be masked.
// 3. Control frames (Ping, Pong, Close) must have payload <= 125 bytes and cannot be fragmented.
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ Handshake     │ O(Header)   │ O(Header)   │
// ├───────────────┼─────────────┼─────────────┤
// │ Parse Frame   │ O(Payload)  │ O(Payload)  │
// ├───────────────┼─────────────┼─────────────┤
// │ Masking       │ O(Payload)  │ O(1)        │
// └───────────────┴─────────────┴─────────────┘

/// Maximum allowed payload length for a single frame (16 MiB).
/// Rejecting larger frames before allocation prevents an attacker-controlled
/// 64-bit length prefix from triggering a capacity-overflow panic or OOM abort.
const MAX_PAYLOAD: u64 = 16 * 1024 * 1024;

/// Per RFC 6455 §5.5, control frames (Close/Ping/Pong) MUST carry a payload of
/// at most 125 bytes and MUST NOT use the 126/127 extended-length encoding.
const MAX_CONTROL_PAYLOAD: u64 = 125;

/// Represents a WebSocket Message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    Text(String),
    Binary(Vec<u8>),
    Ping(Vec<u8>),
    Pong(Vec<u8>),
    Close,
}

/// Represents the type of a WebSocket frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Opcode {
    Continuation = 0x0,
    Text = 0x1,
    Binary = 0x2,
    Close = 0x8,
    Ping = 0x9,
    Pong = 0xA,
}

impl Opcode {
    const fn from_u8(byte: u8) -> Option<Self> {
        match byte {
            0x0 => Some(Self::Continuation),
            0x1 => Some(Self::Text),
            0x2 => Some(Self::Binary),
            0x8 => Some(Self::Close),
            0x9 => Some(Self::Ping),
            0xA => Some(Self::Pong),
            _ => None,
        }
    }

    /// Returns true for control frames (Close/Ping/Pong), which are subject to
    /// the RFC 6455 §5.5 payload limit of 125 bytes.
    const fn is_control(self) -> bool {
        matches!(self, Self::Close | Self::Ping | Self::Pong)
    }
}

/// A wrapper around a stream (e.g., `TcpStream`) that speaks WebSocket.
pub struct WebSocketConnection<S: Read + Write> {
    stream: BufReader<S>,
    is_server: bool,
}

impl<S: Read + Write> WebSocketConnection<S> {
    /// Creates a new WebSocket connection from an established stream.
    /// Assumes the handshake has already completed.
    pub fn new(stream: S, is_server: bool) -> Self {
        Self {
            stream: BufReader::new(stream),
            is_server,
        }
    }

    /// Performs the Server-side handshake.
    /// Reads the HTTP Upgrade request and sends the 101 Switching Protocols response.
    pub fn perform_server_handshake(stream: S) -> io::Result<Self> {
        // Use a buffered reader to parse HTTP headers
        let mut reader = BufReader::new(stream);
        let mut headers = Vec::new();

        loop {
            let mut line = String::new();
            // GOTCHA: read_line includes the newline.
            let bytes = reader.read_line(&mut line)?;
            if bytes == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "Stream closed during handshake",
                ));
            }
            if line == "\r\n" || line == "\n" {
                break;
            }
            headers.push(line.trim().to_string());
        }

        // Find Sec-WebSocket-Key
        let key_header = headers
            .iter()
            .find(|h| h.to_lowercase().starts_with("sec-websocket-key:"))
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "Missing Sec-WebSocket-Key")
            })?;

        let key = key_header.split(':').nth(1).unwrap_or("").trim();

        // Compute Accept Key
        let accept_key = generate_accept_key(key);

        // Send Response
        let response = format!(
            "HTTP/1.1 101 Switching Protocols\r\n\
             Upgrade: websocket\r\n\
             Connection: Upgrade\r\n\
             Sec-WebSocket-Accept: {accept_key}\r\n\r\n"
        );

        // Unwrap the stream from BufReader to write to it, then wrap back?
        // BufReader takes ownership of stream. `into_inner()` gives it back.
        let mut stream = reader.into_inner();
        stream.write_all(response.as_bytes())?;
        stream.flush()?;

        Ok(Self::new(stream, true))
    }

    /// Reads a single message from the connection.
    /// Handles fragmentation (not fully implemented here, assumes single frame messages for simplicity)
    /// and control frames.
    pub fn read_message(&mut self) -> io::Result<Message> {
        let (header, payload) = self.read_frame()?;

        match header.opcode {
            Opcode::Text => {
                let text = String::from_utf8(payload)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                Ok(Message::Text(text))
            }
            Opcode::Binary => Ok(Message::Binary(payload)),
            Opcode::Close => Ok(Message::Close),
            Opcode::Ping => Ok(Message::Ping(payload)),
            Opcode::Pong => Ok(Message::Pong(payload)),
            Opcode::Continuation => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Fragmentation not supported",
            )),
        }
    }

    /// Writes a message to the connection.
    pub fn write_message(&mut self, msg: Message) -> io::Result<()> {
        let (opcode, payload) = match msg {
            Message::Text(t) => (Opcode::Text, t.into_bytes()),
            Message::Binary(b) => (Opcode::Binary, b),
            Message::Close => (Opcode::Close, vec![]),
            Message::Ping(b) => (Opcode::Ping, b),
            Message::Pong(b) => (Opcode::Pong, b),
        };

        self.write_frame(opcode, &payload)
    }

    // RUST INSIGHT: Low-level Frame Parsing
    fn read_frame(&mut self) -> io::Result<(FrameHeader, Vec<u8>)> {
        let mut head = [0u8; 2];
        self.stream.read_exact(&mut head)?;

        let first_byte = head[0];
        let second_byte = head[1];

        let fin = (first_byte & 0x80) != 0;
        let opcode = Opcode::from_u8(first_byte & 0x0F)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Invalid Opcode"))?;

        let masked = (second_byte & 0x80) != 0;
        let mut payload_len = u64::from(second_byte & 0x7F);

        if payload_len == 126 {
            let mut len_bytes = [0u8; 2];
            self.stream.read_exact(&mut len_bytes)?;
            payload_len = u64::from(u16::from_be_bytes(len_bytes));
        } else if payload_len == 127 {
            let mut len_bytes = [0u8; 8];
            self.stream.read_exact(&mut len_bytes)?;
            payload_len = u64::from_be_bytes(len_bytes);
        }

        // RUST INSIGHT: Security - Masking
        // Server MUST receive masked frames. Client MUST receive unmasked frames.
        // We enforce this if we know our role.
        if self.is_server && !masked {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Client sent unmasked frame",
            ));
        }
        if !self.is_server && masked {
            // Technically allowed but unusual for server to mask. RFC says server must NOT mask.
        }

        let masking_key = if masked {
            let mut key = [0u8; 4];
            self.stream.read_exact(&mut key)?;
            Some(key)
        } else {
            None
        };

        // SECURITY: Bound the payload length *before* allocating. An attacker can
        // set the 127-marker and claim up to u64::MAX bytes; allocating that
        // eagerly panics (capacity overflow) or aborts (OOM). Reject oversized
        // frames, and enforce the RFC 6455 §5.5 limit for control frames.
        if opcode.is_control() && payload_len > MAX_CONTROL_PAYLOAD {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Control frame payload exceeds 125 bytes",
            ));
        }
        if payload_len > MAX_PAYLOAD {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Frame payload too large",
            ));
        }

        let mut payload = vec![0u8; payload_len as usize];
        self.stream.read_exact(&mut payload)?;

        if let Some(key) = masking_key {
            for (i, byte) in payload.iter_mut().enumerate() {
                *byte ^= key[i % 4];
            }
        }

        Ok((
            FrameHeader {
                fin,
                opcode,
                masked,
                payload_len,
            },
            payload,
        ))
    }

    fn write_frame(&mut self, opcode: Opcode, payload: &[u8]) -> io::Result<()> {
        let first_byte = 0x80 | (opcode as u8); // FIN set + Opcode

        let mut len_bytes = Vec::new();
        let payload_len_code;

        if payload.len() <= 125 {
            payload_len_code = payload.len() as u8;
        } else if payload.len() <= 65535 {
            payload_len_code = 126;
            len_bytes.extend_from_slice(&(payload.len() as u16).to_be_bytes());
        } else {
            payload_len_code = 127;
            len_bytes.extend_from_slice(&(payload.len() as u64).to_be_bytes());
        }

        // Masking
        // Client MUST mask. Server MUST NOT mask.
        // We'll simplify and say if we are server, we don't mask.
        // If we are client (not fully supported here but for completeness), we should.
        let mask_bit = if self.is_server { 0x00 } else { 0x80 };

        let second_byte = mask_bit | payload_len_code;

        // Use the underlying stream writer
        let stream = self.stream.get_mut();
        stream.write_all(&[first_byte, second_byte])?;
        stream.write_all(&len_bytes)?;

        if self.is_server {
            stream.write_all(payload)?;
        } else {
            // Generate random mask (dummy here, should be random)
            let mask_key = [1, 2, 3, 4];
            stream.write_all(&mask_key)?;

            let masked_payload: Vec<u8> = payload
                .iter()
                .enumerate()
                .map(|(i, b)| b ^ mask_key[i % 4])
                .collect();
            stream.write_all(&masked_payload)?;
        }

        stream.flush()?;
        Ok(())
    }
}

#[allow(dead_code)]
struct FrameHeader {
    fin: bool,
    opcode: Opcode,
    masked: bool,
    payload_len: u64,
}

// =========================================================================================
// SHA-1 Implementation (Minimal)
// =========================================================================================

fn generate_accept_key(key: &str) -> String {
    let guid = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
    let input = format!("{key}{guid}");
    let digest = sha1(input.as_bytes());
    base64::encode(digest)
}

// FIPS 180-1 SHA-1
fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h0 = 0x67452301u32;
    let mut h1 = 0xEFCDAB89u32;
    let mut h2 = 0x98BADCFEu32;
    let mut h3 = 0x10325476u32;
    let mut h4 = 0xC3D2E1F0u32;

    let mut message = data.to_vec();
    let original_len_bits = (data.len() as u64) * 8;

    // Padding
    message.push(0x80);
    while (message.len() * 8) % 512 != 448 {
        message.push(0x00);
    }
    message.extend_from_slice(&original_len_bits.to_be_bytes());

    for chunk in message.chunks(64) {
        let mut w = [0u32; 80];

        for (i, bytes) in chunk.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes(bytes.try_into().unwrap());
        }

        for i in 16..80 {
            let x = w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16];
            w[i] = x.rotate_left(1);
        }

        let mut a = h0;
        let mut b = h1;
        let mut c = h2;
        let mut d = h3;
        let mut e = h4;

        for i in 0..80 {
            let (f, k) = if i < 20 {
                ((b & c) | ((!b) & d), 0x5A827999)
            } else if i < 40 {
                (b ^ c ^ d, 0x6ED9EBA1)
            } else if i < 60 {
                ((b & c) | (b & d) | (c & d), 0x8F1BBCDC)
            } else {
                (b ^ c ^ d, 0xCA62C1D6)
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

    let mut result = [0u8; 20];
    result[0..4].copy_from_slice(&h0.to_be_bytes());
    result[4..8].copy_from_slice(&h1.to_be_bytes());
    result[8..12].copy_from_slice(&h2.to_be_bytes());
    result[12..16].copy_from_slice(&h3.to_be_bytes());
    result[16..20].copy_from_slice(&h4.to_be_bytes());
    result
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `tungstenite`: Handles fragmentation, control frame interleaving, and TLS (via native-tls/rustls).
//   Also has strict compliance checks (UTF-8 validation, reserved bits).
//
// Missing vs. Production:
// - **Fragmentation**: We assume one frame = one message. Real implementation must buffer fragments.
// - **Control Interleaving**: Control frames can appear in the middle of a fragmented message.
// - **UTF-8 Validation**: We just verify it parses, but strict UTF-8 checking is required for Text frames.
// - **Close Handshake**: We assume immediate close, real protocol has a close code and reason exchange.
//
// Next Steps:
// 1. Implement fragmentation support.
// 2. Add full Close handshake logic.

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_sha1() {
        // "abc"
        let digest = sha1(b"abc");
        let expected = [
            0xA9, 0x99, 0x3E, 0x36, 0x47, 0x06, 0x81, 0x6A, 0xBA, 0x3E, 0x25, 0x71, 0x78, 0x50,
            0xC2, 0x6C, 0x9C, 0xD0, 0xD8, 0x9D,
        ];
        assert_eq!(digest, expected);
    }

    #[test]
    fn test_accept_key_generation() {
        // RFC 6455 Example
        let key = "dGhlIHNhbXBsZSBub25jZQ==";
        let accept = generate_accept_key(key);
        assert_eq!(accept, "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
    }

    #[test]
    fn test_read_frame_unmasked_text() {
        // FIN=1, Text(1), Unmasked, Len=5, "Hello"
        let mut data = vec![
            0x81, // FIN + Text
            0x05, // Len 5
        ];
        data.extend_from_slice(b"Hello");

        let cursor = Cursor::new(data);
        let mut conn = WebSocketConnection::new(cursor, false); // acting as client reading server response

        let msg = conn.read_message().unwrap();
        match msg {
            Message::Text(s) => assert_eq!(s, "Hello"),
            _ => panic!("Expected text message"),
        }
    }

    #[test]
    fn test_read_frame_masked_binary() {
        // FIN=1, Binary(2), Masked, Len=4
        let mask = [0x10, 0x20, 0x30, 0x40];
        let payload = [0xAA, 0xBB, 0xCC, 0xDD];
        let masked_payload: Vec<u8> = payload
            .iter()
            .enumerate()
            .map(|(i, b)| b ^ mask[i % 4])
            .collect();

        let mut data = vec![
            0x82, // FIN + Binary
            0x84, // Masked + Len 4
        ];
        data.extend_from_slice(&mask);
        data.extend_from_slice(&masked_payload);

        let cursor = Cursor::new(data);
        let mut conn = WebSocketConnection::new(cursor, true); // acting as server reading client request

        let msg = conn.read_message().unwrap();
        match msg {
            Message::Binary(b) => assert_eq!(b, payload),
            _ => panic!("Expected binary message"),
        }
    }

    #[test]
    fn test_read_frame_rejects_huge_payload_len() {
        // FIN + Binary, masked (server role), 127-marker, u64::MAX length, 4-byte key.
        // Before the fix this triggered `vec![0u8; u64::MAX as usize]` -> panic/abort.
        let mut data = vec![
            0x82, // FIN + Binary
            0xFF, // Masked + 127 (extended 64-bit length)
        ];
        data.extend_from_slice(&u64::MAX.to_be_bytes());
        data.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // masking key

        let cursor = Cursor::new(data);
        let mut conn = WebSocketConnection::new(cursor, true);

        let err = conn.read_message().expect_err("oversized frame must error");
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn test_read_frame_rejects_oversized_control_frame() {
        // Ping (control frame) with unmasked payload length 126 (> 125 limit).
        // read as client (is_server=false) so the mask check is skipped.
        let data = vec![
            0x89, // FIN + Ping
            0x7E, // 126 -> 16-bit extended length follows
            0x00, 0x7E, // length = 126, exceeds control-frame max of 125
        ];

        let cursor = Cursor::new(data);
        let mut conn = WebSocketConnection::new(cursor, false);

        let err = conn
            .read_message()
            .expect_err("oversized control frame must error");
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn test_write_frame_server_to_client() {
        let mut buf = Vec::new();
        let cursor = Cursor::new(&mut buf);
        let mut conn = WebSocketConnection::new(cursor, true);

        conn.write_message(Message::Text("Hi".to_string())).unwrap();

        // Check buf
        // 0x81 (FIN+Text), 0x02 (Len 2), "Hi"
        assert_eq!(buf, vec![0x81, 0x02, b'H', b'i']);
    }

    #[test]
    fn test_handshake() {
        let input = b"GET /chat HTTP/1.1\r\n\
                      Host: server.example.com\r\n\
                      Upgrade: websocket\r\n\
                      Connection: Upgrade\r\n\
                      Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
                      Origin: http://example.com\r\n\
                      Sec-WebSocket-Protocol: chat, superchat\r\n\
                      Sec-WebSocket-Version: 13\r\n\r\n";

        let mut cursor = Cursor::new(Vec::new());
        cursor.write_all(input).unwrap();
        cursor.set_position(0);

        let _conn = WebSocketConnection::perform_server_handshake(cursor).unwrap();

        // Verify response in cursor (which was generic S)
        // Wait, `perform_server_handshake` consumes the stream into BufReader, then returns `WebSocketConnection`.
        // The `WebSocketConnection` holds `BufReader<S>`.
        // The underlying stream `S` (Cursor) has been written to.
        // We need to access the data written to the Cursor.

        // Since `conn` owns `stream`, we can't easily check `buf` if `buf` was moved?
        // Ah, `Cursor<Vec<u8>>` owns the Vec.
        // If I passed `Cursor<&mut Vec<u8>>`?
    }

    #[test]
    fn test_handshake_output() {
        let mut output = Vec::new();
        {
            let mut cursor = Cursor::new(&mut output);
            // Pre-populate input for read
            // But cursor is Read+Write. If I write input, position advances.
            // I need to write input, reset pos, then call handshake.
            cursor
                .write_all(b"GET / HTTP/1.1\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n")
                .unwrap();
            cursor.set_position(0);

            let _conn = WebSocketConnection::perform_server_handshake(cursor).unwrap();
        }
        // Check output. It should contain the original request (written to cursor) AND the response (appended).
        // Actually, `Cursor` works like a file. If I write at pos 0, I overwrite?
        // No, I wrote, pos is at end. `set_position(0)`.
        // Handshake reads from 0. `read_line` advances pos.
        // Then it writes response at current pos (end of headers).

        let s = String::from_utf8(output).unwrap();
        assert!(s.contains("HTTP/1.1 101 Switching Protocols"));
        assert!(s.contains("Sec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo="));
    }
}
