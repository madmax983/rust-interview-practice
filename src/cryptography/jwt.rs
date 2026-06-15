//! # JSON Web Token (JWT) Implementation
//!
//! A foundational implementation of generating and verifying JSON Web Tokens
//! using HMAC-SHA256 (HS256) signatures and base64url encoding.
//!
//! **Replaces Crates:** `jsonwebtoken`, `biscuit`
//!
//! **Real-world Usage:**
//! - Stateless authentication for web applications and APIs.
//! - Single Sign-On (SSO) systems (OIDC).
//! - Passing signed, verifiable claims between microservices.
//!
//! **Why build it yourself?**
//! JWTs appear magical, but they are just three base64-encoded JSON strings concatenated with dots.
//! Building it from scratch teaches you about cryptographic signing (preventing tampering) versus
//! encryption (hiding data), how base64url differs from standard base64, and the importance of
//! constant-time comparison to prevent timing attacks during signature verification.

use crate::cryptography::sha256;
use crate::serialization::base64;
use std::time::{SystemTime, UNIX_EPOCH};

// =========================================================================================
// Architecture
// =========================================================================================
//
// Token Format:
//
//      base64url(Header) . base64url(Payload) . base64url(Signature)
//
//      Header: {"alg":"HS256","typ":"JWT"}
//      Payload: {"sub":"1234567890","name":"John Doe","iat":1516239022}
//      Signature: HMAC-SHA256(Secret, Header . "." . Payload)
//
// Invariants:
// 1. A token without a valid signature must be rejected.
// 2. An expired token (`exp` claim < current time) must be rejected.
// 3. Signature comparison MUST be constant-time to prevent timing attacks.
//
// Complexity:
// ┌───────────────┬──────────────┬─────────────┐
// │ Operation     │ Time         │ Space       │
// ├───────────────┼──────────────┼─────────────┤
// │ Sign          │ O(N)         │ O(N)        │
// │ Verify        │ O(N)         │ O(N)        │
// └───────────────┴──────────────┴─────────────┘
// N is the length of the JSON string.
//
// Design Decisions:
// - **Algorithm**: We only support `HS256` (HMAC with SHA-256) for simplicity.
//   - *Tradeoff*: Real crates support RSA/ECDSA which require complex asymmetric cryptography libraries.
// - **JSON Serialization**: We use raw string formatting and parsing for educational purposes to avoid `serde_json` dependency,
//   though a production system would definitely use `serde_json`.

/// JWT Header (Simplified)
#[derive(Debug, PartialEq)]
pub struct Header {
    pub alg: String,
    pub typ: String,
}

impl Header {
    fn new() -> Self {
        Self {
            alg: "HS256".to_string(),
            typ: "JWT".to_string(),
        }
    }

    // A very naive JSON serialization.
    fn to_json(&self) -> String {
        use std::fmt::Write;
        let mut json = String::with_capacity(64);
        let _ = write!(json, r#"{{"alg":"{}","typ":"{}"}}"#, self.alg, self.typ);
        json
    }
}

/// JWT Payload
#[derive(Debug, PartialEq, Clone)]
pub struct Claims {
    pub sub: String,
    pub exp: Option<u64>,
    pub iat: Option<u64>,
    // In reality, there are many more standard claims and custom claims.
}

impl Claims {
    pub fn new(sub: String, exp: Option<u64>, iat: Option<u64>) -> Self {
        Self { sub, exp, iat }
    }

    fn to_json(&self) -> String {
        use std::fmt::Write;
        let mut json = String::with_capacity(128);
        let _ = write!(json, r#"{{"sub":"{}""#, self.sub);
        if let Some(exp) = self.exp {
            let _ = write!(json, r#","exp":{}"#, exp);
        }
        if let Some(iat) = self.iat {
            let _ = write!(json, r#","iat":{}"#, iat);
        }
        json.push('}');
        json
    }

    fn from_json(json: &str) -> Option<Self> {
        // Very naive parser. Production uses `serde_json`.
        let clean = json.trim().trim_matches(|c| c == '{' || c == '}');
        let mut sub = String::new();
        let mut exp = None;
        let mut iat = None;

        for part in clean.split(',') {
            let mut kv = part.splitn(2, ':');
            let key = kv.next()?.trim().trim_matches('"');
            let val = kv.next()?.trim();

            match key {
                "sub" => sub = val.trim_matches('"').to_string(),
                "exp" => exp = val.parse().ok(),
                "iat" => iat = val.parse().ok(),
                _ => {}
            }
        }

        if sub.is_empty() {
            return None;
        }

        Some(Self { sub, exp, iat })
    }
}

/// Constant-time string comparison to prevent timing attacks.
///
/// # RUST INSIGHT: Security
/// Standard `==` on strings returns early if characters don't match. An attacker
/// can measure the time it takes to verify a signature to guess characters one by one.
/// Constant-time comparison ensures we check every byte regardless of mismatches.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }

    let mut result = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }
    result == 0
}

/// A rudimentary HMAC implementation using our custom SHA-256.
///
/// HMAC(K, m) = H((K ^ opad) || H((K ^ ipad) || m))
fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    let block_size = 64; // SHA-256 block size is 64 bytes

    // 1. Standardize key length
    let mut k = [0u8; 64];
    if key.len() > block_size {
        // Hash it
        let mut hasher = sha256::Sha256::new();
        hasher.update(key);
        let hashed = hasher.finalize();
        k[..32].copy_from_slice(&hashed);
    } else {
        k[..key.len()].copy_from_slice(key);
    }

    // 2. Prepare inner and outer pads
    let mut o_key_pad = [0x5c; 64];
    let mut i_key_pad = [0x36; 64];

    for i in 0..block_size {
        o_key_pad[i] ^= k[i];
        i_key_pad[i] ^= k[i];
    }

    // 3. Inner hash: H((K ^ ipad) || m)
    let mut inner_hasher = sha256::Sha256::new();
    inner_hasher.update(&i_key_pad);
    inner_hasher.update(message);
    let inner_hash = inner_hasher.finalize();

    // 4. Outer hash: H((K ^ opad) || inner_hash)
    let mut outer_hasher = sha256::Sha256::new();
    outer_hasher.update(&o_key_pad);
    outer_hasher.update(&inner_hash);
    outer_hasher.finalize()
}

/// URL-safe Base64 encoder (replaces '+' with '-', '/' with '_', removes '=' padding).
fn base64url_encode(data: &[u8]) -> String {
    let standard = base64::encode(data);
    standard
        .replace('+', "-")
        .replace('/', "_")
        .replace('=', "")
}

/// URL-safe Base64 decoder.
fn base64url_decode(data: &str) -> Option<Vec<u8>> {
    let mut standard = data.replace('-', "+").replace('_', "/");
    // Add padding back if necessary
    match standard.len() % 4 {
        2 => standard.push_str("=="),
        3 => standard.push('='),
        _ => {}
    }
    base64::decode(&standard).ok()
}

/// A trait defining the capabilities of a JWT handler.
/// Demonstrates how to abstract the signing algorithm and validation logic.
pub trait JwtHandler {
    fn encode(&self, claims: &Claims) -> String;
    fn decode(&self, token: &str) -> Result<Claims, &'static str>;
}

/// A standard HS256 JWT implementation.
pub struct Hs256Jwt {
    // GOTCHA: Secrets should be treated with care. In a real system, you'd
    // likely use a secure wrapper type (e.g. `secrecy::SecretVec`) to prevent
    // accidental logging.
    secret: Vec<u8>,
}

impl Hs256Jwt {
    pub fn new(secret: &[u8]) -> Self {
        Self {
            secret: secret.to_vec(),
        }
    }
}

impl JwtHandler for Hs256Jwt {
    /// Encodes and signs a JWT.
    fn encode(&self, claims: &Claims) -> String {
        let header = Header::new();

        // PRODUCTION NOTE: Canonical crates use `serde_json` to serialize claims
        // to handle arbitrary JSON payloads robustly. Our simple toy uses manual serialization.
        let b64_header = base64url_encode(header.to_json().as_bytes());
        let b64_payload = base64url_encode(claims.to_json().as_bytes());

        let mut token = String::with_capacity(b64_header.len() + b64_payload.len() + 100);
        token.push_str(&b64_header);
        token.push('.');
        token.push_str(&b64_payload);

        let signature = hmac_sha256(&self.secret, token.as_bytes());
        let b64_signature = base64url_encode(&signature);

        token.push('.');
        token.push_str(&b64_signature);
        token
    }

    /// Decodes and verifies a JWT.
    fn decode(&self, token: &str) -> Result<Claims, &'static str> {
        let mut parts = token.split('.');
        let b64_header = parts.next().ok_or("Invalid token format")?;
        let b64_payload = parts.next().ok_or("Invalid token format")?;
        let b64_signature = parts.next().ok_or("Invalid token format")?;
        if parts.next().is_some() {
            return Err("Invalid token format");
        }

        // 1. Verify Signature
        let mut message = String::with_capacity(b64_header.len() + b64_payload.len() + 1);
        message.push_str(b64_header);
        message.push('.');
        message.push_str(b64_payload);

        let expected_sig = hmac_sha256(&self.secret, message.as_bytes());
        let expected_b64_sig = base64url_encode(&expected_sig);

        if !constant_time_eq(b64_signature.as_bytes(), expected_b64_sig.as_bytes()) {
            return Err("Invalid signature");
        }

        // 2. Decode Payload
        let payload_bytes = base64url_decode(b64_payload).ok_or("Failed to decode payload")?;
        let payload_str =
            String::from_utf8(payload_bytes).map_err(|_| "Payload is not valid UTF-8")?;

        let claims = Claims::from_json(&payload_str).ok_or("Failed to parse claims JSON")?;

        // 3. Verify Expiration
        if let Some(exp) = claims.exp {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            if now >= exp {
                return Err("Token expired");
            }
        }

        Ok(claims)
    }
}

// =========================================================================================
// Footer
// =========================================================================================
//
// Comparison to Canonical Crates:
// - `jsonwebtoken`: Supports dynamic algorithms (HS256, RS256, ES256, etc.) using `ring` or `openssl`,
//   fully integrates with `serde` for typed custom claims, handles standard claims validation completely
//   (like `iss`, `aud`, `nbf`), and provides leeway options for clock skew.
//
// Missing vs. Production:
// - **Algorithms**: We only implement HS256. Production systems often use RS256 (asymmetric) so the verification
//   key can be public.
// - **`serde_json` Integration**: Our JSON parser is a fragile toy meant for learning; real crates use robust parsers.
// - **Complete Claim Validation**: We only validate `exp`. Real libraries allow strictly validating `iss` (issuer),
//   `aud` (audience), and `nbf` (not before).
//
// Next Steps:
// 1. Replace the manual JSON serialization with `serde` bounds (e.g., `<T: Serialize + Deserialize>`).
// 2. Add RS256 support (requires parsing PEM certificates and using RSA crates).
//
// Benchmarking Note:
// Use `criterion` to benchmark the throughput of `encode` and `decode` operations.
// The performance bottleneck in HS256 JWT is typically the JSON parsing/serialization and
// base64 encoding/decoding, rather than the HMAC-SHA256 operations themselves.
// Pass varying payload sizes to evaluate serialization overhead vs cryptographic overhead.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_and_decode() {
        let secret = b"my_super_secret_key";
        let handler = Hs256Jwt::new(secret);
        let claims = Claims::new("user_123".to_string(), None, None);

        let token = handler.encode(&claims);
        assert_eq!(token.split('.').count(), 3);

        let decoded = handler.decode(&token).unwrap();
        assert_eq!(decoded.sub, "user_123");
    }

    #[test]
    fn test_invalid_signature() {
        let secret = b"my_super_secret_key";
        let handler = Hs256Jwt::new(secret);
        let claims = Claims::new("user_123".to_string(), None, None);

        let token = handler.encode(&claims);
        let bad_handler = Hs256Jwt::new(b"wrong_key");

        let result = bad_handler.decode(&token);
        assert_eq!(result, Err("Invalid signature"));
    }

    #[test]
    fn test_tampered_payload() {
        let secret = b"my_super_secret_key";
        let handler = Hs256Jwt::new(secret);
        let claims = Claims::new("user_123".to_string(), None, None);

        let token = handler.encode(&claims);
        let mut parts: Vec<&str> = token.split('.').collect();

        // Tamper with payload
        let malicious_claims = Claims::new("admin_user".to_string(), None, None);
        let malicious_b64 = base64url_encode(malicious_claims.to_json().as_bytes());
        parts[1] = &malicious_b64;

        let tampered_token = parts.join(".");

        let result = handler.decode(&tampered_token);
        assert_eq!(result, Err("Invalid signature"));
    }

    #[test]
    fn test_expired_token() {
        let secret = b"my_super_secret_key";
        let handler = Hs256Jwt::new(secret);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Expired 10 seconds ago
        let claims = Claims::new("user_123".to_string(), Some(now - 10), None);

        let token = handler.encode(&claims);

        let result = handler.decode(&token);
        assert_eq!(result, Err("Token expired"));
    }

    #[test]
    fn test_base64url() {
        let data = b"hello+world/foo=?bar";
        let encoded = base64url_encode(data);
        assert!(!encoded.contains('+'));
        assert!(!encoded.contains('/'));
        assert!(!encoded.contains('='));

        let decoded = base64url_decode(&encoded).unwrap();
        assert_eq!(decoded, data);
    }
}
