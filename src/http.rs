//! Browser HTTP API, WebSocket spectating, and the embedded SPA.

use crate::app::App;
use crate::error::CallError;
use axum::body::Body;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use axum_extra::extract::cookie::{Cookie, CookieJar};
use rust_embed::Embed;
use serde::Deserialize;
use tower_http::trace::TraceLayer;
use serde_json::{json, Value};
use std::sync::atomic::Ordering;
use uuid::Uuid;

#[derive(Embed)]
#[folder = "web/dist"]
struct Assets;

pub fn router(app: App) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/mcp", post(crate::mcp::mcp_post).get(crate::mcp::mcp_get).delete(crate::mcp::mcp_delete))
        .route("/ws/match/:id", get(ws_match))
        .route("/api/auth/register", post(register))
        .route("/api/auth/login", post(login))
        .route("/api/auth/logout", post(logout))
        .route("/api/me", get(me))
        .route("/api/agents", get(list_agents).post(create_agent))
        .route("/api/agents/:id/tokens", post(issue_token))
        .route("/api/agents/:id/tokens/:tid", axum::routing::delete(revoke_token))
        .route("/api/games", get(games))
        .route("/api/games/:id", get(game))
        .route("/api/games/:id/unpublish", post(unpublish_game))
        .route("/api/games/:id/hide", post(hide_game))
        .route("/api/uploads/check", post(check_rules))
        .route("/api/uploads/publish", post(publish_rules))
        .route("/sdk/game.ts", get(sdk_ts))
        .route("/examples/tic-tac-toe.js", get(example_js))
        .route("/api/matches", get(matches))
        .route("/api/matches/:id", get(match_one))
        .route("/api/matches/:id/log.jsonl", get(match_log))
        .route("/api/leaderboard", get(leaderboard))
        .route("/api/users/:handle", get(user_profile))
        .route("/api/agents/:id/profile", get(agent_profile))
        .route("/ws/leaderboard/:game", get(ws_leaderboard))
        .route("/.well-known/arena-keys.json", get(keys_doc))
        .route("/.well-known/arena-skill.json", get(skill_doc))
        .route("/skill/arena-skill-v1.0.0.zip", get(skill_zip))
        .fallback(spa)
        .layer(TraceLayer::new_for_http())
        .with_state(app)
}

async fn healthz() -> &'static str {
    "ok"
}

async fn readyz(State(app): State<App>) -> Result<&'static str, CallError> {
    sqlx::query("SELECT 1").execute(app.pool()).await.map_err(|_| CallError::new(503, "not_ready", "数据库未就绪"))?;
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
    let (user, sid) = app.register(&body.email, &body.password, &body.handle).await?;
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
        .create_agent(&user, &body.name, body.description.as_deref(), body.model_label.as_deref(), body.allow_comments)
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

async fn check_rules(State(app): State<App>, jar: CookieJar, Json(body): Json<SourceBody>) -> Result<Json<Value>, CallError> {
    let user = require_user(&app, &jar).await?;
    Ok(Json(app.check_rules(&user, &body.source).await?))
}

async fn publish_rules(State(app): State<App>, jar: CookieJar, Json(body): Json<SourceBody>) -> Result<Json<Value>, CallError> {
    let user = require_user(&app, &jar).await?;
    let slug = body.slug.unwrap_or_default();
    Ok(Json(
        app.publish_rules(&user, &body.source, &slug, body.name.as_deref(), body.description.as_deref())
            .await?,
    ))
}

async fn unpublish_game(State(app): State<App>, jar: CookieJar, Path(id): Path<String>) -> Result<Json<Value>, CallError> {
    let user = require_user(&app, &jar).await?;
    Ok(Json(app.unpublish_game(&user, &id).await?))
}

async fn hide_game(State(app): State<App>, jar: CookieJar, Path(id): Path<String>) -> Result<Json<Value>, CallError> {
    let user = require_user(&app, &jar).await?;
    Ok(Json(app.hide_game(&user, &id).await?))
}

async fn sdk_ts() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], include_str!("../sdk/game.ts"))
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

async fn matches(State(app): State<App>, Query(q): Query<MatchQuery>) -> Result<Json<Value>, CallError> {
    let _ = q.live;
    Ok(Json(json!(app.list_live().await?)))
}

async fn match_one(State(app): State<App>, Path(id): Path<Uuid>) -> Result<Json<Value>, CallError> {
    Ok(Json(app.public_match(id).await?))
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
    Json(app.skill_doc())
}

async fn skill_zip() -> impl IntoResponse {
    let bytes = skill_zip_bytes();
    (
        [
            (header::CONTENT_TYPE, "application/zip"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"arena-skill-v1.0.0.zip\"",
            ),
        ],
        bytes,
    )
}

fn skill_zip_bytes() -> Vec<u8> {
    use std::io::Write;
    use std::sync::OnceLock;
    static ZIP: OnceLock<Vec<u8>> = OnceLock::new();
    ZIP.get_or_init(|| {
        let mut cursor = std::io::Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut cursor);
            let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
            zip.start_file("arena/SKILL.md", opts).ok();
            zip.write_all(include_str!("../skill/arena/SKILL.md").as_bytes()).ok();
            zip.start_file("arena/scripts/sign.mjs", opts).ok();
            zip.write_all(include_str!("../skill/arena/scripts/sign.mjs").as_bytes()).ok();
            zip.finish().ok();
        }
        cursor.into_inner()
    })
    .clone()
}

#[derive(Deserialize)]
struct BoardQuery {
    game: Option<String>,
}

async fn leaderboard(State(app): State<App>, Query(q): Query<BoardQuery>) -> Result<Json<Value>, CallError> {
    Ok(Json(app.leaderboard(q.game.as_deref().unwrap_or("gomoku")).await?))
}

async fn user_profile(State(app): State<App>, Path(handle): Path<String>) -> Result<Json<Value>, CallError> {
    Ok(Json(app.user_profile(&handle).await?))
}

async fn agent_profile(State(app): State<App>, Path(id): Path<Uuid>) -> Result<Json<Value>, CallError> {
    Ok(Json(app.agent_profile(id).await?))
}

async fn ws_leaderboard(State(app): State<App>, Path(game): Path<String>, ws: WebSocketUpgrade) -> Response {
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
                    if socket.send(Message::Text(json!({"type":"leaderboard","data": body}).to_string())).await.is_err() {
                        break;
                    }
                }
            }
        }
    }
}

async fn ws_match(State(app): State<App>, Path(id): Path<Uuid>, ws: WebSocketUpgrade) -> Result<impl IntoResponse, CallError> {
    let _ = app.public_match(id).await?;
    let slot = app.slot(id);
    Ok(ws.on_upgrade(move |socket| handle_socket(app, id, slot, socket)))
}

async fn handle_socket(app: App, id: Uuid, slot: std::sync::Arc<crate::room::Slot>, mut socket: WebSocket) {
    let n = slot.viewers.fetch_add(1, Ordering::Relaxed) + 1;
    let mut rx = slot.events.subscribe();
    if let Ok(snap) = app.public_match(id).await {
        let _ = socket
            .send(Message::Text(json!({"type":"snapshot","data": snap}).to_string()))
            .await;
    }
    let _ = socket
        .send(Message::Text(json!({"type":"viewers","count": n}).to_string()))
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
                        if socket.send(Message::Text(text)).await.is_err() { break; }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        if let Ok(snap) = app.public_match(id).await {
                            let msg = json!({"type":"snapshot","data": snap}).to_string();
                            if socket.send(Message::Text(msg)).await.is_err() { break; }
                        }
                    }
                    Err(_) => break,
                }
            }
            _ = ping.tick() => {
                if socket.send(Message::Ping(Vec::new())).await.is_err() { break; }
            }
        }
    }
    let n = slot.viewers.fetch_sub(1, Ordering::Relaxed) - 1;
    let _ = slot.events.send(json!({"type":"viewers","count": n}).to_string());
}

async fn spa(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };
    if let Some(file) = Assets::get(path) {
        return asset_response(path, file.data.as_ref());
    }
    if !path.starts_with("api/") && !path.contains('.') {
        if let Some(file) = Assets::get("index.html") {
            return asset_response("index.html", file.data.as_ref());
        }
    }
    (StatusCode::NOT_FOUND, "not found").into_response()
}

fn asset_response(path: &str, data: &[u8]) -> Response {
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    Response::builder()
        .header(header::CONTENT_TYPE, mime.as_ref())
        .body(Body::from(data.to_vec()))
        .unwrap()
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
    })
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
    if let Some(v) = headers.get("x-forwarded-proto").and_then(|v| v.to_str().ok()) {
        return v.split(',').next().unwrap_or("").trim() == "https";
    }
    if let Some(v) = headers.get("cf-visitor").and_then(|v| v.to_str().ok()) {
        return v.contains("https");
    }
    app.config().cookie_secure
}
