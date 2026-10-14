//! Browser HTTP API, WebSocket spectating, and the embedded SPA.

use crate::app::App;
use crate::error::CallError;
use axum::body::Body;
use axum::body::Bytes;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use axum_extra::extract::cookie::{Cookie, CookieJar};
use rust_embed::Embed;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::atomic::Ordering;
use tower_http::trace::TraceLayer;
use uuid::Uuid;

#[derive(Embed)]
#[folder = "web/dist"]
struct Assets;

pub fn router(app: App) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route(
            "/mcp",
            post(crate::mcp::mcp_post)
                .get(crate::mcp::mcp_get)
                .delete(crate::mcp::mcp_delete),
        )
        .route("/ws/match/{id}", get(ws_match))
        .route("/api/auth/register", post(register))
        .route("/api/auth/login", post(login))
        .route("/api/auth/logout", post(logout))
        .route("/api/auth/password", post(set_password))
        .route("/api/auth/github/unlink", post(github_unlink))
        .route("/auth/github", get(github_start))
        .route("/auth/github/callback", get(github_callback))
        .route("/connect.sh", get(connect_sh))
        .route("/connect.ps1", get(connect_ps1))
        .route("/api/me", get(me))
        .route("/api/agents", get(list_agents).post(create_agent))
        .route("/api/agents/{id}/tokens", post(issue_token))
        .route(
            "/api/agents/{id}/tokens/{tid}",
            axum::routing::delete(revoke_token),
        )
        .route("/api/games", get(games))
        .route("/api/games/{id}", get(game))
        .route("/api/games/{id}/unpublish", post(unpublish_game))
        .route("/api/games/{id}/hide", post(hide_game))
        .route("/api/uploads/check", post(check_rules))
        .route("/api/uploads/publish", post(publish_rules))
        .route("/sdk/game.ts", get(sdk_ts))
        .route("/examples/tic-tac-toe.js", get(example_js))
        .route("/api/matches", get(matches))
        .route("/api/rooms", post(create_room_http))
        .route("/api/rooms/{code}", get(room_by_code))
        .route("/ws/lobby", get(ws_lobby))
        .route("/api/matches/{id}", get(match_one))
        .route("/api/matches/{id}/leave", post(leave_match))
        .route("/api/matches/{id}/log.jsonl", get(match_log))
        .route("/api/matches/{id}/events", get(match_events))
        .route("/api/leaderboard", get(leaderboard))
        .route("/api/users/{handle}", get(user_profile))
        .route("/api/agents/{id}/profile", get(agent_profile))
        .route("/api/cosmetics", get(cosmetics_catalog))
        .route(
            "/api/agents/{id}/cosmetics",
            get(agent_cosmetics).post(update_cosmetics),
        )
        .route(
            "/api/agents/{id}/avatar",
            post(upload_avatar).delete(clear_avatar),
        )
        .route("/api/me/unlocks", get(fresh_unlocks).post(ack_unlocks))
        .route("/avatars/{file}", get(avatar_file))
        .route("/ws/leaderboard/{game}", get(ws_leaderboard))
        .route("/.well-known/arena-keys.json", get(keys_doc))
        .route("/.well-known/arena-skill.json", get(skill_doc))
        .route("/skill/arena-skill-v1.0.0.zip", get(skill_zip))
        .route("/cli.tgz", get(cli_tgz))
        .fallback(spa)
        .layer(TraceLayer::new_for_http())
        .with_state(app)
}

async fn healthz() -> &'static str {
    "ok"
}

async fn readyz(State(app): State<App>) -> Result<&'static str, CallError> {
    sqlx::query("SELECT 1")
        .execute(app.pool())
        .await
        .map_err(|_| CallError::new(503, "not_ready", "数据库未就绪"))?;
    Ok("ok")
}

#[derive(Deserialize)]
struct RegisterBody {
    email: String,
    password: String,
    handle: String,
}

async fn register(
    State(app): State<App>,
    headers: HeaderMap,
    jar: CookieJar,
    Json(body): Json<RegisterBody>,
) -> Result<impl IntoResponse, CallError> {
    let (user, sid) = app
        .register(&body.email, &body.password, &body.handle)
        .await?;
    let jar = jar.add(session_cookie(&sid, secure(&headers, &app)));
    Ok((jar, Json(json!({"user": user_json(&user)}))))
}

#[derive(Deserialize)]
struct LoginBody {
    email: String,
    password: String,
}

async fn login(
    State(app): State<App>,
    headers: HeaderMap,
    jar: CookieJar,
    Json(body): Json<LoginBody>,
) -> Result<impl IntoResponse, CallError> {
    let (user, sid) = app.login(&body.email, &body.password).await?;
    let jar = jar.add(session_cookie(&sid, secure(&headers, &app)));
    Ok((jar, Json(json!({"user": user_json(&user)}))))
}

async fn logout(State(app): State<App>, jar: CookieJar) -> Result<impl IntoResponse, CallError> {
    if let Some(c) = jar.get("arena_session") {
        app.logout(c.value()).await?;
    }
    let mut dead = Cookie::from("arena_session");
    dead.set_path("/");
    Ok((jar.remove(dead), Json(json!({"ok": true}))))
}

async fn me(State(app): State<App>, jar: CookieJar) -> Result<Json<Value>, CallError> {
    let user = require_user(&app, &jar).await?;
    let agents = app.list_agents(&user).await?;
    Ok(Json(json!({"user": user_json(&user), "agents": agents})))
}

#[derive(Deserialize)]
struct AgentBody {
    name: String,
    description: Option<String>,
    model_label: Option<String>,
    #[serde(default)]
    allow_comments: bool,
}

async fn create_agent(
    State(app): State<App>,
    jar: CookieJar,
    Json(body): Json<AgentBody>,
) -> Result<Json<Value>, CallError> {
    let user = require_user(&app, &jar).await?;
    let created = app
        .create_agent(
            &user,
            &body.name,
            body.description.as_deref(),
            body.model_label.as_deref(),
            body.allow_comments,
        )
        .await?;
    Ok(Json(created))
}

async fn list_agents(State(app): State<App>, jar: CookieJar) -> Result<Json<Value>, CallError> {
    let user = require_user(&app, &jar).await?;
    Ok(Json(json!(app.list_agents(&user).await?)))
}

async fn issue_token(
    State(app): State<App>,
    jar: CookieJar,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, CallError> {
    let user = require_user(&app, &jar).await?;
    Ok(Json(app.issue_token(&user, id).await?))
}

async fn revoke_token(
    State(app): State<App>,
    jar: CookieJar,
    Path((id, tid)): Path<(Uuid, Uuid)>,
) -> Result<Json<Value>, CallError> {
    let user = require_user(&app, &jar).await?;
    app.revoke_token(&user, id, tid).await?;
    Ok(Json(json!({"ok": true})))
}

async fn games(State(app): State<App>) -> Json<Value> {
    Json(json!(app.list_games()))
}

async fn game(State(app): State<App>, Path(id): Path<String>) -> Result<Json<Value>, CallError> {
    Ok(Json(app.game_detail(&id)?))
}

#[derive(Deserialize)]
struct SourceBody {
    source: String,
    slug: Option<String>,
    name: Option<String>,
    description: Option<String>,
}

async fn check_rules(
    State(app): State<App>,
    jar: CookieJar,
    Json(body): Json<SourceBody>,
) -> Result<Json<Value>, CallError> {
    let user = require_user(&app, &jar).await?;
    Ok(Json(app.check_rules(&user, &body.source).await?))
}

async fn publish_rules(
    State(app): State<App>,
    jar: CookieJar,
    Json(body): Json<SourceBody>,
) -> Result<Json<Value>, CallError> {
    let user = require_user(&app, &jar).await?;
    let slug = body.slug.unwrap_or_default();
    Ok(Json(
        app.publish_rules(
            &user,
            &body.source,
            &slug,
            body.name.as_deref(),
            body.description.as_deref(),
        )
        .await?,
    ))
}

async fn unpublish_game(
    State(app): State<App>,
    jar: CookieJar,
    Path(id): Path<String>,
) -> Result<Json<Value>, CallError> {
    let user = require_user(&app, &jar).await?;
    Ok(Json(app.unpublish_game(&user, &id).await?))
}

async fn hide_game(
    State(app): State<App>,
    jar: CookieJar,
    Path(id): Path<String>,
) -> Result<Json<Value>, CallError> {
    let user = require_user(&app, &jar).await?;
    Ok(Json(app.hide_game(&user, &id).await?))
}

async fn sdk_ts() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        include_str!("../sdk/game.ts"),
    )
}

async fn example_js() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        include_str!("fixtures/tic-tac-toe.js"),
    )
}

#[derive(Deserialize)]
struct MatchQuery {
    live: Option<i32>,
}

async fn matches(
    State(app): State<App>,
    Query(q): Query<MatchQuery>,
) -> Result<Json<Value>, CallError> {
    let _ = q.live;
    Ok(Json(json!(app.list_live().await?)))
}

#[derive(Deserialize)]
struct CreateRoomBody {
    agent_id: Uuid,
    game: String,
    variant: Option<String>,
    time_limit_sec: Option<i64>,
    timeout_policy: Option<String>,
    max_consecutive_timeouts: Option<i64>,
}

async fn create_room_http(
    State(app): State<App>,
    jar: CookieJar,
    Json(body): Json<CreateRoomBody>,
) -> Result<Json<Value>, CallError> {
    let user = require_user(&app, &jar).await?;
    let payload = json!({
        "game": body.game,
        "variant": body.variant,
        "time_limit_sec": body.time_limit_sec,
        "timeout_policy": body.timeout_policy,
        "max_consecutive_timeouts": body.max_consecutive_timeouts,
    });
    Ok(Json(
        app.create_room_as_user(&user, body.agent_id, &payload)
            .await?,
    ))
}

async fn room_by_code(
    State(app): State<App>,
    Path(code): Path<String>,
) -> Result<Json<Value>, CallError> {
    Ok(Json(app.room_by_code(&code).await?))
}

async fn ws_lobby(State(app): State<App>, ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(move |socket| lobby_socket(app, socket))
}

async fn lobby_socket(app: App, mut socket: WebSocket) {
    let mut last = String::new();
    let mut tick = tokio::time::interval(std::time::Duration::from_secs(2));
    loop {
        tokio::select! {
            incoming = socket.recv() => {
                match incoming {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Ping(p))) => { let _ = socket.send(Message::Pong(p)).await; }
                    Some(Err(_)) => break,
                    _ => {}
                }
            }
            _ = tick.tick() => {
                let Ok(body) = app.list_live().await else { continue };
                let text = serde_json::to_string(&body).unwrap_or_default();
                if text != last {
                    last = text;
                    let msg = json!({"type":"rooms","data": body}).to_string();
                    if socket.send(Message::Text(msg.into())).await.is_err() {
                        break;
                    }
                }
            }
        }
    }
}

async fn match_one(State(app): State<App>, Path(id): Path<Uuid>) -> Result<Json<Value>, CallError> {
    Ok(Json(app.public_match(id).await?))
}

async fn leave_match(
    State(app): State<App>,
    jar: CookieJar,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, CallError> {
    let user = require_user(&app, &jar).await?;
    Ok(Json(app.leave_as_user(user.id, id).await?))
}

#[derive(Deserialize)]
struct EventsQuery {
    token: Option<String>,
}

async fn match_events(
    State(app): State<App>,
    Path(id): Path<Uuid>,
    Query(q): Query<EventsQuery>,
    headers: HeaderMap,
) -> Response {
    let header_token = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| {
            v.strip_prefix("Bearer ")
                .or_else(|| v.strip_prefix("bearer "))
        })
        .map(|s| s.to_string());
    let Some(token) = q.token.or(header_token) else {
        return CallError::new(401, "unauthorized", "缺少 token").into_response();
    };
    let agent = match app.authenticate(token.trim()).await {
        Ok(agent) => agent,
        Err(err) => return err.into_response(),
    };
    let last = headers
        .get("last-event-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let (out_tx, out_rx) = tokio::sync::mpsc::channel::<String>(16);
    tokio::spawn(async move {
        let _ = out_tx.send(crate::sse::comment("open")).await;
        let mut seen = last;
        loop {
            let turn = match app.peek_turn(&agent, id).await {
                Ok(value) => value,
                Err(err) => {
                    let data = json!({"error": err.code, "message": err.message}).to_string();
                    let _ = out_tx.send(crate::sse::event("error", "", &data)).await;
                    break;
                }
            };
            let kind = match turn.get("status").and_then(|v| v.as_str()).unwrap_or("") {
                "your_turn" => "your_turn",
                "game_over" | "match_over" => "game_over",
                _ => "opponent_moved",
            };
            let move_no = turn.get("move_no").and_then(|v| v.as_i64()).unwrap_or(0);
            let match_status = turn
                .get("match_status")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let event_id = format!("{move_no}:{match_status}:{kind}");
            if event_id != seen {
                seen = event_id.clone();
                let data = turn.to_string();
                if out_tx
                    .send(crate::sse::event(kind, &event_id, &data))
                    .await
                    .is_err()
                {
                    break;
                }
            }
            if kind == "game_over" {
                break;
            }
            let slot = app.slot(id);
            let notified = slot.notify.notified();
            tokio::pin!(notified);
            tokio::select! {
                _ = &mut notified => {}
                _ = tokio::time::sleep(std::time::Duration::from_secs(crate::sse::KEEPALIVE_SECS)) => {
                    if out_tx.send(crate::sse::comment("keepalive")).await.is_err() {
                        break;
                    }
                }
            }
        }
    });
    let stream = futures::stream::unfold(out_rx, |mut rx| async move {
        rx.recv()
            .await
            .map(|chunk| (Ok::<_, std::io::Error>(chunk), rx))
    });
    crate::sse::response(stream)
}

async fn match_log(State(app): State<App>, Path(id): Path<Uuid>) -> Result<Response, CallError> {
    let body = app.match_log(id, 0).await?;
    Ok((
        [(header::CONTENT_TYPE, "application/x-ndjson; charset=utf-8")],
        body,
    )
        .into_response())
}

async fn keys_doc(State(app): State<App>) -> Json<Value> {
    Json(app.server_keys_doc())
}

async fn skill_doc(State(app): State<App>) -> Json<Value> {
    let mut doc = app.skill_doc();
    let (_bytes, sha) = skill_bundle(&app.signing_origin(), &app.config().public_base_url);
    doc["sha256"] = json!(sha);
    Json(doc)
}

pub fn cli_tarball() -> &'static [u8] {
    include_bytes!(concat!(env!("OUT_DIR"), "/cli.tgz"))
}

async fn cli_tgz() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "application/gzip"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"cli.tgz\"",
            ),
            (header::CACHE_CONTROL, "public, max-age=300"),
        ],
        cli_tarball(),
    )
}

async fn skill_zip(State(app): State<App>) -> impl IntoResponse {
    let (bytes, _sha) = skill_bundle(&app.signing_origin(), &app.config().public_base_url);
    (
        [
            (header::CONTENT_TYPE, "application/zip"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"arena-skill-v1.0.0.zip\"",
            ),
            (header::CACHE_CONTROL, "public, max-age=300"),
        ],
        bytes,
    )
}

fn skill_bundle(origin: &str, public_base_url: &str) -> (Vec<u8>, String) {
    use std::io::Write;
    use std::sync::OnceLock;
    static ZIP: OnceLock<(Vec<u8>, String)> = OnceLock::new();
    let origin = origin.to_string();
    let public_base_url = public_base_url.to_string();
    ZIP.get_or_init(|| {
        let skill_md = crate::crypto::materialize_skill_md(&origin, &public_base_url);
        let sign_mjs = crate::crypto::materialize_sign_mjs(&origin);
        let mut cursor = std::io::Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut cursor);
            let modified =
                zip::DateTime::from_date_and_time(2026, 1, 1, 0, 0, 0).expect("zip time");
            let opts = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated)
                .last_modified_time(modified)
                .unix_permissions(0o644);
            zip.start_file("arena/SKILL.md", opts).ok();
            zip.write_all(skill_md.as_bytes()).ok();
            zip.start_file("arena/scripts/sign.mjs", opts).ok();
            zip.write_all(sign_mjs.as_bytes()).ok();
            zip.start_file("arena/scripts/usage.mjs", opts).ok();
            zip.write_all(include_str!("../skills/arena/scripts/usage.mjs").as_bytes())
                .ok();
            zip.start_file("arena/scripts/listen.mjs", opts).ok();
            zip.write_all(include_str!("../skills/arena/scripts/listen.mjs").as_bytes())
                .ok();
            zip.finish().ok();
        }
        let bytes = cursor.into_inner();
        let sha = crate::crypto::sha256_hex(&bytes);
        (bytes, sha)
    })
    .clone()
}

#[derive(Deserialize)]
struct BoardQuery {
    game: Option<String>,
}

async fn leaderboard(
    State(app): State<App>,
    Query(q): Query<BoardQuery>,
) -> Result<Json<Value>, CallError> {
    Ok(Json(
        app.leaderboard(q.game.as_deref().unwrap_or("gomoku"))
            .await?,
    ))
}

async fn user_profile(
    State(app): State<App>,
    Path(handle): Path<String>,
) -> Result<Json<Value>, CallError> {
    Ok(Json(app.user_profile(&handle).await?))
}

async fn agent_profile(
    State(app): State<App>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, CallError> {
    Ok(Json(app.agent_profile(id).await?))
}

async fn cosmetics_catalog() -> Json<Value> {
    Json(json!({
        "items": crate::cosmetics::catalog_json(),
        "presets": crate::cosmetics::presets_json(),
    }))
}

async fn agent_cosmetics(
    State(app): State<App>,
    jar: CookieJar,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, CallError> {
    let user = require_user(&app, &jar).await?;
    crate::cosmetics::owned(app.pool(), user.id, id).await?;
    Ok(Json(crate::cosmetics::profile(app.pool(), id).await?))
}

async fn update_cosmetics(
    State(app): State<App>,
    jar: CookieJar,
    Path(id): Path<Uuid>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, CallError> {
    let user = require_user(&app, &jar).await?;
    crate::cosmetics::owned(app.pool(), user.id, id).await?;
    Ok(Json(
        crate::cosmetics::update_profile(app.pool(), &app.config().data_dir, id, &body).await?,
    ))
}

async fn upload_avatar(
    State(app): State<App>,
    jar: CookieJar,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, CallError> {
    let user = require_user(&app, &jar).await?;
    crate::cosmetics::owned(app.pool(), user.id, id).await?;
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    Ok(Json(
        crate::cosmetics::save_avatar(app.pool(), &app.config().data_dir, id, content_type, &body)
            .await?,
    ))
}

async fn clear_avatar(
    State(app): State<App>,
    jar: CookieJar,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, CallError> {
    let user = require_user(&app, &jar).await?;
    crate::cosmetics::owned(app.pool(), user.id, id).await?;
    Ok(Json(
        crate::cosmetics::update_profile(
            app.pool(),
            &app.config().data_dir,
            id,
            &json!({"clear_avatar": true}),
        )
        .await?,
    ))
}

async fn fresh_unlocks(State(app): State<App>, jar: CookieJar) -> Result<Json<Value>, CallError> {
    let user = require_user(&app, &jar).await?;
    Ok(Json(
        crate::cosmetics::fresh_for_user(app.pool(), user.id).await?,
    ))
}

async fn ack_unlocks(State(app): State<App>, jar: CookieJar) -> Result<Json<Value>, CallError> {
    let user = require_user(&app, &jar).await?;
    Ok(Json(
        crate::cosmetics::ack_for_user(app.pool(), user.id).await?,
    ))
}

async fn avatar_file(State(app): State<App>, Path(file): Path<String>) -> Response {
    let Some(id) = file
        .strip_suffix(".webp")
        .and_then(|raw| Uuid::parse_str(raw).ok())
    else {
        return CallError::new(404, "not_found", "没有这张头像").into_response();
    };
    let path = crate::cosmetics::avatar_path(&app.config().data_dir, id);
    match std::fs::read(&path) {
        Ok(bytes) => {
            let mut response = Response::new(Body::from(bytes));
            response.headers_mut().insert(
                header::CONTENT_TYPE,
                header::HeaderValue::from_static("image/webp"),
            );
            response.headers_mut().insert(
                header::CACHE_CONTROL,
                header::HeaderValue::from_static("public, max-age=3600"),
            );
            response
        }
        Err(_) => CallError::new(404, "not_found", "没有这张头像").into_response(),
    }
}

async fn ws_leaderboard(
    State(app): State<App>,
    Path(game): Path<String>,
    ws: WebSocketUpgrade,
) -> Response {
    ws.on_upgrade(move |socket| leaderboard_socket(app, game, socket))
}

async fn leaderboard_socket(app: App, game: String, mut socket: WebSocket) {
    let mut last = String::new();
    let mut tick = tokio::time::interval(std::time::Duration::from_secs(2));
    loop {
        tokio::select! {
            incoming = socket.recv() => {
                match incoming {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(_)) => break,
                    _ => {}
                }
            }
            _ = tick.tick() => {
                let Ok(body) = app.leaderboard(&game).await else { continue };
                let text = body.to_string();
                if text != last {
                    last = text.clone();
                    if socket.send(Message::Text(json!({"type":"leaderboard","data": body}).to_string().into())).await.is_err() {
                        break;
                    }
                }
            }
        }
    }
}

async fn ws_match(
    State(app): State<App>,
    Path(id): Path<Uuid>,
    ws: WebSocketUpgrade,
) -> Result<impl IntoResponse, CallError> {
    let _ = app.public_match(id).await?;
    let slot = app.slot(id);
    Ok(ws.on_upgrade(move |socket| handle_socket(app, id, slot, socket)))
}

async fn handle_socket(
    app: App,
    id: Uuid,
    slot: std::sync::Arc<crate::room::Slot>,
    mut socket: WebSocket,
) {
    let n = slot.viewers.fetch_add(1, Ordering::Relaxed) + 1;
    let mut rx = slot.events.subscribe();
    if let Ok(snap) = app.public_match(id).await {
        let _ = socket
            .send(Message::Text(
                json!({"type":"snapshot","data": snap}).to_string().into(),
            ))
            .await;
    }
    let _ = socket
        .send(Message::Text(
            json!({"type":"viewers","count": n}).to_string().into(),
        ))
        .await;
    let mut ping = tokio::time::interval(std::time::Duration::from_secs(30));
    ping.tick().await;
    loop {
        tokio::select! {
            incoming = socket.recv() => {
                match incoming {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Ping(p))) => { let _ = socket.send(Message::Pong(p)).await; }
                    Some(Err(_)) => break,
                    _ => {}
                }
            }
            ev = rx.recv() => {
                match ev {
                    Ok(text) => {
                        if socket.send(Message::Text(text.into())).await.is_err() { break; }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        if let Ok(snap) = app.public_match(id).await {
                            let msg = json!({"type":"snapshot","data": snap}).to_string();
                            if socket.send(Message::Text(msg.into())).await.is_err() { break; }
                        }
                    }
                    Err(_) => break,
                }
            }
            _ = ping.tick() => {
                if socket.send(Message::Ping(Vec::new().into())).await.is_err() { break; }
            }
        }
    }
    let n = slot.viewers.fetch_sub(1, Ordering::Relaxed) - 1;
    let _ = slot
        .events
        .send(json!({"type":"viewers","count": n}).to_string());
}

async fn spa(headers: HeaderMap, uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };
    if path.ends_with(".br") || path.ends_with(".gz") {
        return (StatusCode::NOT_FOUND, "not found").into_response();
    }
    if let Some(response) = asset_response(path, &headers) {
        return response;
    }
    if !path.starts_with("api/") && !path.starts_with("auth/") && !path.contains('.') {
        if let Some(response) = asset_response("index.html", &headers) {
            return response;
        }
    }
    (StatusCode::NOT_FOUND, "not found").into_response()
}

fn asset_response(path: &str, headers: &HeaderMap) -> Option<Response> {
    let accept = headers
        .get(header::ACCEPT_ENCODING)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let (body, encoding) = encoded_asset(path, accept)?;
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    let cache = cache_control_for(path);
    let mut builder = Response::builder()
        .header(header::CONTENT_TYPE, mime.as_ref())
        .header(header::CACHE_CONTROL, cache)
        .header(header::VARY, "Accept-Encoding");
    if cache.starts_with("public, max-age=31536000") {
        builder = builder
            .header("CDN-Cache-Control", "public, max-age=31536000")
            .header("Cloudflare-CDN-Cache-Control", "public, max-age=31536000");
    }
    if let Some(encoding) = encoding {
        builder = builder.header(header::CONTENT_ENCODING, encoding);
    }
    Some(builder.body(Body::from(body)).unwrap())
}

pub(crate) fn cache_control_for(path: &str) -> &'static str {
    if path == "index.html" || path.ends_with(".html") {
        "no-cache"
    } else if path.starts_with("assets/") {
        "public, max-age=31536000, immutable"
    } else {
        "public, max-age=86400"
    }
}

pub(crate) fn preferred_encodings(accept: &str) -> Vec<&'static str> {
    let mut br = false;
    let mut gzip = false;
    for part in accept.split(',') {
        let mut bits = part.split(';');
        let name = bits.next().unwrap_or("").trim();
        let mut allowed = true;
        for param in bits {
            let param = param.trim();
            if let Some(q) = param.strip_prefix("q=") {
                if q == "0" || q.starts_with("0.0") {
                    allowed = false;
                }
            }
        }
        if !allowed {
            continue;
        }
        if name.eq_ignore_ascii_case("br") {
            br = true;
        } else if name.eq_ignore_ascii_case("gzip") {
            gzip = true;
        }
    }
    let mut out = Vec::new();
    if br {
        out.push("br");
    }
    if gzip {
        out.push("gzip");
    }
    out
}

fn encoded_asset(path: &str, accept: &str) -> Option<(Vec<u8>, Option<&'static str>)> {
    for enc in preferred_encodings(accept) {
        let ext = if enc == "br" { "br" } else { "gz" };
        if let Some(file) = Assets::get(&format!("{path}.{ext}")) {
            return Some((file.data.to_vec(), Some(enc)));
        }
    }
    let file = Assets::get(path)?;
    Some((file.data.to_vec(), None))
}

async fn require_user(app: &App, jar: &CookieJar) -> Result<crate::app::User, CallError> {
    let Some(c) = jar.get("arena_session") else {
        return Err(CallError::new(401, "unauthorized", "请先登录"));
    };
    app.user_by_session(c.value())
        .await?
        .ok_or_else(|| CallError::new(401, "unauthorized", "登录已过期"))
}

fn user_json(user: &crate::app::User) -> Value {
    json!({
        "id": user.id,
        "handle": user.handle,
        "email": user.email,
        "role": user.role,
        "github_login": user.github_login,
        "has_password": user.has_password,
    })
}

#[derive(Deserialize)]
struct OAuthStartQuery {
    intent: Option<String>,
}

async fn github_start(
    State(app): State<App>,
    headers: HeaderMap,
    jar: CookieJar,
    Query(q): Query<OAuthStartQuery>,
) -> Response {
    if !app.config().github_ready() {
        return redirect(jar, "/login?error=github_unconfigured");
    }
    let intent = q.intent.unwrap_or_else(|| "login".into());
    let (purpose, user_id) = if intent == "link" {
        match require_user(&app, &jar).await {
            Ok(user) => ("link", Some(user.id)),
            Err(_) => return redirect(jar, "/login?error=github_login_required"),
        }
    } else {
        ("login", None)
    };
    let state = match app.begin_oauth(purpose, user_id).await {
        Ok(state) => state,
        Err(_) => return redirect(jar, "/login?error=github_upstream"),
    };
    let url = crate::github::authorize_url(app.config(), &state);
    let jar = jar.add(oauth_state_cookie(&state, secure(&headers, &app)));
    redirect(jar, &url)
}

#[derive(Deserialize)]
struct OAuthCallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

async fn github_callback(
    State(app): State<App>,
    headers: HeaderMap,
    jar: CookieJar,
    Query(q): Query<OAuthCallbackQuery>,
) -> Response {
    let state = q.state.unwrap_or_default();
    let cookie = jar
        .get("arena_oauth_state")
        .map(|c| c.value().to_string())
        .unwrap_or_default();
    let jar = clear_oauth_cookie(jar);
    if state.is_empty() || cookie != state {
        return redirect(jar, "/login?error=github_state");
    }
    let pending = match app.take_oauth_state(&state).await {
        Ok(Some(pending)) => pending,
        _ => return redirect(jar, "/login?error=github_state"),
    };
    let fail = |jar: CookieJar, code: &str| -> Response {
        let loc = if pending.purpose == "link" {
            format!("/agents?error={code}")
        } else {
            format!("/login?error={code}")
        };
        redirect(jar, &loc)
    };
    if q.error.is_some() {
        return fail(jar, "github_denied");
    }
    let Some(code) = q.code.filter(|s| !s.is_empty()) else {
        return fail(jar, "github_denied");
    };
    let gh = match crate::github::exchange(app.config(), &code).await {
        Ok(gh) => gh,
        Err(err) => {
            let code = if err.code == "github_email" {
                "github_email"
            } else {
                "github_upstream"
            };
            return fail(jar, code);
        }
    };
    if pending.purpose == "link" {
        let Some(user_id) = pending.user_id else {
            return redirect(jar, "/login?error=github_state");
        };
        if let Err(err) = app.link_github(user_id, &gh).await {
            let code = if err.code == "github_taken" || err.code == "conflict" {
                "github_taken"
            } else {
                "github_upstream"
            };
            return redirect(jar, &format!("/agents?error={code}"));
        }
        let sid = match app.create_session(user_id).await {
            Ok(sid) => sid,
            Err(_) => return redirect(jar, "/agents?error=github_upstream"),
        };
        let jar = jar.add(session_cookie(&sid, secure(&headers, &app)));
        return redirect(jar, "/agents?github=linked");
    }
    match app.login_with_github(&gh).await {
        Ok((_, sid)) => {
            let jar = jar.add(session_cookie(&sid, secure(&headers, &app)));
            redirect(jar, "/agents")
        }
        Err(err) => {
            let code = match err.code {
                "github_email" => "github_email",
                "github_taken" | "conflict" => "github_taken",
                _ => "github_upstream",
            };
            redirect(jar, &format!("/login?error={code}"))
        }
    }
}

async fn github_unlink(State(app): State<App>, jar: CookieJar) -> Result<Json<Value>, CallError> {
    let user = require_user(&app, &jar).await?;
    let user = app.unlink_github(&user).await?;
    Ok(Json(json!({"user": user_json(&user)})))
}

#[derive(Deserialize)]
struct PasswordBody {
    password: String,
    current_password: Option<String>,
}

async fn set_password(
    State(app): State<App>,
    jar: CookieJar,
    Json(body): Json<PasswordBody>,
) -> Result<Json<Value>, CallError> {
    let user = require_user(&app, &jar).await?;
    let user = app
        .set_password(&user, &body.password, body.current_password.as_deref())
        .await?;
    Ok(Json(json!({"user": user_json(&user)})))
}

async fn connect_sh(State(app): State<App>) -> impl IntoResponse {
    script_response(
        include_str!("../scripts/connect.sh"),
        &app.config().public_base_url,
    )
}

async fn connect_ps1(State(app): State<App>) -> impl IntoResponse {
    script_response(
        include_str!("../scripts/connect.ps1"),
        &app.config().public_base_url,
    )
}

fn script_response(source: &str, base: &str) -> Response {
    let body = source.replace("https://agent.idou.bid", base.trim_end_matches('/'));
    Response::builder()
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-cache")
        .body(Body::from(body))
        .unwrap()
}

fn redirect(jar: CookieJar, loc: &str) -> Response {
    (jar, (StatusCode::FOUND, [(header::LOCATION, loc)])).into_response()
}

fn oauth_state_cookie(state: &str, secure: bool) -> Cookie<'static> {
    Cookie::build(("arena_oauth_state", state.to_string()))
        .path("/")
        .http_only(true)
        .same_site(cookie::SameSite::Lax)
        .max_age(cookie::time::Duration::minutes(10))
        .secure(secure)
        .build()
}

fn clear_oauth_cookie(jar: CookieJar) -> CookieJar {
    let mut dead = Cookie::from("arena_oauth_state");
    dead.set_path("/");
    jar.remove(dead)
}

fn session_cookie(sid: &str, secure: bool) -> Cookie<'static> {
    Cookie::build(("arena_session", sid.to_string()))
        .path("/")
        .http_only(true)
        .same_site(cookie::SameSite::Lax)
        .max_age(cookie::time::Duration::days(30))
        .secure(secure)
        .build()
}

fn secure(headers: &HeaderMap, app: &App) -> bool {
    if let Some(v) = headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
    {
        return v.split(',').next().unwrap_or("").trim() == "https";
    }
    if let Some(v) = headers.get("cf-visitor").and_then(|v| v.to_str().ok()) {
        return v.contains("https");
    }
    app.config().cookie_secure
}

#[cfg(test)]
mod cache_tests {
    use super::{cache_control_for, preferred_encodings};

    #[test]
    fn hashed_assets_are_immutable_and_html_is_not() {
        assert_eq!(
            cache_control_for("assets/index-abc123.js"),
            "public, max-age=31536000, immutable"
        );
        assert_eq!(
            cache_control_for("assets/index-abc123.css"),
            "public, max-age=31536000, immutable"
        );
        assert_eq!(cache_control_for("index.html"), "no-cache");
        assert_eq!(cache_control_for("favicon.svg"), "public, max-age=86400");
    }

    #[test]
    fn brotli_wins_when_the_client_accepts_it() {
        assert_eq!(preferred_encodings("gzip, deflate, br"), vec!["br", "gzip"]);
        assert_eq!(preferred_encodings("gzip"), vec!["gzip"]);
        assert_eq!(preferred_encodings("br;q=0, gzip"), vec!["gzip"]);
        assert!(preferred_encodings("identity").is_empty());
    }
}
