//! The built-in opponent: an OpenAI-compatible model that plays through the
//! same signed-move pipeline as any other agent. It owns a server-side agent
//! and key, so its moves appear in the log exactly like an agent's.

use crate::app::App;
use crate::config::Config;
use crate::crypto::{b64url_encode, commitment_of, kid_of, now_ms, sign_value};
use crate::room::Cmd;
use ed25519_dalek::{Signer, SigningKey};
use rand::rand_core::UnwrapErr;
use rand::rngs::SysRng;
use rand::RngExt;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::collections::HashSet;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::oneshot;
use uuid::Uuid;

const BOT_HANDLE: &str = "arena-ai";
const BOT_SLUG: &str = "arena-ai";
const TICK: Duration = Duration::from_millis(800);
const LLM_TIMEOUT: Duration = Duration::from_secs(75);
const VIEW_TIMEOUT: Duration = Duration::from_secs(3);

pub struct AiBot {
    pub agent_id: Uuid,
    pub owner_id: Uuid,
    pub name: String,
    pub kid: String,
    pub model: String,
    pub games: Vec<String>,
    key: SigningKey,
    base_url: String,
    api_key: String,
    http: reqwest::Client,
    in_flight: Mutex<HashSet<Uuid>>,
}

impl AiBot {
    pub async fn bootstrap(cfg: &Config, pool: &PgPool) -> anyhow::Result<Option<Self>> {
        if !cfg.ai_ready() {
            return Ok(None);
        }
        let key = load_or_create_key(&cfg.data_dir)?;
        let public_raw = key.verifying_key().to_bytes();
        let kid = kid_of(&public_raw, "ak");
        let public_b64 = b64url_encode(&public_raw);

        sqlx::query(
            "INSERT INTO users (id, handle, email, role) VALUES ($1, $2, $3, 'user')
             ON CONFLICT (handle) DO NOTHING",
        )
        .bind(Uuid::new_v4())
        .bind(BOT_HANDLE)
        .bind("arena-ai@localhost")
        .execute(pool)
        .await?;
        let owner_id: Uuid = sqlx::query_scalar("SELECT id FROM users WHERE handle = $1")
            .bind(BOT_HANDLE)
            .fetch_one(pool)
            .await?;

        let existing: Option<Uuid> =
            sqlx::query_scalar("SELECT id FROM agents WHERE owner_id = $1 AND slug = $2")
                .bind(owner_id)
                .bind(BOT_SLUG)
                .fetch_optional(pool)
                .await?;
        let agent_id = match existing {
            Some(id) => {
                sqlx::query("UPDATE agents SET name = $2, model_label = $3 WHERE id = $1")
                    .bind(id)
                    .bind(&cfg.ai_name)
                    .bind(&cfg.ai_model)
                    .execute(pool)
                    .await?;
                id
            }
            None => {
                let id = Uuid::new_v4();
                sqlx::query(
                    "INSERT INTO agents (id, owner_id, name, slug, description, model_label, visibility, color)
                     VALUES ($1, $2, $3, $4, $5, $6, 'public', '#2dd4bf')",
                )
                .bind(id)
                .bind(owner_id)
                .bind(&cfg.ai_name)
                .bind(BOT_SLUG)
                .bind("服务器内置对手，由大模型走子。")
                .bind(&cfg.ai_model)
                .execute(pool)
                .await?;
                id
            }
        };

        let challenge = "server-bootstrap";
        let pop = key.sign(challenge.as_bytes()).to_bytes();
        sqlx::query(
            "INSERT INTO agent_keys
               (kid, agent_id, alg, public_key, public_key_b64, label, status, pop_signature, pop_challenge)
             VALUES ($1, $2, 'ed25519', $3, $4, 'server', 'active', $5, $6)
             ON CONFLICT (kid) DO NOTHING",
        )
        .bind(&kid)
        .bind(agent_id)
        .bind(public_raw.as_slice())
        .bind(&public_b64)
        .bind(pop.as_slice())
        .bind(challenge)
        .execute(pool)
        .await?;

        let http = reqwest::Client::builder().timeout(LLM_TIMEOUT).build()?;
        tracing::info!(%kid, model = %cfg.ai_model, "built-in AI opponent ready");
        Ok(Some(Self {
            agent_id,
            owner_id,
            name: cfg.ai_name.clone(),
            kid,
            model: cfg.ai_model.clone(),
            games: cfg.ai_games.clone(),
            key,
            base_url: cfg.ai_base_url.clone(),
            api_key: cfg.ai_api_key.clone(),
            http,
            in_flight: Mutex::new(HashSet::new()),
        }))
    }

    pub fn spawn(self: &Arc<Self>, app: App) {
        let bot = Arc::clone(self);
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(TICK).await;
                if let Err(err) = bot.tick(&app).await {
                    tracing::warn!(error = %err, "ai tick");
                }
            }
        });
    }

    async fn tick(self: &Arc<Self>, app: &App) -> anyhow::Result<()> {
        let ids: Vec<Uuid> = sqlx::query_scalar(
            "SELECT DISTINCT m.id FROM matches m
             JOIN match_players p ON p.match_id = m.id
             WHERE p.agent_id = $1 AND m.status = 'running'",
        )
        .bind(self.agent_id)
        .fetch_all(app.pool())
        .await?;
        for id in ids {
            if self.in_flight.lock().expect("ai in flight").contains(&id) {
                continue;
            }
            let Some(tx) = app.slot(id).cmd.lock().expect("cmd lock").clone() else {
                continue;
            };
            let (reply, rx) = oneshot::channel();
            if tx
                .send(Cmd::Agent {
                    agent_id: self.agent_id,
                    reply,
                })
                .await
                .is_err()
            {
                continue;
            }
            let Ok(Ok(view)) = tokio::time::timeout(VIEW_TIMEOUT, rx).await else {
                continue;
            };
            if view
                .get("signing_request")
                .map(Value::is_null)
                .unwrap_or(true)
            {
                continue;
            }
            self.in_flight.lock().expect("ai in flight").insert(id);
            let bot = Arc::clone(self);
            let app = app.clone();
            tokio::spawn(async move {
                if let Err(err) = bot.play_one(&app, id, view).await {
                    tracing::warn!(error = %err, %id, "ai move");
                }
                bot.in_flight.lock().expect("ai in flight").remove(&id);
            });
        }
        Ok(())
    }

    async fn play_one(&self, app: &App, id: Uuid, view: Value) -> anyhow::Result<()> {
        let request = view.get("signing_request").cloned().unwrap_or(Value::Null);
        if request.is_null() {
            return Ok(());
        }
        let legal = view
            .get("legal_actions")
            .cloned()
            .unwrap_or_else(|| json!([]));
        if legal.as_array().map(Vec::is_empty).unwrap_or(true) {
            return Ok(());
        }
        let deadline = request
            .pointer("/payload/deadline_at")
            .and_then(Value::as_i64)
            .unwrap_or(0);
        if deadline > 0 && now_ms() > deadline - 1_500 {
            return Ok(());
        }

        let (action, private_data) = self.choose(&view, &legal, deadline).await?;
        let domain = request
            .get("domain")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("signing request has no domain"))?;
        let mut payload = request
            .get("payload")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("signing request has no payload"))?;
        payload["action"] = action;
        payload["issued_at"] = json!(now_ms());
        let mut private = None;
        if let Some(data) = private_data {
            let mut salt_bytes = [0u8; 32];
            rand::rng().fill(&mut salt_bytes);
            let salt = hex::encode(salt_bytes);
            let commitment = commitment_of(&data, &salt)
                .ok_or_else(|| anyhow::anyhow!("could not commit private data"))?;
            payload["commitment"] = json!(commitment);
            private = Some(json!({"data": data, "salt": salt}));
        }
        let signature = sign_value(&self.key, &domain, &payload);
        let envelope = json!({
            "domain": domain,
            "payload": payload,
            "alg": "ed25519",
            "kid": self.kid,
            "signature": signature,
            "client": "arena-ai/1",
        });

        let Some(tx) = app.slot(id).cmd.lock().expect("cmd lock").clone() else {
            return Ok(());
        };
        let (reply, rx) = oneshot::channel();
        tx.send(Cmd::Move {
            agent_id: self.agent_id,
            envelope,
            private,
            reply,
        })
        .await?;
        let result = tokio::time::timeout(Duration::from_secs(5), rx).await??;
        if result.get("accepted") != Some(&Value::Bool(true)) {
            anyhow::bail!("move rejected: {result}");
        }
        Ok(())
    }

    async fn choose(
        &self,
        view: &Value,
        legal: &Value,
        deadline: i64,
    ) -> anyhow::Result<(Value, Option<Value>)> {
        let mut feedback = String::new();
        for _ in 0..2 {
            let prompt = self.prompt(view, legal, &feedback);
            let reply = self.chat(&prompt).await?;
            match parse_choice(&reply) {
                Some((action, private)) if action_ok(&action, legal) => {
                    return Ok((action, private));
                }
                Some((action, _)) => {
                    feedback = format!(
                        "That action {action} is not in legal_actions. Copy one entry verbatim."
                    );
                }
                None => {
                    feedback =
                        "Return a choose_move tool call whose action is one entry from legal_actions."
                            .to_string();
                }
            }
            if deadline > 0 && now_ms() > deadline - 1_500 {
                break;
            }
        }
        anyhow::bail!("model did not return a legal action")
    }

    fn prompt(&self, view: &Value, legal: &Value, feedback: &str) -> String {
        let game = view
            .get("game_id")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let seat = view.get("seat").and_then(Value::as_i64).unwrap_or(0);
        let remaining = view
            .get("remaining_ms")
            .and_then(Value::as_i64)
            .unwrap_or(0);
        let state = view.get("view").cloned().unwrap_or(Value::Null);
        let mut prompt = format!(
            "You are {name}, playing {game} on Agent Arena. You are seat {seat} and it is your turn.\n\
             Pick exactly one action from legal_actions and submit it with the choose_move tool.\n\
             Rules:\n\
             - action must be one object copied from legal_actions, with the same type and data.\n\
             - Do not invent moves or coordinates that are not in legal_actions.\n\
             - If an action needs hidden data, put that data in the private field.\n\
             - Reply with the tool call only.\n\n\
             remaining_ms: {remaining}\n\n\
             board view:\n{state}\n\nlegal_actions:\n{legal}\n",
            name = self.name,
            state = serde_json::to_string(&state).unwrap_or_default(),
            legal = serde_json::to_string(legal).unwrap_or_default(),
        );
        if !feedback.is_empty() {
            prompt.push_str("\nPrevious attempt was rejected: ");
            prompt.push_str(feedback);
            prompt.push('\n');
        }
        prompt
    }

    async fn chat(&self, prompt: &str) -> anyhow::Result<Value> {
        let body = json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": "You are a careful board-game player. Always answer with a choose_move tool call."},
                {"role": "user", "content": prompt},
            ],
            "tools": [{
                "type": "function",
                "function": {
                    "name": "choose_move",
                    "description": "Submit the one action you want to play this turn.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "action": {"type": "object", "description": "One entry copied from legal_actions."},
                            "private": {"type": "object", "description": "Hidden data, only when the action needs it."},
                        },
                        "required": ["action"],
                    },
                },
            }],
            "tool_choice": {"type": "function", "function": {"name": "choose_move"}},
            "temperature": 0.2,
            "max_tokens": 2048,
        });
        let url = format!("{}/chat/completions", self.base_url);
        let mut response = self
            .http
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await?;
        if response.status() == reqwest::StatusCode::BAD_REQUEST {
            // Some compatible servers reject a forced tool_choice; let the model choose.
            let mut relaxed = body.clone();
            relaxed["tool_choice"] = json!("auto");
            response = self
                .http
                .post(&url)
                .bearer_auth(&self.api_key)
                .json(&relaxed)
                .send()
                .await?;
        }
        let status = response.status();
        let text = response.text().await?;
        if !status.is_success() {
            anyhow::bail!("model returned {status}: {}", truncate(&text, 300));
        }
        serde_json::from_str(&text).map_err(|err| anyhow::anyhow!("model reply is not JSON: {err}"))
    }
}

pub fn parse_choice(reply: &Value) -> Option<(Value, Option<Value>)> {
    if let Some(args) = reply.pointer("/choices/0/message/tool_calls/0/function/arguments") {
        if let Some(text) = args.as_str() {
            if let Ok(parsed) = serde_json::from_str::<Value>(text) {
                if let Some(found) = extract_choice(parsed) {
                    return Some(found);
                }
            }
        } else if args.is_object() {
            return extract_choice(args.clone());
        }
    }
    let content = reply
        .pointer("/choices/0/message/content")
        .and_then(Value::as_str)?;
    extract_choice(json_in_text(content)?)
}

fn extract_choice(value: Value) -> Option<(Value, Option<Value>)> {
    if let Some(action) = value.get("action") {
        if action.is_object() {
            let private = value.get("private").filter(|p| p.is_object()).cloned();
            return Some((action.clone(), private));
        }
    }
    if value.get("type").is_some() {
        return Some((value, None));
    }
    None
}

fn json_in_text(text: &str) -> Option<Value> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    serde_json::from_str(&text[start..=end]).ok()
}

fn action_ok(action: &Value, legal: &Value) -> bool {
    let Some(kind) = action.get("type").and_then(Value::as_str) else {
        return false;
    };
    let Some(entries) = legal.as_array() else {
        return false;
    };
    entries.iter().any(|entry| {
        entry.get("type").and_then(Value::as_str) == Some(kind)
            && match entry.get("data") {
                Some(expected) => action.get("data") == Some(expected),
                None => true,
            }
    })
}

fn truncate(text: &str, max: usize) -> String {
    let mut out: String = text.chars().take(max).collect();
    if text.chars().count() > max {
        out.push('…');
    }
    out
}

fn load_or_create_key(dir: &Path) -> anyhow::Result<SigningKey> {
    let keys = dir.join("keys");
    std::fs::create_dir_all(&keys)?;
    let path = keys.join("ai-agent.ed25519");
    if path.exists() {
        let bytes = std::fs::read(&path)?;
        let raw: [u8; 32] = bytes
            .as_slice()
            .try_into()
            .map_err(|_| anyhow::anyhow!("AI key at {} is not 32 bytes", path.display()))?;
        return Ok(SigningKey::from_bytes(&raw));
    }
    let key = SigningKey::generate(&mut UnwrapErr(SysRng));
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create_new(true).mode(0o600);
        opts.open(&path)?.write_all(&key.to_bytes())?;
    }
    #[cfg(not(unix))]
    {
        std::fs::write(&path, key.to_bytes())?;
    }
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_tool_call() {
        let reply = json!({
            "choices": [{"message": {"tool_calls": [{"function": {
                "name": "choose_move",
                "arguments": "{\"action\":{\"type\":\"place\",\"data\":{\"at\":\"h8\"}}}"
            }}]}}]
        });
        let (action, private) = parse_choice(&reply).expect("choice");
        assert_eq!(action["type"], "place");
        assert_eq!(action["data"]["at"], "h8");
        assert!(private.is_none());
    }

    #[test]
    fn reads_json_in_content() {
        let reply = json!({
            "choices": [{"message": {"content": "I will play:\n```json\n{\"action\":{\"type\":\"move\",\"data\":{\"from\":\"b3\",\"to\":\"b10\"}},\"private\":{\"x\":1}}\n```"}}]
        });
        let (action, private) = parse_choice(&reply).expect("choice");
        assert_eq!(action["type"], "move");
        assert_eq!(private.expect("private")["x"], 1);
    }

    #[test]
    fn rejects_actions_outside_the_legal_set() {
        let legal = json!([{"type": "place", "data": {"at": "h8"}}]);
        assert!(action_ok(
            &json!({"type": "place", "data": {"at": "h8"}}),
            &legal
        ));
        assert!(!action_ok(
            &json!({"type": "place", "data": {"at": "a1"}}),
            &legal
        ));
        assert!(!action_ok(&json!({"type": "pass"}), &legal));
    }

    #[test]
    fn signs_an_envelope_the_room_can_verify() {
        let key = SigningKey::generate(&mut UnwrapErr(SysRng));
        let public = b64url_encode(&key.verifying_key().to_bytes());
        let domain = json!({"chainless": true, "name": "AgentArena", "origin": "localhost", "purpose": "move", "version": "1"});
        let payload = json!({"match_id": "m", "seat": 1, "seq": 3, "prev_hash": "0"});
        let signature = sign_value(&key, &domain, &payload);
        assert!(crate::crypto::verify_value(
            &public, &domain, &payload, &signature
        ));
    }
}
