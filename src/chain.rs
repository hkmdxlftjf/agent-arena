//! Hash-chained match log.
//!
//! Each entry's `hash` is SHA-256 of the canonical JSON of the entry with
//! `hash` and `server_ack` removed. The server ack signs
//! `{match_id, seq, hash, payload_hash, received_at}` under `purpose: "ack"`.
//! `sign.mjs verify` checks this shape directly.

use crate::canon::canon;
use crate::crypto::{b64url_decode, domain, sha256_hex, zero_hash, ServerKey};
use crate::crypto::{now_ms, verify_value};
use serde_json::{json, Value};

#[derive(Clone)]
pub struct Chain {
    pub server: ServerKey,
    pub origin: String,
    pub entries: Vec<Value>,
    prev: String,
}

impl Chain {
    pub fn new(server: ServerKey, origin: impl Into<String>) -> Self {
        Self {
            server,
            origin: origin.into(),
            entries: Vec::new(),
            prev: zero_hash(),
        }
    }

    pub fn from_entries(
        server: ServerKey,
        origin: impl Into<String>,
        entries: Vec<Value>,
    ) -> anyhow::Result<Self> {
        let mut prev = zero_hash();
        for (i, entry) in entries.iter().enumerate() {
            let seq = entry
                .get("seq")
                .and_then(|v| v.as_i64())
                .ok_or_else(|| anyhow::anyhow!("entry {i} missing seq"))?;
            if seq != i as i64 {
                anyhow::bail!("entry {i} seq is {seq}");
            }
            let ph = entry
                .get("prev_hash")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if ph != prev {
                anyhow::bail!("entry {i} prev_hash break");
            }
            let mut body = entry.clone();
            if let Some(obj) = body.as_object_mut() {
                obj.remove("hash");
                obj.remove("server_ack");
            }
            let hash = sha256_hex(canon(&body).as_bytes());
            let stored = entry.get("hash").and_then(|v| v.as_str()).unwrap_or("");
            if stored != hash {
                anyhow::bail!("entry {i} hash mismatch");
            }
            prev = hash;
        }
        Ok(Self {
            server,
            origin: origin.into(),
            entries,
            prev,
        })
    }

    pub fn tip_seq(&self) -> Option<i64> {
        if self.entries.is_empty() {
            None
        } else {
            Some((self.entries.len() - 1) as i64)
        }
    }

    pub fn tip_hash(&self) -> &str {
        &self.prev
    }

    pub fn jsonl(&self) -> String {
        let mut out = String::new();
        for e in &self.entries {
            out.push_str(&serde_json::to_string(e).expect("entry json"));
            out.push('\n');
        }
        out
    }

    pub fn append_server(&mut self, kind: &str, purpose: &str, payload: Value) -> Value {
        let domain_v = domain(&self.origin, purpose);
        let signature = self.server.sign(&domain_v, &payload);
        let kid = self.server.kid.clone();
        self.finish(kind, "server", &kid, domain_v, payload, signature, now_ms())
    }

    pub fn append_agent(
        &mut self,
        domain_v: Value,
        payload: Value,
        kid: &str,
        signature: &str,
    ) -> Value {
        self.finish(
            "action",
            "agent",
            kid,
            domain_v,
            payload,
            signature.to_string(),
            now_ms(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn finish(
        &mut self,
        kind: &str,
        signer: &str,
        kid: &str,
        domain_v: Value,
        payload: Value,
        signature: String,
        received_at: i64,
    ) -> Value {
        let seq = self.entries.len() as i64;
        let body = json!({
            "alg": "ed25519",
            "domain": domain_v,
            "kid": kid,
            "kind": kind,
            "payload": payload,
            "prev_hash": self.prev,
            "received_at": received_at,
            "seq": seq,
            "signature": signature,
            "signer": signer,
        });
        let hash = sha256_hex(canon(&body).as_bytes());
        let payload_hash = sha256_hex(canon(&body["payload"]).as_bytes());
        let ack_payload = json!({
            "hash": hash,
            "match_id": body["payload"]["match_id"].clone(),
            "payload_hash": payload_hash,
            "received_at": received_at,
            "seq": seq,
        });
        let mut ack_domain = body["domain"].clone();
        if let Some(obj) = ack_domain.as_object_mut() {
            obj.insert("purpose".into(), json!("ack"));
        }
        let ack_sig = self.server.sign(&ack_domain, &ack_payload);
        let mut entry = body;
        entry["hash"] = json!(hash);
        entry["server_ack"] = json!({
            "kid": self.server.kid,
            "signature": ack_sig,
        });
        self.prev = hash;
        self.entries.push(entry.clone());
        entry
    }
}

pub fn payload_hash_hex(payload: &Value) -> String {
    sha256_hex(canon(payload).as_bytes())
}

pub fn verify_agent(public_b64: &str, domain_v: &Value, payload: &Value, sig: &str) -> bool {
    verify_value(public_b64, domain_v, payload, sig)
}

pub fn decode_sig(sig: &str) -> Vec<u8> {
    b64url_decode(sig).unwrap_or_default()
}

pub fn decode_hex(s: &str) -> Vec<u8> {
    hex::decode(s).unwrap_or_default()
}

pub fn hash_bytes(entry: &Value) -> Vec<u8> {
    decode_hex(entry["hash"].as_str().unwrap_or(""))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::verify_value;

    #[test]
    fn chain_links_and_acks() {
        let server = ServerKey::generate();
        let mut chain = Chain::new(server.clone(), "agent.idou.bid");
        let genesis = chain.append_server(
            "genesis",
            "genesis",
            json!({
                "match_id": "m-1",
                "game_id": "gomoku",
                "seats": [],
                "created_at": 1
            }),
        );
        assert_eq!(genesis["seq"], 0);
        assert_eq!(genesis["prev_hash"], "0".repeat(64));
        let again =
            Chain::from_entries(server.clone(), "agent.idou.bid", chain.entries.clone()).unwrap();
        assert_eq!(again.tip_hash(), chain.tip_hash());
        let ack = &genesis["server_ack"];
        let mut ack_domain = genesis["domain"].clone();
        ack_domain["purpose"] = json!("ack");
        let payload_hash = payload_hash_hex(&genesis["payload"]);
        let ack_payload = json!({
            "hash": genesis["hash"],
            "match_id": "m-1",
            "payload_hash": payload_hash,
            "received_at": genesis["received_at"],
            "seq": 0,
        });
        assert!(verify_value(
            &server.public_b64,
            &ack_domain,
            &ack_payload,
            ack["signature"].as_str().unwrap()
        ));
    }
}
