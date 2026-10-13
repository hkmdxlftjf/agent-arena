//! Accounts, rooms, and the process-wide handle the HTTP and MCP layers share.

use crate::canon::canon;
use crate::config::Config;
use crate::crypto::{b64url_decode, b64url_encode, domain, domain_ok, kid_of, now_ms, sha256, sha256_hex, verify_value, ServerKey};
use crate::error::CallError;
use crate::game::registry;
use crate::room::{roundtrip, spawn_room, Cmd, Engine, Hub, Slot};
use argon2::password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use chrono::{DateTime, Duration, Utc};
use rand::{Rng, RngCore};
use serde_json::{json, Value};
use sqlx::{PgPool, Postgres, Transaction};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct AgentCtx {
    pub agent_id: Uuid,
    pub owner_id: Uuid,
    pub name: String,
    pub allow_comments: bool,
    pub scopes: Vec<String>,
    pub token_id: Uuid,
}

#[derive(Clone)]
pub struct User {
    pub id: Uuid,
    pub handle: String,
    pub email: String,
    pub role: String,
}

#[allow(dead_code)]
struct GameInfo {
    version_id: Uuid,
    version: String,
    rules_hash: String,
    name: String,
    playable: bool,
    manifest: Value,
}

#[allow(dead_code)]
#[derive(Clone, sqlx::FromRow)]
struct QueueRow {
    id: Uuid,
    agent_id: Uuid,
    owner_id: Uuid,
    game_id: String,
    variant: String,
    kid: String,
    rating: f64,
    enqueued_at: DateTime<Utc>,
}

fn queue_lock() -> &'static tokio::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

fn window_of(at: DateTime<Utc>) -> f64 {
    let secs = (Utc::now() - at).num_seconds().max(0) as f64;
    (100.0 + 50.0 * (secs / 10.0)).min(400.0)
}

struct Inner {
    pool: PgPool,
    server: ServerKey,
    cfg: Config,
    games: HashMap<String, GameInfo>,
    hub: Hub,
}

#[derive(Clone)]
pub struct App {
    inner: Arc<Inner>,
}

impl App {
    pub fn pool(&self) -> &PgPool {
        &self.inner.pool
    }

    pub fn config(&self) -> &Config {
        &self.inner.cfg
    }

    pub fn server(&self) -> &ServerKey {
        &self.inner.server
    }

    pub fn slot(&self, id: Uuid) -> Arc<Slot> {
        self.inner.hub.slot(id)
    }

    pub async fn boot(pool: PgPool, server: ServerKey, cfg: Config) -> anyhow::Result<Self> {
        let key_path = cfg.data_dir.join("keys").join("server.ed25519");
        sqlx::query(
            r#"INSERT INTO server_keys (kid, alg, public_key, public_key_b64, private_key_ref)
               VALUES ($1, 'ed25519', $2, $3, $4)
               ON CONFLICT (kid) DO NOTHING"#,
        )
        .bind(&server.kid)
        .bind(server.public_raw.as_slice())
        .bind(&server.public_b64)
        .bind(key_path.display().to_string())
        .execute(&pool)
        .await?;
        let games = bootstrap_games(&pool).await?;
        let app = Self {
            inner: Arc::new(Inner {
                pool,
                server,
                cfg,
                games,
                hub: Hub::new(),
            }),
        };
        app.recover().await;
        let matcher = app.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                if let Err(err) = matcher.matchmake_once().await {
                    tracing::warn!(error = %err, "matchmaker");
                }
            }
        });
        Ok(app)
    }

    async fn recover(&self) {
        let ids: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM matches WHERE status = 'running'")
            .fetch_all(self.pool())
            .await
            .unwrap_or_default();
        for id in ids {
            if let Err(err) = spawn_room(self.pool().clone(), self.server().clone(), &self.inner.hub, id).await {
                tracing::error!(error = %err, %id, "failed to recover match");
            }
        }
    }

    pub async fn register(&self, email: &str, password: &str, handle: &str) -> Result<(User, String), CallError> {
        let email = email.trim().to_lowercase();
        let handle = handle.trim().to_lowercase();
        if !email.contains('@') || email.len() > 200 {
            return Err(CallError::new(400, "bad_email", "请填写有效邮箱"));
        }
        if !valid_handle(&handle) {
            return Err(CallError::new(400, "bad_handle", "用户名需为 3–20 位小写字母、数字或下划线"));
        }
        let hash = hash_password(password)?;
        let id = Uuid::new_v4();
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
            .fetch_one(self.pool())
            .await
            .map_err(db_err)?;
        let mut role = if count == 0 { "admin" } else { "user" };
        if admin_email(&email) {
            role = "admin";
        }
        let row = sqlx::query_as::<_, (Uuid, String, String, String)>(
            "INSERT INTO users (id, handle, email, email_verified_at, password_hash, role) VALUES ($1, $2, $3, now(), $4, $5) RETURNING id, handle, email, role",
        )
        .bind(id)
        .bind(&handle)
        .bind(&email)
        .bind(&hash)
        .bind(role)
        .fetch_one(self.pool())
        .await
        .map_err(unique_or_db("邮箱或用户名已被占用"))?;
        let user = User { id: row.0, handle: row.1, email: row.2, role: row.3 };
        let sid = self.create_session(user.id).await?;
        Ok((user, sid))
    }

    pub async fn login(&self, email: &str, password: &str) -> Result<(User, String), CallError> {
        let email = email.trim().to_lowercase();
        let row = sqlx::query_as::<_, (Uuid, String, String, String, String)>(
            "SELECT id, handle, email, role, password_hash FROM users WHERE email = $1",
        )
        .bind(&email)
        .fetch_optional(self.pool())
        .await
        .map_err(db_err)?;
        let Some((id, handle, email, role, hash)) = row else {
            return Err(CallError::new(401, "invalid_login", "邮箱或密码不正确"));
        };
        if !verify_password(&hash, password) {
            return Err(CallError::new(401, "invalid_login", "邮箱或密码不正确"));
        }
        let sid = self.create_session(id).await?;
        Ok((User { id, handle, email, role }, sid))
    }

    pub async fn logout(&self, sid: &str) -> Result<(), CallError> {
        sqlx::query("DELETE FROM sessions WHERE id = $1")
            .bind(sid)
            .execute(self.pool())
            .await
            .map_err(db_err)?;
        Ok(())
    }

    pub async fn user_by_session(&self, sid: &str) -> Result<Option<User>, CallError> {
        let row = sqlx::query_as::<_, (Uuid, String, String, String)>(
            r#"SELECT u.id, u.handle, u.email, u.role
               FROM sessions s JOIN users u ON u.id = s.user_id
               WHERE s.id = $1 AND s.expires_at > now()"#,
        )
        .bind(sid)
        .fetch_optional(self.pool())
        .await
        .map_err(db_err)?;
        Ok(row.map(|(id, handle, email, role)| User { id, handle, email, role }))
    }

    async fn create_session(&self, user_id: Uuid) -> Result<String, CallError> {
        let mut raw = [0u8; 32];
        OsRng.fill_bytes(&mut raw);
        let sid = hex::encode(raw);
        sqlx::query("INSERT INTO sessions (id, user_id, expires_at) VALUES ($1, $2, $3)")
            .bind(&sid)
            .bind(user_id)
            .bind(Utc::now() + Duration::days(30))
            .execute(self.pool())
            .await
            .map_err(db_err)?;
        Ok(sid)
    }

    pub async fn create_agent(
        &self,
        user: &User,
        name: &str,
        description: Option<&str>,
        model_label: Option<&str>,
        allow_comments: bool,
    ) -> Result<Value, CallError> {
        let name = name.trim();
        if name.chars().count() < 1 || name.chars().count() > 40 {
            return Err(CallError::new(400, "bad_name", "智能体名称需要 1–40 个字"));
        }
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM agents WHERE owner_id = $1")
            .bind(user.id)
            .fetch_one(self.pool())
            .await
            .map_err(db_err)?;
        if count >= 5 {
            return Err(CallError::new(400, "too_many_agents", "每个用户最多 5 个智能体"));
        }
        let id = Uuid::new_v4();
        let slug = format!("a{}", &id.simple().to_string()[..8]);
        sqlx::query(
            "INSERT INTO agents (id, owner_id, name, slug, description, model_label, allow_comments) VALUES ($1,$2,$3,$4,$5,$6,$7)",
        )
        .bind(id)
        .bind(user.id)
        .bind(name)
        .bind(&slug)
        .bind(description)
        .bind(model_label)
        .bind(allow_comments)
        .execute(self.pool())
        .await
        .map_err(db_err)?;
        Ok(json!({"id": id, "name": name, "slug": slug}))
    }

    pub async fn list_agents(&self, user: &User) -> Result<Vec<Value>, CallError> {
        let rows = sqlx::query_as::<_, (Uuid, String, Option<String>, Option<String>, bool, Option<DateTime<Utc>>)>(
            "SELECT id, name, description, model_label, allow_comments, last_seen_at FROM agents WHERE owner_id = $1 ORDER BY created_at",
        )
        .bind(user.id)
        .fetch_all(self.pool())
        .await
        .map_err(db_err)?;
        let mut out = Vec::new();
        for (id, name, description, model_label, allow_comments, last_seen) in rows {
            let keys = self.keys_of(id).await?;
            let tokens = self.tokens_of(id).await?;
            out.push(json!({
                "id": id,
                "name": name,
                "description": description,
                "model_label": model_label,
                "allow_comments": allow_comments,
                "last_seen_at": last_seen.map(|t| t.timestamp_millis()),
                "connected": last_seen.map(|t| Utc::now().signed_duration_since(t).num_seconds() < 10).unwrap_or(false),
                "keys": keys,
                "tokens": tokens,
            }));
        }
        Ok(out)
    }

    async fn keys_of(&self, agent_id: Uuid) -> Result<Vec<Value>, CallError> {
        let rows = sqlx::query_as::<_, (String, String, String, Option<String>, String, DateTime<Utc>)>(
            "SELECT kid, alg, public_key_b64, label, status, created_at FROM agent_keys WHERE agent_id = $1 ORDER BY created_at",
        )
        .bind(agent_id)
        .fetch_all(self.pool())
        .await
        .map_err(db_err)?;
        Ok(rows
            .into_iter()
            .map(|(kid, alg, public_key, label, status, created)| {
                json!({
                    "kid": kid,
                    "alg": alg,
                    "public_key": public_key,
                    "label": label,
                    "status": status,
                    "created_at": created.timestamp_millis(),
                })
            })
            .collect())
    }

    async fn tokens_of(&self, agent_id: Uuid) -> Result<Vec<Value>, CallError> {
        let rows = sqlx::query_as::<_, (Uuid, String, DateTime<Utc>, Option<DateTime<Utc>>, Option<DateTime<Utc>>)>(
            "SELECT id, token_prefix, created_at, last_used_at, revoked_at FROM agent_tokens WHERE agent_id = $1 ORDER BY created_at DESC",
        )
        .bind(agent_id)
        .fetch_all(self.pool())
        .await
        .map_err(db_err)?;
        Ok(rows
            .into_iter()
            .map(|(id, prefix, created, used, revoked)| {
                json!({
                    "id": id,
                    "prefix": prefix,
                    "created_at": created.timestamp_millis(),
                    "last_used_at": used.map(|t| t.timestamp_millis()),
                    "revoked": revoked.is_some(),
                })
            })
            .collect())
    }

    pub async fn issue_token(&self, user: &User, agent_id: Uuid) -> Result<Value, CallError> {
        self.owned_agent(user, agent_id).await?;
        let mut raw = [0u8; 32];
        OsRng.fill_bytes(&mut raw);
        let token = format!("aa_live_{}_{}", agent_id.simple(), b64url_encode(&raw));
        let hash = sha256(token.as_bytes());
        let prefix = format!("{}…{}", &token[..16], &token[token.len() - 4..]);
        let id = Uuid::new_v4();
        let scopes = vec!["play:casual".to_string(), "play:ranked".to_string(), "rooms:create".to_string()];
        sqlx::query(
            "INSERT INTO agent_tokens (id, agent_id, token_hash, token_prefix, scopes) VALUES ($1,$2,$3,$4,$5)",
        )
        .bind(id)
        .bind(agent_id)
        .bind(hash.as_slice())
        .bind(&prefix)
        .bind(&scopes)
        .execute(self.pool())
        .await
        .map_err(db_err)?;
        Ok(json!({
            "id": id,
            "token": token,
            "prefix": prefix,
            "scopes": scopes,
        }))
    }

    pub async fn revoke_token(&self, user: &User, agent_id: Uuid, token_id: Uuid) -> Result<(), CallError> {
        self.owned_agent(user, agent_id).await?;
        sqlx::query("UPDATE agent_tokens SET revoked_at = now() WHERE id = $1 AND agent_id = $2 AND revoked_at IS NULL")
            .bind(token_id)
            .bind(agent_id)
            .execute(self.pool())
            .await
            .map_err(db_err)?;
        Ok(())
    }

    async fn owned_agent(&self, user: &User, agent_id: Uuid) -> Result<(), CallError> {
        let ok: Option<Uuid> = sqlx::query_scalar("SELECT id FROM agents WHERE id = $1 AND owner_id = $2")
            .bind(agent_id)
            .bind(user.id)
            .fetch_optional(self.pool())
            .await
            .map_err(db_err)?;
        if ok.is_none() {
            return Err(CallError::new(404, "not_found", "找不到这个智能体"));
        }
        Ok(())
    }

    pub async fn authenticate(&self, token: &str) -> Result<AgentCtx, CallError> {
        let hash = sha256(token.as_bytes());
        let row = sqlx::query_as::<_, (Uuid, Uuid, Uuid, String, bool, Vec<String>)>(
            r#"SELECT t.id, a.id, a.owner_id, a.name, a.allow_comments, t.scopes
               FROM agent_tokens t
               JOIN agents a ON a.id = t.agent_id
               WHERE t.token_hash = $1 AND t.revoked_at IS NULL
                 AND (t.expires_at IS NULL OR t.expires_at > now())"#,
        )
        .bind(hash.as_slice())
        .fetch_optional(self.pool())
        .await
        .map_err(db_err)?;
        let Some((token_id, agent_id, owner_id, name, allow_comments, scopes)) = row else {
            return Err(CallError::new(401, "unauthorized", "令牌无效或已吊销"));
        };
        sqlx::query("UPDATE agent_tokens SET last_used_at = now() WHERE id = $1")
            .bind(token_id)
            .execute(self.pool())
            .await
            .ok();
        sqlx::query("UPDATE agents SET last_seen_at = now() WHERE id = $1")
            .bind(agent_id)
            .execute(self.pool())
            .await
            .ok();
        Ok(AgentCtx {
            agent_id,
            owner_id,
            name,
            allow_comments,
            scopes,
            token_id,
        })
    }

    pub async fn whoami(&self, agent: &AgentCtx) -> Result<Value, CallError> {
        let handle: String = sqlx::query_scalar("SELECT handle FROM users WHERE id = $1")
            .bind(agent.owner_id)
            .fetch_one(self.pool())
            .await
            .map_err(db_err)?;
        let keys = self.keys_of(agent.agent_id).await?;
        Ok(json!({
            "agent_id": agent.agent_id,
            "name": agent.name,
            "owner_handle": handle,
            "scopes": agent.scopes,
            "allow_comments": agent.allow_comments,
            "keys": keys,
        }))
    }

    pub fn list_games(&self) -> Vec<Value> {
        registry()
            .list()
            .into_iter()
            .map(|m| serde_json::to_value(&m).unwrap_or(json!({})))
            .collect()
    }

    pub fn game_detail(&self, id: &str) -> Result<Value, CallError> {
        let info = self.inner.games.get(id).ok_or_else(|| CallError::new(404, "not_found", "没有这个游戏"))?;
        Ok(info.manifest.clone())
    }

    pub async fn key_challenge(&self, agent: &AgentCtx) -> Result<Value, CallError> {
        let id = Uuid::new_v4();
        let mut nonce = [0u8; 32];
        OsRng.fill_bytes(&mut nonce);
        let nonce_hex = hex::encode(nonce);
        let expires = Utc::now() + Duration::minutes(10);
        let payload = json!({
            "challenge_id": id.to_string(),
            "agent_id": agent.agent_id.to_string(),
            "nonce": nonce_hex,
            "expires_at": expires.timestamp_millis(),
        });
        sqlx::query(
            "INSERT INTO key_challenges (id, agent_id, nonce, purpose, payload, expires_at) VALUES ($1,$2,$3,'register_key',$4,$5)",
        )
        .bind(id)
        .bind(agent.agent_id)
        .bind(&nonce_hex)
        .bind(&payload)
        .bind(expires)
        .execute(self.pool())
        .await
        .map_err(db_err)?;
        Ok(json!({
            "signing_request": {
                "domain": domain("register_key"),
                "payload": payload,
            }
        }))
    }

    pub async fn register_key(&self, agent: &AgentCtx, envelope: &Value, label: Option<&str>) -> Result<Value, CallError> {
        let domain_v = envelope.get("domain").cloned().unwrap_or(Value::Null);
        let payload = envelope.get("payload").cloned().unwrap_or(Value::Null);
        let signature = envelope.get("signature").and_then(|v| v.as_str()).unwrap_or("");
        if !domain_ok(&domain_v, "register_key") {
            return Err(CallError::new(400, "domain_mismatch", "注册挑战的 domain 不正确"));
        }
        let public_b64 = payload.get("public_key").and_then(|v| v.as_str()).unwrap_or("");
        let alg = payload.get("alg").and_then(|v| v.as_str()).unwrap_or("");
        if alg != "ed25519" || public_b64.is_empty() {
            return Err(CallError::new(400, "bad_signature", "需要 ed25519 公钥"));
        }
        let raw = b64url_decode(public_b64).map_err(|_| CallError::new(400, "bad_signature", "公钥不是 base64url"))?;
        if raw.len() != 32 {
            return Err(CallError::new(400, "bad_signature", "公钥长度不正确"));
        }
        let expect = kid_of(&raw, "ak");
        let kid = payload.get("kid").and_then(|v| v.as_str()).unwrap_or("");
        if kid != expect || envelope.get("kid").and_then(|v| v.as_str()) != Some(kid) {
            return Err(CallError::new(400, "key_mismatch", "kid 与公钥不匹配"));
        }
        if !verify_value(public_b64, &domain_v, &payload, signature) {
            return Err(CallError::new(400, "bad_signature", "持有证明签名无效"));
        }
        let challenge_id = payload.get("challenge_id").and_then(|v| v.as_str()).unwrap_or("");
        let cid = Uuid::parse_str(challenge_id).map_err(|_| CallError::new(400, "context_mismatch", "challenge_id 无效"))?;
        let row = sqlx::query_as::<_, (String, Value, DateTime<Utc>, Option<DateTime<Utc>>)>(
            "SELECT nonce, payload, expires_at, used_at FROM key_challenges WHERE id = $1 AND agent_id = $2",
        )
        .bind(cid)
        .bind(agent.agent_id)
        .fetch_optional(self.pool())
        .await
        .map_err(db_err)?;
        let Some((nonce, stored, expires, used)) = row else {
            return Err(CallError::new(400, "context_mismatch", "找不到这个挑战"));
        };
        if used.is_some() {
            return Err(CallError::new(400, "replay_rejected", "挑战已使用"));
        }
        if expires < Utc::now() {
            return Err(CallError::new(400, "context_mismatch", "挑战已过期"));
        }
        if payload.get("nonce").and_then(|v| v.as_str()) != Some(nonce.as_str())
            || payload.get("agent_id").and_then(|v| v.as_str()) != Some(&agent.agent_id.to_string())
        {
            return Err(CallError::new(400, "context_mismatch", "挑战内容不匹配"));
        }
        let _ = stored;
        let existing: Option<String> = sqlx::query_scalar("SELECT public_key_b64 FROM agent_keys WHERE kid = $1")
            .bind(kid)
            .fetch_optional(self.pool())
            .await
            .map_err(db_err)?;
        if let Some(prev) = existing {
            if prev == public_b64 {
                return Ok(json!({"ok": true, "kid": kid, "idempotent": true}));
            }
            return Err(CallError::new(409, "key_mismatch", "这个 kid 已属于其他密钥"));
        }
        let active: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM agent_keys WHERE agent_id = $1 AND status = 'active'")
            .bind(agent.agent_id)
            .fetch_one(self.pool())
            .await
            .map_err(db_err)?;
        if active >= 3 {
            return Err(CallError::new(400, "too_many_keys", "每个智能体最多 3 把有效密钥"));
        }
        let sig = b64url_decode(signature).unwrap_or_default();
        let mut tx = self.pool().begin().await.map_err(db_err)?;
        sqlx::query(
            r#"INSERT INTO agent_keys (kid, agent_id, alg, public_key, public_key_b64, label, status, pop_signature, pop_challenge)
               VALUES ($1,$2,'ed25519',$3,$4,$5,'active',$6,$7)"#,
        )
        .bind(kid)
        .bind(agent.agent_id)
        .bind(&raw)
        .bind(public_b64)
        .bind(label)
        .bind(&sig)
        .bind(challenge_id)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
        sqlx::query("UPDATE key_challenges SET used_at = now() WHERE id = $1")
            .bind(cid)
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        tx.commit().await.map_err(db_err)?;
        Ok(json!({"ok": true, "kid": kid, "alg": "ed25519", "public_key": public_b64}))
    }

    pub async fn create_room(&self, agent: &AgentCtx, body: &Value) -> Result<Value, CallError> {
        let game_id = body.get("game").and_then(|v| v.as_str()).unwrap_or("gomoku");
        let info = self
            .inner
            .games
            .get(game_id)
            .ok_or_else(|| CallError::new(404, "not_found", "没有这个游戏"))?;
        if !info.playable {
            return Err(CallError::new(400, "game_not_playable", "这款游戏已登记，但本里程碑还不能开局"));
        }
        let manifest = registry().get(game_id).unwrap().manifest();
        let variant = body.get("variant").and_then(|v| v.as_str()).map(|s| s.to_string()).unwrap_or_else(|| {
            manifest.variants.first().map(|v| v.id.clone()).unwrap_or_else(|| "standard".into())
        });
        if !manifest.variants.iter().any(|v| v.id == variant) {
            return Err(CallError::new(400, "bad_variant", "未知变体"));
        }
        let time_limit_sec = body.get("time_limit_sec").and_then(|v| v.as_i64()).unwrap_or(60);
        if !(5..=600).contains(&time_limit_sec) {
            return Err(CallError::new(400, "invalid_time", "单步时限需在 5–600 秒"));
        }
        let policy = body
            .get("timeout_policy")
            .and_then(|v| v.as_str())
            .unwrap_or(manifest.timeout_policy.as_str());
        if !manifest.allowed_timeout_policies.iter().any(|p| p.as_str() == policy) {
            return Err(CallError::new(400, "bad_policy", "这款游戏不允许该超时策略"));
        }
        let max_to = body.get("max_consecutive_timeouts").and_then(|v| v.as_i64()).unwrap_or(3);
        if !(0..=20).contains(&max_to) {
            return Err(CallError::new(400, "bad_policy", "连续超时上限需在 0–20"));
        }
        let allow_comments = body.get("allow_comments").and_then(|v| v.as_bool()).unwrap_or(true);
        let kid = self.resolve_key(agent, body.get("kid").and_then(|v| v.as_str())).await?;
        self.ensure_free(agent).await?;
        self.ensure_not_queued(agent).await?;
        let match_id = Uuid::new_v4();
        let code = self.unique_code().await?;
        let mut tx = self.pool().begin().await.map_err(db_err)?;
        sqlx::query(
            r#"INSERT INTO matches (
                 id, game_version_id, game_id, variant, mode, room_code, created_by,
                 time_limit_ms, timeout_policy, max_consecutive_timeouts, grace_ms,
                 allow_comments, rules_hash, server_kid, status
               ) VALUES (
                 $1,$2,$3,$4,'casual',$5,$6,$7,$8,$9,2000,$10,$11,$12,'waiting'
               )"#,
        )
        .bind(match_id)
        .bind(info.version_id)
        .bind(game_id)
        .bind(&variant)
        .bind(&code)
        .bind(agent.owner_id)
        .bind((time_limit_sec * 1000) as i32)
        .bind(policy)
        .bind(max_to as i32)
        .bind(allow_comments)
        .bind(&info.rules_hash)
        .bind(&self.server().kid)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
        sqlx::query(
            "INSERT INTO match_players (match_id, seat, agent_id, owner_id, kid) VALUES ($1, 0, $2, $3, $4)",
        )
        .bind(match_id)
        .bind(agent.agent_id)
        .bind(agent.owner_id)
        .bind(&kid)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
        tx.commit().await.map_err(db_err)?;
        self.inner.hub.slot(match_id);
        Ok(json!({
            "match_id": match_id,
            "room_code": code,
            "seat": 0,
            "game_id": game_id,
            "variant": variant,
            "status": "waiting",
        }))
    }

    pub async fn join_room(&self, agent: &AgentCtx, code: &str, kid: Option<&str>) -> Result<Value, CallError> {
        let code = code.trim().to_uppercase();
        let kid = self.resolve_key(agent, kid).await?;
        self.ensure_free(agent).await?;
        self.ensure_not_queued(agent).await?;
        let mut tx = self.pool().begin().await.map_err(db_err)?;
        let row = sqlx::query_as::<_, (Uuid, String, String, String, i64)>(
            r#"SELECT m.id, m.status, m.game_id, m.mode,
                      (SELECT COUNT(*) FROM match_players p WHERE p.match_id = m.id)
               FROM matches m WHERE m.room_code = $1 FOR UPDATE"#,
        )
        .bind(&code)
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_err)?;
        let Some((match_id, status, game_id, mode, n)) = row else {
            return Err(CallError::new(404, "not_found", "找不到这个房间码"));
        };
        let max = registry().get(&game_id).map(|g| g.manifest().max_players).unwrap_or(2) as i64;
        if status != "waiting" || n >= max {
            return Err(CallError::new(409, "room_full", "房间已满或已开始"));
        }
        let already: Option<i16> = sqlx::query_scalar("SELECT seat FROM match_players WHERE match_id = $1 AND agent_id = $2")
            .bind(match_id)
            .bind(agent.agent_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(db_err)?;
        if already.is_some() {
            return Err(CallError::new(409, "already_seated", "你已经在这个房间里"));
        }
        if mode == "ranked" {
            let same_owner: Option<i16> = sqlx::query_scalar(
                "SELECT seat FROM match_players WHERE match_id = $1 AND owner_id = $2",
            )
            .bind(match_id)
            .bind(agent.owner_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(db_err)?;
            if same_owner.is_some() {
                return Err(CallError::new(409, "same_owner", "排位不能和自己的另一个智能体同桌"));
            }
        }
        let seat = n as i16;
        sqlx::query("INSERT INTO match_players (match_id, seat, agent_id, owner_id, kid) VALUES ($1, $2, $3, $4, $5)")
            .bind(match_id)
            .bind(seat)
            .bind(agent.agent_id)
            .bind(agent.owner_id)
            .bind(&kid)
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        let starting = n + 1 == max;
        if starting {
            sqlx::query("UPDATE matches SET status = 'running', started_at = now() WHERE id = $1")
                .bind(match_id)
                .execute(&mut *tx)
                .await
                .map_err(db_err)?;
        }
        tx.commit().await.map_err(db_err)?;
        if starting {
            spawn_room(self.pool().clone(), self.server().clone(), &self.inner.hub, match_id)
                .await
                .map_err(|err| CallError::new(500, "start_failed", err.to_string()))?;
        }
        Ok(json!({
            "match_id": match_id,
            "room_code": code,
            "seat": seat,
            "status": if starting { "running" } else { "waiting" },
        }))
    }

    async fn resolve_key(&self, agent: &AgentCtx, kid: Option<&str>) -> Result<String, CallError> {
        if let Some(kid) = kid {
            let ok: Option<String> = sqlx::query_scalar(
                "SELECT kid FROM agent_keys WHERE kid = $1 AND agent_id = $2 AND status = 'active'",
            )
            .bind(kid)
            .bind(agent.agent_id)
            .fetch_optional(self.pool())
            .await
            .map_err(db_err)?;
            return ok.ok_or_else(|| CallError::new(400, "unknown_key", "指定的密钥不可用"));
        }
        let kid: Option<String> = sqlx::query_scalar(
            "SELECT kid FROM agent_keys WHERE agent_id = $1 AND status = 'active' ORDER BY created_at DESC LIMIT 1",
        )
        .bind(agent.agent_id)
        .fetch_optional(self.pool())
        .await
        .map_err(db_err)?;
        kid.ok_or_else(|| CallError::new(400, "no_key", "先注册签名公钥，才能入座"))
    }

    async fn ensure_not_queued(&self, agent: &AgentCtx) -> Result<(), CallError> {
        let queued: Option<Uuid> = sqlx::query_scalar("SELECT id FROM match_queue WHERE agent_id = $1")
            .bind(agent.agent_id)
            .fetch_optional(self.pool())
            .await
            .map_err(db_err)?;
        if queued.is_some() {
            return Err(CallError::new(409, "queued", "这个智能体正在排位队列里"));
        }
        Ok(())
    }

    async fn ensure_free(&self, agent: &AgentCtx) -> Result<(), CallError> {
        let existing: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT m.id FROM match_players mp
               JOIN matches m ON m.id = mp.match_id
               WHERE mp.agent_id = $1 AND m.status IN ('waiting','running')
               LIMIT 1"#,
        )
        .bind(agent.agent_id)
        .fetch_optional(self.pool())
        .await
        .map_err(db_err)?;
        if existing.is_some() {
            return Err(CallError::new(409, "already_seated", "这个智能体已经在另一局里"));
        }
        Ok(())
    }

    async fn unique_code(&self) -> Result<String, CallError> {
        const ALPH: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
        for _ in 0..8 {
            let code: String = {
                let mut rng = rand::thread_rng();
                (0..6)
                    .map(|_| ALPH[rng.gen_range(0..ALPH.len())] as char)
                    .collect()
            };
            let exists: Option<Uuid> = sqlx::query_scalar("SELECT id FROM matches WHERE room_code = $1")
                .bind(&code)
                .fetch_optional(self.pool())
                .await
                .map_err(db_err)?;
            if exists.is_none() {
                return Ok(code);
            }
        }
        Err(CallError::new(500, "code", "生成房间码失败"))
    }

    pub async fn get_state(&self, agent: &AgentCtx, match_id: Uuid) -> Result<Value, CallError> {
        self.touch(agent.agent_id).await;
        if let Some(tx) = self.inner.hub.sender(match_id) {
            return roundtrip(&tx, |reply| Cmd::Agent {
                agent_id: agent.agent_id,
                reply,
            })
            .await;
        }
        let mut body = Engine::read_public(self.pool().clone(), self.server().clone(), match_id)
            .await
            .map_err(|_| CallError::new(404, "not_found", "找不到这局"))?;
        if body.get("error").is_some() {
            return Err(CallError::new(404, "not_found", "找不到这局"));
        }
        let seat = seat_of(&body, agent.agent_id);
        let Some(seat) = seat else {
            return Err(CallError::new(403, "not_seated", "你不在这局里"));
        };
        body["seat"] = json!(seat);
        body["legal_actions"] = json!([]);
        body["signing_request"] = Value::Null;
        body["phase"] = json!(if body["status"] == "finished" || body["status"] == "aborted" { "game_over" } else { "waiting" });
        body["remaining_ms"] = json!(0);
        body["consecutive_timeouts"] = json!(0);
        Ok(body)
    }

    pub async fn wait_for_turn(&self, agent: &AgentCtx, match_id: Uuid) -> Result<Value, CallError> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(25);
        loop {
            let slot = self.inner.hub.slot(match_id);
            let notified = slot.notify.notified();
            tokio::pin!(notified);
            let view = self.get_state(agent, match_id).await?;
            let mapped = map_turn(&view);
            if mapped["status"] != "not_your_turn" {
                return Ok(mapped);
            }
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            if left.is_zero() {
                return Ok(mapped);
            }
            tokio::select! {
                _ = &mut notified => continue,
                _ = tokio::time::sleep(left) => {
                    let view = self.get_state(agent, match_id).await?;
                    return Ok(map_turn(&view));
                }
            }
        }
    }

    pub async fn make_move(&self, agent: &AgentCtx, match_id: Uuid, envelope: Value, private: Option<Value>) -> Result<Value, CallError> {
        self.touch(agent.agent_id).await;
        self.ensure_actor(match_id).await?;
        let Some(tx) = self.inner.hub.sender(match_id) else {
            return Ok(json!({"accepted": false, "error": "game_over", "message": "对局不在进行中"}));
        };
        roundtrip(&tx, |reply| Cmd::Move {
            agent_id: agent.agent_id,
            envelope,
            private,
            reply,
        })
        .await
    }

    pub async fn resign(&self, agent: &AgentCtx, match_id: Uuid) -> Result<Value, CallError> {
        self.touch(agent.agent_id).await;
        self.ensure_actor(match_id).await?;
        let Some(tx) = self.inner.hub.sender(match_id) else {
            return Ok(json!({"accepted": false, "error": "game_over", "message": "对局不在进行中"}));
        };
        roundtrip(&tx, |reply| Cmd::Resign {
            agent_id: agent.agent_id,
            reply,
        })
        .await
    }

    pub async fn join_queue(&self, agent: &AgentCtx, body: &Value) -> Result<Value, CallError> {
        if !agent.scopes.iter().any(|s| s == "play:ranked") {
            return Err(CallError::new(403, "ranked_forbidden", "这个令牌没有排位权限"));
        }
        if let Some(found) = self.ranked_seat(agent.agent_id).await? {
            return Ok(found);
        }
        let game_id = body.get("game").and_then(|v| v.as_str()).unwrap_or("gomoku");
        let info = self.inner.games.get(game_id).ok_or_else(|| CallError::new(404, "not_found", "没有这个游戏"))?;
        if !info.playable {
            return Err(CallError::new(400, "game_not_playable", "这款游戏还不能排位"));
        }
        let manifest = registry().get(game_id).unwrap().manifest();
        if !manifest.rated {
            return Err(CallError::new(400, "unrated", "这款游戏不计入天梯"));
        }
        let variant = body.get("variant").and_then(|v| v.as_str()).map(|s| s.to_string()).unwrap_or_else(|| {
            manifest.variants.first().map(|v| v.id.clone()).unwrap_or_else(|| "standard".into())
        });
        if !manifest.variants.iter().any(|v| v.id == variant) {
            return Err(CallError::new(400, "bad_variant", "未知变体"));
        }
        self.ensure_free(agent).await?;
        let kid = self.resolve_key(agent, body.get("kid").and_then(|v| v.as_str())).await?;
        let rating = self.display_rating(agent.agent_id, game_id).await?;
        let id = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO match_queue (id, agent_id, owner_id, game_id, variant, kid, rating)
               VALUES ($1,$2,$3,$4,$5,$6,$7)
               ON CONFLICT (agent_id) DO UPDATE SET game_id = EXCLUDED.game_id, variant = EXCLUDED.variant,
                 kid = EXCLUDED.kid, rating = EXCLUDED.rating, enqueued_at = now()"#,
        )
        .bind(id)
        .bind(agent.agent_id)
        .bind(agent.owner_id)
        .bind(game_id)
        .bind(&variant)
        .bind(&kid)
        .bind(rating)
        .execute(self.pool())
        .await
        .map_err(db_err)?;
        self.matchmake_once().await.map_err(|err| CallError::new(500, "matchmaker", err.to_string()))?;
        if let Some(found) = self.ranked_seat(agent.agent_id).await? {
            return Ok(found);
        }
        Ok(json!({"status": "queued", "game_id": game_id, "variant": variant, "rating": rating}))
    }

    pub async fn queue_status(&self, agent: &AgentCtx) -> Result<Value, CallError> {
        if let Some(found) = self.ranked_seat(agent.agent_id).await? {
            return Ok(found);
        }
        let row = sqlx::query_as::<_, (String, String, f64, chrono::DateTime<Utc>)>(
            "SELECT game_id, variant, rating, enqueued_at FROM match_queue WHERE agent_id = $1",
        )
        .bind(agent.agent_id)
        .fetch_optional(self.pool())
        .await
        .map_err(db_err)?;
        let Some((game_id, variant, rating, at)) = row else {
            return Ok(json!({"status": "idle"}));
        };
        Ok(json!({
            "status": "queued",
            "game_id": game_id,
            "variant": variant,
            "rating": rating,
            "waiting_ms": (Utc::now() - at).num_milliseconds().max(0),
        }))
    }

    async fn ranked_seat(&self, agent_id: Uuid) -> Result<Option<Value>, CallError> {
        let row = sqlx::query_as::<_, (Uuid, i16, String)>(
            r#"SELECT m.id, mp.seat, m.game_id FROM match_players mp
               JOIN matches m ON m.id = mp.match_id
               WHERE mp.agent_id = $1 AND m.mode = 'ranked' AND m.status = 'running'
               ORDER BY m.started_at DESC NULLS LAST LIMIT 1"#,
        )
        .bind(agent_id)
        .fetch_optional(self.pool())
        .await
        .map_err(db_err)?;
        Ok(row.map(|(id, seat, game_id)| json!({"status": "matched", "match_id": id, "seat": seat, "game_id": game_id})))
    }

    async fn display_rating(&self, agent_id: Uuid, game_id: &str) -> Result<f64, CallError> {
        let rating: Option<f64> = sqlx::query_scalar(
            "SELECT rating FROM ratings WHERE agent_id = $1 AND game_id = $2 AND season = '2026'",
        )
        .bind(agent_id)
        .bind(game_id)
        .fetch_optional(self.pool())
        .await
        .map_err(db_err)?;
        Ok(rating.unwrap_or(1500.0))
    }

    pub async fn matchmake_once(&self) -> anyhow::Result<()> {
        let _guard = queue_lock().lock().await;
        let rows = sqlx::query_as::<_, QueueRow>(
            r#"SELECT id, agent_id, owner_id, game_id, variant, kid, rating, enqueued_at
               FROM match_queue ORDER BY enqueued_at"#,
        )
        .fetch_all(self.pool())
        .await?;
        let mut groups: HashMap<(String, String), Vec<QueueRow>> = HashMap::new();
        for row in rows {
            groups.entry((row.game_id.clone(), row.variant.clone())).or_default().push(row);
        }
        for ((game_id, variant), waiting) in groups {
            let Some(game) = registry().get(&game_id) else { continue };
            let manifest = game.manifest();
            let need = manifest.max_players as usize;
            if waiting.len() < need { continue };
            let mut used = vec![false; waiting.len()];
            let mut batch = Vec::new();
            for i in 0..waiting.len() {
                if used[i] { continue; }
                let mut group = vec![i];
                let mut owners = vec![waiting[i].owner_id];
                let mut rest: Vec<usize> = (0..waiting.len()).filter(|j| *j != i && !used[*j]).collect();
                rest.sort_by(|&a, &b| {
                    let da = (waiting[a].rating - waiting[i].rating).abs();
                    let db = (waiting[b].rating - waiting[i].rating).abs();
                    da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                });
                for j in rest {
                    if owners.contains(&waiting[j].owner_id) { continue; }
                    let fits = group.iter().all(|&g| {
                        let diff = (waiting[g].rating - waiting[j].rating).abs();
                        diff <= window_of(waiting[g].enqueued_at).max(window_of(waiting[j].enqueued_at))
                    });
                    if fits {
                        owners.push(waiting[j].owner_id);
                        group.push(j);
                        if group.len() == need { break; }
                    }
                }
                if group.len() == need {
                    for g in &group { used[*g] = true; }
                    let picked: Vec<QueueRow> = group.into_iter().map(|i| waiting[i].clone()).collect();
                    batch.push(picked);
                }
            }
            let _ = waiting;
            for picked in batch {
                self.start_ranked(&game_id, &variant, &manifest, &picked).await?;
            }
        }
        Ok(())
    }

    async fn start_ranked(&self, game_id: &str, variant: &str, manifest: &crate::game::Manifest, picked: &[QueueRow]) -> anyhow::Result<()> {
        let info = self.inner.games.get(game_id).ok_or_else(|| anyhow::anyhow!("missing game"))?;
        let match_id = Uuid::new_v4();
        let mut tx = self.pool().begin().await?;
        sqlx::query(
            r#"INSERT INTO matches (
                 id, game_version_id, game_id, variant, mode, created_by,
                 time_limit_ms, timeout_policy, max_consecutive_timeouts, grace_ms,
                 allow_comments, rules_hash, server_kid, status, started_at
               ) VALUES (
                 $1,$2,$3,$4,'ranked',$5,$6,$7,$8,2000,false,$9,$10,'running', now()
               )"#,
        )
        .bind(match_id)
        .bind(info.version_id)
        .bind(game_id)
        .bind(variant)
        .bind(picked[0].owner_id)
        .bind(manifest.default_time.per_action_ms as i32)
        .bind(manifest.timeout_policy.as_str())
        .bind(manifest.default_time.max_consecutive_timeouts as i32)
        .bind(&info.rules_hash)
        .bind(&self.server().kid)
        .execute(&mut *tx)
        .await?;
        for (seat, player) in picked.iter().enumerate() {
            sqlx::query("INSERT INTO match_players (match_id, seat, agent_id, owner_id, kid) VALUES ($1,$2,$3,$4,$5)")
                .bind(match_id)
                .bind(seat as i16)
                .bind(player.agent_id)
                .bind(player.owner_id)
                .bind(&player.kid)
                .execute(&mut *tx)
                .await?;
            sqlx::query("DELETE FROM match_queue WHERE agent_id = $1")
                .bind(player.agent_id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        spawn_room(self.pool().clone(), self.server().clone(), &self.inner.hub, match_id).await?;
        Ok(())
    }

    pub async fn leaderboard(&self, game: &str) -> Result<Value, CallError> {
        crate::rating::leaderboard(self.pool(), game).await.map_err(|err| CallError::new(500, "leaderboard", err.to_string()))
    }

    pub async fn user_profile(&self, handle: &str) -> Result<Value, CallError> {
        let user = sqlx::query_as::<_, (Uuid, String, chrono::DateTime<Utc>)>(
            "SELECT id, handle, created_at FROM users WHERE handle = $1",
        )
        .bind(handle)
        .fetch_optional(self.pool())
        .await
        .map_err(db_err)?
        .ok_or_else(|| CallError::new(404, "not_found", "没有这个用户"))?;
        let agents = sqlx::query_as::<_, (Uuid, String, Option<String>)>(
            "SELECT id, name, model_label FROM agents WHERE owner_id = $1 ORDER BY created_at",
        )
        .bind(user.0)
        .fetch_all(self.pool())
        .await
        .map_err(db_err)?;
        let mut listed = Vec::new();
        for (id, name, model) in agents {
            let ratings = self.agent_ratings(id).await?;
            listed.push(json!({"id": id, "name": name, "model_label": model, "ratings": ratings}));
        }
        let recent = self.recent_matches_for_user(user.0).await?;
        Ok(json!({"user": {"handle": user.1, "created_at": user.2.timestamp_millis()}, "agents": listed, "recent": recent}))
    }

    pub async fn agent_profile(&self, id: Uuid) -> Result<Value, CallError> {
        let row = sqlx::query_as::<_, (Uuid, String, Option<String>, Option<String>, String)>(
            r#"SELECT a.id, a.name, a.description, a.model_label, u.handle
               FROM agents a JOIN users u ON u.id = a.owner_id WHERE a.id = $1"#,
        )
        .bind(id)
        .fetch_optional(self.pool())
        .await
        .map_err(db_err)?
        .ok_or_else(|| CallError::new(404, "not_found", "没有这个智能体"))?;
        let ratings = self.agent_ratings(id).await?;
        let recent = self.recent_matches_for_agent(id).await?;
        Ok(json!({
            "agent": {"id": row.0, "name": row.1, "description": row.2, "model_label": row.3, "handle": row.4},
            "ratings": ratings,
            "recent": recent,
        }))
    }

    async fn agent_ratings(&self, id: Uuid) -> Result<Vec<Value>, CallError> {
        let rows = sqlx::query_as::<_, (String, f64, f64, i32, i32, String)>(
            "SELECT game_id, rating, rd, games, wins, system FROM ratings WHERE agent_id = $1 AND season = '2026' ORDER BY game_id",
        )
        .bind(id)
        .fetch_all(self.pool())
        .await
        .map_err(db_err)?;
        Ok(rows.into_iter().map(|(game, rating, rd, games, wins, system)| json!({
            "game_id": game, "rating": rating.round(), "rd": rd.round(), "games": games, "wins": wins,
            "system": system, "provisional": crate::rating::provisional(games, rd),
        })).collect())
    }

    async fn recent_matches_for_user(&self, user: Uuid) -> Result<Vec<Value>, CallError> {
        let rows = sqlx::query_as::<_, (Uuid, String, String, Option<String>, Option<f64>, i16)>(
            r#"SELECT m.id, m.game_id, m.status, m.end_reason, mp.score, mp.seat
               FROM match_players mp JOIN matches m ON m.id = mp.match_id
               WHERE mp.owner_id = $1 ORDER BY m.created_at DESC LIMIT 12"#,
        )
        .bind(user)
        .fetch_all(self.pool())
        .await
        .map_err(db_err)?;
        Ok(rows.into_iter().map(|(id, game, status, reason, score, seat)| json!({
            "id": id, "game_id": game, "status": status, "end_reason": reason, "score": score, "seat": seat,
        })).collect())
    }

    async fn recent_matches_for_agent(&self, agent: Uuid) -> Result<Vec<Value>, CallError> {
        let rows = sqlx::query_as::<_, (Uuid, String, String, Option<String>, Option<f64>, i16)>(
            r#"SELECT m.id, m.game_id, m.status, m.end_reason, mp.score, mp.seat
               FROM match_players mp JOIN matches m ON m.id = mp.match_id
               WHERE mp.agent_id = $1 ORDER BY m.created_at DESC LIMIT 12"#,
        )
        .bind(agent)
        .fetch_all(self.pool())
        .await
        .map_err(db_err)?;
        Ok(rows.into_iter().map(|(id, game, status, reason, score, seat)| json!({
            "id": id, "game_id": game, "status": status, "end_reason": reason, "score": score, "seat": seat,
        })).collect())
    }

    async fn ensure_actor(&self, match_id: Uuid) -> Result<(), CallError> {
        if self.inner.hub.sender(match_id).is_some() {
            return Ok(());
        }
        let status: Option<String> = sqlx::query_scalar("SELECT status FROM matches WHERE id = $1")
            .bind(match_id)
            .fetch_optional(self.pool())
            .await
            .map_err(db_err)?;
        let Some(status) = status else {
            return Err(CallError::new(404, "not_found", "找不到这局"));
        };
        if status == "running" {
            spawn_room(self.pool().clone(), self.server().clone(), &self.inner.hub, match_id)
                .await
                .map_err(|err| CallError::new(500, "start_failed", err.to_string()))?;
        }
        Ok(())
    }

    pub async fn public_match(&self, match_id: Uuid) -> Result<Value, CallError> {
        if let Some(tx) = self.inner.hub.sender(match_id) {
            return roundtrip(&tx, |reply| Cmd::Public { reply }).await;
        }
        let viewers = self.inner.hub.slot(match_id).viewers.clone();
        let mut body = Engine::read_public(self.pool().clone(), self.server().clone(), match_id)
            .await
            .map_err(|_| CallError::new(404, "not_found", "找不到这局"))?;
        if let Some(obj) = body.as_object_mut() {
            obj.insert(
                "viewer_count".into(),
                json!(viewers.load(std::sync::atomic::Ordering::Relaxed)),
            );
        }
        Ok(body)
    }

    pub async fn list_live(&self) -> Result<Vec<Value>, CallError> {
        let rows = sqlx::query_as::<_, (Uuid, String, String, String, Option<String>, Option<String>, Option<i32>, Option<Value>)>(
            r#"SELECT m.id, m.game_id, g.name, m.status, m.room_code, m.variant, m.tip_seq, m.state
               FROM matches m JOIN games g ON g.id = m.game_id
               WHERE m.status IN ('waiting','running')
               ORDER BY m.created_at DESC
               LIMIT 24"#,
        )
        .fetch_all(self.pool())
        .await
        .map_err(db_err)?;
        let mut out = Vec::new();
        for (id, game_id, game_name, status, room_code, variant, tip_seq, state) in rows {
            let seats = sqlx::query_as::<_, (i16, String, String)>(
                r#"SELECT mp.seat, a.name, u.handle
                   FROM match_players mp
                   JOIN agents a ON a.id = mp.agent_id
                   JOIN users u ON u.id = mp.owner_id
                   WHERE mp.match_id = $1 ORDER BY mp.seat"#,
            )
            .bind(id)
            .fetch_all(self.pool())
            .await
            .map_err(db_err)?;
            let preview = state.as_ref().and_then(|s| {
                registry().get(&game_id).and_then(|g| {
                    g.view(s, &crate::game::Viewer { seat: None, omniscient: false }).ok()
                })
            });
            out.push(json!({
                "id": id,
                "game_id": game_id,
                "game_name": game_name,
                "status": status,
                "room_code": room_code,
                "variant": variant,
                "tip_seq": tip_seq,
                "viewer_count": self.inner.hub.slot(id).viewers.load(std::sync::atomic::Ordering::Relaxed),
                "seats": seats.into_iter().map(|(seat, name, handle)| json!({"seat": seat, "agent_name": name, "owner_handle": handle})).collect::<Vec<_>>(),
                "view": preview,
            }));
        }
        Ok(out)
    }

    pub async fn match_log(&self, match_id: Uuid, from_seq: i64) -> Result<String, CallError> {
        let lines: Vec<String> = sqlx::query_scalar(
            "SELECT entry_line FROM match_events WHERE match_id = $1 AND seq >= $2 ORDER BY seq",
        )
        .bind(match_id)
        .bind(from_seq as i32)
        .fetch_all(self.pool())
        .await
        .map_err(db_err)?;
        if lines.is_empty() {
            let exists: Option<Uuid> = sqlx::query_scalar("SELECT id FROM matches WHERE id = $1")
                .bind(match_id)
                .fetch_optional(self.pool())
                .await
                .map_err(db_err)?;
            if exists.is_none() {
                return Err(CallError::new(404, "not_found", "找不到这局"));
            }
        }
        Ok(lines.join("\n") + if lines.is_empty() { "" } else { "\n" })
    }

    pub fn server_keys_doc(&self) -> Value {
        json!({
            "keys": [{
                "kid": self.server().kid,
                "alg": "ed25519",
                "public_key": self.server().public_b64,
            }]
        })
    }

    pub fn skill_doc(&self) -> Value {
        let digest = sha256_hex(include_str!("../skill/arena/scripts/sign.mjs").as_bytes());
        json!({
            "version": "1.0.0",
            "files": {
                "scripts/sign.mjs": {"sha256": digest}
            },
            "url": "/skill/arena-skill-v1.0.0.zip"
        })
    }

    async fn touch(&self, agent_id: Uuid) {
        let _ = sqlx::query("UPDATE agents SET last_seen_at = now() WHERE id = $1")
            .bind(agent_id)
            .execute(self.pool())
            .await;
    }
}

fn seat_of(body: &Value, agent_id: Uuid) -> Option<i32> {
    let id = agent_id.to_string();
    body.get("seats")?
        .as_array()?
        .iter()
        .find(|s| s.get("agent_id").and_then(|v| v.as_str()) == Some(id.as_str()))
        .and_then(|s| s.get("seat").and_then(|v| v.as_i64()))
        .map(|n| n as i32)
}

fn map_turn(view: &Value) -> Value {
    if view.get("error").is_some() {
        return view.clone();
    }
    let status = view.get("status").and_then(|v| v.as_str()).unwrap_or("");
    let phase = view.get("phase").and_then(|v| v.as_str()).unwrap_or("");
    let your_turn = view.get("signing_request").map(|v| v.is_object()).unwrap_or(false);
    let turn_status = if status == "finished" || status == "aborted" || phase == "game_over" {
        "game_over"
    } else if your_turn {
        "your_turn"
    } else {
        "not_your_turn"
    };
    json!({
        "status": turn_status,
        "match_id": view.get("id").cloned().unwrap_or(Value::Null),
        "phase": if turn_status == "game_over" { "game_over" } else { phase },
        "deadline_at": view.get("deadline_at").cloned().unwrap_or(Value::Null),
        "remaining_ms": view.get("remaining_ms").cloned().unwrap_or(json!(0)),
        "time_limit_ms": view.get("time_limit_ms").cloned().unwrap_or(json!(60000)),
        "timeout_policy": view.get("timeout_policy").cloned().unwrap_or(Value::Null),
        "consecutive_timeouts": view.get("consecutive_timeouts").cloned().unwrap_or(json!(0)),
        "events_since": view.get("tip_seq").cloned().unwrap_or(json!(0)),
        "to_act": view.get("to_act").cloned().unwrap_or(json!([])),
    })
}

async fn bootstrap_games(pool: &PgPool) -> anyhow::Result<HashMap<String, GameInfo>> {
    let mut out = HashMap::new();
    for manifest in registry().list() {
        let value = serde_json::to_value(&manifest)?;
        let rules_hash = sha256_hex(canon(&value).as_bytes());
        sqlx::query(
            r#"INSERT INTO games (id, builtin, name, description, ranked_eligible)
               VALUES ($1, true, $2, $3, $4)
               ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name, description = EXCLUDED.description, ranked_eligible = EXCLUDED.ranked_eligible"#,
        )
        .bind(&manifest.id)
        .bind(&manifest.name)
        .bind(&manifest.description)
        .bind(manifest.rated)
        .execute(pool)
        .await?;
        let version_id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO game_versions (id, game_id, version, engine, manifest, rules_hash)
               VALUES ($1, $2, $3, 'native', $4, $5)
               ON CONFLICT (game_id, version) DO UPDATE
                 SET manifest = EXCLUDED.manifest, rules_hash = EXCLUDED.rules_hash
               RETURNING id"#,
        )
        .bind(Uuid::new_v4())
        .bind(&manifest.id)
        .bind(&manifest.version)
        .bind(&value)
        .bind(&rules_hash)
        .fetch_one(pool)
        .await?;
        out.insert(
            manifest.id.clone(),
            GameInfo {
                version_id,
                version: manifest.version,
                rules_hash,
                name: manifest.name,
                playable: manifest.playable,
                manifest: value,
            },
        );
    }
    Ok(out)
}

fn hash_password(password: &str) -> Result<String, CallError> {
    if password.len() < 8 {
        return Err(CallError::new(400, "weak_password", "密码至少 8 位"));
    }
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|err| CallError::new(500, "hash", err.to_string()))?;
    Ok(hash.to_string())
}

fn verify_password(hash: &str, password: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(hash) else {
        return false;
    };
    Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok()
}

fn valid_handle(handle: &str) -> bool {
    let n = handle.chars().count();
    (3..=20).contains(&n) && handle.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

fn admin_email(email: &str) -> bool {
    std::env::var("ADMIN_EMAILS")
        .unwrap_or_default()
        .split(',')
        .any(|s| s.trim().eq_ignore_ascii_case(email))
}

fn db_err(err: sqlx::Error) -> CallError {
    tracing::error!(error = %err, "database");
    CallError::new(500, "database", "数据库暂时不可用")
}

fn unique_or_db(conflict: &'static str) -> impl Fn(sqlx::Error) -> CallError {
    move |err: sqlx::Error| {
        if let sqlx::Error::Database(db) = &err {
            if db.code().as_deref() == Some("23505") {
                return CallError::new(409, "conflict", conflict);
            }
        }
        db_err(err)
    }
}

#[allow(dead_code)]
fn _now() -> i64 {
    now_ms()
}

#[allow(dead_code)]
async fn _touch_tx(_tx: &mut Transaction<'_, Postgres>) {}
