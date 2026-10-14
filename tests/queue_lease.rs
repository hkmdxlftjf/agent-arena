//! Ranked queue lease: leave, expiry, auto-leave, profile, startup cleanup.

mod common;

use arena::app::App;
use arena::config::Config;
use arena::crypto::ServerKey;
use axum::body::Body;
use axum::http::Request;
use serde_json::json;
use sqlx::PgPool;
use std::path::Path;
use tower::ServiceExt;
use uuid::Uuid;

fn cfg(dir: &Path) -> Config {
    Config::for_test(dir.to_path_buf())
}

async fn seat(app: &App, email: &str, handle: &str, name: &str) -> arena::app::AgentCtx {
    let (user, _) = app.register(email, "password123", handle).await.unwrap();
    let created = app
        .create_agent(&user, name, None, Some("queue-test"), false)
        .await
        .unwrap();
    let agent_id = Uuid::parse_str(created["id"].as_str().unwrap()).unwrap();
    let issued = app.issue_token(&user, agent_id).await.unwrap();
    let ctx = app
        .authenticate(issued["token"].as_str().unwrap())
        .await
        .unwrap();
    let home = common::fresh_home(handle);
    let key = common::keygen(&home);
    let challenge = app.key_challenge(&ctx).await.unwrap();
    let envelope = common::sign(&home, &challenge["signing_request"], None, None);
    let registered = app
        .register_key(&ctx, &envelope, Some("test"))
        .await
        .unwrap();
    assert_eq!(registered["kid"], key["kid"]);
    ctx
}

async fn age(pool: &PgPool, agent_id: Uuid, minutes: i64) {
    sqlx::query(
        "UPDATE match_queue SET last_seen_at = now() - ($1::int * interval '1 minute') WHERE agent_id = $2",
    )
    .bind(minutes as i32)
    .bind(agent_id)
    .execute(pool)
    .await
    .unwrap();
}

#[sqlx::test]
async fn queue_expires_and_rooms_leave_it(pool: PgPool) {
    let dir = common::fresh_home("queue");
    let server = ServerKey::load_or_create(&dir).unwrap();
    let app = App::boot(pool.clone(), server, cfg(&dir)).await.unwrap();
    let queued = seat(&app, "q1@example.com", "queue_one", "甲").await;
    let other = seat(&app, "q2@example.com", "queue_two", "乙").await;
    let third = seat(&app, "q3@example.com", "queue_three", "丙").await;

    let joined = app
        .join_queue(&queued, &json!({"game": "gomoku", "variant": "free"}))
        .await
        .unwrap();
    assert_eq!(joined["status"], json!("queued"), "{joined}");

    let left = app.leave_queue(&queued).await.unwrap();
    assert_eq!(left["status"], json!("idle"));
    assert_eq!(left["left"], json!(true));
    let again = app.leave_queue(&queued).await.unwrap();
    assert_eq!(again["left"], json!(false));
    assert_eq!(
        app.queue_status(&queued).await.unwrap()["status"],
        json!("idle")
    );

    app.join_queue(&queued, &json!({"game": "gomoku", "variant": "free"}))
        .await
        .unwrap();
    let seen_before: chrono::DateTime<chrono::Utc> =
        sqlx::query_scalar("SELECT last_seen_at FROM match_queue WHERE agent_id = $1")
            .bind(queued.agent_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    age(&pool, queued.agent_id, 2).await;
    let profile = app.mcp_profile(&queued).await.unwrap();
    assert_eq!(profile["queue"]["status"], json!("queued"));
    assert_eq!(profile["queue"]["game_id"], json!("gomoku"));
    let seen_profile: chrono::DateTime<chrono::Utc> =
        sqlx::query_scalar("SELECT last_seen_at FROM match_queue WHERE agent_id = $1")
            .bind(queued.agent_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        seen_profile < seen_before,
        "profile must not refresh the lease"
    );

    let status = app.queue_status(&queued).await.unwrap();
    assert_eq!(status["status"], json!("queued"), "{status}");
    let seen_status: chrono::DateTime<chrono::Utc> =
        sqlx::query_scalar("SELECT last_seen_at FROM match_queue WHERE agent_id = $1")
            .bind(queued.agent_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        seen_status > seen_profile,
        "queue_status refreshes the lease"
    );

    let room = app
        .create_room(&queued, &json!({"game": "gomoku", "variant": "free"}))
        .await
        .unwrap();
    assert_eq!(
        app.queue_status(&queued).await.unwrap()["status"],
        json!("idle")
    );
    let code = room["room_code"].as_str().unwrap();

    app.join_queue(&other, &json!({"game": "gomoku", "variant": "free"}))
        .await
        .unwrap();
    let joined_room = app.join_room(&other, code, None).await.unwrap();
    assert_eq!(joined_room["status"], json!("running"));
    assert_eq!(
        app.queue_snapshot(other.agent_id).await.unwrap()["status"],
        json!("idle")
    );

    app.join_queue(&third, &json!({"game": "gomoku", "variant": "free"}))
        .await
        .unwrap();
    age(&pool, third.agent_id, 6).await;
    let dropped = app.expire_queue().await.unwrap();
    assert_eq!(dropped, 1);
    assert_eq!(
        app.queue_snapshot(third.agent_id).await.unwrap()["status"],
        json!("idle")
    );

    let stale = seat(&app, "q4@example.com", "queue_four", "丁").await;
    let fresh = seat(&app, "q5@example.com", "queue_five", "戊").await;
    app.join_queue(&stale, &json!({"game": "gomoku", "variant": "free"}))
        .await
        .unwrap();
    age(&pool, stale.agent_id, 6).await;
    let waiting = app
        .join_queue(&fresh, &json!({"game": "gomoku", "variant": "free"}))
        .await
        .unwrap();
    assert_eq!(waiting["status"], json!("queued"), "{waiting}");
    assert_eq!(
        app.queue_snapshot(stale.agent_id).await.unwrap()["status"],
        json!("idle")
    );

    age(&pool, fresh.agent_id, 6).await;
    let restarted = App::boot(
        pool.clone(),
        ServerKey::load_or_create(&dir).unwrap(),
        cfg(&dir),
    )
    .await
    .unwrap();
    let left_in_queue: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM match_queue WHERE agent_id = $1")
            .bind(fresh.agent_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(left_in_queue, 0);
    assert_eq!(
        restarted.queue_snapshot(fresh.agent_id).await.unwrap()["status"],
        json!("idle")
    );

    let rank_a = seat(&app, "q6@example.com", "queue_six", "己").await;
    let rank_b = seat(&app, "q7@example.com", "queue_seven", "庚").await;
    app.join_queue(&rank_a, &json!({"game": "gomoku", "variant": "free"}))
        .await
        .unwrap();
    let matched = app
        .join_queue(&rank_b, &json!({"game": "gomoku", "variant": "free"}))
        .await
        .unwrap();
    assert_eq!(matched["status"], json!("matched"), "{matched}");
    let snap = app.mcp_profile(&rank_a).await.unwrap();
    assert_eq!(snap["queue"]["status"], json!("matched"));
    assert!(snap["queue"]["match_id"].is_string());
}

#[sqlx::test]
async fn closing_the_mcp_session_leaves_the_queue(pool: PgPool) {
    let dir = common::fresh_home("queue-mcp");
    let server = ServerKey::load_or_create(&dir).unwrap();
    let app = App::boot(pool.clone(), server, cfg(&dir)).await.unwrap();
    let (user, _) = app
        .register("qm@example.com", "password123", "queue_mcp")
        .await
        .unwrap();
    let created = app
        .create_agent(&user, "会话", None, Some("queue-test"), false)
        .await
        .unwrap();
    let agent_id = Uuid::parse_str(created["id"].as_str().unwrap()).unwrap();
    let issued = app.issue_token(&user, agent_id).await.unwrap();
    let token = issued["token"].as_str().unwrap().to_string();
    let ctx = app.authenticate(&token).await.unwrap();
    let home = common::fresh_home("queue_mcp_key");
    let key = common::keygen(&home);
    let challenge = app.key_challenge(&ctx).await.unwrap();
    let envelope = common::sign(&home, &challenge["signing_request"], None, None);
    app.register_key(&ctx, &envelope, Some("test"))
        .await
        .unwrap();
    assert!(key["kid"].is_string());

    app.join_queue(&ctx, &json!({"game": "gomoku", "variant": "free"}))
        .await
        .unwrap();
    let router = arena::http::router(app.clone());
    let opened = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/mcp")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .header("accept", "application/json")
                .body(Body::from(
                    r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"queue-test","version":"0"}}}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(opened.status(), 200);
    let sid = opened
        .headers()
        .get("mcp-session-id")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();

    let closed = router
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/mcp")
                .header("mcp-session-id", sid)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(closed.status(), 204);
    assert_eq!(
        app.queue_snapshot(ctx.agent_id).await.unwrap()["status"],
        json!("idle")
    );
}
