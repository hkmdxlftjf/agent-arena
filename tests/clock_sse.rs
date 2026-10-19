//! Per-action clock, compact state size, and SSE framing.

mod common;

use arena::app::App;
use arena::config::Config;
use arena::crypto::ServerKey;
use axum::body::Body;
use axum::http::Request;
use futures::{Stream, StreamExt};
use serde_json::json;
use sqlx::PgPool;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tower::ServiceExt;
use uuid::Uuid;

fn cfg(dir: &Path) -> Config {
    Config::for_test(dir.to_path_buf())
}

async fn seat(
    app: &App,
    email: &str,
    handle: &str,
    name: &str,
) -> (arena::app::AgentCtx, PathBuf, String) {
    let (user, _) = app
        .register(email, "password123", handle)
        .await
        .expect("register");
    let created = app
        .create_agent(&user, name, None, Some("clock"), false)
        .await
        .unwrap();
    let agent_id = Uuid::parse_str(created["id"].as_str().unwrap()).unwrap();
    let issued = app.issue_token(&user, agent_id).await.unwrap();
    let token = issued["token"].as_str().unwrap().to_string();
    let ctx = app.authenticate(&token).await.unwrap();
    let home = common::fresh_home(handle);
    let key = common::keygen(&home);
    let challenge = app.key_challenge(&ctx).await.unwrap();
    let envelope = common::sign(&home, &challenge["signing_request"], None, None);
    let registered = app
        .register_key(&ctx, &envelope, Some("test"))
        .await
        .unwrap();
    assert_eq!(registered["kid"], key["kid"]);
    (ctx, home, token)
}

fn place(at: &str) -> String {
    format!(r#"{{"type":"place","data":{{"at":"{at}"}}}}"#)
}

async fn move_at(app: &App, ctx: &arena::app::AgentCtx, home: &Path, match_id: Uuid, at: &str) {
    let state = app.get_state(ctx, match_id).await.unwrap();
    assert!(state["signing_request"].is_object(), "{state}");
    let envelope = common::sign(home, &state["signing_request"], Some(&place(at)), None);
    let accepted = app.make_move(ctx, match_id, envelope, None).await.unwrap();
    assert_eq!(accepted["accepted"], json!(true), "{accepted}");
}

#[sqlx::test]
async fn action_clock_is_one_of_four_values(pool: PgPool) {
    let dir = common::fresh_home("clock");
    let server = ServerKey::load_or_create(&dir).unwrap();
    let app = App::boot(pool, server, cfg(&dir)).await.unwrap();
    let (black, _, _) = seat(&app, "c1@example.com", "clock_black", "黑").await;
    let (white, _, _) = seat(&app, "c2@example.com", "clock_white", "白").await;

    let omitted = app
        .create_room(&black, &json!({"game": "gomoku"}))
        .await
        .unwrap();
    assert_eq!(omitted["time_limit_sec"], json!(180));
    let id = Uuid::parse_str(omitted["match_id"].as_str().unwrap()).unwrap();
    let public = app.public_match(id).await.unwrap();
    assert_eq!(public["time_limit_ms"], json!(180_000));

    let slow = app
        .create_room(&white, &json!({"game": "gomoku", "time_limit_sec": 120}))
        .await
        .unwrap();
    let slow_id = Uuid::parse_str(slow["match_id"].as_str().unwrap()).unwrap();
    assert_eq!(
        app.public_match(slow_id).await.unwrap()["time_limit_ms"],
        json!(120_000)
    );

    for bad in [30_i64, 90] {
        let err = app
            .create_room(&black, &json!({"game": "gomoku", "time_limit_sec": bad}))
            .await
            .unwrap_err();
        assert_eq!(err.code, "invalid_time");
    }

    let by_code = app
        .room_by_code(omitted["room_code"].as_str().unwrap())
        .await
        .unwrap();
    assert_eq!(by_code["id"], json!(id.to_string()));

    let live = app.list_live().await.unwrap();
    let card = live
        .iter()
        .find(|row| row["id"] == json!(id.to_string()))
        .unwrap();
    assert_eq!(card["time_limit_ms"], json!(180_000));
    let expires = card["expires_at"].as_i64().unwrap();
    assert!(expires > card["server_time"].as_i64().unwrap());

    let (rank_a, _, _) = seat(&app, "c3@example.com", "clock_rank_a", "甲").await;
    let (rank_b, _, _) = seat(&app, "c4@example.com", "clock_rank_b", "乙").await;
    let queued = app
        .join_queue(&rank_a, &json!({"game": "gomoku", "variant": "free"}))
        .await
        .unwrap();
    assert_eq!(queued["status"], json!("queued"));
    let matched = app
        .join_queue(&rank_b, &json!({"game": "gomoku", "variant": "free"}))
        .await
        .unwrap();
    assert_eq!(matched["status"], json!("matched"), "{matched}");
    let ranked = Uuid::parse_str(matched["match_id"].as_str().unwrap()).unwrap();
    assert_eq!(
        app.public_match(ranked).await.unwrap()["time_limit_ms"],
        json!(180_000)
    );
}

#[sqlx::test]
async fn compact_state_stays_under_two_kilobytes(pool: PgPool) {
    let dir = common::fresh_home("compact");
    let server = ServerKey::load_or_create(&dir).unwrap();
    let app = App::boot(pool, server, cfg(&dir)).await.unwrap();
    let (black, black_home, _) = seat(&app, "p1@example.com", "pack_black", "黑").await;
    let (white, white_home, _) = seat(&app, "p2@example.com", "pack_white", "白").await;

    let room = app
        .create_room(&black, &json!({"game": "gomoku", "variant": "free"}))
        .await
        .unwrap();
    let match_id = Uuid::parse_str(room["match_id"].as_str().unwrap()).unwrap();
    app.join_room(&white, room["room_code"].as_str().unwrap(), None)
        .await
        .unwrap();
    move_at(&app, &black, &black_home, match_id, "h8").await;
    move_at(&app, &white, &white_home, match_id, "i8").await;
    move_at(&app, &black, &black_home, match_id, "h9").await;
    move_at(&app, &white, &white_home, match_id, "i9").await;

    let compact = app.get_state(&black, match_id).await.unwrap();
    let compact_bytes = serde_json::to_vec(&compact).unwrap().len();
    let verbose = app
        .get_state_query(&black, match_id, &json!({"format": "verbose"}))
        .await
        .unwrap();
    let verbose_bytes = serde_json::to_vec(&verbose).unwrap().len();
    eprintln!("gomoku compact {compact_bytes} verbose {verbose_bytes}");
    assert!(compact.get("legal_actions").is_none(), "{compact}");
    assert!(compact["prev_hash"].as_str().unwrap().len() > 8);
    assert!(compact["board"]["stones"].as_array().unwrap().len() >= 4);
    assert!(compact["signing_request"].is_object());
    assert!(compact_bytes < 2048, "gomoku compact {compact_bytes}");
    assert!(verbose_bytes > compact_bytes);
    assert!(verbose["legal_actions"].as_array().unwrap().len() > 50);

    let near = app
        .get_legal_moves(&black, match_id, &json!({"filter": "near"}))
        .await
        .unwrap();
    let all = app
        .get_legal_moves(&black, match_id, &json!({}))
        .await
        .unwrap();
    assert!(near["returned"].as_u64().unwrap() < all["returned"].as_u64().unwrap());
    assert!(near["returned"].as_u64().unwrap() > 0);

    let history = app
        .get_history(&black, match_id, &json!({"from_move": 0, "limit": 20}))
        .await
        .unwrap();
    let seqs: Vec<i64> = history["moves"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|m| m["seq"].as_i64())
        .collect();
    assert!(seqs.len() >= 4, "{history}");
    let later = app
        .get_history(
            &black,
            match_id,
            &json!({"from_move": seqs[2], "limit": 20}),
        )
        .await
        .unwrap();
    assert!(later["returned"].as_u64().unwrap() < history["returned"].as_u64().unwrap());

    let illegal = app
        .make_move(
            &black,
            match_id,
            common::sign(
                &black_home,
                &compact["signing_request"],
                Some(&place("z9")),
                None,
            ),
            None,
        )
        .await
        .unwrap();
    assert_eq!(illegal["error"], json!("illegal_action"));
    assert!(illegal.get("legal_actions").is_none(), "{illegal}");
    assert!(illegal["message"]
        .as_str()
        .unwrap()
        .contains("get_legal_moves"));
    assert!(serde_json::to_vec(&illegal).unwrap().len() < 800);

    let (red, _, _) = seat(&app, "r@example.com", "pack_red", "红").await;
    let (blue, _, _) = seat(&app, "b@example.com", "pack_blue", "黑方").await;
    let xroom = app
        .create_room(&red, &json!({"game": "xiangqi"}))
        .await
        .unwrap();
    let xid = Uuid::parse_str(xroom["match_id"].as_str().unwrap()).unwrap();
    app.join_room(&blue, xroom["room_code"].as_str().unwrap(), None)
        .await
        .unwrap();
    let xcompact = app.get_state(&red, xid).await.unwrap();
    let xbytes = serde_json::to_vec(&xcompact).unwrap().len();
    let xverbose = app
        .get_state_query(&red, xid, &json!({"format": "verbose"}))
        .await
        .unwrap();
    let xvbytes = serde_json::to_vec(&xverbose).unwrap().len();
    eprintln!("xiangqi compact {xbytes} verbose {xvbytes}");
    assert_eq!(xcompact["board_kind"], json!("fen"));
    assert!(xcompact["board"].as_str().unwrap().contains('/'));
    assert!(xcompact["signing_request"].is_object());
    assert!(xbytes < 2048, "xiangqi compact {xbytes}");
    assert!(xvbytes > xbytes);
    let _ = std::fs::write(
        "/tmp/arena-payload-sizes.txt",
        format!("gomoku compact {compact_bytes} verbose {verbose_bytes}\nxiangqi compact {xbytes} verbose {xvbytes}\n"),
    );
}

#[sqlx::test]
async fn match_events_are_unbuffered_and_resumable(pool: PgPool) {
    let dir = common::fresh_home("sse");
    let server = ServerKey::load_or_create(&dir).unwrap();
    let app = App::boot(pool, server, cfg(&dir)).await.unwrap();
    let (black, _, token) = seat(&app, "s1@example.com", "sse_black", "黑").await;
    let room = app
        .create_room(&black, &json!({"game": "gomoku"}))
        .await
        .unwrap();
    let match_id = room["match_id"].as_str().unwrap();
    let router = arena::http::router(app);

    let missing = router
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/matches/{match_id}/events"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing.status(), 401);

    let res = router
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/matches/{match_id}/events?token={token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let headers = res.headers();
    assert_eq!(headers["cache-control"], "no-cache, no-store, no-transform");
    assert_eq!(headers["x-accel-buffering"], "no");
    assert_eq!(headers["cdn-cache-control"], "no-store");
    assert_eq!(headers["cloudflare-cdn-cache-control"], "no-store");
    assert_eq!(headers["content-encoding"], "identity");
    assert!(headers["content-type"]
        .to_str()
        .unwrap()
        .starts_with("text/event-stream"));

    let mut stream = res.into_body().into_data_stream();
    let first = read_sse(&mut stream, "event:", 3).await;
    assert!(first.contains(": open"), "{first}");
    assert!(first.contains("event: "), "{first}");
    let event_id = first
        .lines()
        .find_map(|line| line.strip_prefix("id: "))
        .unwrap()
        .to_string();
    drop(stream);

    let again = router
        .oneshot(
            Request::builder()
                .uri(format!("/api/matches/{match_id}/events?token={token}"))
                .header("last-event-id", &event_id)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let mut skipped = again.into_body().into_data_stream();
    let quiet = read_sse(&mut skipped, "event:", 2).await;
    assert!(quiet.contains(": open"), "{quiet}");
    assert!(
        !quiet.contains("event:"),
        "Last-Event-ID {event_id} was sent again: {quiet}"
    );

    let compose = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docker-compose.yml"),
    )
    .unwrap();
    assert!(compose.contains("tunnel --no-autoupdate run"));
    assert!(!compose.contains("no-chunked-encoding"));
}

#[sqlx::test]
async fn mcp_wait_pushes_waiting_room_joins_and_leaves(pool: PgPool) {
    let dir = common::fresh_home("room-change-sse");
    let app = App::boot(pool, ServerKey::load_or_create(&dir).unwrap(), cfg(&dir))
        .await
        .unwrap();
    let (host, host_home, token) = seat(&app, "host@example.com", "change_host", "host").await;
    let (guest, guest_home, _) = seat(&app, "guest@example.com", "change_guest", "guest").await;
    let room = app
        .create_room(&host, &json!({"game":"doudizhu"}))
        .await
        .unwrap();
    let id = Uuid::parse_str(room["match_id"].as_str().unwrap()).unwrap();
    let router = arena::http::router(app.clone());

    for joining in [true, false] {
        // Subscribe and await the baseline-ready SSE frame before changing seats.
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/mcp")
                    .header("authorization", format!("Bearer {token}"))
                    .header("content-type", "application/json")
                    .header("accept", "application/json, text/event-stream")
                    .body(Body::from(
                        json!({
                            "jsonrpc":"2.0", "id":1, "method":"tools/call",
                            "params":{"name":"wait_for_turn", "arguments":{"match_id":id}}
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        let mut stream = response.into_body().into_data_stream();
        assert!(read_sse(&mut stream, ": open", 3).await.contains(": open"));

        // When a player joins or leaves a room that still cannot start.
        if joining {
            app.join_room(&guest, room["room_code"].as_str().unwrap(), None)
                .await
                .unwrap();
        } else {
            app.leave_room(guest.agent_id, id).await.unwrap();
        }

        // Then the pending call delivers a real state result without polling.
        let frame = read_sse(&mut stream, "data:", 3).await;
        let data = frame
            .lines()
            .find_map(|line| line.strip_prefix("data: "))
            .expect("room change must wake SSE wait");
        let body: serde_json::Value = serde_json::from_str(data).unwrap();
        let state = &body["result"]["structuredContent"];
        assert_eq!(state["status"], "not_your_turn");
        assert_eq!(state["match_status"], "waiting");
        assert_eq!(
            state["seats"].as_array().unwrap().len(),
            if joining { 2 } else { 1 }
        );
    }
    app.leave_room(host.agent_id, id).await.unwrap();
    for home in [host_home, guest_home, dir] {
        std::fs::remove_dir_all(home).unwrap();
    }
}

async fn read_sse<S>(stream: &mut S, needle: &str, secs: u64) -> String
where
    S: Stream<Item = Result<axum::body::Bytes, axum::Error>> + Unpin,
{
    let mut buf = String::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
    while !buf.contains(needle) {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        if left.is_zero() {
            break;
        }
        match tokio::time::timeout(left, stream.next()).await {
            Ok(Some(Ok(bytes))) => buf.push_str(&String::from_utf8_lossy(&bytes)),
            _ => break,
        }
    }
    buf
}
