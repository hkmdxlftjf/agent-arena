//! One tokio task per live match. The task is the only writer of that match.

use crate::chain::{decode_hex, decode_sig, payload_hash_hex, Chain};
use crate::crypto::{commitment_of, domain, domain_ok, json_i64, now_ms, verify_value, ServerKey};
use crate::game::{result_value, Game, Manifest, TimeoutPolicy, Viewer};
use crate::gomoku::force_terminal;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::PgPool;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::{broadcast, mpsc, oneshot, Notify};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Decision {
    pub seq: i64,
    pub prev_hash: String,
    pub seat: i32,
    pub deadline_at: i64,
}

pub enum Cmd {
    Public {
        reply: oneshot::Sender<Value>,
    },
    Agent {
        agent_id: Uuid,
        reply: oneshot::Sender<Value>,
    },
    Move {
        agent_id: Uuid,
        envelope: Value,
        private: Option<Value>,
        reply: oneshot::Sender<Value>,
    },
    Resign {
        agent_id: Uuid,
        reply: oneshot::Sender<Value>,
    },
}

pub struct Slot {
    pub notify: Arc<Notify>,
    pub events: broadcast::Sender<String>,
    pub viewers: Arc<AtomicUsize>,
    pub cmd: Mutex<Option<mpsc::Sender<Cmd>>>,
}

pub struct Hub {
    slots: Mutex<std::collections::HashMap<Uuid, Arc<Slot>>>,
}

impl Hub {
    pub fn new() -> Self {
        Self {
            slots: Mutex::new(std::collections::HashMap::new()),
        }
    }

    pub fn slot(&self, id: Uuid) -> Arc<Slot> {
        let mut map = self.slots.lock().expect("hub lock");
        map.entry(id)
            .or_insert_with(|| {
                Arc::new(Slot {
                    notify: Arc::new(Notify::new()),
                    events: broadcast::channel(64).0,
                    viewers: Arc::new(AtomicUsize::new(0)),
                    cmd: Mutex::new(None),
                })
            })
            .clone()
    }

    pub fn sender(&self, id: Uuid) -> Option<mpsc::Sender<Cmd>> {
        let map = self.slots.lock().expect("hub lock");
        let slot = map.get(&id).cloned()?;
        drop(map);
        let sender = slot.cmd.lock().expect("cmd lock").clone();
        sender
    }
}

impl Default for Hub {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug)]
struct Secret {
    seat: i32,
    commitment: String,
    data: Value,
    salt: String,
}

struct Player {
    seat: i32,
    agent_id: Uuid,
    name: String,
    handle: String,
    model_label: Option<String>,
    allow_comments: bool,
    kid: String,
    public_b64: String,
    timeouts: i32,
    forfeited: bool,
    last_seen: Option<DateTime<Utc>>,
    score: Option<f64>,
}

pub struct Engine {
    pool: PgPool,
    match_id: Uuid,
    game: Arc<dyn Game>,
    game_id: String,
    game_name: String,
    game_version: String,
    rules_hash: String,
    variant: String,
    mode: String,
    room_code: Option<String>,
    chain: Chain,
    state: Option<Value>,
    players: Vec<Player>,
    decisions: Vec<Decision>,
    secrets: Vec<Secret>,
    status: String,
    result: Option<Value>,
    end_reason: Option<String>,
    seed: [u8; 32],
    seed_hex: String,
    seed_commit: String,
    time_limit_ms: i64,
    grace_ms: i64,
    max_timeouts: i32,
    policy: TimeoutPolicy,
    allow_comments: bool,
    persisted: usize,
    viewers: Arc<AtomicUsize>,
    events: broadcast::Sender<String>,
    notify: Arc<Notify>,
}

#[derive(sqlx::FromRow)]
struct MatchRow {
    game_id: String,
    variant: Option<String>,
    mode: String,
    room_code: Option<String>,
    time_limit_ms: i32,
    timeout_policy: String,
    max_consecutive_timeouts: i32,
    grace_ms: i32,
    allow_comments: bool,
    rules_hash: Option<String>,
    status: String,
    state: Option<Value>,
    open_decisions: Option<Value>,
    seed: Option<String>,
    seed_commit: Option<String>,
    result: Option<Value>,
    end_reason: Option<String>,
    game_version_id: Uuid,
    game_version: String,
    game_name: String,
}

#[derive(sqlx::FromRow)]
struct PlayerRow {
    seat: i16,
    agent_id: Uuid,
    kid: Option<String>,
    timeouts: i32,
    forfeited: bool,
    score: Option<f64>,
    name: String,
    allow_comments: bool,
    model_label: Option<String>,
    last_seen_at: Option<DateTime<Utc>>,
    handle: String,
    public_key_b64: Option<String>,
}

pub async fn spawn_room(pool: PgPool, server: ServerKey, hub: &Hub, id: Uuid) -> anyhow::Result<()> {
    let slot = hub.slot(id);
    let rx = {
        let mut cmd = slot.cmd.lock().expect("cmd lock");
        if cmd.is_some() {
            return Ok(());
        }
        let (tx, rx) = mpsc::channel(64);
        *cmd = Some(tx);
        rx
    };
    let engine = match Engine::begin(
        pool.clone(),
        server,
        id,
        slot.notify.clone(),
        slot.events.clone(),
        slot.viewers.clone(),
    )
    .await
    {
        Ok(engine) => engine,
        Err(err) => {
            *slot.cmd.lock().expect("cmd lock") = None;
            let _ = sqlx::query("UPDATE matches SET status = 'aborted', end_reason = 'start_failed', ended_at = now() WHERE id = $1 AND status = 'running'")
                .bind(id)
                .execute(&pool)
                .await;
            return Err(err);
        }
    };
    tokio::spawn(async move {
        engine.run(rx).await;
    });
    Ok(())
}

impl Engine {
    async fn begin(
        pool: PgPool,
        server: ServerKey,
        id: Uuid,
        notify: Arc<Notify>,
        events: broadcast::Sender<String>,
        viewers: Arc<AtomicUsize>,
    ) -> anyhow::Result<Self> {
        let mut engine = Self::read(pool, server, id, notify, events, viewers).await?;
        if engine.status == "running" && engine.chain.entries.is_empty() {
            engine.open_genesis().await?;
        }
        if engine.status == "running"
            && engine.decisions.is_empty()
            && !engine.is_terminal()
        {
            engine.refresh_decisions();
            engine.commit().await?;
        }
        engine.broadcast();
        Ok(engine)
    }

    pub async fn read_public(pool: PgPool, server: ServerKey, id: Uuid) -> anyhow::Result<Value> {
        let (events, _) = broadcast::channel(1);
        let engine = Self::read(
            pool,
            server,
            id,
            Arc::new(Notify::new()),
            events,
            Arc::new(AtomicUsize::new(0)),
        )
        .await?;
        Ok(engine.public_json())
    }

    async fn read(
        pool: PgPool,
        server: ServerKey,
        id: Uuid,
        notify: Arc<Notify>,
        events: broadcast::Sender<String>,
        viewers: Arc<AtomicUsize>,
    ) -> anyhow::Result<Self> {
        let row = sqlx::query_as::<_, MatchRow>(
            r#"SELECT m.game_id, m.variant, m.mode, m.room_code, m.time_limit_ms, m.timeout_policy,
                      m.max_consecutive_timeouts, m.grace_ms, m.allow_comments, m.rules_hash, m.status,
                      m.state, m.open_decisions, m.seed, m.seed_commit, m.result, m.end_reason,
                      m.game_version_id, gv.version AS game_version, g.name AS game_name
               FROM matches m
               JOIN game_versions gv ON gv.id = m.game_version_id
               JOIN games g ON g.id = m.game_id
               WHERE m.id = $1"#,
        )
        .bind(id)
        .fetch_optional(&pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("match not found"))?;

        let game = load_game(&pool, &row.game_id, row.game_version_id).await?;

        let prows = sqlx::query_as::<_, PlayerRow>(
            r#"SELECT mp.seat, mp.agent_id, mp.kid, mp.timeouts, mp.forfeited, mp.score,
                      a.name, a.allow_comments, a.model_label, a.last_seen_at, u.handle,
                      k.public_key_b64
               FROM match_players mp
               JOIN agents a ON a.id = mp.agent_id
               JOIN users u ON u.id = mp.owner_id
               LEFT JOIN agent_keys k ON k.kid = mp.kid
               WHERE mp.match_id = $1
               ORDER BY mp.seat"#,
        )
        .bind(id)
        .fetch_all(&pool)
        .await?;

        let players = prows
            .into_iter()
            .map(|p| Player {
                seat: p.seat as i32,
                agent_id: p.agent_id,
                name: p.name,
                handle: p.handle,
                model_label: p.model_label,
                allow_comments: p.allow_comments,
                kid: p.kid.unwrap_or_default(),
                public_b64: p.public_key_b64.unwrap_or_default(),
                timeouts: p.timeouts,
                forfeited: p.forfeited,
                last_seen: p.last_seen_at,
                score: p.score,
            })
            .collect();

        let lines: Vec<String> = sqlx::query_scalar(
            "SELECT entry_line FROM match_events WHERE match_id = $1 ORDER BY seq",
        )
        .bind(id)
        .fetch_all(&pool)
        .await?;
        let mut entries = Vec::with_capacity(lines.len());
        for line in lines {
            entries.push(serde_json::from_str(&line)?);
        }
        let chain = Chain::from_entries(server, entries)?;
        let persisted = chain.entries.len();
        let decisions = match &row.open_decisions {
            Some(Value::Array(_)) => serde_json::from_value(row.open_decisions.clone().unwrap())?,
            _ => Vec::new(),
        };
        let secret_rows = sqlx::query_as::<_, (i16, String, Value, String)>(
            "SELECT seat, commitment, data, salt FROM match_secrets WHERE match_id = $1",
        )
        .bind(id)
        .fetch_all(&pool)
        .await?;
        let secrets = secret_rows
            .into_iter()
            .map(|(seat, commitment, data, salt)| Secret { seat: seat as i32, commitment, data, salt })
            .collect();
        let (seed, seed_hex) = match &row.seed {
            Some(h) => (parse_seed(h), h.clone()),
            None => ([0u8; 32], String::new()),
        };

        Ok(Self {
            pool,
            match_id: id,
            game,
            game_id: row.game_id,
            game_name: row.game_name,
            game_version: row.game_version,
            rules_hash: row.rules_hash.unwrap_or_default(),
            variant: row.variant.unwrap_or_else(|| "free".into()),
            mode: row.mode,
            room_code: row.room_code,
            chain,
            state: row.state,
            players,
            decisions,
            secrets,
            status: row.status,
            result: row.result,
            end_reason: row.end_reason,
            seed,
            seed_hex,
            seed_commit: row.seed_commit.unwrap_or_default(),
            time_limit_ms: row.time_limit_ms as i64,
            grace_ms: row.grace_ms as i64,
            max_timeouts: row.max_consecutive_timeouts,
            policy: TimeoutPolicy::parse(&row.timeout_policy).unwrap_or(TimeoutPolicy::Forfeit),
            allow_comments: row.allow_comments,
            persisted,
            viewers,
            events,
            notify,
        })
    }

    async fn open_genesis(&mut self) -> anyhow::Result<()> {
        if self.players.iter().any(|p| p.public_b64.is_empty() || p.kid.is_empty()) {
            anyhow::bail!("seat is missing a registered key");
        }
        let mut seed = [0u8; 32];
        rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut seed);
        self.seed = seed;
        self.seed_hex = hex::encode(seed);
        self.seed_commit = crate::crypto::sha256_hex(&seed);
        let mut rng = crate::game::SeededRng::from_seed_seq(&seed, 0);
        let cfg = crate::game::Config {
            players: self.players.len() as u8,
            variant: self.variant.clone(),
            options: json!({}),
        };
        let (state, _) = self.game.setup(&cfg, &mut rng)?;
        self.state = Some(state);
        let seats: Vec<Value> = self
            .players
            .iter()
            .map(|p| {
                json!({
                    "seat": p.seat,
                    "agent_id": p.agent_id.to_string(),
                    "kid": p.kid,
                    "alg": "ed25519",
                    "public_key": p.public_b64,
                })
            })
            .collect();
        let payload = json!({
            "match_id": self.match_id.to_string(),
            "game_id": self.game_id,
            "game_version": self.game_version,
            "rules_hash": self.rules_hash,
            "variant": self.variant,
            "time_control": {
                "per_action_ms": self.time_limit_ms,
                "max_consecutive_timeouts": self.max_timeouts,
                "grace_ms": self.grace_ms,
            },
            "timeout_policy": self.policy.as_str(),
            "seats": seats,
            "seed_commit": self.seed_commit,
            "server_kid": self.chain.server.kid,
            "created_at": now_ms(),
        });
        self.chain.append_server("genesis", "genesis", payload);
        self.refresh_decisions();
        self.commit().await?;
        Ok(())
    }

    async fn run(mut self, mut rx: mpsc::Receiver<Cmd>) {
        loop {
            let wait = self.ms_until_timeout();
            let sleep = async {
                match wait {
                    Some(ms) => tokio::time::sleep(Duration::from_millis(ms)).await,
                    None => std::future::pending::<()>().await,
                }
            };
            tokio::select! {
                cmd = rx.recv() => {
                    match cmd {
                        Some(cmd) => self.on_cmd(cmd).await,
                        None => break,
                    }
                }
                _ = sleep => {
                    if let Err(err) = self.fire_due().await {
                        tracing::error!(error = %err, match_id = %self.match_id, "timeout handling failed");
                        break;
                    }
                }
            }
        }
    }

    async fn on_cmd(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::Public { reply } => {
                let _ = reply.send(self.public_json());
            }
            Cmd::Agent { agent_id, reply } => {
                self.mark_seen(agent_id);
                let _ = reply.send(self.agent_json(agent_id));
            }
            Cmd::Move { agent_id, envelope, private, reply } => {
                self.mark_seen(agent_id);
                let value = self.handle_move(agent_id, envelope, private).await;
                let _ = reply.send(value);
            }
            Cmd::Resign { agent_id, reply } => {
                self.mark_seen(agent_id);
                let value = self.handle_resign(agent_id).await;
                let _ = reply.send(value);
            }
        }
    }

    fn mark_seen(&mut self, agent_id: Uuid) {
        if let Some(p) = self.players.iter_mut().find(|p| p.agent_id == agent_id) {
            p.last_seen = Some(Utc::now());
        }
    }

    fn ms_until_timeout(&self) -> Option<u64> {
        if self.status != "running" {
            return None;
        }
        let now = now_ms();
        let due = self
            .decisions
            .iter()
            .map(|d| d.deadline_at + self.grace_ms)
            .min()?;
        Some((due - now).max(0) as u64)
    }

    fn is_terminal(&self) -> bool {
        self.state.as_ref().map(|s| self.game.is_terminal(s)).unwrap_or(false)
    }

    fn refresh_decisions(&mut self) {
        if self.status != "running" || self.is_terminal() {
            self.decisions.clear();
            return;
        }
        let Some(state) = self.state.as_ref() else {
            self.decisions.clear();
            return;
        };
        let seats = self.game.to_act(state);
        let seq = self.chain.tip_seq().unwrap_or(0);
        let prev = self.chain.tip_hash().to_string();
        let deadline = now_ms() + self.time_limit_ms;
        let previous = self.decisions.clone();
        self.decisions = seats
            .into_iter()
            .map(|seat| {
                if let Some(existing) = previous.iter().find(|d| d.seat == seat) {
                    if self.find_decision_entry(seat, existing.seq).is_none() {
                        return existing.clone();
                    }
                }
                Decision { seq, prev_hash: prev.clone(), seat, deadline_at: deadline }
            })
            .collect();
    }

    async fn fire_due(&mut self) -> anyhow::Result<()> {
        let now = now_ms();
        let due: Vec<Decision> = self
            .decisions
            .iter()
            .filter(|d| now > d.deadline_at + self.grace_ms)
            .cloned()
            .collect();
        for d in due {
            self.apply_timeout(&d).await?;
        }
        Ok(())
    }

    async fn apply_timeout(&mut self, d: &Decision) -> anyhow::Result<()> {
        if self.status != "running" {
            return Ok(());
        }
        if !self.decisions.iter().any(|x| x.seat == d.seat && x.seq == d.seq) {
            return Ok(());
        }
        let backup_chain = self.chain.clone();
        let backup_state = self.state.clone();
        let backup_decisions = self.decisions.clone();
        let backup_status = self.status.clone();
        let backup_result = self.result.clone();
        let backup_reason = self.end_reason.clone();
        let backup_players = self.player_backup();
        let backup_secrets = self.secrets.clone();

        if let Some(p) = self.players.iter_mut().find(|p| p.seat == d.seat) {
            p.timeouts += 1;
        }
        let timeouts = self.players.iter().find(|p| p.seat == d.seat).map(|p| p.timeouts).unwrap_or(1);
        let mut policy = self.policy;
        let allows_forfeit = self.game.manifest().allowed_timeout_policies.contains(&TimeoutPolicy::Forfeit);
        if allows_forfeit && policy != TimeoutPolicy::Forfeit && self.max_timeouts > 0 && timeouts >= self.max_timeouts {
            policy = TimeoutPolicy::Forfeit;
        }
        let mut rng = crate::game::SeededRng::from_seed_seq(&self.seed, d.seq);
        let decision = match self.game.timeout_decision(
            self.state.as_ref().unwrap(),
            d.seat,
            policy,
            &mut rng,
        ) {
            Ok(d) => d,
            Err(_) => crate::game::TimeoutDecision {
                policy: TimeoutPolicy::Forfeit,
                action: None,
            },
        };
        let fired = now_ms();
        let action_for_log: Value = if let Some(action) = decision.action.clone() {
            match self.game.apply(self.state.as_ref().unwrap(), d.seat, &action, &mut rng) {
                Ok((next, _)) => {
                    self.state = Some(next);
                    action
                }
                Err(err) => {
                    tracing::warn!(error = %err, "auto action illegal, forfeiting");
                    self.forfeit_seat(d.seat);
                    Value::Null
                }
            }
        } else if decision.policy == TimeoutPolicy::Forfeit {
            self.forfeit_seat(d.seat);
            Value::Null
        } else {
            Value::Null
        };
        let policy_str = if action_for_log.is_null() && decision.policy != TimeoutPolicy::AutoAction {
            "forfeit"
        } else if self.is_terminal() && action_for_log.is_null() {
            "forfeit"
        } else {
            decision.policy.as_str()
        };
        let payload = json!({
            "match_id": self.match_id.to_string(),
            "seat": d.seat,
            "decision_seq": d.seq,
            "policy": policy_str,
            "action": action_for_log,
            "deadline_at": d.deadline_at,
            "fired_at": fired,
        });
        self.chain.append_server("auto_action", "auto_action", payload);
        self.finish_if_terminal();
        if !self.is_terminal() {
            self.refresh_decisions();
        } else {
            self.decisions.clear();
        }
        if let Err(err) = self.commit().await {
            self.chain = backup_chain;
            self.state = backup_state;
            self.decisions = backup_decisions;
            self.status = backup_status;
            self.result = backup_result;
            self.end_reason = backup_reason;
            self.restore_players(backup_players);
            self.secrets = backup_secrets;
            return Err(err);
        }
        Ok(())
    }

    fn player_backup(&self) -> Vec<(i32, i32, bool, Option<f64>)> {
        self.players
            .iter()
            .map(|p| (p.seat, p.timeouts, p.forfeited, p.score))
            .collect()
    }

    fn restore_players(&mut self, backup: Vec<(i32, i32, bool, Option<f64>)>) {
        for (seat, timeouts, forfeited, score) in backup {
            if let Some(p) = self.players.iter_mut().find(|p| p.seat == seat) {
                p.timeouts = timeouts;
                p.forfeited = forfeited;
                p.score = score;
            }
        }
    }

    fn opponent(&self, seat: i32) -> i32 {
        self.players
            .iter()
            .find(|p| p.seat != seat)
            .map(|p| p.seat)
            .unwrap_or(0)
    }

    fn forfeit_seat(&mut self, seat: i32) {
        if self.game_id == "gomoku" {
            let winner = self.opponent(seat);
            self.state = Some(force_terminal(self.state.as_ref().unwrap(), winner, "timeout"));
        } else if let Some(state) = self.state.clone() {
            let mut rng = crate::game::SeededRng::from_seed_seq(&self.seed, 0);
            if let Ok((next, _)) = self.game.apply(&state, seat, &json!({"type":"resign"}), &mut rng) {
                self.state = Some(next);
            }
        }
        if let Some(p) = self.players.iter_mut().find(|p| p.seat == seat) {
            p.forfeited = true;
        }
    }

    fn finish_if_terminal(&mut self) {
        if !self.is_terminal() || self.status == "finished" {
            return;
        }
        let state = self.state.as_ref().unwrap();
        let result = match self.game.result(state) {
            Ok(r) => r,
            Err(err) => {
                tracing::error!(error = %err, "terminal state has no result");
                return;
            }
        };
        for p in self.players.iter_mut() {
            p.score = result.scores.get(p.seat as usize).copied();
        }
        if !self.secrets.is_empty() {
            let reveals: Vec<Value> = self.secrets.iter().map(|s| json!({
                "data": s.data,
                "salt": s.salt,
                "commitment": s.commitment,
                "seat": s.seat,
            })).collect();
            self.chain.append_server("reveal", "reveal", json!({
                "match_id": self.match_id.to_string(),
                "reveals": reveals,
            }));
            self.secrets.clear();
        }
        let final_hash = self.chain.tip_hash().to_string();
        let payload = json!({
            "match_id": self.match_id.to_string(),
            "result": result_value(&result),
            "final_hash": final_hash,
            "seed": self.seed_hex,
        });
        self.result = Some(result_value(&result));
        self.end_reason = Some(result.reason.clone());
        self.status = "finished".into();
        self.decisions.clear();
        self.chain.append_server("result", "result", payload);
    }

    async fn handle_resign(&mut self, agent_id: Uuid) -> Value {
        let Some(seat) = self.players.iter().find(|p| p.agent_id == agent_id).map(|p| p.seat) else {
            return json!({"accepted": false, "error": "not_seated", "message": "你不在这局里"});
        };
        if self.status != "running" || self.is_terminal() {
            return json!({"accepted": false, "error": "game_over", "message": "对局不在进行中"});
        }
        let backup_chain = self.chain.clone();
        let backup_state = self.state.clone();
        let backup_decisions = self.decisions.clone();
        let backup_status = self.status.clone();
        let backup_result = self.result.clone();
        let backup_reason = self.end_reason.clone();
        let backup_secrets = self.secrets.clone();
        let decision_seq = self.decisions.iter().find(|d| d.seat == seat).map(|d| d.seq);
        let mut rng = crate::game::SeededRng::from_seed_seq(&self.seed, decision_seq.unwrap_or(0));
        let action = json!({"type": "resign"});
        match self.game.apply(self.state.as_ref().unwrap(), seat, &action, &mut rng) {
            Ok((next, _)) => self.state = Some(next),
            Err(err) => {
                return json!({"accepted": false, "error": "illegal_action", "message": err.0});
            }
        }
        let payload = json!({
            "match_id": self.match_id.to_string(),
            "seat": seat,
            "agent_id": agent_id.to_string(),
            "decision_seq": decision_seq,
            "action": {"type": "resign"},
        });
        self.chain.append_server("resign", "resign", payload);
        self.finish_if_terminal();
        self.decisions.clear();
        if let Err(err) = self.commit().await {
            self.chain = backup_chain;
            self.state = backup_state;
            self.decisions = backup_decisions;
            self.status = backup_status;
            self.result = backup_result;
            self.end_reason = backup_reason;
            self.secrets = backup_secrets;
            return json!({"accepted": false, "error": "persist", "message": err.to_string()});
        }
        let entry = self.chain.entries.last().cloned().unwrap_or(Value::Null);
        // last may be result; find resign
        let resign = self
            .chain
            .entries
            .iter()
            .rev()
            .find(|e| e["kind"] == "resign")
            .cloned()
            .unwrap_or(entry);
        accepted_from(&resign, false)
    }

    async fn handle_move(&mut self, agent_id: Uuid, envelope: Value, private: Option<Value>) -> Value {
        let domain_v = envelope.get("domain").cloned().unwrap_or(Value::Null);
        let payload = envelope.get("payload").cloned().unwrap_or(Value::Null);
        let signature = envelope.get("signature").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let kid = envelope.get("kid").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let alg = envelope.get("alg").and_then(|v| v.as_str()).unwrap_or("");

        let Some(player) = self.players.iter().find(|p| p.agent_id == agent_id) else {
            return json!({"accepted": false, "error": "not_seated", "message": "你不在这局里"});
        };
        let player_seat = player.seat;
        let player_kid = player.kid.clone();
        let player_pub = player.public_b64.clone();
        let player_comments = player.allow_comments;
        let match_id_str = self.match_id.to_string();
        let agent_str = agent_id.to_string();

        if self.status != "running" {
            return self.nack("game_over", "对局不在进行中", Some(player_seat), &payload, json!({}));
        }
        if alg != "ed25519" {
            return self.nack("bad_signature", "仅支持 ed25519", Some(player_seat), &payload, json!({}));
        }
        if !domain_ok(&domain_v, "move") {
            return self.nack("domain_mismatch", "domain 必须是 AgentArena v1 / arena.idou.top / purpose=move", Some(player_seat), &payload, json!({}));
        }
        if json_i64(&payload["v"]) != Some(1) {
            return self.nack("context_mismatch", "载荷 v 必须为 1", Some(player_seat), &payload, json!({}));
        }
        if kid != player_kid || payload.get("kid").and_then(|v| v.as_str()) != Some(player_kid.as_str()) {
            return self.nack("key_mismatch", "签名密钥与本局绑定的密钥不一致", Some(player_seat), &payload, json!({}));
        }
        let key_status: Option<String> = sqlx::query_scalar("SELECT status FROM agent_keys WHERE kid = $1 AND agent_id = $2")
            .bind(&player_kid)
            .bind(agent_id)
            .fetch_optional(&self.pool)
            .await
            .ok()
            .flatten();
        match key_status.as_deref() {
            Some("revoked") => {
                return self.nack("key_revoked", "密钥已吊销，本局只能认输", Some(player_seat), &payload, json!({}));
            }
            Some("active") | Some("rotated") => {}
            _ => {
                return self.nack("unknown_key", "找不到这把密钥", Some(player_seat), &payload, json!({}));
            }
        }
        if !verify_value(&player_pub, &domain_v, &payload, &signature) {
            return self.nack("bad_signature", "签名验证失败", Some(player_seat), &payload, json!({}));
        }
        if payload.get("match_id").and_then(|v| v.as_str()) != Some(match_id_str.as_str())
            || payload.get("game_id").and_then(|v| v.as_str()) != Some(self.game_id.as_str())
            || payload.get("game_version").and_then(|v| v.as_str()) != Some(self.game_version.as_str())
            || payload.get("rules_hash").and_then(|v| v.as_str()) != Some(self.rules_hash.as_str())
            || json_i64(&payload["seat"]) != Some(player_seat as i64)
            || payload.get("agent_id").and_then(|v| v.as_str()) != Some(agent_str.as_str())
        {
            return self.nack("context_mismatch", "match_id / 规则 / 席位 / 智能体与本局不一致", Some(player_seat), &payload, json!({}));
        }
        let Some(signed_seq) = json_i64(&payload["seq"]) else {
            return self.nack("seq_mismatch", "缺少 seq", Some(player_seat), &payload, json!({}));
        };
        let signed_prev = payload.get("prev_hash").and_then(|v| v.as_str()).unwrap_or("");
        let decision = self.decisions.iter().find(|d| d.seat == player_seat).cloned();
        let Some(decision) = decision else {
            if let Some(existing) = self.find_decision_entry(player_seat, signed_seq) {
                if payload_hash_hex(&existing["payload"]) == payload_hash_hex(&payload) {
                    return accepted_from(existing, true);
                }
                return self.nack("replay_rejected", "这个决策点已经有一份不同的签名", Some(player_seat), &payload, json!({}));
            }
            return self.nack("seq_mismatch", "现在没有轮到你行动", Some(player_seat), &payload, json!({}));
        };
        if signed_seq != decision.seq {
            return self.nack("seq_mismatch", "seq 不是当前决策点", Some(player_seat), &payload, json!({}));
        }
        if signed_prev != decision.prev_hash {
            return self.nack("stale_prev_hash", "prev_hash 与当前决策点不一致", Some(player_seat), &payload, json!({}));
        }
        if let Some(existing) = self.find_decision_entry(player_seat, signed_seq) {
            if payload_hash_hex(&existing["payload"]) == payload_hash_hex(&payload) {
                return accepted_from(existing, true);
            }
            return self.nack("replay_rejected", "这个决策点已经有一份不同的签名", Some(player_seat), &payload, json!({}));
        }
        let signed_deadline = json_i64(&payload["deadline_at"]);
        if signed_deadline != Some(decision.deadline_at) {
            return self.nack("context_mismatch", "deadline_at 与服务器记录不一致，请重新 get_state", Some(player_seat), &payload, json!({}));
        }
        if now_ms() > decision.deadline_at + self.grace_ms {
            let start = self.chain.entries.len();
            if let Err(err) = self.apply_timeout(&decision).await {
                return self.nack("persist", err.to_string(), Some(player_seat), &payload, json!({}));
            }
            let auto = self.chain.entries.get(start).cloned().unwrap_or(Value::Null);
            return self.nack(
                "deadline_passed",
                "已超过截止时间，服务器执行了超时动作",
                Some(player_seat),
                &payload,
                json!({"auto_entry": auto}),
            );
        }
        let Some(issued) = json_i64(&payload["issued_at"]) else {
            return self.nack("context_mismatch", "缺少 issued_at", Some(player_seat), &payload, json!({}));
        };
        if (issued - now_ms()).abs() > 5 * 60 * 1000 {
            return self.nack("issued_at_out_of_range", "issued_at 超出 ±5 分钟", Some(player_seat), &payload, json!({}));
        }
        if let Some(comment) = payload.get("comment").and_then(|v| v.as_str()) {
            if comment.chars().count() > 140 {
                return self.nack("comment_too_long", "评论不能超过 140 字", Some(player_seat), &payload, json!({}));
            }
            if !self.allow_comments || !player_comments {
                return self.nack("comment_disabled", "本局或该智能体未开启公开评论", Some(player_seat), &payload, json!({}));
            }
        }
        let action = payload.get("action").cloned().unwrap_or(Value::Null);
        let legal = self
            .game
            .legal_actions(self.state.as_ref().unwrap(), player_seat)
            .unwrap_or_default();
        if !legal.iter().any(|a| a.get("type") == action.get("type")) {
            return self.nack(
                "illegal_action",
                "动作不合法",
                Some(player_seat),
                &payload,
                json!({"legal_actions": legal}),
            );
        }
        let commitment = payload.get("commitment").and_then(|v| v.as_str()).map(|s| s.to_string());
        let mut apply_action = action.clone();
        let mut staged_secret: Option<Secret> = None;
        if let Some(commitment) = commitment {
            let Some(private) = private.as_ref() else {
                return self.nack("missing_private", "带 commitment 的动作必须附上 private", Some(player_seat), &payload, json!({}));
            };
            let data = private.get("data").cloned().unwrap_or(Value::Null);
            let salt = private.get("salt").and_then(|v| v.as_str()).unwrap_or("");
            if commitment_of(&data, salt).as_deref() != Some(commitment.as_str()) {
                return self.nack("bad_commitment", "承诺和私密数据对不上", Some(player_seat), &payload, json!({}));
            }
            if let Some(obj) = apply_action.as_object_mut() {
                obj.insert("_private".into(), json!({"data": data.clone(), "salt": salt}));
            }
            staged_secret = Some(Secret { seat: player_seat, commitment, data, salt: salt.to_string() });
        } else if private.is_some() {
            return self.nack("unexpected_private", "没有 commitment 时不要附带 private", Some(player_seat), &payload, json!({}));
        }

        let backup_chain = self.chain.clone();
        let backup_state = self.state.clone();
        let backup_decisions = self.decisions.clone();
        let backup_status = self.status.clone();
        let backup_result = self.result.clone();
        let backup_reason = self.end_reason.clone();
        let backup_players = self.player_backup();
        let backup_secrets = self.secrets.clone();
        let mut rng = crate::game::SeededRng::from_seed_seq(&self.seed, signed_seq);
        match self.game.apply(self.state.as_ref().unwrap(), player_seat, &apply_action, &mut rng) {
            Ok((next, _)) => self.state = Some(next),
            Err(err) => {
                return self.nack("illegal_action", err.0, Some(player_seat), &payload, json!({"legal_actions": legal}));
            }
        }
        if let Some(secret) = staged_secret {
            self.secrets.push(secret);
        }
        if let Some(p) = self.players.iter_mut().find(|p| p.seat == player_seat) {
            p.timeouts = 0;
        }
        self.chain.append_agent(domain_v, payload.clone(), &kid, &signature);
        self.finish_if_terminal();
        if self.status == "running" {
            self.refresh_decisions();
        }
        if let Err(err) = self.commit().await {
            self.chain = backup_chain;
            self.state = backup_state;
            self.decisions = backup_decisions;
            self.status = backup_status;
            self.result = backup_result;
            self.end_reason = backup_reason;
            self.restore_players(backup_players);
            self.secrets = backup_secrets;
            return self.nack("persist", err.to_string(), Some(player_seat), &payload, json!({}));
        }
        let _ = sqlx::query("UPDATE agent_keys SET last_signed_at = now() WHERE kid = $1")
            .bind(&kid)
            .execute(&self.pool)
            .await;
        let entry = self
            .chain
            .entries
            .iter()
            .rev()
            .find(|e| e["kind"] == "action")
            .cloned()
            .unwrap_or(Value::Null);
        accepted_from(&entry, false)
    }

    fn find_decision_entry(&self, seat: i32, decision_seq: i64) -> Option<&Value> {
        self.chain.entries.iter().find(|e| {
            let kind = e["kind"].as_str().unwrap_or("");
            if !matches!(kind, "action" | "auto_action" | "resign") {
                return false;
            }
            let p = &e["payload"];
            let s = json_i64(&p["seat"]);
            let d = if kind == "action" {
                json_i64(&p["seq"])
            } else {
                json_i64(&p["decision_seq"])
            };
            s == Some(seat as i64) && d == Some(decision_seq)
        })
    }

    fn nack(&self, code: &'static str, message: impl Into<String>, seat: Option<i32>, payload: &Value, extra: Value) -> Value {
        let message = message.into();
        let received_at = now_ms();
        let ph = payload_hash_hex(payload);
        let nack_payload = json!({
            "code": code,
            "match_id": self.match_id.to_string(),
            "payload_hash": ph,
            "received_at": received_at,
            "seat": seat,
        });
        let d = domain("nack");
        let sig = self.chain.server.sign(&d, &nack_payload);
        tracing::info!(code, seat, match_id = %self.match_id, "move rejected");
        let pool = self.pool.clone();
        let match_id = self.match_id;
        let sig_bytes = decode_sig(&sig);
        let hash_bytes = decode_hex(&ph);
        tokio::spawn(async move {
            let _ = sqlx::query(
                "INSERT INTO move_rejections (match_id, seat, payload_hash, code, nack_sig, received_at) VALUES ($1, $2, $3, $4, $5, $6)",
            )
            .bind(match_id)
            .bind(seat.map(|s| s as i16))
            .bind(hash_bytes)
            .bind(code)
            .bind(sig_bytes)
            .bind(ms_to_dt(received_at))
            .execute(&pool)
            .await;
        });
        let mut body = json!({
            "accepted": false,
            "error": code,
            "message": message,
            "nack": {
                "server_kid": self.chain.server.kid,
                "signature": sig,
                "received_at": received_at,
                "payload": nack_payload,
            }
        });
        if let (Some(obj), Some(extra_obj)) = (body.as_object_mut(), extra.as_object()) {
            for (k, v) in extra_obj {
                obj.insert(k.clone(), v.clone());
            }
        }
        body
    }

    async fn commit(&mut self) -> anyhow::Result<()> {
        let mut tx = self.pool.begin().await?;
        for entry in &self.chain.entries[self.persisted..] {
            insert_event(&mut tx, self.match_id, entry).await?;
        }
        let decisions = serde_json::to_value(&self.decisions)?;
        sqlx::query(
            r#"UPDATE matches SET
                 state = $2,
                 open_decisions = $3,
                 tip_seq = $4,
                 tip_hash = $5,
                 status = $6,
                 result = $7,
                 ended_at = CASE WHEN $6 = 'finished' THEN COALESCE(ended_at, now()) ELSE ended_at END,
                 end_reason = $8,
                 seed = $9,
                 seed_commit = $10,
                 started_at = COALESCE(started_at, now())
               WHERE id = $1"#,
        )
        .bind(self.match_id)
        .bind(&self.state)
        .bind(decisions)
        .bind(self.chain.tip_seq().map(|s| s as i32))
        .bind(self.chain.tip_hash())
        .bind(&self.status)
        .bind(&self.result)
        .bind(&self.end_reason)
        .bind(&self.seed_hex)
        .bind(&self.seed_commit)
        .execute(&mut *tx)
        .await?;
        for p in &self.players {
            sqlx::query(
                "UPDATE match_players SET timeouts = $3, forfeited = $4, score = $5 WHERE match_id = $1 AND seat = $2",
            )
            .bind(self.match_id)
            .bind(p.seat as i16)
            .bind(p.timeouts)
            .bind(p.forfeited)
            .bind(p.score)
            .execute(&mut *tx)
            .await?;
        }
        sqlx::query("DELETE FROM match_secrets WHERE match_id = $1")
            .bind(self.match_id)
            .execute(&mut *tx)
            .await?;
        for secret in &self.secrets {
            sqlx::query(
                "INSERT INTO match_secrets (match_id, seat, commitment, data, salt) VALUES ($1,$2,$3,$4,$5)",
            )
            .bind(self.match_id)
            .bind(secret.seat as i16)
            .bind(&secret.commitment)
            .bind(&secret.data)
            .bind(&secret.salt)
            .execute(&mut *tx)
            .await?;
        }
        let settling = self.chain.entries[self.persisted..].iter().any(|e| e["kind"] == "result");
        if settling && self.mode == "ranked" {
            let participants: Vec<(Uuid, f64)> = self.players.iter().map(|p| (p.agent_id, p.score.unwrap_or(0.0))).collect();
            crate::rating::apply(&mut tx, self.match_id, &self.game_id, &participants).await?;
        }
        tx.commit().await?;
        self.persisted = self.chain.entries.len();
        self.broadcast();
        Ok(())
    }

    fn broadcast(&self) {
        let msg = json!({"type": "snapshot", "data": self.public_json()}).to_string();
        let _ = self.events.send(msg);
        self.notify.notify_waiters();
    }

    fn public_json(&self) -> Value {
        let now = now_ms();
        let to_act: Vec<i32> = if self.status == "running" {
            self.state.as_ref().map(|s| self.game.to_act(s)).unwrap_or_default()
        } else {
            Vec::new()
        };
        let deadline_at = self.decisions.iter().map(|d| d.deadline_at).min();
        let view = self
            .state
            .as_ref()
            .map(|s| {
                self.game
                    .view(s, &Viewer { seat: None, omniscient: self.status == "finished" })
                    .unwrap_or(Value::Null)
            })
            .unwrap_or(Value::Null);
        let seats: Vec<Value> = self
            .players
            .iter()
            .map(|p| {
                let connected = p
                    .last_seen
                    .map(|t| Utc::now().signed_duration_since(t).num_seconds() < 10)
                    .unwrap_or(false);
                let role = seat_role(&self.game_id, p.seat);
                json!({
                    "seat": p.seat,
                    "agent_id": p.agent_id,
                    "agent_name": p.name,
                    "owner_handle": p.handle,
                    "model_label": p.model_label,
                    "kid": p.kid,
                    "public_key": p.public_b64,
                    "connected": connected,
                    "timeouts": p.timeouts,
                    "forfeited": p.forfeited,
                    "role": role,
                    "allow_comments": p.allow_comments,
                })
            })
            .collect();
        let seed = if self.status == "finished" {
            Value::String(self.seed_hex.clone())
        } else {
            Value::Null
        };
        json!({
            "id": self.match_id,
            "game_id": self.game_id,
            "game_name": self.game_name,
            "game_version": self.game_version,
            "rules_hash": self.rules_hash,
            "variant": self.variant,
            "mode": self.mode,
            "status": self.status,
            "room_code": self.room_code,
            "time_limit_ms": self.time_limit_ms,
            "grace_ms": self.grace_ms,
            "timeout_policy": self.policy.as_str(),
            "max_consecutive_timeouts": self.max_timeouts,
            "allow_comments": self.allow_comments,
            "seed_commit": self.seed_commit,
            "seed": seed,
            "result": self.result,
            "end_reason": self.end_reason,
            "tip_seq": self.chain.tip_seq().unwrap_or(-1),
            "viewer_count": self.viewers.load(Ordering::Relaxed),
            "server_time": now,
            "to_act": to_act,
            "deadline_at": deadline_at,
            "view": view,
            "seats": seats,
            "entries": self.chain.entries,
            "server_keys": [{
                "kid": self.chain.server.kid,
                "alg": "ed25519",
                "public_key": self.chain.server.public_b64,
            }],
        })
    }

    fn agent_json(&self, agent_id: Uuid) -> Value {
        let Some(player) = self.players.iter().find(|p| p.agent_id == agent_id) else {
            return json!({"error": "not_seated", "message": "你不在这局里"});
        };
        let mut body = self.public_json();
        if let Some(state) = &self.state {
            body["view"] = self.game.view(state, &Viewer {
                seat: Some(player.seat),
                omniscient: self.status == "finished",
            }).unwrap_or(Value::Null);
        }
        let your_turn = self.decisions.iter().any(|d| d.seat == player.seat) && self.status == "running";
        body["seat"] = json!(player.seat);
        body["consecutive_timeouts"] = json!(player.timeouts);
        body["phase"] = json!(phase_of(&self.status));
        if your_turn {
            let legal = self
                .state
                .as_ref()
                .and_then(|s| self.game.legal_actions(s, player.seat).ok())
                .unwrap_or_default();
            body["legal_actions"] = json!(legal);
            if let Some(d) = self.decisions.iter().find(|d| d.seat == player.seat) {
                let remaining = (d.deadline_at - now_ms()).max(0);
                body["remaining_ms"] = json!(remaining);
                body["signing_request"] = json!({
                    "domain": domain("move"),
                    "payload": {
                        "v": 1,
                        "match_id": self.match_id.to_string(),
                        "game_id": self.game_id,
                        "game_version": self.game_version,
                        "rules_hash": self.rules_hash,
                        "seat": player.seat,
                        "agent_id": player.agent_id.to_string(),
                        "kid": player.kid,
                        "seq": d.seq,
                        "prev_hash": d.prev_hash,
                        "deadline_at": d.deadline_at,
                    }
                });
            }
        } else {
            body["legal_actions"] = json!([]);
            body["signing_request"] = Value::Null;
            body["remaining_ms"] = json!(0);
        }
        body["time_limit_ms"] = json!(self.time_limit_ms);
        body["timeout_policy"] = json!(self.policy.as_str());
        body
    }
}

fn seat_role(game: &str, seat: i32) -> &'static str {
    match game {
        "gomoku" | "go" => if seat == 0 { "黑" } else { "白" },
        "xiangqi" => if seat == 0 { "红" } else { "黑" },
        "junqi" => if seat == 0 { "南" } else { "北" },
        "doudizhu" => match seat { 0 => "甲", 1 => "乙", _ => "丙" },
        "mahjong-sichuan" => match seat { 0 => "东", 1 => "南", 2 => "西", _ => "北" },
        _ => match seat {
            0 => "甲",
            1 => "乙",
            2 => "丙",
            3 => "丁",
            _ => "席",
        },
    }
}

async fn load_game(pool: &PgPool, game_id: &str, version_id: Uuid) -> anyhow::Result<Arc<dyn Game>> {
    if let Some(game) = crate::game::registry().get(game_id) {
        return Ok(game);
    }
    let row = sqlx::query_as::<_, (Option<String>, Value)>("SELECT source, manifest FROM game_versions WHERE id = $1")
        .bind(version_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("missing rules {game_id}"))?;
    let source = row.0.ok_or_else(|| anyhow::anyhow!("uploaded rules {game_id} have no source"))?;
    let manifest: Manifest = serde_json::from_value(row.1)?;
    Ok(Arc::new(crate::sandbox::JsGame::from_stored(source, manifest)))
}

fn phase_of(status: &str) -> &'static str {
    match status {
        "waiting" => "waiting",
        "running" => "play",
        _ => "game_over",
    }
}

fn accepted_from(entry: &Value, idempotent: bool) -> Value {
    json!({
        "accepted": true,
        "idempotent": idempotent,
        "entry": {
            "seq": entry["seq"],
            "hash": entry["hash"],
            "prev_hash": entry["prev_hash"],
        },
        "ack": {
            "server_kid": entry["server_ack"]["kid"],
            "signature": entry["server_ack"]["signature"],
            "received_at": entry["received_at"],
        }
    })
}

fn parse_seed(hex_str: &str) -> [u8; 32] {
    let v = hex::decode(hex_str).unwrap_or_default();
    let mut a = [0u8; 32];
    let n = v.len().min(32);
    a[..n].copy_from_slice(&v[..n]);
    a
}

fn ms_to_dt(ms: i64) -> DateTime<Utc> {
    DateTime::from_timestamp_millis(ms).unwrap_or_else(Utc::now)
}

async fn insert_event(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    match_id: Uuid,
    entry: &Value,
) -> anyhow::Result<()> {
    let seq = json_i64(&entry["seq"]).unwrap_or(0) as i32;
    let kind = entry["kind"].as_str().unwrap_or("").to_string();
    let (seat, decision_seq) = event_seat_decision(entry);
    let action = entry["payload"].get("action").cloned().filter(|v| !v.is_null());
    let signer = entry["signer"].as_str().unwrap_or("server").to_string();
    let kid = entry["kid"].as_str().unwrap_or("").to_string();
    let alg = entry["alg"].as_str().unwrap_or("ed25519").to_string();
    let payload = entry["payload"].clone();
    let payload_hash = decode_hex(&payload_hash_hex(&payload));
    let signature = decode_sig(entry["signature"].as_str().unwrap_or(""));
    let prev_hash = decode_hex(entry["prev_hash"].as_str().unwrap_or(""));
    let hash = decode_hex(entry["hash"].as_str().unwrap_or(""));
    let server_ack_sig = decode_sig(entry["server_ack"]["signature"].as_str().unwrap_or(""));
    let received_at = ms_to_dt(json_i64(&entry["received_at"]).unwrap_or_else(now_ms));
    let entry_line = serde_json::to_string(entry)?;
    sqlx::query(
        r#"INSERT INTO match_events (
             match_id, seq, seat, kind, action, signer, kid, alg, decision_seq,
             payload, payload_hash, signature, prev_hash, hash, server_ack_sig, received_at, entry_line
           ) VALUES (
             $1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17
           )"#,
    )
    .bind(match_id)
    .bind(seq)
    .bind(seat)
    .bind(kind)
    .bind(action)
    .bind(signer)
    .bind(kid)
    .bind(alg)
    .bind(decision_seq)
    .bind(payload)
    .bind(payload_hash)
    .bind(signature)
    .bind(prev_hash)
    .bind(hash)
    .bind(server_ack_sig)
    .bind(received_at)
    .bind(entry_line)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn event_seat_decision(entry: &Value) -> (Option<i16>, Option<i32>) {
    let kind = entry["kind"].as_str().unwrap_or("");
    if !matches!(kind, "action" | "auto_action" | "resign") {
        return (None, None);
    }
    let p = &entry["payload"];
    let seat = json_i64(&p["seat"]).map(|s| s as i16);
    let decision = if kind == "action" {
        json_i64(&p["seq"])
    } else {
        json_i64(&p["decision_seq"])
    }
    .map(|n| n as i32);
    (seat, decision)
}

pub async fn roundtrip(
    sender: &mpsc::Sender<Cmd>,
    build: impl FnOnce(oneshot::Sender<Value>) -> Cmd,
) -> Result<Value, crate::error::CallError> {
    let (reply, rx) = oneshot::channel();
    sender
        .send(build(reply))
        .await
        .map_err(|_| crate::error::CallError::new(500, "actor_down", "对局进程已停止"))?;
    rx.await
        .map_err(|_| crate::error::CallError::new(500, "actor_down", "对局进程已停止"))
}
