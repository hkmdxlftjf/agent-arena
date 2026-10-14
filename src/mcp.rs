//! Streamable HTTP MCP. One JSON-RPC message per POST; SSE is only the
//! server-initiated channel (heartbeats). Bearer tokens identify the agent.

use crate::app::{AgentCtx, App};
use crate::error::CallError;
use crate::sse;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use futures::stream;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, watch};
use uuid::Uuid;

struct Session {
    agent_id: Uuid,
    tx: broadcast::Sender<String>,
    stops: Mutex<HashMap<Uuid, watch::Sender<bool>>>,
}

fn sessions() -> &'static Mutex<HashMap<String, Session>> {
    static S: OnceLock<Mutex<HashMap<String, Session>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(HashMap::new()))
}

fn new_session(agent_id: Uuid) -> Session {
    Session {
        agent_id,
        tx: broadcast::channel(64).0,
        stops: Mutex::new(HashMap::new()),
    }
}

fn limiter() -> &'static Mutex<HashMap<Uuid, (u32, Instant)>> {
    static L: OnceLock<Mutex<HashMap<Uuid, (u32, Instant)>>> = OnceLock::new();
    L.get_or_init(|| Mutex::new(HashMap::new()))
}

pub async fn mcp_post(State(app): State<App>, headers: HeaderMap, body: String) -> Response {
    let agent = match bearer(&app, &headers).await {
        Ok(a) => a,
        Err(err) => return Responseish::err(StatusCode::UNAUTHORIZED, err).into_response(),
    };
    if !allow(&agent.agent_id) {
        return Responseish::err(
            StatusCode::TOO_MANY_REQUESTS,
            CallError::new(429, "rate_limited", "调用过于频繁"),
        )
        .into_response();
    }
    let msg: Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(_) => {
            return Responseish::json(
                StatusCode::OK,
                rpc_error(Value::Null, -32700, "parse error"),
                None,
            )
            .into_response()
        }
    };
    if msg.get("id").is_none() {
        return Responseish::empty(StatusCode::ACCEPTED).into_response();
    }
    let id = msg.get("id").cloned().unwrap_or(Value::Null);
    let method = msg.get("method").and_then(|v| v.as_str()).unwrap_or("");
    let params = msg.get("params").cloned().unwrap_or(json!({}));
    if method != "initialize" {
        if let Some(sid) = header(&headers, "mcp-session-id") {
            let ok = sessions()
                .lock()
                .expect("sessions")
                .get(sid)
                .map(|s| s.agent_id)
                == Some(agent.agent_id);
            if !ok {
                return Responseish::err(
                    StatusCode::NOT_FOUND,
                    CallError::new(404, "session", "MCP 会话不存在"),
                )
                .into_response();
            }
        }
    }
    let (result, new_session) = match method {
        "initialize" => {
            let sid = Uuid::new_v4().to_string();
            sessions()
                .lock()
                .expect("sessions")
                .insert(sid.clone(), new_session(agent.agent_id));
            let proto = params
                .get("protocolVersion")
                .and_then(|v| v.as_str())
                .unwrap_or("2025-03-26");
            (
                json!({
                    "protocolVersion": proto,
                    "capabilities": {
                        "tools": {},
                        "resources": {"subscribe": true}
                    },
                    "serverInfo": {"name": "agent-arena", "version": "0.1.0"},
                    "instructions": INSTRUCTIONS,
                }),
                Some(sid),
            )
        }
        "ping" => {
            let _ = app.heartbeat_queue(agent.agent_id).await;
            (json!({}), None)
        }
        "tools/list" => (json!({"tools": tools()}), None),
        "tools/call" => {
            let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            if name == "wait_for_turn" && sse::accepts_sse(&headers) {
                let match_id = match parse_match(&args) {
                    Ok(id) => id,
                    Err(err) => {
                        return Responseish::json(
                            StatusCode::OK,
                            json!({"jsonrpc":"2.0","id": id, "result": {
                                "content": [{"type":"text","text": err.message}],
                                "isError": true
                            }}),
                            None,
                        )
                        .into_response();
                    }
                };
                let progress = params
                    .pointer("/_meta/progressToken")
                    .cloned()
                    .unwrap_or(json!("wait"));
                return stream_wait(app, agent, match_id, id, progress).await;
            }
            match call_tool(&app, &agent, name, &args).await {
                Ok(value) => (
                    json!({
                        "content": [{"type": "text", "text": serde_json::to_string(&value).unwrap_or_else(|_| "{}".into())}],
                        "isError": value.get("accepted") == Some(&json!(false)) || value.get("error").is_some(),
                        "structuredContent": value,
                    }),
                    None,
                ),
                Err(err) => (
                    json!({
                        "content": [{"type": "text", "text": serde_json::to_string(&json!({"error": err.code, "message": err.message})).unwrap()}],
                        "isError": true,
                    }),
                    None,
                ),
            }
        }
        "resources/list" => (
            json!({"resources": [{
                "uriTemplate": "arena://match/{id}/state",
                "name": "match-state",
                "description": "本席位的紧凑局面。订阅后，GET /mcp 的 SSE 会收到 notifications/resources/updated，params.state 是紧凑局面。",
                "mimeType": "application/json"
            }]}),
            None,
        ),
        "resources/read" => {
            let uri = params.get("uri").and_then(|v| v.as_str()).unwrap_or("");
            match read_resource(&app, &agent, uri).await {
                Ok(value) => (value, None),
                Err(err) => {
                    return Responseish::json(
                        StatusCode::OK,
                        rpc_error(id.clone(), -32000, &err.message),
                        None,
                    )
                    .into_response();
                }
            }
        }
        "resources/subscribe" => {
            let uri = params.get("uri").and_then(|v| v.as_str()).unwrap_or("");
            match subscribe_resource(&app, &agent, header(&headers, "mcp-session-id"), uri).await {
                Ok(value) => (value, None),
                Err(err) => {
                    return Responseish::json(
                        StatusCode::OK,
                        rpc_error(id.clone(), -32000, &err.message),
                        None,
                    )
                    .into_response();
                }
            }
        }
        "resources/unsubscribe" => {
            let uri = params.get("uri").and_then(|v| v.as_str()).unwrap_or("");
            (
                unsubscribe_resource(header(&headers, "mcp-session-id"), uri),
                None,
            )
        }
        _ => {
            return Responseish::json(
                StatusCode::OK,
                rpc_error(id, -32601, "method not found"),
                None,
            )
            .into_response();
        }
    };
    Responseish::json(
        StatusCode::OK,
        json!({"jsonrpc": "2.0", "id": id, "result": result}),
        new_session,
    )
    .into_response()
}

pub async fn mcp_get(headers: HeaderMap) -> Response {
    let Some(sid) = header(&headers, "mcp-session-id") else {
        return (StatusCode::BAD_REQUEST, "missing session").into_response();
    };
    let tx = {
        let map = sessions().lock().expect("sessions");
        let Some(session) = map.get(sid) else {
            return (StatusCode::NOT_FOUND, "session").into_response();
        };
        session.tx.clone()
    };
    let mut inbound = tx.subscribe();
    let (out_tx, out_rx) = tokio::sync::mpsc::channel::<String>(32);
    tokio::spawn(async move {
        if out_tx.send(sse::comment("open")).await.is_err() {
            return;
        }
        let mut tick = tokio::time::interval(Duration::from_secs(sse::KEEPALIVE_SECS));
        tick.tick().await;
        let mut n = 0u64;
        loop {
            tokio::select! {
                msg = inbound.recv() => {
                    match msg {
                        Ok(frame) => {
                            if out_tx.send(frame).await.is_err() { break; }
                        }
                        Err(broadcast::error::RecvError::Lagged(_)) => {
                            if out_tx.send(sse::comment("lag")).await.is_err() { break; }
                        }
                        Err(_) => break,
                    }
                }
                _ = tick.tick() => {
                    n += 1;
                    if out_tx.send(sse::comment(&format!("keepalive {n}"))).await.is_err() {
                        break;
                    }
                }
            }
        }
    });
    let stream = stream::unfold(out_rx, |mut rx| async move {
        rx.recv().await.map(|chunk| (Ok(chunk), rx))
    });
    sse::response(stream)
}

pub async fn mcp_delete(State(app): State<App>, headers: HeaderMap) -> StatusCode {
    let session = header(&headers, "mcp-session-id")
        .and_then(|sid| sessions().lock().expect("sessions").remove(sid));
    if let Some(session) = session {
        for stop in session.stops.lock().expect("stops").values() {
            let _ = stop.send(false);
        }
        let _ = app.dequeue(session.agent_id).await;
    }
    StatusCode::NO_CONTENT
}

async fn stream_wait(
    app: App,
    agent: AgentCtx,
    match_id: Uuid,
    rpc_id: Value,
    progress: Value,
) -> Response {
    let _ = app.heartbeat_queue(agent.agent_id).await;
    let (out_tx, out_rx) = tokio::sync::mpsc::channel::<String>(16);
    tokio::spawn(async move {
        let _ = out_tx.send(sse::comment("open")).await;
        let started = Instant::now();
        let mut limit = Duration::from_secs(180);
        let mut limited = false;
        let mut ticks = 0u64;
        loop {
            let turn = match app.peek_turn(&agent, match_id).await {
                Ok(value) => value,
                Err(err) => {
                    let msg = json!({"jsonrpc":"2.0","id": rpc_id, "error": {"code": -32000, "message": err.message}});
                    let _ = out_tx
                        .send(sse::event("message", "", &msg.to_string()))
                        .await;
                    break;
                }
            };
            if !limited {
                limit = hold_for(&turn);
                limited = true;
            }
            let status = turn.get("status").and_then(|v| v.as_str()).unwrap_or("");
            let done = matches!(status, "your_turn" | "game_over" | "match_over")
                || started.elapsed() >= limit;
            if done {
                let result = json!({
                    "content": [{"type": "text", "text": turn.to_string()}],
                    "isError": false,
                    "structuredContent": turn,
                });
                let msg = json!({"jsonrpc":"2.0","id": rpc_id, "result": result});
                let _ = out_tx
                    .send(sse::event("message", "", &msg.to_string()))
                    .await;
                break;
            }
            let slot = app.slot(match_id);
            let notified = slot.notify.notified();
            tokio::pin!(notified);
            tokio::select! {
                _ = &mut notified => {}
                _ = tokio::time::sleep(Duration::from_secs(sse::KEEPALIVE_SECS)) => {
                    ticks += 1;
                    if out_tx.send(sse::comment(&format!("keepalive {ticks}"))).await.is_err() {
                        break;
                    }
                    let note = json!({
                        "jsonrpc": "2.0",
                        "method": "notifications/progress",
                        "params": {
                            "progressToken": progress,
                            "progress": ticks,
                            "message": "waiting"
                        }
                    });
                    if out_tx
                        .send(sse::event("message", "", &note.to_string()))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
            }
        }
    });
    let stream = stream::unfold(out_rx, |mut rx| async move {
        rx.recv().await.map(|chunk| (Ok(chunk), rx))
    });
    sse::response(stream)
}

fn hold_for(turn: &Value) -> Duration {
    let now = crate::crypto::now_ms();
    let deadline = turn
        .get("deadline_at")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let expires = turn.get("expires_at").and_then(|v| v.as_i64()).unwrap_or(0);
    let ms = if deadline > now {
        deadline - now + 3_000
    } else if expires > now {
        expires - now
    } else {
        180_000
    };
    Duration::from_millis(ms.clamp(5_000, 600_000) as u64)
}

fn parse_state_uri(uri: &str) -> Result<Uuid, CallError> {
    let rest = uri
        .strip_prefix("arena://match/")
        .and_then(|s| s.strip_suffix("/state"))
        .unwrap_or("");
    Uuid::parse_str(rest)
        .map_err(|_| CallError::new(400, "bad_uri", "资源地址应为 arena://match/<id>/state"))
}

async fn read_resource(app: &App, agent: &AgentCtx, uri: &str) -> Result<Value, CallError> {
    let id = parse_state_uri(uri)?;
    let state = app.get_state(agent, id).await?;
    Ok(json!({
        "contents": [{
            "uri": uri,
            "mimeType": "application/json",
            "text": state.to_string()
        }]
    }))
}

async fn subscribe_resource(
    app: &App,
    agent: &AgentCtx,
    sid: Option<&str>,
    uri: &str,
) -> Result<Value, CallError> {
    let Some(sid) = sid else {
        return Err(CallError::new(
            400,
            "session",
            "订阅需要 mcp-session-id，通知走 GET /mcp",
        ));
    };
    let id = parse_state_uri(uri)?;
    let _ = app.get_state(agent, id).await?;
    let (tx, stop_rx) = {
        let mut map = sessions().lock().expect("sessions");
        let session = map
            .get_mut(sid)
            .ok_or_else(|| CallError::new(404, "session", "MCP 会话不存在"))?;
        if session.agent_id != agent.agent_id {
            return Err(CallError::new(404, "session", "MCP 会话不存在"));
        }
        let (stop_tx, stop_rx) = watch::channel(true);
        if let Some(old) = session.stops.lock().expect("stops").insert(id, stop_tx) {
            let _ = old.send(false);
        }
        (session.tx.clone(), stop_rx)
    };
    let app = app.clone();
    let agent = agent.clone();
    let owned = uri.to_string();
    tokio::spawn(async move {
        watch_resource(app, agent, id, owned, tx, stop_rx).await;
    });
    Ok(json!({"uri": uri}))
}

fn unsubscribe_resource(sid: Option<&str>, uri: &str) -> Value {
    let Ok(id) = parse_state_uri(uri) else {
        return json!({"ok": false});
    };
    if let Some(sid) = sid {
        if let Some(session) = sessions().lock().expect("sessions").get(sid) {
            if let Some(stop) = session.stops.lock().expect("stops").remove(&id) {
                let _ = stop.send(false);
            }
        }
    }
    json!({"ok": true})
}

fn stable_state(state: &Value) -> String {
    let mut copy = state.clone();
    if let Some(obj) = copy.as_object_mut() {
        obj.remove("remaining_ms");
        obj.remove("remaining_sec");
    }
    copy.to_string()
}

async fn watch_resource(
    app: App,
    agent: AgentCtx,
    match_id: Uuid,
    uri: String,
    tx: broadcast::Sender<String>,
    mut stop: watch::Receiver<bool>,
) {
    let mut inbound = app.slot(match_id).events.subscribe();
    let mut last = String::new();
    loop {
        if !*stop.borrow() {
            break;
        }
        if let Ok(state) = app.get_state(&agent, match_id).await {
            let text = stable_state(&state);
            if text != last {
                last = text;
                let note = json!({
                    "jsonrpc": "2.0",
                    "method": "notifications/resources/updated",
                    "params": {"uri": uri, "state": state}
                });
                if tx
                    .send(sse::event("message", "", &note.to_string()))
                    .is_err()
                {
                    break;
                }
            }
        }
        tokio::select! {
            changed = stop.changed() => {
                if changed.is_err() || !*stop.borrow() { break; }
            }
            ev = inbound.recv() => {
                if matches!(ev, Err(broadcast::error::RecvError::Closed)) {
                    inbound = app.slot(match_id).events.subscribe();
                }
            }
            _ = tokio::time::sleep(Duration::from_secs(2)) => {}
        }
    }
}

pub struct Responseish {
    status: StatusCode,
    body: Option<Value>,
    session: Option<String>,
    empty: bool,
}

impl Responseish {
    fn json(status: StatusCode, body: Value, session: Option<String>) -> Self {
        Self {
            status,
            body: Some(body),
            session,
            empty: false,
        }
    }
    fn empty(status: StatusCode) -> Self {
        Self {
            status,
            body: None,
            session: None,
            empty: true,
        }
    }
    fn err(status: StatusCode, err: CallError) -> Self {
        Self {
            status,
            body: Some(json!({"error": err.code, "message": err.message})),
            session: None,
            empty: false,
        }
    }
}

impl IntoResponse for Responseish {
    fn into_response(self) -> Response {
        let mut res = if self.empty {
            self.status.into_response()
        } else {
            (self.status, Json(self.body.unwrap_or(json!({})))).into_response()
        };
        if let Some(sid) = self.session {
            res.headers_mut()
                .insert("mcp-session-id", sid.parse().unwrap());
        }
        res
    }
}

async fn call_tool(
    app: &App,
    agent: &AgentCtx,
    name: &str,
    args: &Value,
) -> Result<Value, CallError> {
    match name {
        "whoami" => app.whoami(agent).await,
        "list_games" => Ok(json!({"games": app.list_games()})),
        "create_room" => app.create_room(agent, args).await,
        "join_room" => {
            let code = args.get("code").and_then(|v| v.as_str()).unwrap_or("");
            app.join_room(agent, code, args.get("kid").and_then(|v| v.as_str()))
                .await
        }
        "get_state" => {
            let id = parse_match(args)?;
            app.get_state_query(agent, id, args).await
        }
        "get_legal_moves" => {
            let id = parse_match(args)?;
            app.get_legal_moves(agent, id, args).await
        }
        "get_history" => {
            let id = parse_match(args)?;
            app.get_history(agent, id, args).await
        }
        "wait_for_turn" => {
            let id = parse_match(args)?;
            app.wait_for_turn(agent, id).await
        }
        "make_move" => {
            let id = parse_match(args)?;
            let envelope = args.get("envelope").cloned().unwrap_or(Value::Null);
            let private = args.get("private").cloned();
            app.make_move(agent, id, envelope, private).await
        }
        "join_queue" => app.join_queue(agent, args).await,
        "queue_status" => app.queue_status(agent).await,
        "leave_queue" => app.leave_queue(agent).await,
        "resign" => {
            let id = parse_match(args)?;
            app.resign(agent, id).await
        }
        "leave_room" => {
            let id = parse_match(args)?;
            app.leave_room(agent.agent_id, id).await
        }
        "get_match_log" => {
            let id = parse_match(args)?;
            let from = args.get("from_seq").and_then(|v| v.as_i64()).unwrap_or(0);
            let jsonl = app.match_log(id, from).await?;
            let entries: Vec<Value> = jsonl
                .lines()
                .filter(|l| !l.trim().is_empty())
                .filter_map(|l| serde_json::from_str(l).ok())
                .collect();
            Ok(json!({"jsonl": jsonl, "entries": entries}))
        }
        "get_key_challenge" => app.key_challenge(agent).await,
        "register_agent_key" => {
            let envelope = args.get("envelope").cloned().unwrap_or(Value::Null);
            app.register_key(agent, &envelope, args.get("label").and_then(|v| v.as_str()))
                .await
        }
        "get_profile" => app.mcp_profile(agent).await,
        "update_profile" => {
            crate::cosmetics::update_profile(
                app.pool(),
                &app.config().data_dir,
                agent.agent_id,
                args,
            )
            .await
        }
        _ => Err(CallError::new(400, "unknown_tool", "未知工具")),
    }
}

fn parse_match(args: &Value) -> Result<Uuid, CallError> {
    let raw = args.get("match_id").and_then(|v| v.as_str()).unwrap_or("");
    Uuid::parse_str(raw).map_err(|_| CallError::new(400, "bad_match", "match_id 不是 UUID"))
}

fn tools() -> Value {
    json!([
        tool("whoami", "当前智能体身份、权限和已注册公钥。", json!({"type":"object","properties":{}})),
        tool("list_games", "可对弈的游戏、变体、默认限时和超时策略。", json!({"type":"object","properties":{}})),
        tool("create_room", "创建休闲房间。若这个智能体正在排位，会先离开队列。game 可以是 gomoku、xiangqi、go、junqi、doudizhu、mahjong-sichuan，或已发布的上传规则（id 形如 u_井字棋标识）。上传规则不计入天梯。time_limit_sec 只能是 60、120、180、300，不填则 180。排位固定 180 秒，不看这个参数。", json!({
            "type":"object",
            "properties":{
                "game":{"type":"string"},
                "variant":{"type":"string"},
                "time_limit_sec":{"type":"integer","enum":[60,120,180,300]},
                "timeout_policy":{"type":"string"},
                "max_consecutive_timeouts":{"type":"integer"},
                "allow_comments":{"type":"boolean"},
                "kid":{"type":"string"}
            },
            "required":["game"]
        })),
        tool("join_room", "用房间码入座。若这个智能体正在排位，会先离开队列。", json!({
            "type":"object",
            "properties":{"code":{"type":"string"},"kid":{"type":"string"}},
            "required":["code"]
        })),
        tool("get_state", "紧凑局面：棋盘摘要、上一两步、prev_hash 和 signing_request。默认不带合法着法、不带整条日志。include_legal_moves 为 true 时可加 filter=near 和 limit。history 为 none、last_8 或 full。format=verbose 才返回旧的完整视图。", json!({
            "type":"object",
            "properties":{
                "match_id":{"type":"string"},
                "include_legal_moves":{"type":"boolean"},
                "filter":{"type":"string"},
                "limit":{"type":"integer"},
                "history":{"type":"string"},
                "format":{"type":"string","enum":["compact","verbose"]}
            },
            "required":["match_id"]
        })),
        tool("get_legal_moves", "单独取合法着法。filter=near 只留离上一手近的点（五子棋、围棋）。limit 截断。", json!({
            "type":"object",
            "properties":{
                "match_id":{"type":"string"},
                "filter":{"type":"string"},
                "limit":{"type":"integer"}
            },
            "required":["match_id"]
        })),
        tool("get_history", "单独取着法记录。from_move 是起始序号，limit 默认 30。", json!({
            "type":"object",
            "properties":{
                "match_id":{"type":"string"},
                "from_move":{"type":"integer"},
                "limit":{"type":"integer"}
            },
            "required":["match_id"]
        })),
        tool("wait_for_turn", "等到轮到你或对局结束。客户端接受 text/event-stream 时，这条调用改走 SSE：轮到你立刻返回紧凑局面，等待期间每 15 秒一条注释和一条 progress，可撑过 Cloudflare Tunnel。不接受 SSE 时仍最多阻塞 25 秒后返回 not_your_turn。", json!({
            "type":"object",
            "properties":{"match_id":{"type":"string"}},
            "required":["match_id"]
        })),
        tool("make_move", "提交 sign.mjs 产出的签名信封。承诺-揭示时附上 private: {data, salt}。", json!({
            "type":"object",
            "properties":{"match_id":{"type":"string"},"envelope":{"type":"object"},"private":{"type":"object"}},
            "required":["match_id","envelope"]
        })),
        tool("join_queue", "进入排位队列。凑齐人数后返回 matched 和 match_id。等待时要定期调用 queue_status，否则 5 分钟后记录失效。", json!({
            "type":"object",
            "properties":{"game":{"type":"string"},"variant":{"type":"string"},"kid":{"type":"string"}},
            "required":["game"]
        })),
        tool("queue_status", "查看自己是在排队、已匹配，还是空闲。调用一次会把排队心跳续 5 分钟。", json!({"type":"object","properties":{}})),
        tool("leave_queue", "退出排位队列。返回 idle，left 表示刚才是否在队列里。关闭 MCP 会话也会退出。超过 5 分钟没有 queue_status、wait_for_turn 或 ping，记录会自动失效。", json!({"type":"object","properties":{}})),
        tool("resign", "认输。由已鉴权的令牌发起，服务器签名记入日志。", json!({
            "type":"object",
            "properties":{"match_id":{"type":"string"}},
            "required":["match_id"]
        })),
        tool("leave_room", "离开房间。开局前腾出席位，房间没人则关闭，没有胜负。开局后立即判负，对方获胜，服务器签名记入日志。", json!({
            "type":"object",
            "properties":{"match_id":{"type":"string"}},
            "required":["match_id"]
        })),
        tool("get_match_log", "哈希链对局日志。", json!({
            "type":"object",
            "properties":{"match_id":{"type":"string"},"from_seq":{"type":"integer"}},
            "required":["match_id"]
        })),
        tool("get_key_challenge", "获取注册公钥用的 signing_request。", json!({"type":"object","properties":{}})),
        tool("register_agent_key", "提交对挑战的签名以登记 Ed25519 公钥。", json!({
            "type":"object",
            "properties":{"envelope":{"type":"object"},"label":{"type":"string"}},
            "required":["envelope"]
        })),
        tool("get_profile", "自己的名字、称号、简介、头像预设、颜色、已解锁和已装备的装扮，以及 queue（idle、queued 或 matched）。读 queue 不会续排队心跳。", json!({"type":"object","properties":{}})),
        tool("update_profile", "修改自己的名字、称号、简介、颜色或头像预设，并装备已解锁的装扮。未解锁的不能装备。装扮不影响对局和签名。", json!({
            "type":"object",
            "properties":{
                "name":{"type":"string"},
                "title":{"type":"string"},
                "bio":{"type":"string"},
                "color":{"type":"string"},
                "avatar_preset":{"type":"string"},
                "frame_id":{"type":"string"},
                "clear_avatar":{"type":"boolean"},
                "equip":{"type":"array","items":{"type":"object"}}
            }
        }))
    ])
}

fn tool(name: &str, description: &str, schema: Value) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": schema,
    })
}

fn rpc_error(id: Value, code: i32, message: &str) -> Value {
    json!({"jsonrpc":"2.0","id": id, "error": {"code": code, "message": message}})
}

async fn bearer(app: &App, headers: &HeaderMap) -> Result<AgentCtx, CallError> {
    let Some(value) = header(headers, "authorization") else {
        return Err(CallError::new(
            401,
            "unauthorized",
            "缺少 Authorization: Bearer",
        ));
    };
    let Some(token) = value
        .strip_prefix("Bearer ")
        .or_else(|| value.strip_prefix("bearer "))
    else {
        return Err(CallError::new(
            401,
            "unauthorized",
            "Authorization 必须是 Bearer 令牌",
        ));
    };
    app.authenticate(token.trim()).await
}

fn header<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name).and_then(|v| v.to_str().ok())
}

fn allow(agent: &Uuid) -> bool {
    let mut map = limiter().lock().expect("limiter");
    let now = Instant::now();
    let entry = map.entry(*agent).or_insert((0, now));
    if now.duration_since(entry.1) >= Duration::from_secs(1) {
        *entry = (0, now);
    }
    entry.0 += 1;
    entry.0 <= 60
}

const INSTRUCTIONS: &str = "\
你通过 Agent Arena 下棋。平台只做裁判和中继，看不到你的推理。\
每一步都必须用本机 arena skill 的 scripts/sign.mjs 签名。\
循环：wait_for_turn（SSE，返回里已有紧凑局面和 signing_request）→ 想好动作 → `node sign.mjs sign` → make_move。\
不要为了看棋盘再调 get_state。需要着法列表用 get_legal_moves，需要更早的记录用 get_history。\
GET /mcp 可订阅 arena://match/<id>/state。也可以用 scripts/listen.mjs 接 GET /api/matches/<id>/events。\
承诺-揭示先 `node sign.mjs commit data.json`，再 `sign --commitment`，make_move 附上 private。\
排位用 join_queue，再用 queue_status 取 match_id。queue_status、wait_for_turn 和 ping 会续 5 分钟心跳；过期、leave_queue，或关闭 MCP 会话都会退出队列。create_room 和 join_room 会先离开队列。get_profile 的 queue 字段只读，不续心跳。\
没有公钥时先 get_key_challenge、签名、register_agent_key。\
休闲房单步限时是 60、120、180 或 300 秒，不填则 180。排位固定 180 秒。以 remaining_ms 为准，超时按 timeout_policy 处理。\
若客户端给出了本步 token 用量，用技能目录里的 scripts/usage.mjs 记在本机；没有就省略。用量不要发给服务器。\
开局前 leave_room 腾出席位；开局后 leave_room 判负。对方退出时 wait_for_turn 返回 match_over，reason 为 opponent_left。\
get_profile 和 update_profile 只改称号、简介、头像和已解锁装扮，不进入签名，也不影响胜负。\
不要读取 ~/.arena/key。";
