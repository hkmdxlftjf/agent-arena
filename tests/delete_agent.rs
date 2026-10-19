//! Owner deletion preserves match identities while ending management access.

mod common;

use arena::app::{AgentCtx, App, User};
use arena::config::Config;
use arena::crypto::ServerKey;
use axum::body::{to_bytes, Body};
use axum::http::Request;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::path::PathBuf;
use tower::ServiceExt;
use uuid::Uuid;

struct Fixture {
    app: App,
    dir: PathBuf,
    owner: User,
    session: String,
    agent: AgentCtx,
    token: String,
}

impl Fixture {
    async fn new(pool: PgPool) -> Self {
        let database: String = sqlx::query_scalar("SELECT current_database()")
            .fetch_one(&pool)
            .await
            .unwrap();
        println!("scratch database: {database}");
        let dir =
            PathBuf::from("/tmp/arena-delete-20261016").join(format!("test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let app = App::boot(
            pool,
            ServerKey::load_or_create(&dir).unwrap(),
            Config::for_test(dir.clone()),
        )
        .await
        .unwrap();
        let (owner, session) = app
            .register("owner@example.com", "password123", "delete_owner")
            .await
            .unwrap();
        let created = app
            .create_agent(&owner, "Delete me", None, None, false)
            .await
            .unwrap();
        let id = Uuid::parse_str(created["id"].as_str().unwrap()).unwrap();
        let issued = app.issue_token(&owner, id).await.unwrap();
        let token = issued["token"].as_str().unwrap().to_string();
        let agent = app.authenticate(&token).await.unwrap();
        let key = common::keygen(&dir);
        let challenge = app.key_challenge(&agent).await.unwrap();
        let signed = common::sign(&dir, &challenge["signing_request"], None, None);
        let registered = app.register_key(&agent, &signed, None).await.unwrap();
        assert_eq!(registered["kid"], key["kid"]);
        Self {
            app,
            dir,
            owner,
            session,
            agent,
            token,
        }
    }

    async fn request(&self, method: &str, uri: &str, session: Option<&str>) -> (u16, Value) {
        let mut request = Request::builder().method(method).uri(uri);
        if let Some(session) = session {
            request = request.header("cookie", format!("arena_session={session}"));
        }
        let response = arena::http::router(self.app.clone())
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status().as_u16();
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        (status, serde_json::from_slice(&bytes).unwrap())
    }

    fn uri(&self) -> String {
        format!("/api/agents/{}", self.agent.agent_id)
    }

    async fn delete(&self) -> (u16, Value) {
        self.request("DELETE", &self.uri(), Some(&self.session))
            .await
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.dir) {
            eprintln!("fixture cleanup {}: {error}", self.dir.display());
        }
    }
}

#[sqlx::test]
async fn owner_deletion_hides_agent_and_removes_idle_queue(pool: PgPool) {
    // Given an owned agent waiting only in the ranked queue.
    let f = Fixture::new(pool).await;
    let queued = f
        .app
        .join_queue(&f.agent, &json!({"game": "gomoku", "variant": "free"}))
        .await
        .unwrap();
    assert_eq!(queued["status"], "queued");

    // When its owner deletes it through the HTTP route.
    let result = f.delete().await;

    // Then management hides it, its identity remains, and its queue row is gone.
    assert_eq!(result, (200, json!({"ok": true})));
    assert!(f.app.list_agents(&f.owner).await.unwrap().is_empty());
    let (_, me) = f.request("GET", "/api/me", Some(&f.session)).await;
    assert_eq!(me["agents"], json!([]));
    assert_eq!(
        f.app.user_profile(&f.owner.handle).await.unwrap()["agents"],
        json!([])
    );
    let deleted: bool =
        sqlx::query_scalar("SELECT deleted_at IS NOT NULL FROM agents WHERE id = $1")
            .bind(f.agent.agent_id)
            .fetch_one(f.app.pool())
            .await
            .unwrap();
    assert!(deleted);
    assert_eq!(
        f.app.queue_snapshot(f.agent.agent_id).await.unwrap()["status"],
        "idle"
    );
}

#[sqlx::test]
async fn deletion_denies_strangers_and_unauthenticated_requests(pool: PgPool) {
    // Given another account with administrator privileges but no ownership.
    let f = Fixture::new(pool).await;
    let (stranger, stranger_session) = f
        .app
        .register("stranger@example.com", "password123", "delete_stranger")
        .await
        .unwrap();
    sqlx::query("UPDATE users SET role = 'admin' WHERE id = $1")
        .bind(stranger.id)
        .execute(f.app.pool())
        .await
        .unwrap();

    // When requests have no owner session or target an unknown identity.
    for (session, id, expected) in [
        (None, f.agent.agent_id, 401),
        (Some(stranger_session.as_str()), f.agent.agent_id, 404),
        (Some(f.session.as_str()), Uuid::new_v4(), 404),
    ] {
        let (status, _) = f
            .request("DELETE", &format!("/api/agents/{id}"), session)
            .await;
        assert_eq!(status, expected);
    }

    // Then the real owner still has an active, authenticated agent.
    assert_eq!(f.app.list_agents(&f.owner).await.unwrap().len(), 1);
    assert!(f.app.authenticate(&f.token).await.is_ok());
}

#[sqlx::test]
async fn repeated_deletion_returns_not_found(pool: PgPool) {
    // Given an already deleted agent.
    let f = Fixture::new(pool).await;
    assert_eq!(f.delete().await.0, 200);
    let before: chrono::DateTime<chrono::Utc> =
        sqlx::query_scalar("SELECT deleted_at FROM agents WHERE id = $1")
            .bind(f.agent.agent_id)
            .fetch_one(f.app.pool())
            .await
            .unwrap();

    // When the owner repeats the deletion.
    assert_eq!(f.delete().await.0, 404);

    // Then the original deletion timestamp is unchanged.
    let after: chrono::DateTime<chrono::Utc> =
        sqlx::query_scalar("SELECT deleted_at FROM agents WHERE id = $1")
            .bind(f.agent.agent_id)
            .fetch_one(f.app.pool())
            .await
            .unwrap();
    assert_eq!(before, after);
}

#[sqlx::test]
async fn deletion_releases_five_agent_quota(pool: PgPool) {
    // Given an owner at the five-agent limit.
    let f = Fixture::new(pool).await;
    for _ in 0..4 {
        f.app
            .create_agent(&f.owner, "Keep me", None, None, false)
            .await
            .unwrap();
    }
    let error = f
        .app
        .create_agent(&f.owner, "Over quota", None, None, false)
        .await
        .unwrap_err();
    assert_eq!(error.code, "too_many_agents");

    // When one identity is deleted.
    assert_eq!(f.delete().await.0, 200);

    // Then a replacement can take its slot.
    f.app
        .create_agent(&f.owner, "Replacement", None, None, false)
        .await
        .unwrap();
    assert_eq!(f.app.list_agents(&f.owner).await.unwrap().len(), 5);
}

#[sqlx::test]
async fn deleted_tokens_and_owned_operations_are_rejected(pool: PgPool) {
    // Given an agent with an issued token and a cached authenticated context.
    let f = Fixture::new(pool).await;
    assert_eq!(f.delete().await.0, 200);

    // When token authentication and owner operations target that identity.
    let error = f.app.authenticate(&f.token).await.err().unwrap();
    assert_eq!(error.status, 401);
    for (method, suffix) in [
        ("POST", "/tokens"),
        ("DELETE", &format!("/tokens/{}", f.agent.token_id)),
        ("GET", "/cosmetics"),
        ("DELETE", "/avatar"),
    ] {
        assert_eq!(
            f.request(method, &format!("{}{suffix}", f.uri()), Some(&f.session))
                .await
                .0,
            404
        );
    }
    let response = arena::http::router(f.app.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/mcp")
                .header("authorization", format!("Bearer {}", f.token))
                .body(Body::from(
                    r#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 401);

    // Then neither an owner room operation nor in-flight admission revives it.
    let body = json!({"game": "gomoku", "variant": "free"});
    assert_eq!(
        f.app
            .create_room_as_user(&f.owner, f.agent.agent_id, &body)
            .await
            .unwrap_err()
            .status,
        404
    );
    assert_eq!(
        f.app.create_room(&f.agent, &body).await.unwrap_err().status,
        404
    );
    assert_eq!(
        f.app
            .join_room(&f.agent, "ABCDEF", None)
            .await
            .unwrap_err()
            .status,
        404
    );
    assert_eq!(
        f.app.join_queue(&f.agent, &body).await.unwrap_err().status,
        404
    );
    assert_eq!(
        arena::cosmetics::update_profile(
            f.app.pool(),
            &f.dir,
            f.agent.agent_id,
            &json!({"name": "Revived"})
        )
        .await
        .unwrap_err()
        .status,
        404
    );
    assert_eq!(
        arena::cosmetics::save_avatar(f.app.pool(), &f.dir, f.agent.agent_id, "image/png", &[])
            .await
            .unwrap_err()
            .status,
        404
    );
}

#[sqlx::test]
async fn waiting_and_running_matches_block_deletion(pool: PgPool) {
    // Given a seated agent in each active match status.
    let f = Fixture::new(pool).await;
    let room = f
        .app
        .create_room(&f.agent, &json!({"game": "gomoku", "variant": "free"}))
        .await
        .unwrap();
    let id = Uuid::parse_str(room["match_id"].as_str().unwrap()).unwrap();
    for status in ["waiting", "running"] {
        sqlx::query("UPDATE matches SET status = $2 WHERE id = $1")
            .bind(id)
            .bind(status)
            .execute(f.app.pool())
            .await
            .unwrap();

        // When its owner tries deletion.
        let (status, body) = f.delete().await;

        // Then deletion conflicts without removing its identity or seat.
        assert_eq!(status, 409);
        assert_eq!(body["error"], "agent_in_match");
        assert!(f.app.authenticate(&f.token).await.is_ok());
        let seats: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM match_players WHERE match_id = $1 AND agent_id = $2",
        )
        .bind(id)
        .bind(f.agent.agent_id)
        .fetch_one(f.app.pool())
        .await
        .unwrap();
        assert_eq!(seats, 1);
    }
}

#[sqlx::test]
async fn finished_match_history_survives_agent_deletion(pool: PgPool) {
    // Given a real completed match with signed server events.
    let f = Fixture::new(pool).await;
    let room = f
        .app
        .create_room(&f.agent, &json!({"game": "gomoku", "variant": "free"}))
        .await
        .unwrap();
    let (other_user, _) = f
        .app
        .register("other@example.com", "password123", "delete_other")
        .await
        .unwrap();
    let other = f
        .app
        .create_agent(&other_user, "Other", None, None, false)
        .await
        .unwrap();
    let other_id = Uuid::parse_str(other["id"].as_str().unwrap()).unwrap();
    let issued = f.app.issue_token(&other_user, other_id).await.unwrap();
    let ctx = f
        .app
        .authenticate(issued["token"].as_str().unwrap())
        .await
        .unwrap();
    let key_dir = f.dir.join("other");
    std::fs::create_dir_all(&key_dir).unwrap();
    common::keygen(&key_dir);
    let challenge = f.app.key_challenge(&ctx).await.unwrap();
    let signed = common::sign(&key_dir, &challenge["signing_request"], None, None);
    f.app.register_key(&ctx, &signed, None).await.unwrap();
    f.app
        .join_room(&ctx, room["room_code"].as_str().unwrap(), None)
        .await
        .unwrap();
    let id = Uuid::parse_str(room["match_id"].as_str().unwrap()).unwrap();
    f.app.resign(&f.agent, id).await.unwrap();
    f.app.shutdown_match(id).await.unwrap();
    let before = f.app.public_match(id).await.unwrap();
    let log_before = f.app.match_log(id, 0).await.unwrap();
    assert_eq!(before["status"], "finished");
    assert!(!log_before.is_empty());

    // When the owner deletes the finished participant.
    assert_eq!(f.delete().await, (200, json!({"ok": true})));

    // Then public match data, signing keys and event logs are preserved.
    let after = f.app.public_match(id).await.unwrap();
    for field in [
        "id",
        "status",
        "result",
        "entries",
        "server_keys",
        "tip_seq",
    ] {
        assert_eq!(after[field], before[field], "{field}");
    }
    for seat in 0..2 {
        for field in [
            "agent_id",
            "agent_name",
            "kid",
            "public_key",
            "owner_handle",
        ] {
            assert_eq!(after["seats"][seat][field], before["seats"][seat][field]);
        }
    }
    assert_eq!(f.app.match_log(id, 0).await.unwrap(), log_before);
    assert_eq!(
        f.app.agent_profile(f.agent.agent_id).await.unwrap()["agent"]["id"],
        json!(f.agent.agent_id)
    );
}

#[sqlx::test]
async fn deletion_serializes_with_room_admission(pool: PgPool) {
    // Given an unseated agent.
    let f = Fixture::new(pool).await;
    let body = json!({"game": "gomoku", "variant": "free"});

    // When deletion and room creation race, with no timing assumptions.
    let (deleted, room) = tokio::join!(
        f.app.delete_agent(&f.owner, f.agent.agent_id),
        f.app.create_room(&f.agent, &body),
    );

    // Then exactly one wins and the other observes its committed lifecycle state.
    match (deleted, room) {
        (Ok(()), Err(error)) => assert_eq!(error.status, 404),
        (Err(error), Ok(_)) => assert_eq!(error.status, 409),
        outcome => panic!("inconsistent deletion/admission outcome: {outcome:?}"),
    }
}
