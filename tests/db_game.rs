//! Two signed agents play a scripted gomoku game through the match actor.
//! The exported log must pass `sign.mjs verify` against Postgres.

mod common;

use arena::app::App;
use arena::config::Config;
use arena::crypto::ServerKey;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::path::Path;
use uuid::Uuid;

fn cfg(dir: &Path) -> Config {
    Config {
        database_url: std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgres://arena:arena@127.0.0.1:5432/arena".into()),
        bind: "127.0.0.1:0".into(),
        data_dir: dir.to_path_buf(),
        public_url: "http://127.0.0.1".into(),
        cookie_secure: false,
    }
}

async fn seat(app: &App, email: &str, handle: &str, name: &str) -> (arena::app::AgentCtx, std::path::PathBuf) {
    let (user, _) = app.register(email, "password123", handle).await.expect("register");
    let created = app.create_agent(&user, name, None, Some("e2e-script"), false).await.unwrap();
    let agent_id = Uuid::parse_str(created["id"].as_str().unwrap()).unwrap();
    let issued = app.issue_token(&user, agent_id).await.unwrap();
    let ctx = app.authenticate(issued["token"].as_str().unwrap()).await.unwrap();
    let home = common::fresh_home(handle);
    let key = common::keygen(&home);
    let challenge = app.key_challenge(&ctx).await.unwrap();
    let envelope = common::sign(&home, &challenge["signing_request"], None, None);
    let registered = app.register_key(&ctx, &envelope, Some("test")).await.unwrap();
    assert_eq!(registered["kid"], key["kid"]);
    (ctx, home)
}

fn place(at: &str) -> String {
    format!(r#"{{"type":"place","data":{{"at":"{at}"}}}}"#)
}

async fn move_at(app: &App, ctx: &arena::app::AgentCtx, home: &Path, match_id: Uuid, at: &str) -> Value {
    let state = app.get_state(ctx, match_id).await.unwrap();
    assert!(state["signing_request"].is_object(), "expected a turn, got {state}");
    let envelope = common::sign(home, &state["signing_request"], Some(&place(at)), None);
    let accepted = app.make_move(ctx, match_id, envelope.clone()).await.unwrap();
    assert_eq!(accepted["accepted"], json!(true), "{accepted}");
    envelope
}

#[sqlx::test]
async fn signed_gomoku_log_verifies(pool: PgPool) {
    let dir = common::fresh_home("data");
    let server = ServerKey::load_or_create(&dir).unwrap();
    let app = App::boot(pool, server, cfg(&dir)).await.unwrap();

    let (black, black_home) = seat(&app, "black@example.com", "black_agent", "黑子").await;
    let (white, white_home) = seat(&app, "white@example.com", "white_agent", "白子").await;

    let denied = app.create_room(&black, &json!({"game": "xiangqi"})).await.unwrap_err();
    assert_eq!(denied.code, "game_not_playable");

    let room = app
        .create_room(&black, &json!({"game": "gomoku", "variant": "free", "time_limit_sec": 60}))
        .await
        .unwrap();
    let match_id = Uuid::parse_str(room["match_id"].as_str().unwrap()).unwrap();
    let code = room["room_code"].as_str().unwrap();
    let joined = app.join_room(&white, code, None).await.unwrap();
    assert_eq!(joined["status"], json!("running"));
    assert_eq!(joined["seat"], json!(1));

    let script = [
        (&black, black_home.as_path(), "h8"),
        (&white, white_home.as_path(), "i8"),
        (&black, black_home.as_path(), "h9"),
        (&white, white_home.as_path(), "i9"),
        (&black, black_home.as_path(), "h10"),
        (&white, white_home.as_path(), "i10"),
        (&black, black_home.as_path(), "h11"),
        (&white, white_home.as_path(), "i11"),
        (&black, black_home.as_path(), "h12"),
    ];
    let first = move_at(&app, script[0].0, script[0].1, match_id, script[0].2).await;
    let again = app.make_move(script[0].0, match_id, first).await.unwrap();
    assert_eq!(again["accepted"], json!(true), "{again}");
    assert_eq!(again["idempotent"], json!(true), "{again}");
    for (ctx, home, at) in script.into_iter().skip(1) {
        move_at(&app, ctx, home, match_id, at).await;
    }

    let public = app.public_match(match_id).await.unwrap();
    assert_eq!(public["status"], json!("finished"), "{public}");
    assert_eq!(public["view"]["winner"], json!(0));
    let line = public["view"]["winLine"].as_array().unwrap();
    assert!(line.iter().any(|v| v == "h8") && line.iter().any(|v| v == "h12"));

    let log = app.match_log(match_id, 0).await.unwrap();
    let report = common::verify(&dir, &log, &app.server_keys_doc());
    assert_eq!(report["ok"], json!(true), "{report}\n{log}");
    assert_eq!(report["agent_sigs"], json!(9), "{report}");
    assert!(report["acks"].as_i64().unwrap() >= 11, "{report}");
}
