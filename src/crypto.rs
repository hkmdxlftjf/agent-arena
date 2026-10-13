//! Ed25519 signatures in the exact envelope `sign.mjs` produces.
//!
//! signing_input = "\x19AgentArena Signed Message:\n" || SHA-256(JCS(domain)) || SHA-256(JCS(payload))

use crate::canon::canon;
use base64::engine::general_purpose::{URL_SAFE, URL_SAFE_NO_PAD};
use base64::Engine;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::Path;

pub const ORIGIN: &str = "arena.idou.top";
pub const PREFIX: &[u8] = b"\x19AgentArena Signed Message:\n";

pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(sha256(bytes))
}

/// `sign.mjs` commitment: hex(SHA-256("arena-commit-v1" || utf8(canon(data)) || salt_bytes)).
pub fn commitment_of(data: &Value, salt_hex: &str) -> Option<String> {
    if salt_hex.is_empty() || salt_hex.len() % 2 != 0 || !salt_hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let salt = hex::decode(salt_hex).ok()?;
    let canonical = canon(data);
    let mut hasher = Sha256::new();
    hasher.update(b"arena-commit-v1");
    hasher.update(canonical.as_bytes());
    hasher.update(&salt);
    Some(hex::encode(hasher.finalize()))
}

pub fn b64url_encode(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

pub fn b64url_decode(s: &str) -> Result<Vec<u8>, ()> {
    URL_SAFE_NO_PAD
        .decode(s)
        .or_else(|_| URL_SAFE.decode(s))
        .map_err(|_| ())
}

/// RFC 4648 base32, lowercase, no padding. Same bit loop as `sign.mjs`.
pub fn base32_lower(buf: &[u8]) -> String {
    const A: &[u8] = b"abcdefghijklmnopqrstuvwxyz234567";
    let mut bits: u32 = 0;
    let mut val: u32 = 0;
    let mut s = String::new();
    for &byte in buf {
        val = (val << 8) | byte as u32;
        bits += 8;
        while bits >= 5 {
            s.push(A[((val >> (bits - 5)) & 31) as usize] as char);
            bits -= 5;
        }
        if bits == 0 {
            val = 0;
        } else {
            val &= (1u32 << bits) - 1;
        }
    }
    if bits > 0 {
        s.push(A[((val << (5 - bits)) & 31) as usize] as char);
    }
    s
}

pub fn kid_of(raw_pub: &[u8], prefix: &str) -> String {
    let digest = sha256(raw_pub);
    let b32 = base32_lower(&digest);
    format!("{prefix}_{}", &b32[..20])
}

pub fn domain(purpose: &str) -> Value {
    serde_json::json!({
        "chainless": true,
        "name": "AgentArena",
        "origin": ORIGIN,
        "purpose": purpose,
        "version": "1"
    })
}

pub fn signing_input(domain_v: &Value, payload: &Value) -> Vec<u8> {
    let mut out = PREFIX.to_vec();
    out.extend(sha256(canon(domain_v).as_bytes()));
    out.extend(sha256(canon(payload).as_bytes()));
    out
}

pub fn sign_value(sk: &SigningKey, domain_v: &Value, payload: &Value) -> String {
    let sig = sk.sign(&signing_input(domain_v, payload));
    b64url_encode(&sig.to_bytes())
}

pub fn verify_value(public_b64: &str, domain_v: &Value, payload: &Value, sig_b64: &str) -> bool {
    let Ok(raw) = b64url_decode(public_b64) else {
        return false;
    };
    let Ok(raw32) = <[u8; 32]>::try_from(raw.as_slice()) else {
        return false;
    };
    let Ok(vk) = VerifyingKey::from_bytes(&raw32) else {
        return false;
    };
    let Ok(sig_bytes) = b64url_decode(sig_b64) else {
        return false;
    };
    let Ok(sig) = Signature::from_slice(&sig_bytes) else {
        return false;
    };
    vk.verify(&signing_input(domain_v, payload), &sig).is_ok()
}

pub fn domain_ok(d: &Value, purpose: &str) -> bool {
    d.get("name").and_then(|v| v.as_str()) == Some("AgentArena")
        && d.get("version").and_then(|v| v.as_str()) == Some("1")
        && d.get("origin").and_then(|v| v.as_str()) == Some(ORIGIN)
        && d.get("purpose").and_then(|v| v.as_str()) == Some(purpose)
        && d.get("chainless") == Some(&Value::Bool(true))
}

#[derive(Clone)]
pub struct ServerKey {
    pub kid: String,
    pub public_b64: String,
    pub public_raw: [u8; 32],
    signing: SigningKey,
}

impl ServerKey {
    pub fn generate() -> Self {
        let signing = SigningKey::generate(&mut OsRng);
        Self::from_signing(signing)
    }

    fn from_signing(signing: SigningKey) -> Self {
        let public_raw = signing.verifying_key().to_bytes();
        let public_b64 = b64url_encode(&public_raw);
        let kid = kid_of(&public_raw, "sk");
        Self {
            kid,
            public_b64,
            public_raw,
            signing,
        }
    }

    pub fn load_or_create(dir: &Path) -> anyhow::Result<Self> {
        let keys = dir.join("keys");
        std::fs::create_dir_all(&keys)?;
        let path = keys.join("server.ed25519");
        if path.exists() {
            let bytes = std::fs::read(&path)?;
            let raw: [u8; 32] = bytes
                .as_slice()
                .try_into()
                .map_err(|_| anyhow::anyhow!("server key at {} is not 32 bytes", path.display()))?;
            Ok(Self::from_signing(SigningKey::from_bytes(&raw)))
        } else {
            let key = Self::generate();
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                let mut opts = std::fs::OpenOptions::new();
                opts.write(true).create_new(true).mode(0o600);
                use std::io::Write;
                opts.open(&path)?.write_all(&key.signing.to_bytes())?;
            }
            #[cfg(not(unix))]
            {
                std::fs::write(&path, key.signing.to_bytes())?;
            }
            Ok(key)
        }
    }

    pub fn sign(&self, domain_v: &Value, payload: &Value) -> String {
        sign_value(&self.signing, domain_v, payload)
    }

    pub fn verifying_key(&self) -> VerifyingKey {
        self.signing.verifying_key()
    }
}

pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

pub fn json_i64(v: &Value) -> Option<i64> {
    v.as_i64()
        .or_else(|| v.as_u64().and_then(|u| i64::try_from(u).ok()))
}

pub fn zero_hash() -> String {
    "0".repeat(64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn sign_then_verify() {
        let sk = SigningKey::generate(&mut OsRng);
        let key = ServerKey::from_signing(sk);
        let d = domain("move");
        let p = json!({"match_id": "m", "seat": 0, "seq": 1});
        let sig = key.sign(&d, &p);
        assert!(verify_value(&key.public_b64, &d, &p, &sig));
        let mut tampered = p.clone();
        tampered["seat"] = json!(1);
        assert!(!verify_value(&key.public_b64, &d, &tampered, &sig));
    }

    #[test]
    fn kid_is_stable_length() {
        let raw = [7u8; 32];
        let kid = kid_of(&raw, "ak");
        assert!(kid.starts_with("ak_"));
        assert_eq!(kid.len(), 3 + 20);
    }
}
