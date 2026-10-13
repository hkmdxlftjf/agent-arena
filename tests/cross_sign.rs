//! A signature produced by `sign.mjs` must verify in Rust, and a log Rust
//! assembles from that envelope must pass `sign.mjs verify`.

mod common;

use arena::chain::Chain;
use arena::crypto::{domain, now_ms, verify_value, zero_hash, ServerKey};
use serde_json::json;

#[test]
fn node_signature_roundtrips_through_the_hash_chain() {
    let home = common::fresh_home("cross");
    let key = common::keygen(&home);
    let server = ServerKey::generate();
    let mut chain = Chain::new(server.clone());
    let match_id = "11111111-1111-1111-1111-111111111111";
    chain.append_server(
        "genesis",
        "genesis",
        json!({
            "match_id": match_id,
            "game_id": "gomoku",
            "game_version": "1.0.0",
            "rules_hash": "abc",
            "variant": "free",
            "seats": [{
                "seat": 0,
                "agent_id": "22222222-2222-2222-2222-222222222222",
                "kid": key["kid"],
                "alg": "ed25519",
                "public_key": key["public_key"],
            }],
            "created_at": 1,
        }),
    );
    let request = json!({
        "domain": domain("move"),
        "payload": {
            "v": 1,
            "match_id": match_id,
            "game_id": "gomoku",
            "game_version": "1.0.0",
            "rules_hash": "abc",
            "seat": 0,
            "agent_id": "22222222-2222-2222-2222-222222222222",
            "seq": 0,
            "prev_hash": chain.tip_hash(),
            "deadline_at": now_ms() + 120_000,
        }
    });
    let envelope = common::sign(
        &home,
        &request,
        Some(r#"{"type":"place","data":{"at":"h8"}}"#),
        Some("抢占天元。"),
    );
    assert!(verify_value(
        key["public_key"].as_str().unwrap(),
        &envelope["domain"],
        &envelope["payload"],
        envelope["signature"].as_str().unwrap(),
    ));
    assert_eq!(envelope["payload"]["prev_hash"], chain.tip_hash());
    assert_eq!(envelope["kid"], key["kid"]);
    chain.append_agent(
        envelope["domain"].clone(),
        envelope["payload"].clone(),
        envelope["kid"].as_str().unwrap(),
        envelope["signature"].as_str().unwrap(),
    );
    chain.append_server(
        "result",
        "result",
        json!({
            "match_id": match_id,
            "result": {"scores": [1, 0.5], "ranking": [[0], [1]], "reason": "five_in_row"},
            "final_hash": chain.tip_hash(),
            "seed": "ab".repeat(16),
        }),
    );
    assert_ne!(chain.entries[0]["prev_hash"], json!(""));
    assert_eq!(chain.entries[0]["prev_hash"], json!(zero_hash()));
    let report = common::verify(
        &home,
        &chain.jsonl(),
        &json!({
            "keys": [{"kid": server.kid, "alg": "ed25519", "public_key": server.public_b64}]
        }),
    );
    assert_eq!(report["ok"], json!(true), "{report}");
    assert_eq!(report["agent_sigs"], json!(1));
    assert_eq!(report["acks"], json!(3));
    let _ = std::fs::remove_dir_all(&home);
}
