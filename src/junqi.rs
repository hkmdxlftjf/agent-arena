//! 两人军棋（陆战棋）。
//!
//! 每方 5 列 × 6 行。南方面（席位 0）横线 1–6，北方面横线 7–12。
//! 大本营：a1、e1、a12、e12。行营不能被吃，里面的子也不能吃子。
//! 铁路：左右两列（不含大本营）、底线 b–d、双方前线整行。工兵可在铁路上拐弯。
//!
//! 布阵是同时的承诺-揭示。公开日志只有 commitment；`make_move` 另附 private。
//! 碰子由裁判比较，双方只看见吃掉 / 被吃 / 同尽，身份到终局揭示。
//! 超时：布阵自动摆一副合法阵；走子自动停着；连续超时达到上限后判负。

use crate::game::*;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};

const NAMES: [&str; 12] = [
    "司令", "军长", "师长", "旅长", "团长", "营长", "连长", "排长", "工兵", "炸弹", "地雷", "军旗",
];
const COUNTS: [usize; 12] = [1, 1, 2, 2, 2, 2, 3, 3, 3, 2, 3, 1];
const RANKS: [i8; 12] = [9, 8, 7, 6, 5, 4, 3, 2, 1, 0, 0, 0];

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Piece {
    owner: u8,
    name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct JState {
    phase: String,
    #[serde(default)]
    board: HashMap<String, Piece>,
    #[serde(default)]
    deployed: Vec<bool>,
    turn: u8,
    #[serde(default)]
    last: Option<Value>,
    winner: Option<i8>,
    #[serde(default)]
    reason: Option<String>,
}

pub struct Junqi;

impl Junqi {
    fn load(state: &Value) -> Result<JState, GameError> {
        serde_json::from_value(state.clone()).map_err(|e| GameError(e.to_string()))
    }
}

pub fn standard_layout(seat: u8) -> Vec<Value> {
    let south = [
        ("a1", "军旗"),
        ("b1", "司令"),
        ("c1", "军长"),
        ("d1", "师长"),
        ("a2", "地雷"),
        ("b2", "地雷"),
        ("c2", "地雷"),
        ("d2", "炸弹"),
        ("e2", "炸弹"),
        ("a3", "师长"),
        ("c3", "旅长"),
        ("e3", "旅长"),
        ("a4", "团长"),
        ("b4", "团长"),
        ("d4", "营长"),
        ("e4", "营长"),
        ("a5", "连长"),
        ("c5", "连长"),
        ("e5", "连长"),
        ("d5", "排长"),
        ("a6", "排长"),
        ("b6", "排长"),
        ("c6", "工兵"),
        ("d6", "工兵"),
        ("e6", "工兵"),
    ];
    south
        .iter()
        .map(|(at, name)| {
            let at = if seat == 0 {
                (*at).to_string()
            } else {
                mirror(at)
            };
            json!({"at": at, "piece": name})
        })
        .collect()
}

fn mirror(at: &str) -> String {
    let (f, r) = parse(at).unwrap();
    square(f, 13 - r)
}

fn square(f: usize, r: usize) -> String {
    format!("{}{}", (b'a' + f as u8) as char, r)
}

fn parse(s: &str) -> Option<(usize, usize)> {
    let b = s.as_bytes();
    if b.len() < 2 {
        return None;
    }
    let f = (b[0].to_ascii_lowercase() - b'a') as usize;
    let r: usize = s[1..].parse().ok()?;
    if f < 5 && (1..=12).contains(&r) {
        Some((f, r))
    } else {
        None
    }
}

fn is_hq(f: usize, r: usize) -> bool {
    (r == 1 || r == 12) && (f == 0 || f == 4)
}

fn is_camp(f: usize, r: usize) -> bool {
    matches!(
        (f, r),
        (1, 3) | (3, 3) | (2, 4) | (1, 5) | (3, 5) | (1, 10) | (3, 10) | (2, 9) | (1, 8) | (3, 8)
    )
}

fn own_half(seat: u8, r: usize) -> bool {
    if seat == 0 {
        (1..=6).contains(&r)
    } else {
        (7..=12).contains(&r)
    }
}

fn on_rail(f: usize, r: usize) -> bool {
    if is_hq(f, r) || is_camp(f, r) {
        return false;
    }
    if f == 0 || f == 4 {
        return (2..=11).contains(&r);
    }
    if r == 1 || r == 12 {
        return (1..=3).contains(&f);
    }
    r == 6 || r == 7
}

fn rail_neighbors(f: usize, r: usize) -> Vec<(usize, usize)> {
    if !on_rail(f, r) {
        return Vec::new();
    }
    let mut v = Vec::new();
    for (df, dr) in [(1i32, 0), (-1, 0), (0, 1), (0, -1)] {
        let nf = f as i32 + df;
        let nr = r as i32 + dr;
        if (0..5).contains(&nf) && (1..=12).contains(&nr) && on_rail(nf as usize, nr as usize) {
            let straight = if df != 0 {
                r == 1 || r == 12 || r == 6 || r == 7
            } else {
                f == 0 || f == 4 || ((r == 6 || r == 7) && (nr == 6 || nr == 7))
            };
            if straight {
                v.push((nf as usize, nr as usize));
            }
        }
    }
    v
}

fn orthogonal(f: usize, r: usize) -> Vec<(usize, usize)> {
    let mut v = Vec::new();
    for (df, dr) in [(1i32, 0), (-1, 0), (0, 1), (0, -1)] {
        let nf = f as i32 + df;
        let nr = r as i32 + dr;
        if (0..5).contains(&nf) && (1..=12).contains(&nr) {
            v.push((nf as usize, nr as usize));
        }
    }
    v
}

fn rank_of(name: &str) -> i8 {
    NAMES
        .iter()
        .position(|n| *n == name)
        .map(|i| RANKS[i])
        .unwrap_or(0)
}

fn movable(name: &str) -> bool {
    name != "地雷" && name != "军旗"
}

fn fight(att: &str, def: &str) -> &'static str {
    if def == "军旗" {
        return "flag";
    }
    if att == "炸弹" || def == "炸弹" {
        return "both";
    }
    if def == "地雷" {
        return if att == "工兵" { "eat" } else { "eaten" };
    }
    if att == "地雷" || att == "军旗" {
        return "eaten";
    }
    let a = rank_of(att);
    let d = rank_of(def);
    if a > d {
        "eat"
    } else if a < d {
        "eaten"
    } else {
        "both"
    }
}

fn placements_of(action: &Value) -> Option<Vec<(String, String)>> {
    let data = action
        .get("_private")
        .and_then(|p| p.get("data"))
        .or_else(|| action.get("data"));
    let arr = data?.get("placements")?.as_array()?;
    let mut out = Vec::new();
    for item in arr {
        let at = item.get("at").and_then(|v| v.as_str())?.to_string();
        let piece = item.get("piece").and_then(|v| v.as_str())?.to_string();
        out.push((at, piece));
    }
    Some(out)
}

fn validate_layout(
    seat: u8,
    items: &[(String, String)],
) -> Result<HashMap<String, Piece>, GameError> {
    if items.len() != 25 {
        return Err(GameError("布阵必须正好 25 枚".into()));
    }
    let mut counts = HashMap::<String, usize>::new();
    let mut board = HashMap::new();
    for (at, name) in items {
        if !NAMES.contains(&name.as_str()) {
            return Err(GameError(format!("没有这种棋子：{name}")));
        }
        let (f, r) = parse(at).ok_or_else(|| GameError(format!("坐标 {at} 不在棋盘上")))?;
        if !own_half(seat, r) {
            return Err(GameError("只能布在自己的半场".into()));
        }
        if board.contains_key(at) {
            return Err(GameError("同一格放了两枚".into()));
        }
        if is_hq(f, r) && name != "军旗" {
            return Err(GameError("大本营只能放军旗".into()));
        }
        if name == "军旗" && !is_hq(f, r) {
            return Err(GameError("军旗必须放在大本营".into()));
        }
        if name == "地雷" && !(if seat == 0 { r <= 2 } else { r >= 11 }) {
            return Err(GameError("地雷只能放在后两排".into()));
        }
        *counts.entry(name.clone()).or_default() += 1;
        board.insert(
            at.clone(),
            Piece {
                owner: seat,
                name: name.clone(),
            },
        );
    }
    for (i, name) in NAMES.iter().enumerate() {
        if counts.get(*name).copied().unwrap_or(0) != COUNTS[i] {
            return Err(GameError(format!("{name} 的数量不对")));
        }
    }
    Ok(board)
}

fn destinations(state: &JState, at: &str) -> Vec<String> {
    let Some(piece) = state.board.get(at) else {
        return Vec::new();
    };
    if !movable(&piece.name) {
        return Vec::new();
    }
    let (f, r) = parse(at).unwrap();
    let from_camp = is_camp(f, r);
    let mut out = Vec::new();
    let consider =
        |out: &mut Vec<String>, state: &JState, nf: usize, nr: usize, from_camp: bool| {
            let dest = square(nf, nr);
            if let Some(other) = state.board.get(&dest) {
                if other.owner == state.board.get(at).map(|p| p.owner).unwrap_or(9) {
                    return;
                }
                if from_camp || is_camp(nf, nr) {
                    return;
                }
                if is_hq(nf, nr) && other.name != "军旗" {
                    return;
                }
            } else if is_hq(nf, nr) {
                return;
            }
            out.push(dest);
        };
    for (nf, nr) in orthogonal(f, r) {
        consider(&mut out, state, nf, nr, from_camp);
    }
    if !from_camp && on_rail(f, r) {
        if piece.name == "工兵" {
            let mut q = VecDeque::new();
            let mut seen = std::collections::HashSet::new();
            q.push_back((f, r));
            seen.insert((f, r));
            while let Some((cf, cr)) = q.pop_front() {
                for (nf, nr) in rail_neighbors(cf, cr) {
                    if seen.contains(&(nf, nr)) {
                        continue;
                    }
                    let dest = square(nf, nr);
                    if let Some(other) = state.board.get(&dest) {
                        if other.owner != piece.owner && !is_camp(nf, nr) {
                            out.push(dest);
                        }
                        continue;
                    }
                    seen.insert((nf, nr));
                    out.push(dest);
                    q.push_back((nf, nr));
                }
            }
        } else {
            for (df, dr) in [(1i32, 0), (-1, 0), (0, 1), (0, -1)] {
                let mut nf = f as i32 + df;
                let mut nr = r as i32 + dr;
                while (0..5).contains(&nf)
                    && (1..=12).contains(&nr)
                    && on_rail(nf as usize, nr as usize)
                    && rail_neighbors((nf - df) as usize, (nr - dr) as usize)
                        .contains(&(nf as usize, nr as usize))
                {
                    let dest = square(nf as usize, nr as usize);
                    if let Some(other) = state.board.get(&dest) {
                        if other.owner != piece.owner && !is_camp(nf as usize, nr as usize) {
                            out.push(dest);
                        }
                        break;
                    }
                    out.push(dest);
                    nf += df;
                    nr += dr;
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

fn any_move(state: &JState, seat: u8) -> bool {
    state
        .board
        .iter()
        .any(|(at, p)| p.owner == seat && !destinations(state, at).is_empty())
}

fn show_piece(piece: &Piece, viewer: &Viewer) -> Value {
    let see = viewer.omniscient || viewer.seat == Some(piece.owner as i32);
    json!({
        "owner": piece.owner,
        "name": if see { Value::String(piece.name.clone()) } else { Value::String("暗".into()) },
        "hidden": !see,
    })
}

impl Game for Junqi {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "junqi".into(),
            name: "军棋".into(),
            version: "1.0.0".into(),
            min_players: 2,
            max_players: 2,
            traits: vec![Trait::Board, Trait::HiddenInfo, Trait::MultiPhase],
            variants: vec![Variant {
                id: "standard".into(),
                name: "两人暗棋".into(),
                options: None,
            }],
            default_time: TimeControl {
                per_action_ms: 60_000,
                max_consecutive_timeouts: 3,
                grace_ms: 2_000,
            },
            timeout_policy: TimeoutPolicy::AutoPass,
            allowed_timeout_policies: vec![
                TimeoutPolicy::AutoPass,
                TimeoutPolicy::AutoAction,
                TimeoutPolicy::Forfeit,
            ],
            action_schema: json!({"type":"object"}),
            rules_markdown: JUNQI_RULES.into(),
            renderer: "junqi".into(),
            rated: true,
            engine: "native".into(),
            description: "两人暗棋。承诺-揭示布阵，裁判暗中比子，终局才公开身份。".into(),
            playable: true,
        }
    }

    fn setup(&self, cfg: &Config, _: &mut dyn ArenaRng) -> Result<(Value, Vec<Value>), GameError> {
        if cfg.variant != "standard" {
            return Err(GameError("未知变体".into()));
        }
        let s = JState {
            phase: "deploy".into(),
            board: HashMap::new(),
            deployed: vec![false, false],
            turn: 0,
            last: None,
            winner: None,
            reason: None,
        };
        Ok((serde_json::to_value(s).unwrap(), Vec::new()))
    }

    fn to_act(&self, state: &Value) -> Vec<Seat> {
        let Ok(s) = Self::load(state) else {
            return Vec::new();
        };
        if s.winner.is_some() {
            return Vec::new();
        }
        if s.phase == "deploy" {
            return s
                .deployed
                .iter()
                .enumerate()
                .filter(|(_, d)| !**d)
                .map(|(i, _)| i as Seat)
                .collect();
        }
        vec![s.turn as Seat]
    }

    fn view(&self, state: &Value, viewer: &Viewer) -> Result<Value, GameError> {
        let s = Self::load(state)?;
        let mut cells = Vec::new();
        for r in 1..=12 {
            for f in 0..5 {
                let at = square(f, r);
                let zone = if is_hq(f, r) {
                    "hq"
                } else if is_camp(f, r) {
                    "camp"
                } else if on_rail(f, r) {
                    "rail"
                } else {
                    "road"
                };
                let piece = s.board.get(&at).map(|p| show_piece(p, viewer));
                cells.push(json!({"at": at, "zone": zone, "piece": piece}));
            }
        }
        Ok(json!({
            "renderer": "junqi",
            "phase": s.phase,
            "cells": cells,
            "deployed": s.deployed,
            "last": s.last,
            "toAct": self.to_act(state),
            "winner": s.winner,
            "reason": s.reason,
        }))
    }

    fn legal_actions(&self, state: &Value, seat: Seat) -> Result<Vec<Value>, GameError> {
        let s = Self::load(state)?;
        if s.winner.is_some() {
            return Ok(Vec::new());
        }
        let mut actions = vec![json!({"type":"resign"})];
        if s.phase == "deploy" {
            if seat >= 0 && (seat as usize) < s.deployed.len() && !s.deployed[seat as usize] {
                actions.push(json!({"type":"deploy_commit"}));
            }
            return Ok(actions);
        }
        if seat as u8 != s.turn {
            return Ok(actions);
        }
        for (at, piece) in &s.board {
            if piece.owner != s.turn {
                continue;
            }
            for dest in destinations(&s, at) {
                actions.push(json!({"type":"move","data":{"from": at, "to": dest}}));
            }
        }
        Ok(actions)
    }

    fn apply(
        &self,
        state: &Value,
        seat: Seat,
        action: &Value,
        _: &mut dyn ArenaRng,
    ) -> Result<(Value, Vec<Value>), GameError> {
        let mut s = Self::load(state)?;
        if s.winner.is_some() {
            return Err(GameError("对局已结束".into()));
        }
        let kind = action.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if kind == "resign" || kind == "timeout_forfeit" {
            s.winner = Some(1 - seat as i8);
            s.reason = Some(if kind == "resign" {
                "resign".into()
            } else {
                "timeout".into()
            });
            s.phase = "over".into();
            return Ok((serde_json::to_value(s).unwrap(), Vec::new()));
        }
        if s.phase == "deploy" {
            if kind != "deploy_commit" && kind != "deploy" {
                return Err(GameError("布阵阶段只能提交承诺".into()));
            }
            if s.deployed.get(seat as usize).copied().unwrap_or(true) {
                return Err(GameError("你已经布过阵".into()));
            }
            let items = placements_of(action).ok_or_else(|| GameError("缺少布阵".into()))?;
            let placed = validate_layout(seat as u8, &items)?;
            for (at, piece) in placed {
                s.board.insert(at, piece);
            }
            s.deployed[seat as usize] = true;
            if s.deployed.iter().all(|d| *d) {
                s.phase = "play".into();
                s.turn = 0;
                if !any_move(&s, 0) {
                    s.winner = Some(1);
                    s.reason = Some("no_moves".into());
                    s.phase = "over".into();
                }
            }
            return Ok((serde_json::to_value(s).unwrap(), Vec::new()));
        }
        if seat as u8 != s.turn {
            return Err(GameError("还没轮到你".into()));
        }
        if kind == "pass" {
            s.turn = 1 - s.turn;
            s.last = Some(json!({"type":"pass","seat": seat}));
            if !any_move(&s, s.turn) {
                s.winner = Some(seat as i8);
                s.reason = Some("no_moves".into());
                s.phase = "over".into();
            }
            return Ok((serde_json::to_value(s).unwrap(), Vec::new()));
        }
        if kind != "move" {
            return Err(GameError("未知动作".into()));
        }
        let from = action["data"]["from"].as_str().unwrap_or("").to_string();
        let to = action["data"]["to"].as_str().unwrap_or("").to_string();
        if !destinations(&s, &from).iter().any(|d| d == &to) {
            return Err(GameError("这步棋不合法".into()));
        }
        let attacker = s
            .board
            .get(&from)
            .cloned()
            .ok_or_else(|| GameError("起点没有棋子".into()))?;
        if attacker.owner != seat as u8 {
            return Err(GameError("不能动对方的子".into()));
        }
        let outcome = if let Some(defender) = s.board.get(&to).cloned() {
            let result = fight(&attacker.name, &defender.name);
            match result {
                "flag" => {
                    s.board.remove(&from);
                    s.board.insert(to.clone(), attacker);
                    s.winner = Some(seat as i8);
                    s.reason = Some("flag".into());
                    s.phase = "over".into();
                }
                "eat" => {
                    s.board.remove(&from);
                    s.board.insert(to.clone(), attacker);
                }
                "eaten" => {
                    s.board.remove(&from);
                }
                _ => {
                    s.board.remove(&from);
                    s.board.remove(&to);
                }
            }
            result
        } else {
            s.board.remove(&from);
            s.board.insert(to.clone(), attacker);
            "move"
        };
        s.last = Some(json!({"from": from, "to": to, "result": outcome, "seat": seat}));
        if s.winner.is_none() {
            s.turn = 1 - s.turn;
            if !any_move(&s, s.turn) {
                s.winner = Some(1 - s.turn as i8);
                s.reason = Some("no_moves".into());
                s.phase = "over".into();
            }
        }
        Ok((serde_json::to_value(s).unwrap(), Vec::new()))
    }

    fn is_terminal(&self, state: &Value) -> bool {
        Self::load(state)
            .map(|s| s.winner.is_some())
            .unwrap_or(true)
    }

    fn result(&self, state: &Value) -> Result<GameResult, GameError> {
        let s = Self::load(state)?;
        let winner = s.winner.ok_or_else(|| GameError("对局还没结束".into()))?;
        let (scores, ranking) = if winner == 0 {
            (vec![1.0, 0.0], vec![vec![0], vec![1]])
        } else {
            (vec![0.0, 1.0], vec![vec![1], vec![0]])
        };
        Ok(GameResult {
            scores,
            ranking,
            teams: None,
            reason: s.reason.unwrap_or_else(|| "junqi".into()),
        })
    }

    fn timeout_decision(
        &self,
        state: &Value,
        seat: Seat,
        policy: TimeoutPolicy,
        _: &mut dyn ArenaRng,
    ) -> Result<TimeoutDecision, GameError> {
        if policy == TimeoutPolicy::Forfeit {
            return Ok(TimeoutDecision {
                policy,
                action: Some(json!({"type":"resign"})),
            });
        }
        let s = Self::load(state)?;
        if s.phase == "deploy" {
            let placements = standard_layout(seat as u8);
            return Ok(TimeoutDecision {
                policy: TimeoutPolicy::AutoAction,
                action: Some(json!({"type":"deploy","data":{"placements": placements}})),
            });
        }
        Ok(TimeoutDecision {
            policy: TimeoutPolicy::AutoPass,
            action: Some(json!({"type":"pass"})),
        })
    }
}

const JUNQI_RULES: &str = r#"# 军棋

两人各 25 子：司令、军长、师长×2、旅长×2、团长×2、营长×2、连长×3、排长×3、工兵×3、炸弹×2、地雷×3、军旗。

南方面 a–e × 1–6，北方面 7–12。军旗只放在己方大本营，地雷只放在后两排。行营里的子不能被吃，也不能吃子。

布阵同时进行：签名动作是 `{type:"deploy_commit"}`，载荷带 `commitment`，`make_move` 另附 `private: {data, salt}`。棋子身份到终局才写入揭示条目。

铁路上可直行任意格；工兵可沿铁路拐弯。炸弹同尽，工兵排雷，其余比阶级，相同同尽。夺旗或对方无子可走即胜。

走子超时自动停着。布阵超时由服务器摆开一副公开的合法阵。连续超时达到上限后判负。
"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> Value {
        let g = Junqi;
        let mut rng = crate::game::SeededRng::from_seed_seq(&[7; 32], 0);
        g.setup(
            &Config {
                players: 2,
                variant: "standard".into(),
                options: json!({}),
            },
            &mut rng,
        )
        .unwrap()
        .0
    }

    fn deploy(state: Value, seat: i32) -> Value {
        let g = Junqi;
        let mut rng = crate::game::SeededRng::from_seed_seq(&[7; 32], seat as i64);
        let action = json!({"type":"deploy","data":{"placements": standard_layout(seat as u8)}});
        g.apply(&state, seat, &action, &mut rng).unwrap().0
    }

    #[test]
    fn layout_counts_and_flag_capture() {
        let state = deploy(deploy(setup(), 0), 1);
        let s = Junqi::load(&state).unwrap();
        assert_eq!(s.phase, "play");
        assert_eq!(s.board.len(), 50);
        let g = Junqi;
        let mut rng = crate::game::SeededRng::from_seed_seq(&[1; 32], 0);
        let moved = g
            .apply(
                &state,
                0,
                &json!({"type":"move","data":{"from":"b4","to":"b3"}}),
                &mut rng,
            )
            .unwrap()
            .0;
        assert_eq!(Junqi::load(&moved).unwrap().turn, 1);

        let mut board = HashMap::new();
        board.insert(
            "a1".into(),
            Piece {
                owner: 0,
                name: "军旗".into(),
            },
        );
        board.insert(
            "a2".into(),
            Piece {
                owner: 1,
                name: "司令".into(),
            },
        );
        let s = JState {
            phase: "play".into(),
            board,
            deployed: vec![true, true],
            turn: 1,
            last: None,
            winner: None,
            reason: None,
        };
        let state = serde_json::to_value(s).unwrap();
        let next = g
            .apply(
                &state,
                1,
                &json!({"type":"move","data":{"from":"a2","to":"a1"}}),
                &mut rng,
            )
            .unwrap()
            .0;
        let result = g.result(&next).unwrap();
        assert_eq!(result.reason, "flag");
        assert_eq!(result.scores, vec![0.0, 1.0]);
    }

    #[test]
    fn referee_hides_ranks_and_resolves_combat() {
        assert_eq!(fight("司令", "军长"), "eat");
        assert_eq!(fight("工兵", "地雷"), "eat");
        assert_eq!(fight("排长", "地雷"), "eaten");
        assert_eq!(fight("炸弹", "司令"), "both");
        assert_eq!(fight("师长", "军旗"), "flag");
        let g = Junqi;
        let mut board = HashMap::new();
        board.insert(
            "c6".into(),
            Piece {
                owner: 0,
                name: "炸弹".into(),
            },
        );
        board.insert(
            "c7".into(),
            Piece {
                owner: 1,
                name: "司令".into(),
            },
        );
        board.insert(
            "a1".into(),
            Piece {
                owner: 0,
                name: "军旗".into(),
            },
        );
        board.insert(
            "a12".into(),
            Piece {
                owner: 1,
                name: "军旗".into(),
            },
        );
        let s = JState {
            phase: "play".into(),
            board,
            deployed: vec![true, true],
            turn: 0,
            last: None,
            winner: None,
            reason: None,
        };
        let state = serde_json::to_value(s).unwrap();
        let view = g
            .view(
                &state,
                &Viewer {
                    seat: Some(0),
                    omniscient: false,
                },
            )
            .unwrap();
        let cells = view["cells"].as_array().unwrap();
        let bomb = cells.iter().find(|c| c["at"] == "c6").unwrap();
        let enemy = cells.iter().find(|c| c["at"] == "c7").unwrap();
        assert_eq!(bomb["piece"]["name"], "炸弹");
        assert_eq!(enemy["piece"]["name"], "暗");
        let mut rng = crate::game::SeededRng::from_seed_seq(&[2; 32], 0);
        let next = g
            .apply(
                &state,
                0,
                &json!({"type":"move","data":{"from":"c6","to":"c7"}}),
                &mut rng,
            )
            .unwrap()
            .0;
        assert_eq!(next["last"]["result"], "both");
        assert!(!Junqi::load(&next).unwrap().board.contains_key("c7"));
    }
}
