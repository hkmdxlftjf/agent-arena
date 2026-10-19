//! Read-only preflight through the real authenticated MCP HTTP surface.

mod common;

use arena::app::{AgentCtx, App};
use arena::config::Config;
use arena::crypto::{commitment_of, now_ms, ServerKey};
use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use serde_json::{json, Value};
use sqlx::PgPool;
use std::path::PathBuf;
use tower::ServiceExt;
use uuid::Uuid;

struct Seat {
    ctx: AgentCtx,
    token: String,
    home: PathBuf,
    kid: Value,
}

struct Fixture {
    app: App,
    pool: PgPool,
    id: Uuid,
    black: Seat,
    white: Seat,
    dir: PathBuf,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        for path in [&self.dir, &self.black.home, &self.white.home] {
            std::fs::remove_dir_all(path).unwrap();
        }
    }
}

async fn seat(app: &App, label: &str, comments: bool) -> Seat {
    let (user, _) = app
        .register(&format!("{label}@example.com"), "password123", label)
        .await
        .unwrap();
    let created = app
        .create_agent(&user, label, None, Some("preflight"), comments)
        .await
        .unwrap();
    let id = created["id"].as_str().unwrap().parse().unwrap();
    let token = app.issue_token(&user, id).await.unwrap()["token"]
        .as_str()
        .unwrap()
        .to_string();
    let ctx = app.authenticate(&token).await.unwrap();
    let home = common::fresh_home(label);
    let key = common::keygen(&home);
    let challenge = app.key_challenge(&ctx).await.unwrap();
    let envelope = common::sign(&home, &challenge["signing_request"], None, None);
    let registered = app
        .register_key(&ctx, &envelope, Some("test"))
        .await
        .unwrap();
    assert_eq!(registered["kid"], key["kid"]);
    Seat {
        ctx,
        token,
        home,
        kid: key["kid"].clone(),
    }
}

async fn fixture(pool: PgPool, game: &str, comments: bool) -> Fixture {
    let dir = common::fresh_home("validate-data");
    let app = App::boot(
        pool.clone(),
        ServerKey::load_or_create(&dir).unwrap(),
        Config::for_test(dir.clone()),
    )
    .await
    .unwrap();
    let black = seat(&app, "validate_black", comments).await;
    let white = seat(&app, "validate_white", comments).await;
    let room = app
        .create_room(
            &black.ctx,
            &json!({"game":game, "time_limit_sec":300, "allow_comments":comments}),
        )
        .await
        .unwrap();
    let id = room["match_id"].as_str().unwrap().parse().unwrap();
    app.join_room(&white.ctx, room["room_code"].as_str().unwrap(), None)
        .await
        .unwrap();
    Fixture {
        app,
        pool,
        id,
        black,
        white,
        dir,
    }
}

async fn rpc(app: &App, token: &str, method: &str, params: Value) -> Value {
    let response = arena::http::router(app.clone())
        .oneshot(
            Request::post("/mcp")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "jsonrpc":"2.0", "id":1, "method":method, "params":params
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

async fn validate(f: &Fixture, seat: &Seat, args: &Value) -> Value {
    let result = rpc(
        &f.app,
        &seat.token,
        "tools/call",
        json!({
            "name":"validate_move", "arguments":args
        }),
    )
    .await;
    let value = result["result"]["structuredContent"].clone();
    assert!(value["valid"].is_boolean(), "{result}");
    assert_eq!(
        result["result"]["isError"],
        value["valid"] == false,
        "{result}"
    );
    assert_eq!(
        serde_json::from_str::<Value>(result["result"]["content"][0]["text"].as_str().unwrap())
            .unwrap(),
        value
    );
    assert!(value.get("nack").is_none(), "{value}");
    value
}

async fn args(f: &Fixture, seat: &Seat, action: Value) -> Value {
    let state = f.app.get_state(&seat.ctx, f.id).await.unwrap();
    assert!(state["signing_request"].is_object(), "{state}");
    json!({"match_id":f.id, "signing_request":state["signing_request"], "action":action})
}

// Compare complete persisted rows, including decisions/seen flags, timestamps,
// timeout counters, chain events, staged secrets, and rejection records.
async fn persisted(f: &Fixture) -> Value {
    let mut snapshot = json!({});
    for (table, query) in [
        ("matches", "SELECT COALESCE(jsonb_agg(to_jsonb(t) ORDER BY to_jsonb(t)::text), '[]'::jsonb) FROM matches t"),
        ("match_players", "SELECT COALESCE(jsonb_agg(to_jsonb(t) ORDER BY to_jsonb(t)::text), '[]'::jsonb) FROM match_players t"),
        ("match_events", "SELECT COALESCE(jsonb_agg(to_jsonb(t) ORDER BY to_jsonb(t)::text), '[]'::jsonb) FROM match_events t"),
        ("match_secrets", "SELECT COALESCE(jsonb_agg(to_jsonb(t) ORDER BY to_jsonb(t)::text), '[]'::jsonb) FROM match_secrets t"),
        ("move_rejections", "SELECT COALESCE(jsonb_agg(to_jsonb(t) ORDER BY to_jsonb(t)::text), '[]'::jsonb) FROM move_rejections t"),
        ("agents", "SELECT COALESCE(jsonb_agg(to_jsonb(t) ORDER BY to_jsonb(t)::text), '[]'::jsonb) FROM agents t"),
        ("agent_tokens", "SELECT COALESCE(jsonb_agg(to_jsonb(t) ORDER BY to_jsonb(t)::text), '[]'::jsonb) FROM agent_tokens t"),
        ("agent_keys", "SELECT COALESCE(jsonb_agg(to_jsonb(t) ORDER BY to_jsonb(t)::text), '[]'::jsonb) FROM agent_keys t"),
    ] {
        snapshot[table] = sqlx::query_scalar::<_, Value>(query)
            .fetch_one(&f.pool)
            .await
            .unwrap();
    }
    snapshot
}

async fn observable(f: &Fixture, seat: &Seat) -> Value {
    let mut view = f
        .app
        .get_state_query(&seat.ctx, f.id, &json!({"format":"verbose"}))
        .await
        .unwrap();
    let object = view.as_object_mut().unwrap();
    object.remove("server_time");
    object.remove("remaining_ms");
    // Connectivity is a derived wall-clock indicator, not game state.
    for seat in view["seats"].as_array_mut().unwrap() {
        seat.as_object_mut().unwrap().remove("connected");
    }
    let history = f
        .app
        .get_history(&seat.ctx, f.id, &json!({}))
        .await
        .unwrap();
    json!({"view":view,"history":history})
}

fn place(at: &str) -> Value {
    json!({"type":"place","data":{"at":at}})
}

#[sqlx::test]
async fn legal_and_illegal_actions_leave_view_history_and_database_unchanged(pool: PgPool) {
    // Given a live decision and the advertised MCP schema.
    let f = fixture(pool, "gomoku", false).await;
    let initialized = rpc(&f.app, &f.black.token, "initialize", json!({})).await;
    assert!(initialized["result"]["capabilities"]["tools"].is_object());
    let listed = rpc(&f.app, &f.black.token, "tools/list", json!({})).await;
    let schema = &listed["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["name"] == "validate_move")
        .unwrap()["inputSchema"];
    assert_eq!(
        schema["required"],
        json!(["match_id", "signing_request", "action"])
    );
    let original = args(&f, &f.black, place("h8")).await;
    let before_view = observable(&f, &f.black).await;
    let before_db = persisted(&f).await;

    // When preflighting both legal actions and type-correct invalid coordinates.
    for (action, valid) in [
        (place("h8"), true),
        (place("a1"), true),
        (place("z99"), false),
        (json!({"type":"place"}), false),
        (json!({"type":"unknown"}), false),
        (json!({"type":"place","data":{"at":42}}), false),
    ] {
        let mut request = original.clone();
        request["action"] = action;
        let response = validate(&f, &f.black, &request).await;
        assert_eq!(response["valid"], valid, "{response}");
        if !valid {
            assert_eq!(response["error"], "illegal_action");
        }
    }

    // Then neither success nor failure writes anything or advances the actor.
    assert_eq!(persisted(&f).await, before_db);
    assert_eq!(observable(&f, &f.black).await, before_view);
}

#[sqlx::test]
async fn changed_original_requests_and_wrong_identity_are_rejected_without_writes(pool: PgPool) {
    // Given the exact original domain/payload template.
    let f = fixture(pool, "gomoku", false).await;
    let original = args(&f, &f.black, place("h8")).await;
    let before = persisted(&f).await;

    // When any binding changes, including extra fields that signers add later.
    for (pointer, value) in [
        ("/domain/name", json!("Other")),
        ("/domain/version", json!("2")),
        ("/domain/origin", json!("other.example")),
        ("/domain/purpose", json!("register")),
        ("/domain/chainless", json!(false)),
        ("/payload/v", json!(2)),
        ("/payload/match_id", json!(Uuid::new_v4())),
        ("/payload/agent_id", json!(f.white.ctx.agent_id)),
        ("/payload/seat", json!(1)),
        ("/payload/kid", f.white.kid.clone()),
        ("/payload/game_id", json!("junqi")),
        ("/payload/game_version", json!("99.0.0")),
        ("/payload/rules_hash", json!("changed")),
        ("/payload/seq", json!(999)),
        ("/payload/prev_hash", json!("00".repeat(32))),
        ("/payload/deadline_at", json!(now_ms() - 1)),
    ] {
        let mut request = original.clone();
        *request["signing_request"].pointer_mut(pointer).unwrap() = value;
        assert_eq!(
            validate(&f, &f.black, &request).await["error"],
            "context_mismatch"
        );
    }
    let mut extra = original.clone();
    extra["signing_request"]["payload"]["issued_at"] = json!(now_ms());
    assert_eq!(
        validate(&f, &f.black, &extra).await["error"],
        "context_mismatch"
    );
    assert_eq!(validate(&f, &f.white, &original).await["valid"], false);
    let outsider = Seat {
        ctx: f.black.ctx.clone(),
        token: "invalid-token".into(),
        home: PathBuf::new(),
        kid: Value::Null,
    };
    let response = arena::http::router(f.app.clone())
        .oneshot(
            Request::post("/mcp")
                .header("authorization", format!("Bearer {}", outsider.token))
                .body(Body::from(
                    json!({"jsonrpc":"2.0","id":1,"method":"tools/call",
                "params":{"name":"validate_move","arguments":original}})
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let mut not_seated = f.black.ctx.clone();
    not_seated.agent_id = Uuid::new_v4();
    assert_eq!(
        f.app
            .validate_move(&not_seated, f.id, &original)
            .await
            .unwrap()["error"],
        "not_seated"
    );

    // Then all failures are ordinary validation responses, without nacks.
    assert_eq!(persisted(&f).await, before);
}

#[sqlx::test]
async fn preflight_preserves_signed_submission_and_rejects_stale_or_occupied_moves(pool: PgPool) {
    // Given a valid preflight, with the temporary real Node signing helper.
    let f = fixture(pool, "gomoku", false).await;
    let original = args(&f, &f.black, place("h8")).await;
    assert_eq!(
        validate(&f, &f.black, &original).await,
        json!({"valid":true})
    );

    // When the unchanged action is signed and submitted through MCP.
    let envelope = common::sign(
        &f.black.home,
        &original["signing_request"],
        Some(&place("h8").to_string()),
        None,
    );
    let accepted = rpc(
        &f.app,
        &f.black.token,
        "tools/call",
        json!({
            "name":"make_move", "arguments":{"match_id":f.id,"envelope":envelope}
        }),
    )
    .await;
    assert_eq!(
        accepted["result"]["structuredContent"]["accepted"], true,
        "{accepted}"
    );

    // Then old decisions and illegal parameters are rejected against real state.
    let white = args(&f, &f.white, place("h8")).await;
    let before_view = observable(&f, &f.white).await;
    let before = persisted(&f).await;
    assert_eq!(validate(&f, &f.black, &original).await["valid"], false);
    assert_eq!(
        validate(&f, &f.white, &white).await["error"],
        "illegal_action"
    );
    assert_eq!(persisted(&f).await, before);
    assert_eq!(observable(&f, &f.white).await, before_view);
    assert_eq!(before_view["view"]["move_count"], 1);
    let replay = f
        .app
        .make_move(&f.black.ctx, f.id, envelope, None)
        .await
        .unwrap();
    assert_eq!(replay["accepted"], true);
    assert_eq!(replay["idempotent"], true);
}

#[sqlx::test]
async fn comments_use_actual_length_and_permission_checks_without_writes(pool: PgPool) {
    // Given an enabled-comment room and agent.
    let f = fixture(pool, "gomoku", true).await;
    let original = args(&f, &f.black, place("h8")).await;
    let before = persisted(&f).await;

    // When validating Unicode limits and malformed optional fields.
    for (comment, error) in [
        (json!("界".repeat(140)), None),
        (json!("界".repeat(141)), Some("comment_too_long")),
        (json!(42), Some("invalid_arguments")),
    ] {
        let mut request = original.clone();
        request["comment"] = comment;
        let response = validate(&f, &f.black, &request).await;
        match error {
            Some(error) => assert_eq!(response["error"], error, "{response}"),
            None => assert_eq!(response, json!({"valid":true})),
        }
    }
    // Then validation itself has no persistence.
    assert_eq!(persisted(&f).await, before);

    // Given disabled comments in persisted state, load read-only, not via begin.
    f.app.shutdown_match(f.id).await.unwrap();
    sqlx::query("UPDATE matches SET allow_comments = false WHERE id = $1")
        .bind(f.id)
        .execute(&f.pool)
        .await
        .unwrap();
    let mut request = original;
    request["comment"] = json!("comment");
    let before = persisted(&f).await;
    assert_eq!(
        validate(&f, &f.black, &request).await["error"],
        "comment_disabled"
    );
    assert_eq!(persisted(&f).await, before);
}

#[sqlx::test]
async fn hidden_actions_validate_commitment_and_real_layout_without_marking_seen(pool: PgPool) {
    // Given simultaneous hidden deployment; white has not viewed this decision.
    let f = fixture(pool, "junqi", false).await;
    let mut original = args(&f, &f.black, json!({"type":"deploy_commit"})).await;
    original["signing_request"]["payload"]["agent_id"] = json!(f.white.ctx.agent_id);
    original["signing_request"]["payload"]["seat"] = json!(1);
    original["signing_request"]["payload"]["kid"] = f.white.kid.clone();
    let data = json!({"placements":arena::junqi::standard_layout(1)});
    let salt = "ab".repeat(32);
    original["commitment"] = json!(commitment_of(&data, &salt).unwrap());
    original["private"] = json!({"data":data,"salt":salt});
    let before = persisted(&f).await;
    let decisions = &before["matches"][0]["open_decisions"];
    assert_eq!(
        decisions
            .as_array()
            .unwrap()
            .iter()
            .find(|d| d["seat"] == 1)
            .unwrap()["seen"],
        false
    );

    // When valid, missing, mismatched and correctly committed illegal layouts
    // are submitted to the same full-action validator.
    assert_eq!(
        validate(&f, &f.white, &original).await,
        json!({"valid":true})
    );
    let mut bad = original.clone();
    bad.as_object_mut().unwrap().remove("private");
    assert_eq!(
        validate(&f, &f.white, &bad).await["error"],
        "missing_private"
    );
    bad = original.clone();
    bad["private"]["salt"] = json!("cd".repeat(32));
    assert_eq!(
        validate(&f, &f.white, &bad).await["error"],
        "bad_commitment"
    );
    bad = original.clone();
    bad.as_object_mut().unwrap().remove("commitment");
    assert_eq!(
        validate(&f, &f.white, &bad).await["error"],
        "unexpected_private"
    );
    bad = original.clone();
    bad["private"]["data"]["placements"]
        .as_array_mut()
        .unwrap()
        .pop();
    bad["commitment"] = json!(commitment_of(&bad["private"]["data"], &salt).unwrap());
    assert_eq!(
        validate(&f, &f.white, &bad).await["error"],
        "illegal_action"
    );

    // Then validation staged no secrets, decisions, events, or seen flags.
    assert_eq!(persisted(&f).await, before);
}

#[sqlx::test]
async fn expired_original_and_revoked_key_fail_without_startup_timeout_mutations(pool: PgPool) {
    // Given a stopped actor and a revoked match key.
    let f = fixture(pool, "gomoku", false).await;
    let mut original = args(&f, &f.black, place("h8")).await;
    f.app.shutdown_match(f.id).await.unwrap();
    sqlx::query("UPDATE agent_keys SET status = 'revoked' WHERE kid = $1")
        .bind(f.black.kid.as_str().unwrap())
        .execute(&f.pool)
        .await
        .unwrap();
    let before = persisted(&f).await;
    assert_eq!(
        validate(&f, &f.black, &original).await["error"],
        "key_revoked"
    );
    assert_eq!(persisted(&f).await, before);

    // When the actual stored original deadline is expired (no clock sleeps).
    sqlx::query("UPDATE matches SET open_decisions = jsonb_set(open_decisions, '{0,deadline_at}', '1') WHERE id = $1")
        .bind(f.id).execute(&f.pool).await.unwrap();
    original["signing_request"]["payload"]["deadline_at"] = json!(1);
    let before = persisted(&f).await;
    assert_eq!(
        validate(&f, &f.black, &original).await["error"],
        "deadline_passed"
    );

    // Then read-only loading did not execute a timeout or persist a nack.
    assert_eq!(persisted(&f).await, before);
    assert!(f.app.slot(f.id).cmd.lock().unwrap().is_none());
}
