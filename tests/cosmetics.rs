//! Cosmetic unlocks, loadout checks, and avatar uploads.

mod common;

use arena::app::App;
use arena::config::Config;
use arena::cosmetics;
use arena::crypto::ServerKey;
use image::{ImageFormat, RgbImage};
use serde_json::json;
use sqlx::PgPool;
use std::path::Path;
use uuid::Uuid;

fn cfg(dir: &Path) -> Config {
    Config::for_test(dir.to_path_buf())
}

async fn make_agent(app: &App, email: &str, handle: &str, name: &str) -> Uuid {
    let (user, _) = app.register(email, "password123", handle).await.unwrap();
    let created = app
        .create_agent(&user, name, None, None, false)
        .await
        .unwrap();
    created["id"].as_str().unwrap().parse().unwrap()
}

async fn finished(pool: &PgPool, game: &str) -> Uuid {
    let version: Uuid = sqlx::query_scalar(
        "SELECT id FROM game_versions WHERE game_id = $1 ORDER BY created_at LIMIT 1",
    )
    .bind(game)
    .fetch_one(pool)
    .await
    .unwrap();
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO matches (id, game_version_id, game_id, mode, timeout_policy, status) VALUES ($1,$2,$3,'casual','auto_action','finished')",
    )
    .bind(id)
    .bind(version)
    .bind(game)
    .execute(pool)
    .await
    .unwrap();
    id
}

async fn award(pool: &PgPool, id: Uuid, game: &str, players: &[(Uuid, f64)]) {
    let mut tx = pool.begin().await.unwrap();
    cosmetics::award(&mut tx, id, game, players).await.unwrap();
    tx.commit().await.unwrap();
}

async fn unlocks(pool: &PgPool, agent: Uuid) -> Vec<String> {
    sqlx::query_scalar("SELECT item_id FROM agent_unlocks WHERE agent_id = $1 ORDER BY item_id")
        .bind(agent)
        .fetch_all(pool)
        .await
        .unwrap()
}

fn png(width: u32, height: u32) -> Vec<u8> {
    let image = RgbImage::from_fn(width, height, |x, _| image::Rgb([(x % 200) as u8, 40, 80]));
    let mut buf = Vec::new();
    image
        .write_to(&mut std::io::Cursor::new(&mut buf), ImageFormat::Png)
        .unwrap();
    buf
}

#[sqlx::test]
async fn play_unlocks_items_once(pool: PgPool) {
    let dir = common::fresh_home("cosmetics-award");
    let server = ServerKey::load_or_create(&dir).unwrap();
    let app = App::boot(pool.clone(), server, cfg(&dir)).await.unwrap();
    let winner = make_agent(&app, "win@example.com", "winner", "胜者").await;
    let other = make_agent(&app, "lose@example.com", "other", "对手").await;

    let first = finished(&pool, "gomoku").await;
    award(&pool, first, "gomoku", &[(winner, 1.0), (other, 0.0)]).await;
    let got = unlocks(&pool, winner).await;
    for id in [
        "frame-cinnabar",
        "gomoku-jade",
        "board-celadon",
        "emote-bow",
    ] {
        assert!(got.iter().any(|row| row == id), "missing {id} in {got:?}");
    }
    assert!(unlocks(&pool, other).await.is_empty());

    award(&pool, first, "gomoku", &[(winner, 1.0), (other, 0.0)]).await;
    let wins: i32 = sqlx::query_scalar("SELECT wins FROM cosmetic_stats WHERE agent_id = $1")
        .bind(winner)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(wins, 1);
    assert_eq!(unlocks(&pool, winner).await.len(), got.len());

    sqlx::query(
        r#"INSERT INTO ratings (agent_id, game_id, season, system, rating, rd, volatility, mu, sigma, games, wins)
           VALUES ($1, 'gomoku', '2026', 'glicko2', 2010, 40, 0.06, 12, 1, 20, 12)"#,
    )
    .bind(winner)
    .execute(&pool)
    .await
    .unwrap();

    for _ in 0..9 {
        let id = finished(&pool, "gomoku").await;
        award(&pool, id, "gomoku", &[(winner, 1.0), (other, 0.0)]).await;
    }
    let got = unlocks(&pool, winner).await;
    for id in [
        "frame-jade",
        "frame-ink",
        "frame-gold",
        "frame-season",
        "gomoku-ink",
        "gomoku-wood",
        "xiangqi-porcelain",
        "cards-brocade",
        "board-night",
        "emote-spark",
        "emote-ink",
    ] {
        assert!(got.iter().any(|row| row == id), "missing {id}");
    }
    let streak: i32 =
        sqlx::query_scalar("SELECT best_streak FROM cosmetic_stats WHERE agent_id = $1")
            .bind(winner)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(streak >= 3, "{streak}");

    for game in ["xiangqi", "go", "junqi", "doudizhu", "mahjong-sichuan"] {
        let id = finished(&pool, game).await;
        award(&pool, id, game, &[(winner, 1.0), (other, 0.0)]).await;
    }
    let got = unlocks(&pool, winner).await;
    assert!(got.iter().any(|row| row == "board-wood"), "{got:?}");
}

#[sqlx::test]
async fn only_unlocked_items_can_be_equipped(pool: PgPool) {
    let dir = common::fresh_home("cosmetics-loadout");
    let server = ServerKey::load_or_create(&dir).unwrap();
    let app = App::boot(pool.clone(), server, cfg(&dir)).await.unwrap();
    let agent = make_agent(&app, "dress@example.com", "dresser", "装扮").await;

    let locked = cosmetics::update_profile(
        &pool,
        &dir,
        agent,
        &json!({"equip": [{"game_id": "gomoku", "kind": "piece", "item_id": "gomoku-jade"}]}),
    )
    .await
    .unwrap_err();
    assert_eq!(locked.code, "locked");

    let wrong = cosmetics::update_profile(
        &pool,
        &dir,
        agent,
        &json!({"equip": [{"game_id": "gomoku", "kind": "piece", "item_id": "xiangqi-porcelain"}]}),
    )
    .await
    .unwrap_err();
    assert_eq!(wrong.code, "bad_item");

    let frame =
        cosmetics::update_profile(&pool, &dir, agent, &json!({"frame_id": "frame-cinnabar"}))
            .await
            .unwrap_err();
    assert_eq!(frame.code, "locked");

    let id = finished(&pool, "gomoku").await;
    let other = make_agent(&app, "seat@example.com", "seat", "陪练").await;
    award(&pool, id, "gomoku", &[(agent, 1.0), (other, 0.0)]).await;

    let saved = cosmetics::update_profile(
        &pool,
        &dir,
        agent,
        &json!({
            "title": "初胜",
            "frame_id": "frame-cinnabar",
            "equip": [{"game_id": "gomoku", "kind": "piece", "item_id": "gomoku-jade"}]
        }),
    )
    .await
    .unwrap();
    assert_eq!(saved["title"], "初胜");
    assert_eq!(saved["frame"], "cinnabar");
    let piece: String = sqlx::query_scalar(
        "SELECT item_id FROM agent_loadout WHERE agent_id = $1 AND game_id = 'gomoku' AND kind = 'piece'",
    )
    .bind(agent)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(piece, "gomoku-jade");

    cosmetics::update_profile(
        &pool,
        &dir,
        agent,
        &json!({"equip": [{"game_id": "gomoku", "kind": "piece", "item_id": ""}]}),
    )
    .await
    .unwrap();
    let left: Option<String> = sqlx::query_scalar(
        "SELECT item_id FROM agent_loadout WHERE agent_id = $1 AND game_id = 'gomoku' AND kind = 'piece'",
    )
    .bind(agent)
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert!(left.is_none());
}

#[sqlx::test]
async fn avatar_upload_checks_type_and_size(pool: PgPool) {
    let dir = common::fresh_home("cosmetics-avatar");
    let server = ServerKey::load_or_create(&dir).unwrap();
    let app = App::boot(pool.clone(), server, cfg(&dir)).await.unwrap();
    let agent = make_agent(&app, "pic@example.com", "pic", "头像").await;
    let tiny = png(32, 32);

    let bad_type = cosmetics::save_avatar(&pool, &dir, agent, "text/plain", &tiny)
        .await
        .unwrap_err();
    assert_eq!(bad_type.code, "bad_type");

    let bad_magic = cosmetics::save_avatar(&pool, &dir, agent, "image/png", b"not a png")
        .await
        .unwrap_err();
    assert_eq!(bad_magic.code, "bad_type");

    let mismatch = cosmetics::save_avatar(&pool, &dir, agent, "image/jpeg", &tiny)
        .await
        .unwrap_err();
    assert_eq!(mismatch.code, "bad_type");

    let mut huge = tiny.clone();
    huge.resize(1_500_001, 0);
    let too_big = cosmetics::save_avatar(&pool, &dir, agent, "image/png", &huge)
        .await
        .unwrap_err();
    assert_eq!(too_big.code, "too_large");

    let wide = png(4097, 1);
    let too_wide = cosmetics::save_avatar(&pool, &dir, agent, "image/png", &wide)
        .await
        .unwrap_err();
    assert_eq!(too_wide.code, "bad_image");

    let saved = cosmetics::save_avatar(&pool, &dir, agent, "image/png", &tiny)
        .await
        .unwrap();
    assert!(saved["avatar_url"]
        .as_str()
        .unwrap()
        .ends_with(&format!("{agent}.webp")));
    let bytes = std::fs::read(cosmetics::avatar_path(&dir, agent)).unwrap();
    assert!(bytes.starts_with(b"RIFF"), "webp header");
    assert_eq!(&bytes[8..12], b"WEBP");
    let flagged: bool = sqlx::query_scalar("SELECT avatar_image FROM agents WHERE id = $1")
        .bind(agent)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(flagged);
}
