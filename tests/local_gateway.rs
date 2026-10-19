//! Real stdio gateway -> HTTP MCP -> actor -> signed game action.

mod common;

use arena::app::{AgentCtx, App};
use arena::config::Config;
use arena::crypto::ServerKey;
use axum::body::{to_bytes, Body};
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::{from_fn_with_state, Next};
use axum::response::{IntoResponse, Response};
use serde_json::{json, Value};
use sqlx::PgPool;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::process::Command;
use tokio::task::JoinSet;
use uuid::Uuid;

async fn drop_move_response(
    State(drop_once): State<Arc<AtomicBool>>,
    request: Request,
    next: Next,
) -> Response {
    let (parts, body) = request.into_parts();
    let bytes = to_bytes(body, 1_000_000).await.unwrap();
    let message: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    let is_move = message["method"] == "tools/call" && message["params"]["name"] == "make_move";
    let response = next
        .run(Request::from_parts(parts, Body::from(bytes)))
        .await;
    if is_move && drop_once.swap(false, Ordering::SeqCst) {
        // The actor has already committed. The client cannot see its receipt.
        StatusCode::GATEWAY_TIMEOUT.into_response()
    } else {
        response
    }
}

async fn seat(app: &App, label: &str) -> (AgentCtx, PathBuf, String) {
    let (user, _) = app
        .register(&format!("{label}@example.com"), "password123", label)
        .await
        .unwrap();
    let created = app
        .create_agent(&user, label, None, None, false)
        .await
        .unwrap();
    let id = Uuid::parse_str(created["id"].as_str().unwrap()).unwrap();
    let issued = app.issue_token(&user, id).await.unwrap();
    let token = issued["token"].as_str().unwrap().to_owned();
    let agent = app.authenticate(&token).await.unwrap();
    let home = common::fresh_home(label);
    common::keygen(&home);
    let challenge = app.key_challenge(&agent).await.unwrap();
    let envelope = common::sign(&home, &challenge["signing_request"], None, None);
    app.register_key(&agent, &envelope, None).await.unwrap();
    std::fs::remove_file(home.join("req.json")).unwrap();
    (agent, home, token)
}

async fn run_case(pool: PgPool, scenario: &str) {
    // Given an isolated referee and two genuinely registered local signers.
    let dir = common::fresh_home("gateway-server");
    let app = App::boot(
        pool,
        ServerKey::load_or_create(&dir).unwrap(),
        Config::for_test(dir.clone()),
    )
    .await
    .unwrap();
    let (black, black_home, token) = seat(&app, "local_black").await;
    let (white, white_home, _) = seat(&app, "local_white").await;
    let room = app
        .create_room(&black, &json!({"game":"gomoku","variant":"free"}))
        .await
        .unwrap();
    let id = Uuid::parse_str(room["match_id"].as_str().unwrap()).unwrap();
    app.join_room(&white, room["room_code"].as_str().unwrap(), None)
        .await
        .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let drop_once = Arc::new(AtomicBool::new(scenario == "lost-response"));
    let router =
        arena::http::router(app.clone()).layer(from_fn_with_state(drop_once, drop_move_response));
    let mut servers = JoinSet::new();
    servers.spawn(async move { axum::serve(listener, router).await });
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    // When the real Node CLI drives the local MCP server over stdio.
    let mut command = Command::new("node");
    command
        .arg(root.join("scripts/test-gateway-e2e.mjs"))
        .arg(scenario)
        .env("ARENA_TOKEN", &token)
        .env("ARENA_HOME", &black_home)
        .env("ARENA_BASE_URL", format!("http://{address}"))
        .env("ARENA_SIGN_SCRIPT", common::sign_mjs())
        .env("ARENA_ALLOWED_ORIGINS", "agent.idou.bid")
        .env("ARENA_TEST_MATCH", id.to_string())
        .kill_on_drop(true);
    let output = tokio::time::timeout(Duration::from_secs(45), command.output())
        .await
        .expect("stdio scenario timed out")
        .expect("node execution");

    // Then the actual referee history agrees with the user-visible result.
    assert!(
        output.status.success(),
        "scenario {scenario}: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let state = app
        .get_state_query(&black, id, &json!({"history":"full"}))
        .await
        .unwrap();
    let actions: Vec<_> = state["entries"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| entry["kind"] == "action")
        .collect();
    assert_eq!(actions.len(), usize::from(scenario != "invalid"));
    if let Some(action) = actions.first() {
        assert_eq!(action["payload"]["action"]["data"]["at"], "h8");
        assert!(action["signature"].is_string());
    }
    servers.shutdown().await;
    std::fs::remove_dir_all(dir).unwrap();
    std::fs::remove_dir_all(black_home).unwrap();
    std::fs::remove_dir_all(white_home).unwrap();
}

#[sqlx::test]
async fn local_stdio_move_is_signed_and_accepted(pool: PgPool) {
    run_case(pool, "accepted").await;
}

#[sqlx::test]
async fn local_stdio_illegal_move_is_rejected_before_signing(pool: PgPool) {
    run_case(pool, "invalid").await;
}

#[sqlx::test]
async fn local_stdio_restart_recovers_a_lost_acceptance_response(pool: PgPool) {
    run_case(pool, "lost-response").await;
}
