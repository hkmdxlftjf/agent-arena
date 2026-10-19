//! Compact seat view. The actor still keeps the full chain and the full legal
//! list; this is what `get_state` and `wait_for_turn` send unless asked otherwise.

use serde_json::{json, Value};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum History {
    None,
    Last(usize),
    Full,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Compact,
    Verbose,
}

#[derive(Clone)]
pub struct StateOpts {
    pub include_legal: bool,
    pub legal_filter: Option<String>,
    pub legal_limit: Option<usize>,
    pub history: History,
    pub format: Format,
}

impl Default for StateOpts {
    fn default() -> Self {
        Self {
            include_legal: false,
            legal_filter: None,
            legal_limit: None,
            history: History::None,
            format: Format::Compact,
        }
    }
}

impl StateOpts {
    pub fn parse(args: &Value) -> Self {
        let mut opts = Self::default();
        if args.get("include_legal_moves").and_then(|v| v.as_bool()) == Some(true) {
            opts.include_legal = true;
        }
        if let Some(filter) = args.get("filter").and_then(|v| v.as_str()) {
            opts.legal_filter = Some(filter.to_string());
        }
        if let Some(limit) = args.get("limit").and_then(|v| v.as_u64()) {
            opts.legal_limit = Some(limit as usize);
        }
        match args
            .get("history")
            .and_then(|v| v.as_str())
            .unwrap_or("none")
        {
            "full" => opts.history = History::Full,
            "none" => opts.history = History::None,
            other => {
                let n = other
                    .strip_prefix("last_")
                    .or_else(|| other.strip_prefix("last"))
                    .and_then(|s| s.trim_matches('_').parse::<usize>().ok())
                    .or_else(|| {
                        args.get("history_n")
                            .and_then(|v| v.as_u64())
                            .map(|n| n as usize)
                    })
                    .unwrap_or(8);
                if other == "last" || other.starts_with("last") {
                    opts.history = History::Last(n.clamp(1, 200));
                }
            }
        }
        if args.get("format").and_then(|v| v.as_str()) == Some("verbose") {
            opts.format = Format::Verbose;
        }
        opts
    }
}

pub fn project(full: &Value, opts: &StateOpts) -> Value {
    if full.get("error").is_some() {
        return full.clone();
    }
    if opts.format == Format::Verbose {
        let mut body = full.clone();
        if let Some(actions) = take_legal(full, opts) {
            let truncated = was_truncated(full, &actions, opts);
            body["legal_actions"] = json!(actions);
            body["legal_truncated"] = json!(truncated);
        }
        return body;
    }
    let view = full.get("view").cloned().unwrap_or(Value::Null);
    let game = full
        .get("game_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let seat = full.get("seat").and_then(|v| v.as_i64());
    let signing = full.get("signing_request").cloned().unwrap_or(Value::Null);
    let prev_hash = signing
        .pointer("/payload/prev_hash")
        .cloned()
        .or_else(|| tip_hash(full.get("entries")))
        .unwrap_or(Value::Null);
    let remaining_ms = full
        .get("remaining_ms")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let (board_kind, board) = compact_board(&game, &view, seat);
    let mut body = json!({
        "match_id": full.get("id").cloned().unwrap_or(Value::Null),
        "game": game,
        "game_name": full.get("game_name").cloned().unwrap_or(Value::Null),
        "variant": full.get("variant").cloned().unwrap_or(Value::Null),
        "seat": full.get("seat").cloned().unwrap_or(Value::Null),
        "status": full.get("status").cloned().unwrap_or(Value::Null),
        "phase": view.get("phase").cloned().or_else(|| full.get("phase").cloned()).unwrap_or(Value::Null),
        "to_act": full.get("to_act").cloned().unwrap_or(json!([])),
        "your_turn": signing.is_object(),
        "deadline_at": signing.pointer("/payload/deadline_at").cloned().or_else(|| full.get("deadline_at").cloned()).unwrap_or(Value::Null),
        "remaining_ms": remaining_ms,
        "remaining_sec": (remaining_ms / 1000).max(0),
        "time_limit_ms": full.get("time_limit_ms").cloned().unwrap_or(Value::Null),
        "timeout_policy": full.get("timeout_policy").cloned().unwrap_or(Value::Null),
        "move_no": move_no(full, &view),
        "prev_hash": prev_hash,
        "signing_request": if signing.is_object() { signing } else { Value::Null },
        "board_kind": board_kind,
        "board": board,
        "last_moves": last_moves(full.get("entries"), 3),
        "scores": view.get("points").cloned().or_else(|| full.pointer("/result/scores").cloned()).unwrap_or(Value::Null),
        "counts": view.get("counts").cloned().unwrap_or(Value::Null),
        "hand": own_hand(&view, seat),
        "consecutive_timeouts": full.get("consecutive_timeouts").cloned().unwrap_or(json!(0)),
        "room_code": full.get("room_code").cloned().unwrap_or(Value::Null),
        "expires_at": full.get("expires_at").cloned().unwrap_or(Value::Null),
        "end_reason": full.get("end_reason").cloned().unwrap_or(Value::Null),
        "result": full.get("result").cloned().unwrap_or(Value::Null),
    });
    if full.get("status").and_then(Value::as_str) == Some("waiting") {
        body["seats"] = full.get("seats").cloned().unwrap_or(json!([]));
    }
    if let Some(actions) = take_legal(full, opts) {
        let truncated = was_truncated(full, &actions, opts);
        body["legal_actions"] = json!(actions);
        body["legal_truncated"] = json!(truncated);
    }
    match opts.history {
        History::None => {}
        History::Last(n) => {
            body["history"] = json!(last_moves(full.get("entries"), n));
        }
        History::Full => {
            body["entries"] = full.get("entries").cloned().unwrap_or(json!([]));
        }
    }
    body
}

pub fn legal_page(full: &Value, opts: &StateOpts) -> Value {
    let actions = take_legal(full, opts).unwrap_or_default();
    let total = full
        .get("legal_actions")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    json!({
        "match_id": full.get("id").cloned().unwrap_or(Value::Null),
        "your_turn": full.get("signing_request").map(|v| v.is_object()).unwrap_or(false),
        "total": total,
        "returned": actions.len(),
        "truncated": actions.len() < total,
        "actions": actions,
    })
}

pub fn history_page(full: &Value, from_move: i64, limit: usize) -> Value {
    let moves = last_moves(full.get("entries"), usize::MAX);
    let filtered: Vec<Value> = moves
        .into_iter()
        .filter(|m| m.get("seq").and_then(|v| v.as_i64()).unwrap_or(0) >= from_move)
        .take(limit)
        .collect();
    json!({
        "match_id": full.get("id").cloned().unwrap_or(Value::Null),
        "from_move": from_move,
        "returned": filtered.len(),
        "moves": filtered,
    })
}

fn was_truncated(full: &Value, returned: &[Value], opts: &StateOpts) -> bool {
    let total = full
        .get("legal_actions")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    returned.len() < total
        || opts.legal_limit.is_some() && returned.len() == opts.legal_limit.unwrap()
}

fn take_legal(full: &Value, opts: &StateOpts) -> Option<Vec<Value>> {
    if !opts.include_legal && opts.format == Format::Compact {
        return None;
    }
    let mut actions = full
        .get("legal_actions")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    if opts.legal_filter.as_deref() == Some("near") {
        if let Some(origin) = last_coord(full.get("view")) {
            let near: Vec<Value> = actions
                .iter()
                .filter(|action| action_near(action, origin))
                .cloned()
                .collect();
            if !near.is_empty() {
                actions = near;
            }
        }
    }
    if let Some(limit) = opts.legal_limit {
        actions.truncate(limit);
    }
    Some(actions)
}

fn move_no(full: &Value, view: &Value) -> i64 {
    view.get("moves")
        .and_then(|v| v.as_i64())
        .or_else(|| full.get("move_count").and_then(|v| v.as_i64()))
        .unwrap_or(0)
}

fn tip_hash(entries: Option<&Value>) -> Option<Value> {
    entries?
        .as_array()?
        .last()?
        .get("hash")
        .cloned()
        .or_else(|| {
            entries?
                .as_array()?
                .last()?
                .get("payload")
                .and_then(|p| p.get("prev_hash"))
                .cloned()
        })
}

fn last_moves(entries: Option<&Value>, n: usize) -> Vec<Value> {
    let Some(arr) = entries.and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in arr.iter().rev() {
        let kind = entry.get("kind").and_then(|v| v.as_str()).unwrap_or("");
        if kind != "action" && kind != "auto_action" {
            continue;
        }
        let payload = entry.get("payload").cloned().unwrap_or(Value::Null);
        out.push(json!({
            "seq": entry.get("seq").cloned().unwrap_or(Value::Null),
            "seat": payload.get("seat").cloned().unwrap_or(Value::Null),
            "action": payload.get("action").cloned().unwrap_or(Value::Null),
            "kind": kind,
        }));
        if out.len() == n {
            break;
        }
    }
    out.reverse();
    out
}

fn own_hand(view: &Value, seat: Option<i64>) -> Value {
    let Some(seat) = seat else {
        return Value::Null;
    };
    view.get("hands")
        .and_then(|v| v.as_array())
        .and_then(|hands| hands.get(seat as usize))
        .cloned()
        .unwrap_or(Value::Null)
}

fn compact_board(game: &str, view: &Value, _seat: Option<i64>) -> (&'static str, Value) {
    match game {
        "gomoku" => (
            "coords",
            json!({
                "size": view.get("size").cloned().unwrap_or(json!(15)),
                "stones": gomoku_stones(view),
            }),
        ),
        "xiangqi" => ("fen", json!(xiangqi_fen(view))),
        "go" => (
            "sgf",
            json!({
                "size": view.get("size").cloned().unwrap_or(json!(19)),
                "komi": view.get("komi").cloned().unwrap_or(Value::Null),
                "sgf": go_sgf(view),
            }),
        ),
        "doudizhu" | "mahjong-sichuan" => (
            "cards",
            json!({
                "phase": view.get("phase").cloned().unwrap_or(Value::Null),
                "counts": view.get("counts").cloned().unwrap_or(Value::Null),
                "hand": Value::Null,
                "bid": view.get("bid").cloned().unwrap_or(Value::Null),
                "landlord": view.get("landlord").cloned().unwrap_or(Value::Null),
                "dingque": view.get("dingque").cloned().unwrap_or(Value::Null),
                "wall": view.get("wall").cloned().unwrap_or(Value::Null),
                "last": view.get("last").cloned().or_else(|| view.get("lastDraw").cloned()).unwrap_or(Value::Null),
            }),
        ),
        "junqi" => (
            "pieces",
            json!({
                "mine": junqi_mine(view),
                "last": view.get("last").cloned().unwrap_or(Value::Null),
            }),
        ),
        _ => ("summary", shrink_view(view)),
    }
}

fn gomoku_stones(view: &Value) -> Vec<String> {
    view.get("stones")
        .and_then(|v| v.as_array())
        .map(|stones| {
            stones
                .iter()
                .filter_map(|stone| {
                    let at = stone.get("at").and_then(|v| v.as_str())?;
                    let seat = stone.get("seat").and_then(|v| v.as_i64()).unwrap_or(0);
                    Some(format!("{}{at}", if seat == 0 { "B" } else { "W" }))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn xiangqi_fen(view: &Value) -> String {
    let mut board = [['.'; 9]; 10];
    let letters = ["", "k", "a", "b", "n", "r", "c", "p"];
    if let Some(pieces) = view.get("pieces").and_then(|v| v.as_array()) {
        for piece in pieces {
            let Some(at) = piece.get("at").and_then(|v| v.as_str()) else {
                continue;
            };
            let bytes = at.as_bytes();
            if bytes.len() < 2 {
                continue;
            }
            let file = (bytes[0].to_ascii_lowercase() - b'a') as usize;
            let Ok(rank) = at[1..].parse::<usize>() else {
                continue;
            };
            if !(1..=10).contains(&rank) || file >= 9 {
                continue;
            }
            let row = 10 - rank;
            let kind = piece.get("kind").and_then(|v| v.as_i64()).unwrap_or(0) as usize;
            let side = piece.get("side").and_then(|v| v.as_i64()).unwrap_or(0);
            let mut ch = letters
                .get(kind)
                .copied()
                .unwrap_or("x")
                .chars()
                .next()
                .unwrap_or('x');
            if side == 0 {
                ch = ch.to_ascii_uppercase();
            }
            board[row][file] = ch;
        }
    }
    let mut ranks = Vec::new();
    for row in &board {
        let mut rank = String::new();
        let mut empty = 0;
        for cell in row {
            if *cell == '.' {
                empty += 1;
            } else {
                if empty > 0 {
                    rank.push_str(&empty.to_string());
                    empty = 0;
                }
                rank.push(*cell);
            }
        }
        if empty > 0 {
            rank.push_str(&empty.to_string());
        }
        ranks.push(rank);
    }
    let turn = view
        .get("toAct")
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    format!("{} {}", ranks.join("/"), if turn == 0 { "w" } else { "b" })
}

fn go_sgf(view: &Value) -> String {
    let Some(points) = view.get("points").and_then(|v| v.as_array()) else {
        return String::new();
    };
    points
        .iter()
        .filter_map(|point| {
            let at = point.get("at").and_then(|v| v.as_str())?;
            let seat = point.get("seat").and_then(|v| v.as_i64()).unwrap_or(0);
            Some(format!("{}[{at}]", if seat == 0 { "B" } else { "W" }))
        })
        .collect::<Vec<_>>()
        .join(";")
}

fn junqi_mine(view: &Value) -> Value {
    let Some(cells) = view.get("cells").and_then(|v| v.as_array()) else {
        return json!([]);
    };
    let mine: Vec<Value> = cells
        .iter()
        .filter_map(|cell| {
            let at = cell.get("at").cloned()?;
            let piece = cell
                .get("piece")
                .cloned()
                .or_else(|| cell.get("label").cloned())
                .unwrap_or(Value::Null);
            if piece.is_null() {
                return None;
            }
            Some(json!({"at": at, "piece": piece}))
        })
        .take(40)
        .collect();
    json!(mine)
}

fn shrink_view(view: &Value) -> Value {
    let Some(obj) = view.as_object() else {
        return Value::Null;
    };
    let mut out = serde_json::Map::new();
    for (key, value) in obj {
        match value {
            Value::Array(items) if items.len() > 24 => {
                out.insert(key.clone(), json!({"len": items.len()}));
            }
            Value::Object(map) if map.len() > 12 => {
                out.insert(key.clone(), json!({"keys": map.len()}));
            }
            other => {
                out.insert(key.clone(), other.clone());
            }
        }
    }
    Value::Object(out)
}

fn last_coord(view: Option<&Value>) -> Option<(i32, i32)> {
    let view = view?;
    if let Some(at) = view.get("lastMove").and_then(|v| v.as_str()) {
        return parse_coord(at);
    }
    if let Some(at) = view.get("last").and_then(|v| v.as_str()) {
        return parse_coord(at);
    }
    if let Some(to) = view
        .get("lastMove")
        .and_then(|v| v.get("to"))
        .and_then(|v| v.as_str())
    {
        return parse_coord(to);
    }
    None
}

fn action_near(action: &Value, origin: (i32, i32)) -> bool {
    let data = action.get("data").cloned().unwrap_or(Value::Null);
    let at = data
        .get("at")
        .and_then(|v| v.as_str())
        .or_else(|| data.get("to").and_then(|v| v.as_str()));
    let Some(at) = at.and_then(parse_coord) else {
        return true;
    };
    (at.0 - origin.0).abs() <= 2 && (at.1 - origin.1).abs() <= 2
}

fn parse_coord(raw: &str) -> Option<(i32, i32)> {
    let raw = raw.trim();
    let bytes = raw.as_bytes();
    if bytes.len() < 2 || !bytes[0].is_ascii_alphabetic() {
        return None;
    }
    let file = (bytes[0].to_ascii_lowercase() - b'a') as i32;
    let rank: i32 = raw[1..].parse().ok()?;
    Some((file, rank))
}
