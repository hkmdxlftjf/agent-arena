//! Uploaded rules: immutable versions, owner unpublish, admin hide.

mod common;

use arena::app::App;
use arena::config::Config;
use arena::crypto::ServerKey;
use serde_json::json;
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

const SOURCE: &str = include_str!("../src/fixtures/tic-tac-toe.js");

#[sqlx::test]
async fn versions_stay_immutable_and_unpublish_blocks_rooms(pool: PgPool) {
    let dir = common::fresh_home("upload-data");
    let server = ServerKey::load_or_create(&dir).unwrap();
    let app = App::boot(pool.clone(), server, cfg(&dir)).await.unwrap();
    let (user, _) = app.register("owner@example.com", "password123", "owner").await.unwrap();
    assert_eq!(user.role, "admin");

    let report = app.check_rules(&user, SOURCE).await.unwrap();
    assert_eq!(report["ok"], json!(true));

    let first = app.publish_rules(&user, SOURCE, "tictactoe", Some("井字棋"), None).await.unwrap();
    assert_eq!(first["version"], "1.0.0");
    assert_eq!(first["id"], "u_tictactoe");

    let second_source = format!("{SOURCE}\n// second\n");
    let second = app.publish_rules(&user, &second_source, "tictactoe", None, None).await.unwrap();
    assert_eq!(second["version"], "1.0.1");
    assert_ne!(first["rules_hash"], second["rules_hash"]);

    let stored: Vec<(String, String)> = sqlx::query_as(
        "SELECT version, source FROM game_versions WHERE game_id = 'u_tictactoe' ORDER BY created_at",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(stored.len(), 2);
    assert_eq!(stored[0].0, "1.0.0");
    assert_eq!(stored[0].1, SOURCE);
    assert!(stored[1].1.contains("// second"));

    let (agent, _) = seat(&app, &user).await;
    let room = app
        .create_room(&agent, &json!({"game": "u_tictactoe", "variant": "3", "time_limit_sec": 30}))
        .await
        .unwrap();
    let match_id: Uuid = room["match_id"].as_str().unwrap().parse().unwrap();
    let logged: String = sqlx::query_scalar("SELECT rules_hash FROM matches WHERE id = $1")
        .bind(match_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(logged, second["rules_hash"].as_str().unwrap());

    app.unpublish_game(&user, "u_tictactoe").await.unwrap();
    let blocked = app
        .create_room(&agent, &json!({"game": "u_tictactoe", "variant": "3", "time_limit_sec": 30}))
        .await
        .unwrap_err();
    assert_eq!(blocked.code, "game_not_playable");

    let again = app.publish_rules(&user, SOURCE, "tictactoe", None, None).await.unwrap();
    assert_eq!(again["version"], "1.0.2");
    app.hide_game(&user, "u_tictactoe").await.unwrap();
    let hidden = app.publish_rules(&user, SOURCE, "tictactoe", None, None).await.unwrap_err();
    assert_eq!(hidden.code, "hidden");
    assert!(app.list_games().iter().all(|game| game["id"] != "u_tictactoe"));
}

async fn seat(app: &App, user: &arena::app::User) -> (arena::app::AgentCtx, std::path::PathBuf) {
    let created = app.create_agent(user, "选手", None, None, false).await.unwrap();
    let agent_id = Uuid::parse_str(created["id"].as_str().unwrap()).unwrap();
    let issued = app.issue_token(user, agent_id).await.unwrap();
    let ctx = app.authenticate(issued["token"].as_str().unwrap()).await.unwrap();
    let home = common::fresh_home("upload-agent");
    common::keygen(&home);
    let challenge = app.key_challenge(&ctx).await.unwrap();
    let envelope = common::sign(&home, &challenge["signing_request"], None, None);
    app.register_key(&ctx, &envelope, Some("test")).await.unwrap();
    (ctx, home)
}
