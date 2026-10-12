//! Streamable HTTP MCP. One JSON-RPC message per POST; SSE is only the
//! server-initiated channel (heartbeats). Bearer tokens identify the agent.

use crate::app::{AgentCtx, App};
use crate::error::CallError;
use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use futures::stream;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use uuid::Uuid;

fn sessions() -> &'static Mutex<HashMap<String, Uuid>> {
    static S: OnceLock<Mutex<HashMap<String, Uuid>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(HashMap::new()))
}

fn limiter() -> &'static Mutex<HashMap<Uuid, (u32, Instant)>> {
    static L: OnceLock<Mutex<HashMap<Uuid, (u32, Instant)>>> = OnceLock::new();
    L.get_or_init(|| Mutex::new(HashMap::new()))
}

pub async fn mcp_post(State(app): State<App>, headers: HeaderMap, body: String) -> Responseish {
    let agent = match bearer(&app, &headers).await {
        Ok(a) => a,
        Err(err) => return Responseish::err(StatusCode::UNAUTHORIZED, err),
    };
    if !allow(&agent.agent_id) {
        return Responseish::err(
            StatusCode::TOO_MANY_REQUESTS,
            CallError::new(429, "rate_limited", "调用过于频繁"),
        );
    }
    let msg: Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(_) => {
            return Responseish::json(
                StatusCode::OK,
                rpc_error(Value::Null, -32700, "parse error"),
                None,
            )
        }
    };
    if msg.get("id").is_none() {
        return Responseish::empty(StatusCode::ACCEPTED);
    }
    let id = msg.get("id").cloned().unwrap_or(Value::Null);
    let method = msg.get("method").and_then(|v| v.as_str()).unwrap_or("");
    let params = msg.get("params").cloned().unwrap_or(json!({}));
    if method != "initialize" {
        if let Some(sid) = header(&headers, "mcp-session-id") {
            let ok = sessions().lock().expect("sessions").get(sid).copied() == Some(agent.agent_id);
            if !ok {
                return Responseish::err(StatusCode::NOT_FOUND, CallError::new(404, "session", "MCP 会话不存在"));
            }
        }
    }
    let (result, new_session) = match method {
        "initialize" => {
            let sid = Uuid::new_v4().to_string();
            sessions().lock().expect("sessions").insert(sid.clone(), agent.agent_id);
            let proto = params
                .get("protocolVersion")
                .and_then(|v| v.as_str())
                .unwrap_or("2025-03-26");
            (
                json!({
                    "protocolVersion": proto,
                    "capabilities": {"tools": {}},
                    "serverInfo": {"name": "agent-arena", "version": "0.1.0"},
                    "instructions": INSTRUCTIONS,
                }),
                Some(sid),
            )
        }
        "ping" => (json!({}), None),
        "tools/list" => (json!({"tools": tools()}), None),
        "tools/call" => {
            let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
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
        _ => {
            return Responseish::json(StatusCode::OK, rpc_error(id, -32601, "method not found"), None);
        }
    };
    Responseish::json(StatusCode::OK, json!({"jsonrpc": "2.0", "id": id, "result": result}), new_session)
}

pub async fn mcp_get(headers: HeaderMap) -> Response {
    if header(&headers, "mcp-session-id").is_none() {
        return (StatusCode::BAD_REQUEST, "missing session").into_response();
    }
    let stream = stream::unfold(0u64, |i| async move {
        tokio::time::sleep(Duration::from_secs(15)).await;
        Some((Ok::<_, std::io::Error>(format!(": ping {i}\n\n")), i + 1))
    });
    response_sse(stream)
}

fn response_sse(stream: impl futures::Stream<Item = Result<String, std::io::Error>> + Send + 'static) -> Response {
    let body = Body::from_stream(stream);
    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/event-stream")
        .header("cache-control", "no-cache")
        .body(body)
        .unwrap()
}

pub async fn mcp_delete(headers: HeaderMap) -> StatusCode {
    if let Some(sid) = header(&headers, "mcp-session-id") {
        sessions().lock().expect("sessions").remove(sid);
    }
    StatusCode::NO_CONTENT
}

pub struct Responseish {
    status: StatusCode,
    body: Option<Value>,
    session: Option<String>,
    empty: bool,
}

impl Responseish {
    fn json(status: StatusCode, body: Value, session: Option<String>) -> Self {
        Self { status, body: Some(body), session, empty: false }
    }
    fn empty(status: StatusCode) -> Self {
        Self { status, body: None, session: None, empty: true }
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
            res.headers_mut().insert("mcp-session-id", sid.parse().unwrap());
        }
        res
    }
}

async fn call_tool(app: &App, agent: &AgentCtx, name: &str, args: &Value) -> Result<Value, CallError> {
    match name {
        "whoami" => app.whoami(agent).await,
        "list_games" => Ok(json!({"games": app.list_games()})),
        "create_room" => app.create_room(agent, args).await,
        "join_room" => {
            let code = args.get("code").and_then(|v| v.as_str()).unwrap_or("");
            app.join_room(agent, code, args.get("kid").and_then(|v| v.as_str())).await
        }
        "get_state" => {
            let id = parse_match(args)?;
            app.get_state(agent, id).await
        }
        "wait_for_turn" => {
            let id = parse_match(args)?;
            app.wait_for_turn(agent, id).await
        }
        "make_move" => {
            let id = parse_match(args)?;
            let envelope = args.get("envelope").cloned().unwrap_or(Value::Null);
            app.make_move(agent, id, envelope).await
        }
        "resign" => {
            let id = parse_match(args)?;
            app.resign(agent, id).await
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
            app.register_key(agent, &envelope, args.get("label").and_then(|v| v.as_str())).await
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
        tool("create_room", "创建休闲房间。game 目前只有 gomoku 可实际开局。", json!({
            "type":"object",
            "properties":{
                "game":{"type":"string"},
                "variant":{"type":"string"},
                "time_limit_sec":{"type":"integer"},
                "timeout_policy":{"type":"string"},
                "max_consecutive_timeouts":{"type":"integer"},
                "allow_comments":{"type":"boolean"},
                "kid":{"type":"string"}
            },
            "required":["game"]
        })),
        tool("join_room", "用房间码入座。", json!({
            "type":"object",
            "properties":{"code":{"type":"string"},"kid":{"type":"string"}},
            "required":["code"]
        })),
        tool("get_state", "本席位视图、合法动作、计时，以及 signing_request。", json!({
            "type":"object",
            "properties":{"match_id":{"type":"string"}},
            "required":["match_id"]
        })),
        tool("wait_for_turn", "最长阻塞 25 秒，直到轮到你、对局结束或超时返回 not_your_turn。", json!({
            "type":"object",
            "properties":{"match_id":{"type":"string"}},
            "required":["match_id"]
        })),
        tool("make_move", "提交 sign.mjs 产出的签名信封。", json!({
            "type":"object",
            "properties":{"match_id":{"type":"string"},"envelope":{"type":"object"}},
            "required":["match_id","envelope"]
        })),
        tool("resign", "认输。由已鉴权的令牌发起，服务器签名记入日志。", json!({
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
        return Err(CallError::new(401, "unauthorized", "缺少 Authorization: Bearer"));
    };
    let Some(token) = value.strip_prefix("Bearer ").or_else(|| value.strip_prefix("bearer ")) else {
        return Err(CallError::new(401, "unauthorized", "Authorization 必须是 Bearer 令牌"));
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
循环：wait_for_turn → get_state → 把 signing_request 交给 `node sign.mjs sign --action '{...}'` → make_move。\
没有公钥时先 get_key_challenge、签名、register_agent_key。\
默认每步 60 秒，超时按 timeout_policy 处理。不要读取 ~/.arena/key。";
