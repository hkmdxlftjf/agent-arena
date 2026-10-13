//! Clocks, room expiry, and leave. Short limits are real sleeps; restarts
//! inject a past deadline so the actor applies it as soon as it loads.

mod common;

use arena::app::App;
use arena::config::Config;
use arena::crypto::{now_ms, ServerKey};
use serde_json::{json, Value};
use sqlx::PgPool;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use uuid::Uuid;

fn cfg(dir: &Path) -> Config {
    Config::for_test(dir.to_path_buf())
}

async fn boot(pool: PgPool, label: &str) -> App {
    let dir = common::fresh_home(label);
    let server = ServerKey::load_or_create(&dir).unwrap();
    App::boot(pool, server, cfg(&dir)).await.unwrap()
}

async fn seat(app: &App, email: &str, handle: &str, name: &str) -> arena::app::AgentCtx {
    let (user, _) = app.register(email, "password123", handle).await.unwrap();
    let created = app
        .create_agent(&user, name, None, None, false)
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

async fn seat_key(
    app: &App,
    email: &str,
    handle: &str,
    name: &str,
) -> (arena::app::AgentCtx, PathBuf) {
    let (user, _) = app.register(email, "password123", handle).await.unwrap();
    let created = app
        .create_agent(&user, name, None, None, false)
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
    (ctx, home)
}

fn id_of(body: &Value) -> Uuid {
    Uuid::parse_str(body["match_id"].as_str().unwrap()).unwrap()
}

async fn start_gomoku(
    app: &App,
    black: &arena::app::AgentCtx,
    white: &arena::app::AgentCtx,
    time_limit_sec: i64,
    max_timeouts: i64,
) -> Uuid {
    let room = app
        .create_room(
            black,
            &json!({
                "game": "gomoku",
                "variant": "free",
                "time_limit_sec": time_limit_sec,
                "timeout_policy": "forfeit",
                "max_consecutive_timeouts": max_timeouts,
            }),
        )
        .await
        .unwrap();
    let match_id = id_of(&room);
    let joined = app
        .join_room(white, room["room_code"].as_str().unwrap(), None)
        .await
        .unwrap();
    assert_eq!(joined["status"], json!("running"), "{joined}");
    match_id
}

async fn until_status(app: &App, match_id: Uuid, want: &str) -> Value {
    let start = Instant::now();
    loop {
        let public = app.public_match(match_id).await.unwrap();
        if public["status"] == want {
            return public;
        }
        if start.elapsed() > Duration::from_secs(20) {
            panic!("status stayed {} after 20s: {public}", public["status"]);
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

async fn rating_events(app: &App, match_id: Uuid) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM rating_events WHERE match_id = $1")
        .bind(match_id)
        .fetch_one(app.pool())
        .await
        .unwrap()
}

async fn play(app: &App, ctx: &arena::app::AgentCtx, home: &Path, match_id: Uuid, action: &str) {
    let state = app.get_state(ctx, match_id).await.unwrap();
    assert!(
        state["signing_request"].is_object(),
        "expected a turn, got {state}"
    );
    let envelope = common::sign(home, &state["signing_request"], Some(action), None);
    let accepted = app.make_move(ctx, match_id, envelope, None).await.unwrap();
    assert_eq!(accepted["accepted"], json!(true), "{accepted}");
}

async fn push_deadline(app: &App, match_id: Uuid, seen: Option<bool>) {
    let past = now_ms() - 30_000;
    if let Some(seen) = seen {
        sqlx::query(
            r#"UPDATE matches
               SET open_decisions = (
                 SELECT COALESCE(jsonb_agg(
                   jsonb_set(
                     jsonb_set(elem, '{deadline_at}', to_jsonb($2::bigint)),
                     '{seen}', to_jsonb($3::bool)
                   )
                 ), '[]'::jsonb)
                 FROM jsonb_array_elements(open_decisions) elem
               )
               WHERE id = $1"#,
        )
        .bind(match_id)
        .bind(past)
        .bind(seen)
        .execute(app.pool())
        .await
        .unwrap();
    } else {
        sqlx::query(
            r#"UPDATE matches
               SET open_decisions = (
                 SELECT COALESCE(jsonb_agg(
                   jsonb_set(elem, '{deadline_at}', to_jsonb($2::bigint))
                 ), '[]'::jsonb)
                 FROM jsonb_array_elements(open_decisions) elem
               )
               WHERE id = $1"#,
        )
        .bind(match_id)
        .bind(past)
        .execute(app.pool())
        .await
        .unwrap();
    }
}

#[sqlx::test]
async fn per_step_timeout_forfeits(pool: PgPool) {
    let app = boot(pool, "step-timeout").await;
    let black = seat(&app, "tb@example.com", "tb_black", "黑").await;
    let white = seat(&app, "tw@example.com", "tb_white", "白").await;
    let match_id = start_gomoku(&app, &black, &white, 5, 1).await;
    let state = app.get_state(&black, match_id).await.unwrap();
    assert!(state["signing_request"].is_object(), "{state}");
    assert!(state["remaining_ms"].as_i64().unwrap() > 0, "{state}");
    assert!(state["deadline_at"].is_number(), "{state}");

    let public = until_status(&app, match_id, "finished").await;
    assert_eq!(public["end_reason"], json!("timeout"), "{public}");
    assert_eq!(public["view"]["winner"], json!(1), "{public}");
    let kinds: Vec<&str> = public["entries"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|e| e["kind"].as_str())
        .collect();
    assert!(kinds.contains(&"auto_action"), "{kinds:?}");
}

#[sqlx::test]
async fn waiting_room_expires(pool: PgPool) {
    let app = boot(pool, "expire").await;
    let black = seat(&app, "eb@example.com", "ex_black", "黑").await;
    let white = seat(&app, "ew@example.com", "ex_white", "白").await;
    let room = app
        .create_room(
            &black,
            &json!({"game": "gomoku", "variant": "free", "time_limit_sec": 30}),
        )
        .await
        .unwrap();
    let match_id = id_of(&room);
    let code = room["room_code"].as_str().unwrap();
    sqlx::query("UPDATE matches SET created_at = now() - interval '11 minutes' WHERE id = $1")
        .bind(match_id)
        .execute(app.pool())
        .await
        .unwrap();
    let n = app.expire_waiting_rooms().await.unwrap();
    assert!(n >= 1, "expired {n}");
    let public = app.public_match(match_id).await.unwrap();
    assert_eq!(public["status"], json!("expired"), "{public}");
    assert_eq!(public["end_reason"], json!("room_expired"));
    assert!(public["result"].is_null(), "{public}");
    let live = app.list_live().await.unwrap();
    assert!(
        live.iter()
            .all(|row| row["id"] != json!(match_id.to_string())),
        "{live:?}"
    );
    let err = app.join_room(&white, code, None).await.unwrap_err();
    assert_eq!(err.code, "room_expired");
    assert_eq!(rating_events(&app, match_id).await, 0);
}

#[sqlx::test]
async fn leave_before_start_frees_or_closes(pool: PgPool) {
    let app = boot(pool, "leave-wait").await;
    let black = seat(&app, "lb@example.com", "lv_black", "黑").await;
    let room = app
        .create_room(
            &black,
            &json!({"game": "gomoku", "variant": "free", "time_limit_sec": 30}),
        )
        .await
        .unwrap();
    let match_id = id_of(&room);
    let left = app.leave_room(black.agent_id, match_id).await.unwrap();
    assert_eq!(left["status"], json!("closed"), "{left}");
    let public = app.public_match(match_id).await.unwrap();
    assert_eq!(public["status"], json!("aborted"), "{public}");
    assert_eq!(public["end_reason"], json!("closed"));
    assert!(public["result"].is_null(), "{public}");
    assert!(public["seats"].as_array().unwrap().is_empty(), "{public}");

    let host = seat(&app, "dh@example.com", "dz_host", "甲").await;
    let other = seat(&app, "do@example.com", "dz_other", "乙").await;
    let room = app
        .create_room(
            &host,
            &json!({"game": "doudizhu", "variant": "standard", "time_limit_sec": 30}),
        )
        .await
        .unwrap();
    let match_id = id_of(&room);
    let joined = app
        .join_room(&other, room["room_code"].as_str().unwrap(), None)
        .await
        .unwrap();
    assert_eq!(joined["status"], json!("waiting"), "{joined}");
    let left = app.leave_room(other.agent_id, match_id).await.unwrap();
    assert_eq!(left["status"], json!("waiting"), "{left}");
    let public = app.public_match(match_id).await.unwrap();
    assert_eq!(public["status"], json!("waiting"));
    assert_eq!(public["seats"].as_array().unwrap().len(), 1, "{public}");
    assert!(public["result"].is_null());
}

#[sqlx::test]
async fn leave_after_start_opponent_wins(pool: PgPool) {
    let app = boot(pool, "leave-run").await;
    let black = seat(&app, "rb@example.com", "run_black", "黑").await;
    let white = seat(&app, "rw@example.com", "run_white", "白").await;
    let match_id = start_gomoku(&app, &black, &white, 60, 3).await;
    let left = app.leave_room(black.agent_id, match_id).await.unwrap();
    assert_eq!(left["accepted"], json!(true), "{left}");
    let turn = app.wait_for_turn(&white, match_id).await.unwrap();
    assert_eq!(turn["status"], json!("match_over"), "{turn}");
    assert_eq!(turn["reason"], json!("opponent_left"), "{turn}");
    let public = app.public_match(match_id).await.unwrap();
    assert_eq!(public["status"], json!("finished"), "{public}");
    assert_eq!(public["end_reason"], json!("leave"));
    assert_eq!(public["result"]["reason"], json!("leave"));
    assert_eq!(public["view"]["winner"], json!(1));
    let kinds: Vec<&str> = public["entries"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|e| e["kind"].as_str())
        .collect();
    assert!(kinds.contains(&"leave"), "{kinds:?}");
}

#[sqlx::test]
async fn ranked_leave_updates_ratings(pool: PgPool) {
    let app = boot(pool, "leave-rank").await;
    let black = seat(&app, "kb@example.com", "rk_black", "黑").await;
    let white = seat(&app, "kw@example.com", "rk_white", "白").await;
    app.join_queue(&black, &json!({"game": "gomoku", "variant": "free"}))
        .await
        .unwrap();
    let matched = app
        .join_queue(&white, &json!({"game": "gomoku", "variant": "free"}))
        .await
        .unwrap();
    assert_eq!(matched["status"], json!("matched"), "{matched}");
    let match_id = id_of(&matched);
    let leaver = if matched["seat"] == 0 { &white } else { &black };
    let left = app.leave_room(leaver.agent_id, match_id).await.unwrap();
    assert_eq!(left["accepted"], json!(true), "{left}");
    let board = app.leaderboard("gomoku").await.unwrap();
    let rows = board["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 2, "{board}");
    let ratings: Vec<f64> = rows.iter().map(|r| r["rating"].as_f64().unwrap()).collect();
    assert!(ratings.iter().any(|r| *r > 1500.0), "{ratings:?}");
    assert!(ratings.iter().any(|r| *r < 1500.0), "{ratings:?}");
}

async fn three(
    app: &App,
    tag: &str,
) -> (
    (arena::app::AgentCtx, PathBuf),
    (arena::app::AgentCtx, PathBuf),
    (arena::app::AgentCtx, PathBuf),
) {
    let a = seat_key(
        app,
        &format!("{tag}a@example.com"),
        &format!("{tag}_a"),
        "甲",
    )
    .await;
    let b = seat_key(
        app,
        &format!("{tag}b@example.com"),
        &format!("{tag}_b"),
        "乙",
    )
    .await;
    let c = seat_key(
        app,
        &format!("{tag}c@example.com"),
        &format!("{tag}_c"),
        "丙",
    )
    .await;
    (a, b, c)
}

async fn start_doudizhu(
    app: &App,
    a: &arena::app::AgentCtx,
    b: &arena::app::AgentCtx,
    c: &arena::app::AgentCtx,
) -> Uuid {
    let room = app
        .create_room(
            a,
            &json!({
                "game": "doudizhu",
                "variant": "standard",
                "time_limit_sec": 60,
                "max_consecutive_timeouts": 3,
            }),
        )
        .await
        .unwrap();
    let match_id = id_of(&room);
    let code = room["room_code"].as_str().unwrap();
    let mid = app.join_room(b, code, None).await.unwrap();
    assert_eq!(mid["status"], json!("waiting"), "{mid}");
    let last = app.join_room(c, code, None).await.unwrap();
    assert_eq!(last["status"], json!("running"), "{last}");
    match_id
}

async fn bid_to_landlord(
    app: &App,
    match_id: Uuid,
    a: &(arena::app::AgentCtx, PathBuf),
    b: &(arena::app::AgentCtx, PathBuf),
    c: &(arena::app::AgentCtx, PathBuf),
) {
    play(
        app,
        &a.0,
        &a.1,
        match_id,
        r#"{"type":"bid","data":{"score":3}}"#,
    )
    .await;
    play(
        app,
        &b.0,
        &b.1,
        match_id,
        r#"{"type":"bid","data":{"score":0}}"#,
    )
    .await;
    play(
        app,
        &c.0,
        &c.1,
        match_id,
        r#"{"type":"bid","data":{"score":0}}"#,
    )
    .await;
    let state = app.get_state(&a.0, match_id).await.unwrap();
    assert_eq!(state["view"]["landlord"], json!(0), "{state}");
}

fn scores_of(public: &Value) -> Vec<f64> {
    public["result"]["scores"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect()
}

#[sqlx::test]
async fn doudizhu_landlord_leave_farmers_win(pool: PgPool) {
    let app = boot(pool, "dz-land").await;
    let (a, b, c) = three(&app, "land").await;
    let match_id = start_doudizhu(&app, &a.0, &b.0, &c.0).await;
    bid_to_landlord(&app, match_id, &a, &b, &c).await;
    let left = app.leave_room(a.0.agent_id, match_id).await.unwrap();
    assert_eq!(left["accepted"], json!(true), "{left}");
    let public = app.public_match(match_id).await.unwrap();
    assert_eq!(public["end_reason"], json!("leave"), "{public}");
    let scores = scores_of(&public);
    assert!(scores[0] < 0.0, "{scores:?}");
    assert!(scores[1] > 0.0 && scores[2] > 0.0, "{scores:?}");
    let turn = app.wait_for_turn(&b.0, match_id).await.unwrap();
    assert_eq!(turn["status"], json!("match_over"), "{turn}");
    assert_eq!(turn["reason"], json!("opponent_left"));
}

#[sqlx::test]
async fn doudizhu_farmer_leave_landlord_wins(pool: PgPool) {
    let app = boot(pool, "dz-farm").await;
    let (a, b, c) = three(&app, "farm").await;
    let match_id = start_doudizhu(&app, &a.0, &b.0, &c.0).await;
    bid_to_landlord(&app, match_id, &a, &b, &c).await;
    let left = app.leave_room(b.0.agent_id, match_id).await.unwrap();
    assert_eq!(left["accepted"], json!(true), "{left}");
    let public = app.public_match(match_id).await.unwrap();
    assert_eq!(public["end_reason"], json!("leave"), "{public}");
    let scores = scores_of(&public);
    assert!(scores[0] > 0.0, "{scores:?}");
    assert!(scores[1] < 0.0, "{scores:?}");
}

#[sqlx::test]
async fn mahjong_leave_splits_cap_and_ends(pool: PgPool) {
    let app = boot(pool, "mj-leave").await;
    let (a, b, c) = three(&app, "mj").await;
    let d = seat(&app, "mjd@example.com", "mj_d", "丁").await;
    let room = app
        .create_room(
            &a.0,
            &json!({
                "game": "mahjong-sichuan",
                "variant": "xuezhan",
                "time_limit_sec": 60,
            }),
        )
        .await
        .unwrap();
    let match_id = id_of(&room);
    let code = room["room_code"].as_str().unwrap();
    assert_eq!(
        app.join_room(&b.0, code, None).await.unwrap()["status"],
        json!("waiting")
    );
    assert_eq!(
        app.join_room(&c.0, code, None).await.unwrap()["status"],
        json!("waiting")
    );
    assert_eq!(
        app.join_room(&d, code, None).await.unwrap()["status"],
        json!("running")
    );
    let left = app.leave_room(a.0.agent_id, match_id).await.unwrap();
    assert_eq!(left["accepted"], json!(true), "{left}");
    let public = app.public_match(match_id).await.unwrap();
    assert_eq!(public["status"], json!("finished"), "{public}");
    assert_eq!(public["end_reason"], json!("leave"));
    let scores = scores_of(&public);
    assert_eq!(scores[0], -16.0, "{scores:?}");
    assert!(scores[1..].iter().all(|s| *s > 0.0), "{scores:?}");
    let sum: f64 = scores.iter().sum();
    assert!(sum.abs() < 0.01, "{scores:?}");
    let turn = app.wait_for_turn(&d, match_id).await.unwrap();
    assert_eq!(turn["status"], json!("match_over"), "{turn}");
    assert_eq!(turn["reason"], json!("opponent_left"));
}

#[sqlx::test]
async fn unseen_timeout_aborts_before_two_moves(pool: PgPool) {
    let app = boot(pool, "abandon").await;
    let black = seat(&app, "ab@example.com", "ab_black", "黑").await;
    let white = seat(&app, "aw@example.com", "ab_white", "白").await;
    let match_id = start_gomoku(&app, &black, &white, 5, 1).await;
    let public = until_status(&app, match_id, "aborted").await;
    assert_eq!(public["end_reason"], json!("abandoned"), "{public}");
    assert!(public["result"].is_null(), "{public}");
    assert_eq!(rating_events(&app, match_id).await, 0);
}

#[sqlx::test]
async fn ranked_abandon_does_not_rate(pool: PgPool) {
    let app = boot(pool, "abandon-rank").await;
    let black = seat(&app, "arb@example.com", "ar_black", "黑").await;
    let white = seat(&app, "arw@example.com", "ar_white", "白").await;
    let match_id = start_gomoku(&app, &black, &white, 60, 1).await;
    app.shutdown_match(match_id).await.unwrap();
    sqlx::query("UPDATE matches SET mode = 'ranked' WHERE id = $1")
        .bind(match_id)
        .execute(app.pool())
        .await
        .unwrap();
    push_deadline(&app, match_id, Some(false)).await;
    app.reopen_match(match_id).await.unwrap();
    let public = app.public_match(match_id).await.unwrap();
    assert_eq!(public["status"], json!("aborted"), "{public}");
    assert_eq!(public["end_reason"], json!("abandoned"));
    assert!(public["result"].is_null(), "{public}");
    assert_eq!(rating_events(&app, match_id).await, 0);
}

#[sqlx::test]
async fn abandon_after_two_moves_each_is_a_loss(pool: PgPool) {
    let app = boot(pool, "abandon-late").await;
    let (black, black_home) = seat_key(&app, "lb2@example.com", "late_black", "黑").await;
    let (white, white_home) = seat_key(&app, "lw2@example.com", "late_white", "白").await;
    let match_id = start_gomoku(&app, &black, &white, 60, 1).await;
    for (ctx, home, at) in [
        (&black, black_home.as_path(), "h8"),
        (&white, white_home.as_path(), "i8"),
        (&black, black_home.as_path(), "h9"),
        (&white, white_home.as_path(), "i9"),
    ] {
        play(
            &app,
            ctx,
            home,
            match_id,
            &format!(r#"{{"type":"place","data":{{"at":"{at}"}}}}"#),
        )
        .await;
    }
    app.shutdown_match(match_id).await.unwrap();
    push_deadline(&app, match_id, Some(false)).await;
    app.reopen_match(match_id).await.unwrap();
    let public = app.public_match(match_id).await.unwrap();
    assert_eq!(public["status"], json!("finished"), "{public}");
    assert_eq!(public["end_reason"], json!("abandoned"));
    assert_eq!(public["view"]["winner"], json!(1), "{public}");
}

#[sqlx::test]
async fn restart_applies_past_deadline(pool: PgPool) {
    let app = boot(pool, "restart").await;
    let black = seat(&app, "sb@example.com", "st_black", "黑").await;
    let white = seat(&app, "sw@example.com", "st_white", "白").await;
    let match_id = start_gomoku(&app, &black, &white, 60, 1).await;
    let state = app.get_state(&black, match_id).await.unwrap();
    assert!(state["signing_request"].is_object(), "{state}");
    app.shutdown_match(match_id).await.unwrap();
    push_deadline(&app, match_id, None).await;
    app.reopen_match(match_id).await.unwrap();
    let public = app.public_match(match_id).await.unwrap();
    assert_eq!(public["status"], json!("finished"), "{public}");
    assert_eq!(public["end_reason"], json!("timeout"), "{public}");
    assert_eq!(public["view"]["winner"], json!(1));
}

#[sqlx::test]
async fn duration_cap_is_a_draw(pool: PgPool) {
    let app = boot(pool, "duration").await;
    let black = seat(&app, "db@example.com", "du_black", "黑").await;
    let white = seat(&app, "dw@example.com", "du_white", "白").await;
    let match_id = start_gomoku(&app, &black, &white, 60, 3).await;
    app.shutdown_match(match_id).await.unwrap();
    sqlx::query("UPDATE matches SET started_at = now() - interval '3 hours' WHERE id = $1")
        .bind(match_id)
        .execute(app.pool())
        .await
        .unwrap();
    app.reopen_match(match_id).await.unwrap();
    let public = app.public_match(match_id).await.unwrap();
    assert_eq!(public["status"], json!("finished"), "{public}");
    assert_eq!(public["end_reason"], json!("duration"));
    assert_eq!(public["result"]["reason"], json!("duration"));
    let scores = scores_of(&public);
    assert!((scores[0] - scores[1]).abs() < 0.01, "{scores:?}");
}
