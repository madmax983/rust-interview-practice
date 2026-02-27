//! # Simple DNS Resolver Implementation
//!
//! Implements a basic DNS resolver that can query a remote DNS server (e.g., 8.8.8.8) over UDP.
//! It handles packet construction, bit-level header parsing, and recursive label decompression.
//!
//! **Replaces Crates:** `trust-dns`, `dns-parser`
//!
//! **Real-world Usage:**
//! - `systemd-resolved` (local DNS stub resolver).
//! - `dnsmasq` (lightweight DNS forwarder).
//! - Browser DNS cache (Chrome/Firefox internal resolvers).
//!
//! **Why build it yourself?**
//! DNS is the backbone of the internet, but its packet format is surprisingly complex (bit flags,
//! big-endian fields, pointer-based compression). Implementing it teaches you:
//! 1. **Binary Protocol Parsing**: Handling raw bytes, endianness, and bit masking.
//! 2. **Recursion in Data Structures**: Handling label pointers (`0xC0`) requires jumping around the buffer.
//! 3. **UDP Networking**: Managing connectionless sockets and reliability (retries/timeouts - though simplified here).

use crate::systems::ttl_cache::TTLCache;
use std::net::{Ipv4Addr, UdpSocket};
use std::time::Duration;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Packet Format (RFC 1035):
//
//      +---------------------+
//      |        Header       |
//      +---------------------+
//      |       Question      | the question for the name server
//      +---------------------+
//      |        Answer       | RRs answering the question
//      +---------------------+
//      |      Authority      | RRs pointing to an authority
//      +---------------------+
//      |      Additional     | RRs holding additional information
//      +---------------------+
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ Parse Packet  │ O(N)        │ O(N)        │
// │ Resolve       │ O(Network)  │ O(1)        │
// └───────────────┴─────────────┴─────────────┘
//
// Design Decisions:
// - **Buffer**: Fixed 512-byte buffer (standard UDP DNS limit).
//   - *Tradeoff*: Simple, stack-allocated (or small heap), no dynamic resizing.
//   - *Limitation*: Can't handle EDNS (Extension Mechanisms for DNS) or large responses (TCP fallback needed for >512 bytes).
// - **Recursion**: Used for label decompression.
//   - *GOTCHA*: Malformed packets can cause infinite recursion loops (pointer to self). We must limit jump depth.

/// Max size of a UDP DNS packet.
const MAX_PACKET_SIZE: usize = 512;

/// A buffer to hold the raw DNS packet bytes.
pub struct BytePacketBuffer {
    pub buf: [u8; MAX_PACKET_SIZE],
    pub pos: usize,
    pub valid_len: usize,
}

/// The result code (RCODE) from the DNS header.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ResultCode {
    NOERROR = 0,
    FORMERR = 1,
    SERVFAIL = 2,
    NXDOMAIN = 3,
    NOTIMP = 4,
    REFUSED = 5,
}

/// The DNS Header.
#[derive(Clone, Debug)]
pub struct DnsHeader {
    pub id: u16, // 16 bits

    // Flags
    pub recursion_desired: bool,    // 1 bit
    pub truncated_message: bool,    // 1 bit
    pub authoritative_answer: bool, // 1 bit
    pub opcode: u8,                 // 4 bits
    pub response: bool,             // 1 bit

    pub rescode: ResultCode,       // 4 bits
    pub checking_disabled: bool,   // 1 bit
    pub authed_data: bool,         // 1 bit
    pub z: bool,                   // 1 bit
    pub recursion_available: bool, // 1 bit

    pub questions: u16,             // 16 bits
    pub answers: u16,               // 16 bits
    pub authoritative_entries: u16, // 16 bits
    pub resource_entries: u16,      // 16 bits
}

/// A DNS Question.
#[derive(Debug, Clone)]
pub struct DnsQuestion {
    pub name: String,
    pub qtype: QueryType,
}

/// A DNS Resource Record.
#[derive(Debug, Clone)]
pub enum DnsRecord {
    UNKNOWN {
        domain: String,
        qtype: u16,
        data_len: u16,
        ttl: u32,
        data: Vec<u8>,
    }, // 0
    A {
        domain: String,
        addr: Ipv4Addr,
        ttl: u32,
    }, // 1
       // We only implement A records for simplicity in this exercise
}

/// Query Type.
#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum QueryType {
    UNKNOWN(u16),
    A, // 1
}

/// The full DNS Packet.
#[derive(Clone, Debug)]
pub struct DnsPacket {
    pub header: DnsHeader,
    pub questions: Vec<DnsQuestion>,
    pub answers: Vec<DnsRecord>,
    pub authorities: Vec<DnsRecord>,
    pub resources: Vec<DnsRecord>,
}

impl BytePacketBuffer {
    pub fn new() -> Self {
        Self {
            buf: [0; MAX_PACKET_SIZE],
            pos: 0,
            valid_len: 0,
        }
    }

    pub fn pos(&self) -> usize {
        self.pos
    }

    pub fn set_valid_len(&mut self, len: usize) {
        self.valid_len = len;
    }

    pub fn step(&mut self, steps: usize) -> Result<(), &'static str> {
        self.pos += steps;
        Ok(())
    }

    pub fn seek(&mut self, pos: usize) -> Result<(), &'static str> {
        self.pos = pos;
        Ok(())
    }

    pub fn read(&mut self) -> Result<u8, &'static str> {
        if self.pos >= self.valid_len {
            return Err("End of buffer");
        }
        let res = self.buf[self.pos];
        self.pos += 1;
        Ok(res)
    }

    pub fn get(&mut self, pos: usize) -> Result<u8, &'static str> {
        if pos >= self.valid_len {
            return Err("End of buffer");
        }
        Ok(self.buf[pos])
    }

    pub fn get_range(&mut self, start: usize, len: usize) -> Result<&[u8], &'static str> {
        if start + len > self.valid_len {
            return Err("End of buffer");
        }
        Ok(&self.buf[start..start + len])
    }

    pub fn read_u16(&mut self) -> Result<u16, &'static str> {
        let res = ((self.read()? as u16) << 8) | (self.read()? as u16);
        Ok(res)
    }

    pub fn read_u32(&mut self) -> Result<u32, &'static str> {
        let res = ((self.read()? as u32) << 24)
            | ((self.read()? as u32) << 16)
            | ((self.read()? as u32) << 8)
            | (self.read()? as u32);
        Ok(res)
    }

    /// Read a domain name from the buffer, handling compression.
    ///
    /// The DNS protocol uses a compression scheme where a label can be a pointer
    /// to a previous occurrence of the same name. A pointer is identified by the
    /// two high bits being set (0xC0).
    pub fn read_qname(&mut self, outstr: &mut String) -> Result<(), &'static str> {
        let mut pos = self.pos;
        let mut jumped = false;
        let mut jumps_performed = 0;
        let max_jumps = 5; // Protect against loops

        let mut delimiter = "";
        loop {
            if jumps_performed > max_jumps {
                return Err("Limit of jumps exceeded");
            }

            let len = self.get(pos)?;

            // If len has two high bits set (0xC0), it's a pointer.
            if (len & 0xC0) == 0xC0 {
                // Update the buffer position to the point after the pointer
                // only if we haven't jumped yet. If we have jumped, we don't
                // update the buffer position because we want to return to where
                // we were before the jump.
                if !jumped {
                    self.seek(pos + 2)?;
                }

                let b2 = self.get(pos + 1)? as u16;
                let offset = (((len as u16) ^ 0xC0) << 8) | b2;
                pos = offset as usize;
                jumped = true;
                jumps_performed += 1;
                continue;
            }

            // Normal label
            pos += 1;

            if len == 0 {
                break;
            }

            outstr.push_str(delimiter);

            let str_buffer = self.get_range(pos, len as usize)?;
            outstr.push_str(&String::from_utf8_lossy(str_buffer).to_lowercase());

            delimiter = ".";
            pos += len as usize;
        }

        if !jumped {
            self.seek(pos)?;
        }

        Ok(())
    }

    pub fn write(&mut self, val: u8) -> Result<(), &'static str> {
        if self.pos >= MAX_PACKET_SIZE {
            return Err("End of buffer");
        }
        self.buf[self.pos] = val;
        self.pos += 1;
        if self.pos > self.valid_len {
            self.valid_len = self.pos;
        }
        Ok(())
    }

    pub fn write_u8(&mut self, val: u8) -> Result<(), &'static str> {
        self.write(val)
    }

    pub fn write_u16(&mut self, val: u16) -> Result<(), &'static str> {
        self.write((val >> 8) as u8)?;
        self.write((val & 0xFF) as u8)?;
        Ok(())
    }

    pub fn write_u32(&mut self, val: u32) -> Result<(), &'static str> {
        self.write(((val >> 24) & 0xFF) as u8)?;
        self.write(((val >> 16) & 0xFF) as u8)?;
        self.write(((val >> 8) & 0xFF) as u8)?;
        self.write((val & 0xFF) as u8)?;
        Ok(())
    }
}

impl Default for BytePacketBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl ResultCode {
    pub fn from_num(num: u8) -> ResultCode {
        match num {
            1 => ResultCode::FORMERR,
            2 => ResultCode::SERVFAIL,
            3 => ResultCode::NXDOMAIN,
            4 => ResultCode::NOTIMP,
            5 => ResultCode::REFUSED,
            0 | _ => ResultCode::NOERROR,
        }
    }
}

impl DnsHeader {
    pub fn new() -> Self {
        Self {
            id: 0,
            recursion_desired: false,
            truncated_message: false,
            authoritative_answer: false,
            opcode: 0,
            response: false,
            rescode: ResultCode::NOERROR,
            checking_disabled: false,
            authed_data: false,
            z: false,
            recursion_available: false,
            questions: 0,
            answers: 0,
            authoritative_entries: 0,
            resource_entries: 0,
        }
    }

    pub fn read(&mut self, buffer: &mut BytePacketBuffer) -> Result<(), &'static str> {
        self.id = buffer.read_u16()?;

        let flags = buffer.read_u16()?;
        let a = (flags >> 8) as u8;
        let b = (flags & 0xFF) as u8;

        self.recursion_desired = (a & (1 << 0)) > 0;
        self.truncated_message = (a & (1 << 1)) > 0;
        self.authoritative_answer = (a & (1 << 2)) > 0;
        self.opcode = (a >> 3) & 0x0F;
        self.response = (a & (1 << 7)) > 0;

        self.rescode = ResultCode::from_num(b & 0x0F);
        self.checking_disabled = (b & (1 << 4)) > 0;
        self.authed_data = (b & (1 << 5)) > 0;
        self.z = (b & (1 << 6)) > 0;
        self.recursion_available = (b & (1 << 7)) > 0;

        self.questions = buffer.read_u16()?;
        self.answers = buffer.read_u16()?;
        self.authoritative_entries = buffer.read_u16()?;
        self.resource_entries = buffer.read_u16()?;

        Ok(())
    }

    pub fn write(&self, buffer: &mut BytePacketBuffer) -> Result<(), &'static str> {
        buffer.write_u16(self.id)?;

        let mut a = 0u8;
        if self.recursion_desired {
            a |= 1 << 0;
        }
        if self.truncated_message {
            a |= 1 << 1;
        }
        if self.authoritative_answer {
            a |= 1 << 2;
        }
        a |= self.opcode << 3;
        if self.response {
            a |= 1 << 7;
        }

        let mut b = 0u8;
        b |= self.rescode as u8;
        if self.checking_disabled {
            b |= 1 << 4;
        }
        if self.authed_data {
            b |= 1 << 5;
        }
        if self.z {
            b |= 1 << 6;
        }
        if self.recursion_available {
            b |= 1 << 7;
        }

        buffer.write(a)?;
        buffer.write(b)?;

        buffer.write_u16(self.questions)?;
        buffer.write_u16(self.answers)?;
        buffer.write_u16(self.authoritative_entries)?;
        buffer.write_u16(self.resource_entries)?;

        Ok(())
    }
}

impl Default for DnsHeader {
    fn default() -> Self {
        Self::new()
    }
}

impl QueryType {
    pub fn to_num(&self) -> u16 {
        match *self {
            QueryType::UNKNOWN(x) => x,
            QueryType::A => 1,
        }
    }

    pub fn from_num(num: u16) -> QueryType {
        match num {
            1 => QueryType::A,
            _ => QueryType::UNKNOWN(num),
        }
    }
}

impl DnsQuestion {
    pub fn new(name: String, qtype: QueryType) -> Self {
        Self { name, qtype }
    }

    pub fn read(&mut self, buffer: &mut BytePacketBuffer) -> Result<(), &'static str> {
        buffer.read_qname(&mut self.name)?;
        self.qtype = QueryType::from_num(buffer.read_u16()?);
        let _ = buffer.read_u16()?; // class
        Ok(())
    }

    pub fn write(&self, buffer: &mut BytePacketBuffer) -> Result<(), &'static str> {
        for label in self.name.split('.') {
            let len = label.len();
            if len == 0 {
                continue;
            }
            if len > 63 {
                return Err("Label exceeds 63 characters");
            }
            buffer.write_u8(len as u8)?;
            for b in label.as_bytes() {
                buffer.write_u8(*b)?;
            }
        }
        buffer.write_u8(0)?; // End of labels

        buffer.write_u16(self.qtype.to_num())?;
        buffer.write_u16(1)?; // class IN
        Ok(())
    }
}

impl DnsRecord {
    pub fn read(buffer: &mut BytePacketBuffer) -> Result<DnsRecord, &'static str> {
        let mut domain = String::new();
        buffer.read_qname(&mut domain)?;

        let qtype_num = buffer.read_u16()?;
        let qtype = QueryType::from_num(qtype_num);
        let _ = buffer.read_u16()?; // class
        let ttl = buffer.read_u32()?;
        let data_len = buffer.read_u16()?;

        match qtype {
            QueryType::A => {
                let raw_addr = buffer.read_u32()?;
                let addr = Ipv4Addr::new(
                    ((raw_addr >> 24) & 0xFF) as u8,
                    ((raw_addr >> 16) & 0xFF) as u8,
                    ((raw_addr >> 8) & 0xFF) as u8,
                    (raw_addr & 0xFF) as u8,
                );
                Ok(DnsRecord::A { domain, addr, ttl })
            }
            QueryType::UNKNOWN(_) => {
                let mut data = Vec::with_capacity(data_len as usize);
                for _ in 0..data_len {
                    data.push(buffer.read()?);
                }
                Ok(DnsRecord::UNKNOWN {
                    domain,
                    qtype: qtype_num,
                    data_len,
                    ttl,
                    data,
                })
            }
        }
    }

    pub fn write(&self, buffer: &mut BytePacketBuffer) -> Result<usize, &'static str> {
        let start_pos = buffer.pos();

        match *self {
            DnsRecord::A {
                ref domain,
                ref addr,
                ttl,
            } => {
                // Name
                for label in domain.split('.') {
                    let len = label.len();
                    if len == 0 {
                        continue;
                    }
                    if len > 63 {
                        return Err("Label exceeds 63 characters");
                    }
                    buffer.write_u8(len as u8)?;
                    for b in label.as_bytes() {
                        buffer.write_u8(*b)?;
                    }
                }
                buffer.write_u8(0)?;

                buffer.write_u16(QueryType::A.to_num())?;
                buffer.write_u16(1)?; // Class IN
                buffer.write_u32(ttl)?;
                buffer.write_u16(4)?; // Data len for IPv4

                let octets = addr.octets();
                buffer.write_u8(octets[0])?;
                buffer.write_u8(octets[1])?;
                buffer.write_u8(octets[2])?;
                buffer.write_u8(octets[3])?;
            }
            DnsRecord::UNKNOWN {
                ref domain,
                qtype,
                data_len,
                ttl,
                ref data,
            } => {
                for label in domain.split('.') {
                    let len = label.len();
                    if len == 0 {
                        continue;
                    }
                    if len > 63 {
                        return Err("Label exceeds 63 characters");
                    }
                    buffer.write_u8(len as u8)?;
                    for b in label.as_bytes() {
                        buffer.write_u8(*b)?;
                    }
                }
                buffer.write_u8(0)?;

                buffer.write_u16(qtype)?;
                buffer.write_u16(1)?;
                buffer.write_u32(ttl)?;
                buffer.write_u16(data_len)?;
                for b in data {
                    buffer.write_u8(*b)?;
                }
            }
        }

        Ok(buffer.pos() - start_pos)
    }
}

impl DnsPacket {
    pub fn new() -> Self {
        Self {
            header: DnsHeader::new(),
            questions: Vec::new(),
            answers: Vec::new(),
            authorities: Vec::new(),
            resources: Vec::new(),
        }
    }

    pub fn from_buffer(buffer: &mut BytePacketBuffer) -> Result<Self, &'static str> {
        let mut result = DnsPacket::new();
        result.header.read(buffer)?;

        for _ in 0..result.header.questions {
            let mut question = DnsQuestion::new("".to_string(), QueryType::UNKNOWN(0));
            question.read(buffer)?;
            result.questions.push(question);
        }

        for _ in 0..result.header.answers {
            let rec = DnsRecord::read(buffer)?;
            result.answers.push(rec);
        }

        for _ in 0..result.header.authoritative_entries {
            let rec = DnsRecord::read(buffer)?;
            result.authorities.push(rec);
        }

        for _ in 0..result.header.resource_entries {
            let rec = DnsRecord::read(buffer)?;
            result.resources.push(rec);
        }

        Ok(result)
    }

    pub fn write(&mut self, buffer: &mut BytePacketBuffer) -> Result<(), &'static str> {
        self.header.questions = self.questions.len() as u16;
        self.header.answers = self.answers.len() as u16;
        self.header.authoritative_entries = self.authorities.len() as u16;
        self.header.resource_entries = self.resources.len() as u16;

        self.header.write(buffer)?;

        for question in &self.questions {
            question.write(buffer)?;
        }

        for rec in &self.answers {
            rec.write(buffer)?;
        }

        for rec in &self.authorities {
            rec.write(buffer)?;
        }

        for rec in &self.resources {
            rec.write(buffer)?;
        }

        Ok(())
    }
}

impl Default for DnsPacket {
    fn default() -> Self {
        Self::new()
    }
}

/// A Simple DNS Resolver.
pub struct DnsResolver {
    server: (String, u16),
    cache: TTLCache<String, Ipv4Addr>,
}

impl DnsResolver {
    /// Creates a new DNS Resolver pointing to the specified server (e.g., ("8.8.8.8", 53)).
    pub fn new(server_ip: &str, server_port: u16) -> Self {
        Self {
            server: (server_ip.to_string(), server_port),
            // Cache valid for 5 minutes (standard-ish default)
            cache: TTLCache::new(Duration::from_secs(300)),
        }
    }

    /// Resolves a domain name to an IPv4 address.
    pub fn resolve(&mut self, qname: &str) -> Result<Ipv4Addr, String> {
        // 1. Check cache
        if let Some(addr) = self.cache.get(&qname.to_string()) {
            return Ok(*addr);
        }

        // 2. Prepare query
        let mut packet = DnsPacket::new();
        packet.header.id = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos()
            & 0xFFFF) as u16;
        packet.header.questions = 1;
        packet.header.recursion_desired = true;
        packet
            .questions
            .push(DnsQuestion::new(qname.to_string(), QueryType::A));

        // 3. Serialize query
        let mut req_buffer = BytePacketBuffer::new();
        packet.write(&mut req_buffer).map_err(|e| e.to_string())?;

        // 4. Send query
        let socket = UdpSocket::bind(("0.0.0.0", 0)).map_err(|e| e.to_string())?;
        socket
            .set_read_timeout(Some(Duration::from_secs(2)))
            .map_err(|e| e.to_string())?;

        socket
            .send_to(
                &req_buffer.buf[0..req_buffer.pos],
                (&self.server.0 as &str, self.server.1),
            )
            .map_err(|e| e.to_string())?;

        // 5. Receive response
        let mut res_buffer = BytePacketBuffer::new();
        let (len, _) = socket
            .recv_from(&mut res_buffer.buf)
            .map_err(|e| e.to_string())?;
        res_buffer.set_valid_len(len);

        // 6. Parse response
        let res_packet = DnsPacket::from_buffer(&mut res_buffer).map_err(|e| e.to_string())?;

        // 7. Extract A record
        for answer in res_packet.answers {
            if let DnsRecord::A {
                domain: _,
                addr,
                ttl: _,
            } = answer
            {
                // Update cache with actual TTL if we could adjust it per item,
                // but our TTLCache has global TTL.
                // We'll just cache it.
                // PRODUCTION NOTE: A real resolver would respect the record's TTL.
                // Our simple TTLCache uses a global TTL.
                self.cache.put(qname.to_string(), addr);
                return Ok(addr);
            }
        }

        Err("No A record found".to_string())
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `trust-dns`: Fully async (Tokio), supports DNSSEC, recursive resolution, caching, etc. Huge codebase.
// - `dns-parser`: Just parses packets, similar to this implementation but without the logic to send/receive.
//
// Missing vs. Production:
// - **Retries**: We try once and fail if timeout.
// - **EDNS**: We don't support Extension Mechanisms for DNS (needed for DNSSEC and large packets).
// - **TCP Fallback**: If the response > 512 bytes (truncated bit set), we should retry over TCP.
// - **IPv6**: We only support A records (IPv4). AAAA records are needed for IPv6.
// - **Security**: No DNSSEC validation. Vulnerable to cache poisoning (ID guessing) though we randomize ID slightly.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_read_write() {
        let mut buf = BytePacketBuffer::new();
        buf.write_u8(1).unwrap();
        buf.write_u16(2).unwrap();
        buf.write_u32(3).unwrap();

        buf.seek(0).unwrap();
        assert_eq!(buf.read().unwrap(), 1);
        assert_eq!(buf.read_u16().unwrap(), 2);
        assert_eq!(buf.read_u32().unwrap(), 3);
    }

    #[test]
    fn test_dns_header_serialization() {
        let mut header = DnsHeader::new();
        header.id = 1234;
        header.recursion_desired = true;

        let mut buf = BytePacketBuffer::new();
        header.write(&mut buf).unwrap();

        buf.seek(0).unwrap();
        let mut header2 = DnsHeader::new();
        header2.read(&mut buf).unwrap();

        assert_eq!(header.id, header2.id);
        assert_eq!(header.recursion_desired, header2.recursion_desired);
    }

    #[test]
    fn test_qname_read_write() {
        let mut buf = BytePacketBuffer::new();
        let name = "www.google.com";
        // Write manually to test encoding
        // 3 www 6 google 3 com 0
        buf.write_u8(3).unwrap();
        for b in b"www" {
            buf.write_u8(*b).unwrap();
        }
        buf.write_u8(6).unwrap();
        for b in b"google" {
            buf.write_u8(*b).unwrap();
        }
        buf.write_u8(3).unwrap();
        for b in b"com" {
            buf.write_u8(*b).unwrap();
        }
        buf.write_u8(0).unwrap();

        buf.seek(0).unwrap();
        let mut out = String::new();
        buf.read_qname(&mut out).unwrap();
        assert_eq!(out, name);
    }

    #[test]
    fn test_qname_compression() {
        // Packet:
        // [Pos 0] 3 www 6 google 3 com 0
        // [Pos X] 11000000 (Pointer to 0) => C0 00
        let mut buf = BytePacketBuffer::new();

        let name_pos = buf.pos();
        // Write "www.google.com"
        buf.write_u8(3).unwrap();
        for b in b"www" {
            buf.write_u8(*b).unwrap();
        }
        buf.write_u8(6).unwrap();
        for b in b"google" {
            buf.write_u8(*b).unwrap();
        }
        buf.write_u8(3).unwrap();
        for b in b"com" {
            buf.write_u8(*b).unwrap();
        }
        buf.write_u8(0).unwrap();

        // Write pointer to it
        // 0xC000 | name_pos
        let ptr = 0xC000 | (name_pos as u16);
        buf.write_u16(ptr).unwrap();

        // Read the pointer
        buf.seek(buf.pos() - 2).unwrap(); // Go back to pointer
        let mut out = String::new();
        buf.read_qname(&mut out).unwrap();
        assert_eq!(out, "www.google.com");
    }

    #[test]
    fn test_packet_end_to_end() {
        let mut packet = DnsPacket::new();
        packet.header.id = 55;
        packet
            .questions
            .push(DnsQuestion::new("test.com".to_string(), QueryType::A));
        packet.answers.push(DnsRecord::A {
            domain: "test.com".to_string(),
            addr: Ipv4Addr::new(1, 2, 3, 4),
            ttl: 100,
        });

        let mut buf = BytePacketBuffer::new();
        packet.write(&mut buf).unwrap();

        buf.seek(0).unwrap();
        let packet2 = DnsPacket::from_buffer(&mut buf).unwrap();

        assert_eq!(packet2.header.id, 55);
        assert_eq!(packet2.questions.len(), 1);
        assert_eq!(packet2.questions[0].name, "test.com");
        assert_eq!(packet2.answers.len(), 1);
        if let DnsRecord::A { domain, addr, .. } = &packet2.answers[0] {
            assert_eq!(domain, "test.com");
            assert_eq!(*addr, Ipv4Addr::new(1, 2, 3, 4));
        } else {
            panic!("Wrong record type");
        }
    }

    #[test]
    fn test_trailing_dot_serialization() {
        let q = DnsQuestion::new("google.com.".to_string(), QueryType::A);
        let mut buf = BytePacketBuffer::new();
        q.write(&mut buf).unwrap();

        // Check if it's same as google.com
        let q2 = DnsQuestion::new("google.com".to_string(), QueryType::A);
        let mut buf2 = BytePacketBuffer::new();
        q2.write(&mut buf2).unwrap();

        assert_eq!(buf.pos, buf2.pos);
        assert_eq!(buf.buf[0..buf.pos], buf2.buf[0..buf2.pos]);
    }

    #[test]
    fn test_unknown_record_preservation() {
        let mut buf = BytePacketBuffer::new();
        // Construct a fake UNKNOWN record (TYPE=999)
        // Name: test.com
        // 4 test 3 com 0
        buf.write_u8(4).unwrap();
        for b in b"test" {
            buf.write_u8(*b).unwrap();
        }
        buf.write_u8(3).unwrap();
        for b in b"com" {
            buf.write_u8(*b).unwrap();
        }
        buf.write_u8(0).unwrap();

        buf.write_u16(999).unwrap(); // TYPE
        buf.write_u16(1).unwrap(); // CLASS
        buf.write_u32(100).unwrap(); // TTL
        buf.write_u16(4).unwrap(); // Data Len
        buf.write_u8(1).unwrap(); // Data
        buf.write_u8(2).unwrap();
        buf.write_u8(3).unwrap();
        buf.write_u8(4).unwrap();

        let end_pos = buf.pos();
        buf.seek(0).unwrap();
        buf.set_valid_len(end_pos); // Must set valid len for reading!
        let rec = DnsRecord::read(&mut buf).unwrap();

        if let DnsRecord::UNKNOWN {
            qtype, ref data, ..
        } = rec
        {
            assert_eq!(qtype, 999);
            assert_eq!(data, &vec![1, 2, 3, 4]);

            // Now write it back
            let mut buf2 = BytePacketBuffer::new();
            rec.write(&mut buf2).unwrap();

            assert_eq!(buf.pos, buf2.pos);
            assert_eq!(buf.buf[0..buf.pos], buf2.buf[0..buf2.pos]);
        } else {
            panic!("Expected UNKNOWN record");
        }
    }
}
