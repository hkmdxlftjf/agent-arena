//! Avatars, titles, and equippable skins.
//!
//! Awarded from finished matches and stored beside the hash chain.
//! `commit` never puts these fields into a signed entry.

use crate::error::CallError;
use serde_json::{json, Value};
use sqlx::{PgPool, Postgres, Transaction};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub const DEFAULT_COLOR: &str = "#8c3a2f";
const MAX_AVATAR_BYTES: usize = 1_500_000;
const SEASON: &str = "2026";

const BUILTIN: &[&str] = &[
    "gomoku",
    "xiangqi",
    "go",
    "junqi",
    "doudizhu",
    "mahjong-sichuan",
];

pub const PRESETS: &[(&str, &str)] = &[
    ("crane", "鹤"),
    ("fox", "狐"),
    ("panda", "猫熊"),
    ("koi", "锦鲤"),
    ("rabbit", "兔"),
    ("tiger", "虎"),
    ("owl", "鸮"),
    ("turtle", "龟"),
    ("cat", "猫"),
    ("deer", "鹿"),
    ("robot-round", "圆机"),
    ("robot-square", "方机"),
    ("ink-general", "墨将"),
    ("ink-horse", "墨马"),
    ("ink-cannon", "墨炮"),
    ("ink-elephant", "墨象"),
    ("ink-soldier", "墨兵"),
    ("ink-chariot", "墨车"),
];

struct Item {
    id: &'static str,
    kind: &'static str,
    game_id: Option<&'static str>,
    name: &'static str,
    rarity: &'static str,
    asset: &'static str,
    rule: &'static str,
    sort: i32,
}

const ITEMS: &[Item] = &[
    Item {
        id: "frame-none",
        kind: "frame",
        game_id: None,
        name: "无框",
        rarity: "common",
        asset: "none",
        rule: "default",
        sort: 0,
    },
    Item {
        id: "frame-cinnabar",
        kind: "frame",
        game_id: None,
        name: "朱砂",
        rarity: "rare",
        asset: "cinnabar",
        rule: "first_win",
        sort: 10,
    },
    Item {
        id: "frame-jade",
        kind: "frame",
        game_id: None,
        name: "翠玉",
        rarity: "rare",
        asset: "jade",
        rule: "wins:10",
        sort: 20,
    },
    Item {
        id: "frame-ink",
        kind: "frame",
        game_id: None,
        name: "松烟",
        rarity: "rare",
        asset: "ink",
        rule: "streak:3",
        sort: 30,
    },
    Item {
        id: "frame-gold",
        kind: "frame",
        game_id: None,
        name: "赤金",
        rarity: "epic",
        asset: "gold",
        rule: "rating:1800",
        sort: 40,
    },
    Item {
        id: "frame-season",
        kind: "frame",
        game_id: None,
        name: "赛季",
        rarity: "epic",
        asset: "season",
        rule: "season_top10",
        sort: 50,
    },
    Item {
        id: "gomoku-classic",
        kind: "piece",
        game_id: Some("gomoku"),
        name: "素石",
        rarity: "common",
        asset: "classic",
        rule: "default",
        sort: 0,
    },
    Item {
        id: "gomoku-jade",
        kind: "piece",
        game_id: Some("gomoku"),
        name: "翠玉子",
        rarity: "rare",
        asset: "jade",
        rule: "first_win",
        sort: 10,
    },
    Item {
        id: "gomoku-ink",
        kind: "piece",
        game_id: Some("gomoku"),
        name: "墨子",
        rarity: "rare",
        asset: "ink",
        rule: "wins:10",
        sort: 20,
    },
    Item {
        id: "gomoku-wood",
        kind: "piece",
        game_id: Some("gomoku"),
        name: "木子",
        rarity: "rare",
        asset: "wood",
        rule: "rating:1600",
        sort: 30,
    },
    Item {
        id: "xiangqi-wood",
        kind: "piece",
        game_id: Some("xiangqi"),
        name: "木棋",
        rarity: "common",
        asset: "wood",
        rule: "default",
        sort: 0,
    },
    Item {
        id: "xiangqi-porcelain",
        kind: "piece",
        game_id: Some("xiangqi"),
        name: "瓷棋",
        rarity: "rare",
        asset: "porcelain",
        rule: "rating:1600",
        sort: 10,
    },
    Item {
        id: "cards-classic",
        kind: "card_back",
        game_id: None,
        name: "素背",
        rarity: "common",
        asset: "classic",
        rule: "default",
        sort: 0,
    },
    Item {
        id: "cards-brocade",
        kind: "card_back",
        game_id: None,
        name: "织锦",
        rarity: "rare",
        asset: "brocade",
        rule: "wins:10",
        sort: 10,
    },
    Item {
        id: "cards-ink",
        kind: "card_back",
        game_id: None,
        name: "水墨背",
        rarity: "rare",
        asset: "ink",
        rule: "streak:3",
        sort: 20,
    },
    Item {
        id: "board-paper",
        kind: "board",
        game_id: None,
        name: "宣纸",
        rarity: "common",
        asset: "paper",
        rule: "default",
        sort: 0,
    },
    Item {
        id: "board-celadon",
        kind: "board",
        game_id: None,
        name: "青瓷",
        rarity: "rare",
        asset: "celadon",
        rule: "first_win",
        sort: 10,
    },
    Item {
        id: "board-night",
        kind: "board",
        game_id: None,
        name: "夜墨",
        rarity: "epic",
        asset: "night",
        rule: "rating:1800",
        sort: 20,
    },
    Item {
        id: "board-wood",
        kind: "board",
        game_id: None,
        name: "檀木",
        rarity: "rare",
        asset: "wood",
        rule: "all_games",
        sort: 30,
    },
    Item {
        id: "emote-none",
        kind: "emote",
        game_id: None,
        name: "无表情",
        rarity: "common",
        asset: "none",
        rule: "default",
        sort: 0,
    },
    Item {
        id: "emote-bow",
        kind: "emote",
        game_id: None,
        name: "拱手",
        rarity: "rare",
        asset: "bow",
        rule: "first_win",
        sort: 10,
    },
    Item {
        id: "emote-spark",
        kind: "emote",
        game_id: None,
        name: "星火",
        rarity: "rare",
        asset: "spark",
        rule: "wins:10",
        sort: 20,
    },
    Item {
        id: "emote-ink",
        kind: "emote",
        game_id: None,
        name: "墨溅",
        rarity: "epic",
        asset: "ink",
        rule: "rating:2000",
        sort: 30,
    },
];

#[derive(Clone, Debug)]
pub struct Appearance {
    pub title: Option<String>,
    pub color: String,
    pub avatar_preset: String,
    pub avatar_image: bool,
    pub frame_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct SeatLook {
    pub title: Option<String>,
    pub color: String,
    pub avatar_preset: String,
    pub avatar_image: bool,
    pub frame: Option<String>,
    pub piece: String,
    pub board: String,
    pub card_back: String,
    pub emote: String,
}

impl SeatLook {
    pub fn to_json(&self, agent_id: Uuid) -> Value {
        json!({
            "title": self.title,
            "color": self.color,
            "avatar_preset": self.avatar_preset,
            "avatar_url": avatar_url(agent_id, self.avatar_image),
            "frame": self.frame,
            "piece": self.piece,
            "board": self.board,
            "card_back": self.card_back,
            "emote": self.emote,
        })
    }
}

pub fn appearance_json(agent_id: Uuid, look: &Appearance) -> Value {
    json!({
        "title": look.title,
        "color": look.color,
        "avatar_preset": look.avatar_preset,
        "avatar_url": avatar_url(agent_id, look.avatar_image),
        "frame": frame_asset(look.frame_id.as_deref()),
    })
}

fn avatar_url(agent_id: Uuid, image: bool) -> Option<String> {
    if image {
        Some(format!("/avatars/{agent_id}.webp"))
    } else {
        None
    }
}

fn frame_asset(id: Option<&str>) -> Option<&'static str> {
    let id = id?;
    let asset = find_item(id)
        .filter(|item| item.kind == "frame")
        .map(|item| item.asset)?;
    if asset == "none" {
        None
    } else {
        Some(asset)
    }
}

fn find_item(id: &str) -> Option<&'static Item> {
    ITEMS.iter().find(|item| item.id == id)
}

fn default_piece(game_id: &str) -> &'static str {
    if game_id == "xiangqi" {
        "wood"
    } else {
        "classic"
    }
}

pub fn catalog_json() -> Value {
    Value::Array(
        ITEMS
            .iter()
            .map(|item| {
                json!({
                    "id": item.id,
                    "kind": item.kind,
                    "game_id": item.game_id,
                    "name": item.name,
                    "rarity": item.rarity,
                    "asset": item.asset,
                    "unlock_rule": item.rule,
                })
            })
            .collect(),
    )
}

pub fn presets_json() -> Value {
    Value::Array(
        PRESETS
            .iter()
            .map(|(id, name)| json!({"id": id, "name": name}))
            .collect(),
    )
}

pub async fn ensure_catalog(pool: &PgPool) -> anyhow::Result<()> {
    for item in ITEMS {
        sqlx::query(
            r#"INSERT INTO cosmetic_items (id, kind, game_id, name, rarity, asset, unlock_rule, sort)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
               ON CONFLICT (id) DO UPDATE SET
                 kind = EXCLUDED.kind,
                 game_id = EXCLUDED.game_id,
                 name = EXCLUDED.name,
                 rarity = EXCLUDED.rarity,
                 asset = EXCLUDED.asset,
                 unlock_rule = EXCLUDED.unlock_rule,
                 sort = EXCLUDED.sort"#,
        )
        .bind(item.id)
        .bind(item.kind)
        .bind(item.game_id)
        .bind(item.name)
        .bind(item.rarity)
        .bind(item.asset)
        .bind(item.rule)
        .bind(item.sort)
        .execute(pool)
        .await?;
    }
    Ok(())
}

pub async fn award(
    tx: &mut Transaction<'_, Postgres>,
    match_id: Uuid,
    game_id: &str,
    players: &[(Uuid, f64)],
) -> anyhow::Result<()> {
    let inserted =
        sqlx::query("INSERT INTO cosmetic_awards (match_id) VALUES ($1) ON CONFLICT DO NOTHING")
            .bind(match_id)
            .execute(&mut **tx)
            .await?;
    if inserted.rows_affected() == 0 {
        return Ok(());
    }
    for (agent_id, score) in players {
        let win = unique_best(players, *score);
        let won: i32 = if win { 1 } else { 0 };
        let stats = sqlx::query_as::<_, (i32, i32, i32, i32, Vec<String>)>(
            r#"INSERT INTO cosmetic_stats AS s (agent_id, wins, games, streak, best_streak, games_played)
               VALUES ($1, $2, 1, $2, $2, ARRAY[$3]::text[])
               ON CONFLICT (agent_id) DO UPDATE SET
                 wins = s.wins + EXCLUDED.wins,
                 games = s.games + 1,
                 streak = CASE WHEN EXCLUDED.wins = 1 THEN s.streak + 1 ELSE 0 END,
                 best_streak = GREATEST(
                   s.best_streak,
                   CASE WHEN EXCLUDED.wins = 1 THEN s.streak + 1 ELSE 0 END
                 ),
                 games_played = (
                   SELECT ARRAY(SELECT DISTINCT x FROM unnest(s.games_played || EXCLUDED.games_played) AS x)
                 )
               RETURNING wins, games, streak, best_streak, games_played"#,
        )
        .bind(agent_id)
        .bind(won)
        .bind(game_id)
        .fetch_one(&mut **tx)
        .await?;
        grant_unlocks(&mut *tx, *agent_id, stats.0, stats.3, &stats.4).await?;
    }
    Ok(())
}

fn unique_best(players: &[(Uuid, f64)], mine: f64) -> bool {
    let best = players
        .iter()
        .map(|(_, score)| *score)
        .fold(f64::NEG_INFINITY, f64::max);
    if (mine - best).abs() > 1e-6 {
        return false;
    }
    players
        .iter()
        .filter(|(_, score)| (*score - best).abs() <= 1e-6)
        .count()
        == 1
}

async fn grant_unlocks(
    tx: &mut Transaction<'_, Postgres>,
    agent_id: Uuid,
    wins: i32,
    best_streak: i32,
    played: &[String],
) -> anyhow::Result<()> {
    let rating: f64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(rating), 0) FROM ratings WHERE agent_id = $1 AND season = $2",
    )
    .bind(agent_id)
    .bind(SEASON)
    .fetch_one(&mut **tx)
    .await?;
    let top10 = in_season_top10(&mut *tx, agent_id).await?;
    for item in ITEMS {
        if item.rule == "default" {
            continue;
        }
        if !rule_met(item.rule, wins, best_streak, played, rating, top10) {
            continue;
        }
        sqlx::query(
            "INSERT INTO agent_unlocks (agent_id, item_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
        )
        .bind(agent_id)
        .bind(item.id)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

fn rule_met(
    rule: &str,
    wins: i32,
    best_streak: i32,
    played: &[String],
    rating: f64,
    top10: bool,
) -> bool {
    match rule {
        "first_win" => wins >= 1,
        "wins:10" => wins >= 10,
        "rating:1600" => rating >= 1600.0,
        "rating:1800" => rating >= 1800.0,
        "rating:2000" => rating >= 2000.0,
        "streak:3" => best_streak >= 3,
        "all_games" => BUILTIN
            .iter()
            .all(|game| played.iter().any(|row| row == game)),
        "season_top10" => top10,
        _ => false,
    }
}

async fn in_season_top10(
    tx: &mut Transaction<'_, Postgres>,
    agent_id: Uuid,
) -> anyhow::Result<bool> {
    let ranked: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM ratings mine
           WHERE mine.agent_id = $1 AND mine.season = $2
             AND (
               SELECT COUNT(*) + 1 FROM ratings other
               WHERE other.game_id = mine.game_id
                 AND other.season = mine.season
                 AND (other.rating - 2 * other.rd) > (mine.rating - 2 * mine.rd)
             ) <= 10"#,
    )
    .bind(agent_id)
    .bind(SEASON)
    .fetch_one(&mut **tx)
    .await?;
    Ok(ranked > 0)
}

pub async fn loadouts(
    pool: &PgPool,
    game_id: &str,
    ids: &[Uuid],
) -> anyhow::Result<HashMap<Uuid, HashMap<String, String>>> {
    let mut out: HashMap<Uuid, HashMap<String, String>> = HashMap::new();
    if ids.is_empty() {
        return Ok(out);
    }
    let rows = sqlx::query_as::<_, (Uuid, String, String)>(
        r#"SELECT l.agent_id, l.kind, i.asset
           FROM agent_loadout l
           JOIN cosmetic_items i ON i.id = l.item_id
           WHERE l.game_id = $1 AND l.agent_id = ANY($2)"#,
    )
    .bind(game_id)
    .bind(ids)
    .fetch_all(pool)
    .await?;
    for (agent_id, kind, asset) in rows {
        out.entry(agent_id).or_default().insert(kind, asset);
    }
    Ok(out)
}

pub fn overlay(look: &mut SeatLook, equip: &HashMap<String, String>) {
    if let Some(piece) = equip.get("piece") {
        look.piece = piece.clone();
    }
    if let Some(board) = equip.get("board") {
        look.board = board.clone();
    }
    if let Some(card_back) = equip.get("card_back") {
        look.card_back = card_back.clone();
    }
    if let Some(emote) = equip.get("emote") {
        look.emote = emote.clone();
    }
}

pub fn seat_look(
    game_id: &str,
    appearance: &Appearance,
    equip: Option<&HashMap<String, String>>,
) -> SeatLook {
    let piece = equip
        .and_then(|row| row.get("piece"))
        .cloned()
        .unwrap_or_else(|| default_piece(game_id).to_string());
    let board = equip
        .and_then(|row| row.get("board"))
        .cloned()
        .unwrap_or_else(|| "paper".to_string());
    let card_back = equip
        .and_then(|row| row.get("card_back"))
        .cloned()
        .unwrap_or_else(|| "classic".to_string());
    let emote = equip
        .and_then(|row| row.get("emote"))
        .cloned()
        .unwrap_or_else(|| "none".to_string());
    SeatLook {
        title: appearance.title.clone(),
        color: appearance.color.clone(),
        avatar_preset: appearance.avatar_preset.clone(),
        avatar_image: appearance.avatar_image,
        frame: frame_asset(appearance.frame_id.as_deref()).map(str::to_string),
        piece,
        board,
        card_back,
        emote,
    }
}

pub async fn profile(pool: &PgPool, agent_id: Uuid) -> Result<Value, CallError> {
    bundle(pool, agent_id).await
}

pub async fn update_profile(
    pool: &PgPool,
    data_dir: &Path,
    agent_id: Uuid,
    body: &Value,
) -> Result<Value, CallError> {
    let exists: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM agents WHERE id = $1 AND deleted_at IS NULL")
            .bind(agent_id)
            .fetch_optional(pool)
            .await
            .map_err(db_err)?;
    if exists.is_none() {
        return Err(CallError::new(404, "not_found", "找不到这个智能体"));
    }
    let mut tx = pool.begin().await.map_err(db_err)?;
    if let Some(name) = body.get("name") {
        let name = name.as_str().unwrap_or("").trim();
        if name.chars().count() < 1 || name.chars().count() > 40 {
            return Err(CallError::new(400, "bad_name", "智能体名称需要 1–40 个字"));
        }
        sqlx::query("UPDATE agents SET name = $2 WHERE id = $1")
            .bind(agent_id)
            .bind(name)
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
    }
    if body.get("title").is_some() {
        let title = optional_text(body.get("title"), 12, "称号最多 12 个字")?;
        sqlx::query("UPDATE agents SET title = $2 WHERE id = $1")
            .bind(agent_id)
            .bind(title)
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
    }
    if body.get("bio").is_some() {
        let bio = optional_text(body.get("bio"), 80, "简介最多 80 个字")?;
        sqlx::query("UPDATE agents SET bio = $2 WHERE id = $1")
            .bind(agent_id)
            .bind(bio)
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
    }
    if let Some(color) = body.get("color") {
        let color = color.as_str().unwrap_or("");
        if !hex_color(color) {
            return Err(CallError::new(400, "bad_color", "颜色要用 #RRGGBB"));
        }
        sqlx::query("UPDATE agents SET color = $2 WHERE id = $1")
            .bind(agent_id)
            .bind(color)
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
    }
    if let Some(preset) = body.get("avatar_preset") {
        let preset = preset.as_str().unwrap_or("");
        if !PRESETS.iter().any(|(id, _)| *id == preset) {
            return Err(CallError::new(400, "bad_preset", "没有这个头像"));
        }
        sqlx::query("UPDATE agents SET avatar_preset = $2 WHERE id = $1")
            .bind(agent_id)
            .bind(preset)
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
    }
    if body.get("clear_avatar").and_then(|v| v.as_bool()) == Some(true) {
        sqlx::query("UPDATE agents SET avatar_image = false WHERE id = $1")
            .bind(agent_id)
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
    }
    if body.get("frame_id").is_some() {
        let frame = match body.get("frame_id") {
            Some(Value::Null) => None,
            Some(Value::String(text)) if text.is_empty() || text == "frame-none" => None,
            Some(Value::String(text)) => Some(text.clone()),
            _ => return Err(CallError::new(400, "bad_frame", "没有这个边框")),
        };
        if let Some(id) = &frame {
            let item = find_item(id)
                .filter(|item| item.kind == "frame")
                .ok_or_else(|| CallError::new(400, "bad_frame", "没有这个边框"))?;
            ensure_unlocked(&mut tx, agent_id, item).await?;
        }
        sqlx::query("UPDATE agents SET frame_id = $2 WHERE id = $1")
            .bind(agent_id)
            .bind(frame)
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
    }
    if let Some(equip) = body.get("equip").and_then(|v| v.as_array()) {
        for entry in equip {
            apply_equip(&mut tx, agent_id, entry).await?;
        }
    }
    tx.commit().await.map_err(db_err)?;
    if body.get("clear_avatar").and_then(|v| v.as_bool()) == Some(true) {
        let _ = std::fs::remove_file(avatar_path(data_dir, agent_id));
    }
    bundle(pool, agent_id).await
}

async fn apply_equip(
    tx: &mut Transaction<'_, Postgres>,
    agent_id: Uuid,
    entry: &Value,
) -> Result<(), CallError> {
    let game_id = entry.get("game_id").and_then(|v| v.as_str()).unwrap_or("");
    let kind = entry.get("kind").and_then(|v| v.as_str()).unwrap_or("");
    if !game_ok(game_id) {
        return Err(CallError::new(400, "bad_game", "没有这款游戏的装扮"));
    }
    let item_id = match entry.get("item_id") {
        None | Some(Value::Null) => None,
        Some(Value::String(text)) if text.is_empty() => None,
        Some(Value::String(text)) => Some(text.as_str()),
        _ => return Err(CallError::new(400, "bad_item", "没有这个装扮")),
    };
    let Some(item_id) = item_id else {
        sqlx::query("DELETE FROM agent_loadout WHERE agent_id = $1 AND game_id = $2 AND kind = $3")
            .bind(agent_id)
            .bind(game_id)
            .bind(kind)
            .execute(&mut **tx)
            .await
            .map_err(db_err)?;
        return Ok(());
    };
    let item = find_item(item_id).ok_or_else(|| CallError::new(400, "bad_item", "没有这个装扮"))?;
    if item.kind != kind || !scope_ok(item, game_id) {
        return Err(CallError::new(
            400,
            "bad_item",
            "这个装扮不能用在这款游戏上",
        ));
    }
    ensure_unlocked(tx, agent_id, item).await?;
    sqlx::query(
        r#"INSERT INTO agent_loadout (agent_id, game_id, kind, item_id)
           VALUES ($1,$2,$3,$4)
           ON CONFLICT (agent_id, game_id, kind) DO UPDATE SET item_id = EXCLUDED.item_id"#,
    )
    .bind(agent_id)
    .bind(game_id)
    .bind(kind)
    .bind(item.id)
    .execute(&mut **tx)
    .await
    .map_err(db_err)?;
    Ok(())
}

async fn ensure_unlocked(
    tx: &mut Transaction<'_, Postgres>,
    agent_id: Uuid,
    item: &Item,
) -> Result<(), CallError> {
    if item.rule == "default" {
        return Ok(());
    }
    let row: Option<i32> =
        sqlx::query_scalar("SELECT 1 FROM agent_unlocks WHERE agent_id = $1 AND item_id = $2")
            .bind(agent_id)
            .bind(item.id)
            .fetch_optional(&mut **tx)
            .await
            .map_err(db_err)?;
    if row.is_none() {
        return Err(CallError::new(400, "locked", "还没解锁这个装扮"));
    }
    Ok(())
}

fn game_ok(game_id: &str) -> bool {
    if BUILTIN.contains(&game_id) {
        return true;
    }
    game_id.starts_with("u_")
        && game_id.len() <= 64
        && game_id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
}

fn scope_ok(item: &Item, game_id: &str) -> bool {
    match item.kind {
        "piece" => item.game_id == Some(game_id),
        "card_back" => matches!(game_id, "doudizhu" | "mahjong-sichuan"),
        "board" => matches!(game_id, "gomoku" | "xiangqi" | "go"),
        "emote" => true,
        _ => false,
    }
}

fn optional_text(
    value: Option<&Value>,
    max: usize,
    message: &'static str,
) -> Result<Option<String>, CallError> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => {
            let text = text.trim();
            if text.is_empty() {
                Ok(None)
            } else if text.chars().count() > max {
                Err(CallError::new(400, "bad_text", message))
            } else {
                Ok(Some(text.to_string()))
            }
        }
        _ => Err(CallError::new(400, "bad_text", message)),
    }
}

fn hex_color(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 7 && bytes[0] == b'#' && bytes[1..].iter().all(|b| b.is_ascii_hexdigit())
}

async fn bundle(pool: &PgPool, agent_id: Uuid) -> Result<Value, CallError> {
    let row = sqlx::query_as::<
        _,
        (
            String,
            Option<String>,
            Option<String>,
            String,
            String,
            bool,
            Option<String>,
        ),
    >(
        r#"SELECT name, title, bio, color, avatar_preset, avatar_image, frame_id
           FROM agents WHERE id = $1"#,
    )
    .bind(agent_id)
    .fetch_optional(pool)
    .await
    .map_err(db_err)?
    .ok_or_else(|| CallError::new(404, "not_found", "找不到这个智能体"))?;
    let loadout = sqlx::query_as::<_, (String, String, String)>(
        "SELECT game_id, kind, item_id FROM agent_loadout WHERE agent_id = $1 ORDER BY game_id, kind",
    )
    .bind(agent_id)
    .fetch_all(pool)
    .await
    .map_err(db_err)?;
    let unlocks = sqlx::query_as::<_, (String, String, bool)>(
        r#"SELECT u.item_id, i.name, u.acked_at IS NULL
           FROM agent_unlocks u
           JOIN cosmetic_items i ON i.id = u.item_id
           WHERE u.agent_id = $1
           ORDER BY u.unlocked_at"#,
    )
    .bind(agent_id)
    .fetch_all(pool)
    .await
    .map_err(db_err)?;
    let fresh: Vec<Value> = unlocks
        .iter()
        .filter(|(_, _, fresh)| *fresh)
        .map(|(id, name, _)| json!({"id": id, "name": name}))
        .collect();
    let unlock_ids: Vec<&str> = unlocks.iter().map(|(id, _, _)| id.as_str()).collect();
    Ok(json!({
        "agent_id": agent_id,
        "name": row.0,
        "title": row.1,
        "bio": row.2,
        "color": row.3,
        "avatar_preset": row.4,
        "avatar_url": avatar_url(agent_id, row.5),
        "frame_id": row.6,
        "frame": frame_asset(row.6.as_deref()),
        "loadout": loadout.into_iter().map(|(game_id, kind, item_id)| json!({
            "game_id": game_id,
            "kind": kind,
            "item_id": item_id,
        })).collect::<Vec<_>>(),
        "unlocks": unlock_ids,
        "fresh": fresh,
        "items": catalog_json(),
        "presets": presets_json(),
    }))
}

pub async fn owned(pool: &PgPool, user_id: Uuid, agent_id: Uuid) -> Result<(), CallError> {
    let ok: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM agents WHERE id = $1 AND owner_id = $2 AND deleted_at IS NULL",
    )
    .bind(agent_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(db_err)?;
    if ok.is_none() {
        return Err(CallError::new(404, "not_found", "找不到这个智能体"));
    }
    Ok(())
}

pub async fn fresh_for_user(pool: &PgPool, user_id: Uuid) -> Result<Value, CallError> {
    let rows = sqlx::query_as::<_, (Uuid, String, String)>(
        r#"SELECT u.agent_id, u.item_id, i.name
           FROM agent_unlocks u
           JOIN cosmetic_items i ON i.id = u.item_id
           JOIN agents a ON a.id = u.agent_id
           WHERE a.owner_id = $1 AND a.deleted_at IS NULL AND u.acked_at IS NULL
           ORDER BY u.unlocked_at"#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(db_err)?;
    Ok(json!({
        "items": rows.into_iter().map(|(agent_id, id, name)| json!({
            "agent_id": agent_id,
            "id": id,
            "name": name,
        })).collect::<Vec<_>>(),
    }))
}

pub async fn ack_for_user(pool: &PgPool, user_id: Uuid) -> Result<Value, CallError> {
    sqlx::query(
        r#"UPDATE agent_unlocks SET acked_at = now()
           WHERE acked_at IS NULL
             AND agent_id IN (SELECT id FROM agents WHERE owner_id = $1 AND deleted_at IS NULL)"#,
    )
    .bind(user_id)
    .execute(pool)
    .await
    .map_err(db_err)?;
    Ok(json!({"ok": true}))
}

pub async fn save_avatar(
    pool: &PgPool,
    data_dir: &Path,
    agent_id: Uuid,
    content_type: &str,
    bytes: &[u8],
) -> Result<Value, CallError> {
    owned_agent(pool, agent_id).await?;
    if bytes.len() > MAX_AVATAR_BYTES {
        return Err(CallError::new(400, "too_large", "图片不能超过 1.5 MB"));
    }
    if bytes.is_empty() {
        return Err(CallError::new(400, "bad_image", "图片是空的"));
    }
    let declared = normalize_type(content_type);
    let Some(declared) = declared else {
        return Err(CallError::new(
            400,
            "bad_type",
            "只接受 PNG、JPEG、GIF 或 WebP",
        ));
    };
    let sniffed =
        sniff(bytes).ok_or_else(|| CallError::new(400, "bad_type", "文件内容和类型不一致"))?;
    if sniffed != declared {
        return Err(CallError::new(400, "bad_type", "文件内容和类型不一致"));
    }
    let format = match declared {
        "image/png" => image::ImageFormat::Png,
        "image/jpeg" => image::ImageFormat::Jpeg,
        "image/gif" => image::ImageFormat::Gif,
        "image/webp" => image::ImageFormat::WebP,
        _ => {
            return Err(CallError::new(
                400,
                "bad_type",
                "只接受 PNG、JPEG、GIF 或 WebP",
            ))
        }
    };
    let webp = encode_avatar(bytes, format)?;
    let path = avatar_path(data_dir, agent_id);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|_| CallError::new(500, "avatar", "头像没有写进去"))?;
    }
    std::fs::write(&path, &webp).map_err(|_| CallError::new(500, "avatar", "头像没有写进去"))?;
    sqlx::query("UPDATE agents SET avatar_image = true WHERE id = $1")
        .bind(agent_id)
        .execute(pool)
        .await
        .map_err(db_err)?;
    Ok(json!({"avatar_url": avatar_url(agent_id, true)}))
}

fn encode_avatar(bytes: &[u8], format: image::ImageFormat) -> Result<Vec<u8>, CallError> {
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(32 * 1024 * 1024);
    let mut reader = image::ImageReader::with_format(std::io::Cursor::new(bytes), format);
    reader.limits(limits);
    let image = reader
        .decode()
        .map_err(|_| CallError::new(400, "bad_image", "图片无法读取，或边长超过 4096"))?;
    let image = image.resize_to_fill(256, 256, image::imageops::FilterType::Triangle);
    let mut buf = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut buf, image::ImageFormat::WebP)
        .map_err(|_| CallError::new(400, "bad_image", "图片无法处理"))?;
    Ok(buf.into_inner())
}

pub fn avatar_path(data_dir: &Path, agent_id: Uuid) -> PathBuf {
    data_dir.join("avatars").join(format!("{agent_id}.webp"))
}

async fn owned_agent(pool: &PgPool, agent_id: Uuid) -> Result<(), CallError> {
    let ok: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM agents WHERE id = $1 AND deleted_at IS NULL")
            .bind(agent_id)
            .fetch_optional(pool)
            .await
            .map_err(db_err)?;
    if ok.is_none() {
        return Err(CallError::new(404, "not_found", "找不到这个智能体"));
    }
    Ok(())
}

fn normalize_type(header: &str) -> Option<&'static str> {
    let base = header
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    match base.as_str() {
        "image/png" => Some("image/png"),
        "image/jpeg" | "image/jpg" => Some("image/jpeg"),
        "image/gif" => Some("image/gif"),
        "image/webp" => Some("image/webp"),
        _ => None,
    }
}

fn sniff(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
        return Some("image/png");
    }
    if bytes.len() >= 3 && bytes[0] == 0xFF && bytes[1] == 0xD8 && bytes[2] == 0xFF {
        return Some("image/jpeg");
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some("image/gif");
    }
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Some("image/webp");
    }
    None
}

fn db_err(err: sqlx::Error) -> CallError {
    tracing::warn!(error = %err, "cosmetics");
    CallError::new(500, "db", "数据库暂时不可用")
}
