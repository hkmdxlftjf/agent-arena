//! 中国象棋。红方席位 0 在下方，黑方席位 1 在上方。
//!
//! 坐标用 ICCS：纵线 `a`–`i` 从左到右（红方视角），横线 `1`–`10` 从下到上。
//! 动作 `{type:"move", data:{from, to}}` 或 `{type:"resign"}`。
//!
//! 和棋：同一（棋盘 + 行棋方）出现第三次时，若刚走子的一方正在将军则判负，否则和棋。
//! 连续 60 个回合（120 个半回合）没有吃子也和棋。困毙视同败。

use crate::game::*;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const KINDS: [&str; 8] = ["", "帅", "仕", "相", "马", "车", "炮", "兵"];
const BLACK_KINDS: [&str; 8] = ["", "将", "士", "象", "马", "车", "炮", "卒"];

#[derive(Clone, Debug, Serialize, Deserialize)]
struct XState {
    board: Vec<i8>,
    turn: u8,
    #[serde(default)]
    last: Option<(String, String)>,
    #[serde(default)]
    history: Vec<String>,
    #[serde(default)]
    quiet: u16,
    winner: Option<i8>,
    #[serde(default)]
    reason: Option<String>,
}

pub struct Xiangqi;

impl Xiangqi {
    fn load(state: &Value) -> Result<XState, GameError> {
        serde_json::from_value(state.clone()).map_err(|e| GameError(e.to_string()))
    }

    fn initial() -> XState {
        let mut b = vec![0i8; 90];
        let red = [5, 4, 3, 2, 1, 2, 3, 4, 5];
        for (f, k) in red.iter().enumerate() {
            b[idx(f, 0)] = *k;
            b[idx(f, 9)] = -*k;
        }
        b[idx(1, 2)] = 6;
        b[idx(7, 2)] = 6;
        b[idx(1, 7)] = -6;
        b[idx(7, 7)] = -6;
        for f in [0, 2, 4, 6, 8] {
            b[idx(f, 3)] = 7;
            b[idx(f, 6)] = -7;
        }
        let mut s = XState {
            board: b,
            turn: 0,
            last: None,
            history: Vec::new(),
            quiet: 0,
            winner: None,
            reason: None,
        };
        s.history.push(pos_key(&s.board, s.turn));
        s
    }
}

fn idx(f: usize, r: usize) -> usize {
    r * 9 + f
}

fn pos_key(board: &[i8], turn: u8) -> String {
    format!("{turn}:{}", board.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(","))
}

fn at_name(f: usize, r: usize) -> String {
    format!("{}{}", (b'a' + f as u8) as char, r + 1)
}

fn parse_at(s: &str) -> Option<(usize, usize)> {
    let b = s.as_bytes();
    if b.len() < 2 {
        return None;
    }
    let f = (b[0].to_ascii_lowercase() - b'a') as usize;
    let r: usize = s[1..].parse().ok()?;
    if f < 9 && (1..=10).contains(&r) {
        Some((f, r - 1))
    } else {
        None
    }
}

fn on(f: i32, r: i32) -> bool {
    (0..9).contains(&f) && (0..10).contains(&r)
}

fn palace(side: i8, f: i32, r: i32) -> bool {
    (3..=5).contains(&f) && if side == 0 { (0..=2).contains(&r) } else { (7..=9).contains(&r) }
}

fn side_of(p: i8) -> i8 {
    if p > 0 { 0 } else { 1 }
}

fn piece_moves(board: &[i8], f: usize, r: usize) -> Vec<(usize, usize)> {
    let p = board[idx(f, r)];
    if p == 0 {
        return Vec::new();
    }
    let side = side_of(p);
    let kind = p.abs();
    let mut out = Vec::new();
    let add = |out: &mut Vec<_>, board: &[i8], nf: i32, nr: i32| {
        if !on(nf, nr) {
            return;
        }
        let t = board[idx(nf as usize, nr as usize)];
        if t == 0 || side_of(t) != side {
            out.push((nf as usize, nr as usize));
        }
    };
    match kind {
        1 => {
            for (df, dr) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let nf = f as i32 + df;
                let nr = r as i32 + dr;
                if palace(side, nf, nr) {
                    add(&mut out, board, nf, nr);
                }
            }
        }
        2 => {
            for (df, dr) in [(1, 1), (1, -1), (-1, 1), (-1, -1)] {
                let nf = f as i32 + df;
                let nr = r as i32 + dr;
                if palace(side, nf, nr) {
                    add(&mut out, board, nf, nr);
                }
            }
        }
        3 => {
            for (df, dr) in [(2, 2), (2, -2), (-2, 2), (-2, -2)] {
                let nf = f as i32 + df;
                let nr = r as i32 + dr;
                let eye_f = f as i32 + df / 2;
                let eye_r = r as i32 + dr / 2;
                let own_side = if side == 0 { nr <= 4 } else { nr >= 5 };
                if own_side && on(nf, nr) && board[idx(eye_f as usize, eye_r as usize)] == 0 {
                    add(&mut out, board, nf, nr);
                }
            }
        }
        4 => {
            let legs = [(1, 0, 2, 1), (1, 0, 2, -1), (-1, 0, -2, 1), (-1, 0, -2, -1), (0, 1, 1, 2), (0, 1, -1, 2), (0, -1, 1, -2), (0, -1, -1, -2)];
            for (lf, lr, df, dr) in legs {
                let ef = f as i32 + lf;
                let er = r as i32 + lr;
                if on(ef, er) && board[idx(ef as usize, er as usize)] == 0 {
                    add(&mut out, board, f as i32 + df, r as i32 + dr);
                }
            }
        }
        5 | 6 => {
            for (df, dr) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let mut screen = 0;
                let mut nf = f as i32 + df;
                let mut nr = r as i32 + dr;
                while on(nf, nr) {
                    let t = board[idx(nf as usize, nr as usize)];
                    if kind == 5 {
                        if t == 0 {
                            out.push((nf as usize, nr as usize));
                        } else {
                            if side_of(t) != side {
                                out.push((nf as usize, nr as usize));
                            }
                            break;
                        }
                    } else if t == 0 {
                        if screen == 0 {
                            out.push((nf as usize, nr as usize));
                        }
                    } else {
                        screen += 1;
                        if screen == 2 {
                            if side_of(t) != side {
                                out.push((nf as usize, nr as usize));
                            }
                            break;
                        }
                    }
                    nf += df;
                    nr += dr;
                }
            }
        }
        7 => {
            let forward = if side == 0 { 1 } else { -1 };
            add(&mut out, board, f as i32, r as i32 + forward);
            let crossed = if side == 0 { r >= 5 } else { r <= 4 };
            if crossed {
                add(&mut out, board, f as i32 + 1, r as i32);
                add(&mut out, board, f as i32 - 1, r as i32);
            }
        }
        _ => {}
    }
    out
}

fn kings_face(board: &[i8]) -> bool {
    let mut rk = None;
    let mut bk = None;
    for r in 0..10 {
        for f in 0..9 {
            let p = board[idx(f, r)];
            if p == 1 {
                rk = Some((f, r));
            } else if p == -1 {
                bk = Some((f, r));
            }
        }
    }
    let (Some((rf, rr)), Some((bf, br))) = (rk, bk) else {
        return false;
    };
    if rf != bf {
        return false;
    }
    ((rr + 1)..br).all(|r| board[idx(rf, r)] == 0)
}

fn attacked(board: &[i8], target_side: i8) -> bool {
    if kings_face(board) {
        return true;
    }
    let king = if target_side == 0 { 1 } else { -1 };
    let mut tf = 0;
    let mut tr = 0;
    let mut found = false;
    for r in 0..10 {
        for f in 0..9 {
            if board[idx(f, r)] == king {
                tf = f;
                tr = r;
                found = true;
            }
        }
    }
    if !found {
        return true;
    }
    for r in 0..10 {
        for f in 0..9 {
            let p = board[idx(f, r)];
            if p != 0 && side_of(p) != target_side && piece_moves(board, f, r).contains(&(tf, tr)) {
                return true;
            }
        }
    }
    false
}

fn piece_destinations(board: &[i8], side: u8) -> Vec<(usize, usize, usize, usize)> {
    let mut out = Vec::new();
    for r in 0..10 {
        for f in 0..9 {
            let p = board[idx(f, r)];
            if p == 0 || side_of(p) != side as i8 {
                continue;
            }
            for (nf, nr) in piece_moves(board, f, r) {
                let mut next = board.to_vec();
                next[idx(nf, nr)] = next[idx(f, r)];
                next[idx(f, r)] = 0;
                if !attacked(&next, side as i8) {
                    out.push((f, r, nf, nr));
                }
            }
        }
    }
    out
}

fn move_action(f: usize, r: usize, nf: usize, nr: usize) -> Value {
    json!({"type":"move","data":{"from": at_name(f, r), "to": at_name(nf, nr)}})
}

fn label(p: i8) -> String {
    let k = p.abs() as usize;
    if p > 0 { KINDS[k].into() } else { BLACK_KINDS[k].into() }
}

impl Game for Xiangqi {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "xiangqi".into(),
            name: "中国象棋".into(),
            version: "1.0.0".into(),
            min_players: 2,
            max_players: 2,
            traits: vec![Trait::Board],
            variants: vec![Variant { id: "standard".into(), name: "标准".into(), options: None }],
            default_time: TimeControl { per_action_ms: 60_000, max_consecutive_timeouts: 3, grace_ms: 2_000 },
            timeout_policy: TimeoutPolicy::Forfeit,
            allowed_timeout_policies: vec![TimeoutPolicy::Forfeit],
            action_schema: json!({"type":"object"}),
            rules_markdown: XIANGQI_RULES.into(),
            renderer: "xiangqi".into(),
            rated: true,
            engine: "native".into(),
            description: "标准 9×10，九宫、河界、飞将、长将判负，60 回合无吃子和棋。".into(),
            playable: true,
        }
    }

    fn setup(&self, cfg: &Config, _: &mut dyn ArenaRng) -> Result<(Value, Vec<Value>), GameError> {
        if cfg.variant != "standard" {
            return Err(GameError("未知变体".into()));
        }
        Ok((serde_json::to_value(Self::initial()).unwrap(), Vec::new()))
    }

    fn to_act(&self, state: &Value) -> Vec<Seat> {
        let Ok(s) = Self::load(state) else { return Vec::new() };
        if s.winner.is_some() { Vec::new() } else { vec![s.turn as Seat] }
    }

    fn view(&self, state: &Value, _: &Viewer) -> Result<Value, GameError> {
        let s = Self::load(state)?;
        let mut pieces = Vec::new();
        for r in 0..10 {
            for f in 0..9 {
                let p = s.board[idx(f, r)];
                if p != 0 {
                    pieces.push(json!({
                        "at": at_name(f, r),
                        "side": side_of(p),
                        "kind": p.abs(),
                        "label": label(p),
                    }));
                }
            }
        }
        let check = s.winner.is_none() && attacked(&s.board, s.turn as i8);
        Ok(json!({
            "renderer": "xiangqi",
            "pieces": pieces,
            "lastMove": s.last.as_ref().map(|(a, b)| json!({"from": a, "to": b})),
            "check": check,
            "toAct": self.to_act(state),
            "winner": s.winner,
            "reason": s.reason,
            "quiet": s.quiet,
        }))
    }

    fn legal_actions(&self, state: &Value, seat: Seat) -> Result<Vec<Value>, GameError> {
        let s = Self::load(state)?;
        if s.winner.is_some() {
            return Ok(Vec::new());
        }
        let mut actions = Vec::new();
        if seat as u8 == s.turn {
            for (f, r, nf, nr) in piece_destinations(&s.board, s.turn) {
                actions.push(move_action(f, r, nf, nr));
            }
        }
        actions.push(json!({"type":"resign"}));
        Ok(actions)
    }

    fn apply(&self, state: &Value, seat: Seat, action: &Value, _: &mut dyn ArenaRng) -> Result<(Value, Vec<Value>), GameError> {
        let mut s = Self::load(state)?;
        if s.winner.is_some() {
            return Err(GameError("对局已结束".into()));
        }
        let kind = action.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if kind == "resign" || kind == "timeout_forfeit" {
            s.winner = Some(1 - seat as i8);
            s.reason = Some(if kind == "resign" { "resign" } else { "timeout" }.into());
            return Ok((serde_json::to_value(s).unwrap(), Vec::new()));
        }
        if seat as u8 != s.turn {
            return Err(GameError("还没轮到你".into()));
        }
        if kind != "move" {
            return Err(GameError("未知动作".into()));
        }
        let from = action["data"]["from"].as_str().and_then(parse_at).ok_or_else(|| GameError("from 不合法".into()))?;
        let to = action["data"]["to"].as_str().and_then(parse_at).ok_or_else(|| GameError("to 不合法".into()))?;
        let legal = piece_destinations(&s.board, s.turn);
        if !legal.iter().any(|m| (m.0, m.1, m.2, m.3) == (from.0, from.1, to.0, to.1)) {
            return Err(GameError("这步棋不合法".into()));
        }
        let captured = s.board[idx(to.0, to.1)] != 0;
        let moving = s.board[idx(from.0, from.1)];
        s.board[idx(to.0, to.1)] = moving;
        s.board[idx(from.0, from.1)] = 0;
        s.last = Some((at_name(from.0, from.1), at_name(to.0, to.1)));
        s.quiet = if captured { 0 } else { s.quiet + 1 };
        s.turn = 1 - s.turn;
        let key = pos_key(&s.board, s.turn);
        let seen = s.history.iter().filter(|k| *k == &key).count() + 1;
        s.history.push(key);
        if seen >= 3 {
            if attacked(&s.board, s.turn as i8) {
                s.winner = Some(s.turn as i8);
                s.reason = Some("perpetual_check".into());
            } else {
                s.winner = Some(-1);
                s.reason = Some("repetition".into());
            }
        } else if s.quiet >= 120 {
            s.winner = Some(-1);
            s.reason = Some("sixty_move".into());
        } else if piece_destinations(&s.board, s.turn).is_empty() {
            s.winner = Some(1 - s.turn as i8);
            s.reason = Some(if attacked(&s.board, s.turn as i8) { "checkmate" } else { "stalemate" }.into());
        }
        Ok((serde_json::to_value(s).unwrap(), Vec::new()))
    }

    fn is_terminal(&self, state: &Value) -> bool {
        Self::load(state).map(|s| s.winner.is_some()).unwrap_or(true)
    }

    fn result(&self, state: &Value) -> Result<GameResult, GameError> {
        let s = Self::load(state)?;
        let winner = s.winner.ok_or_else(|| GameError("对局还没结束".into()))?;
        let (scores, ranking) = match winner {
            0 => (vec![1.0, 0.0], vec![vec![0], vec![1]]),
            1 => (vec![0.0, 1.0], vec![vec![1], vec![0]]),
            _ => (vec![0.5, 0.5], vec![vec![0, 1]]),
        };
        Ok(GameResult {
            scores,
            ranking,
            teams: None,
            reason: s.reason.unwrap_or_else(|| "xiangqi".into()),
        })
    }

    fn timeout_decision(&self, _: &Value, _: Seat, policy: TimeoutPolicy, _: &mut dyn ArenaRng) -> Result<TimeoutDecision, GameError> {
        Ok(TimeoutDecision {
            policy: TimeoutPolicy::Forfeit,
            action: Some(json!({"type":"timeout_forfeit"})),
        })
        .map(|mut d| {
            d.policy = if policy == TimeoutPolicy::Forfeit { policy } else { TimeoutPolicy::Forfeit };
            d
        })
    }
}

const XIANGQI_RULES: &str = r#"# 中国象棋

红方（席位 0）在下，黑方在上。纵线 a–i，横线 1–10。

- 帅/将、仕/士只在九宫内；相/象不能过河，塞象眼；马有蹩腿；炮隔一子吃子；兵/卒过河才能横走。
- 将帅不能在无子阻隔的同一纵线上对脸。
- 同一局面（含行棋方）第三次出现：若刚走的一方正在将军，判负，否则和棋。
- 连续 60 回合没有吃子，和棋。无子可走判负。
- 超时判负。
"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn play(mut state: Value, seat: i32, from: &str, to: &str) -> Value {
        let g = Xiangqi;
        let mut rng = crate::game::SeededRng::from_seed_seq(&[0; 32], 0);
        let action = json!({"type":"move","data":{"from": from, "to": to}});
        let (next, _) = g.apply(&state, seat, &action, &mut rng).unwrap();
        state = next;
        state
    }

    #[test]
    fn opening_horse_and_illegal_are_distinguished() {
        let g = Xiangqi;
        let mut rng = crate::game::SeededRng::from_seed_seq(&[1; 32], 0);
        let (state, _) = g.setup(&Config { players: 2, variant: "standard".into(), options: json!({}) }, &mut rng).unwrap();
        let legal = g.legal_actions(&state, 0).unwrap();
        assert!(legal.iter().any(|a| a["data"]["from"] == "b1" && a["data"]["to"] == "c3"));
        let mut rng = crate::game::SeededRng::from_seed_seq(&[1; 32], 1);
        let err = g.apply(&state, 0, &json!({"type":"move","data":{"from":"b1","to":"b2"}}), &mut rng);
        assert!(err.is_err());
    }

    #[test]
    fn cannon_needs_a_screen_and_pawn_crosses() {
        let g = Xiangqi;
        let mut rng = crate::game::SeededRng::from_seed_seq(&[2; 32], 0);
        let (state, _) = g.setup(&Config { players: 2, variant: "standard".into(), options: json!({}) }, &mut rng).unwrap();
        let mut rng = crate::game::SeededRng::from_seed_seq(&[2; 32], 1);
        assert!(g.apply(&state, 0, &json!({"type":"move","data":{"from":"b3","to":"b8"}}), &mut rng).is_err());
        let state = play(state, 0, "b3", "b10");
        let s = Xiangqi::load(&state).unwrap();
        assert_eq!(s.board[idx(1, 9)], 6, "cannon captures the horse over the opposing cannon");
        let mut rng = crate::game::SeededRng::from_seed_seq(&[2; 32], 2);
        let (state, _) = g.setup(&Config { players: 2, variant: "standard".into(), options: json!({}) }, &mut rng).unwrap();
        let state = play(state, 0, "a4", "a5");
        let s = Xiangqi::load(&state).unwrap();
        assert_eq!(s.board[idx(0, 4)], 7);
        assert_eq!(s.turn, 1);
    }

    #[test]
    fn flying_general_is_illegal_and_mate_is_detected() {
        let mut board = vec![0i8; 90];
        board[idx(4, 0)] = 1;
        board[idx(4, 9)] = -1;
        board[idx(4, 8)] = -2;
        let s = XState { board, turn: 1, last: None, history: vec!["x".into()], quiet: 0, winner: None, reason: None };
        let state = serde_json::to_value(&s).unwrap();
        let g = Xiangqi;
        let mut rng = crate::game::SeededRng::from_seed_seq(&[3; 32], 0);
        let err = g.apply(&state, 1, &json!({"type":"move","data":{"from":"e9","to":"d8"}}), &mut rng);
        assert!(err.is_err(), "{err:?}");

        let mut board = vec![0i8; 90];
        board[idx(4, 0)] = 1;
        board[idx(4, 1)] = -5;
        board[idx(3, 1)] = -7;
        board[idx(5, 1)] = -7;
        board[idx(3, 9)] = -1;
        let s = XState { board, turn: 0, last: None, history: vec!["m".into()], quiet: 0, winner: None, reason: None };
        let state = serde_json::to_value(s).unwrap();
        let moves = piece_destinations(&Xiangqi::load(&state).unwrap().board, 0);
        assert!(moves.is_empty(), "expected mate, got {moves:?}");
    }

    #[test]
    fn resign_scores_the_opponent() {
        let g = Xiangqi;
        let mut rng = crate::game::SeededRng::from_seed_seq(&[4; 32], 0);
        let (state, _) = g.setup(&Config { players: 2, variant: "standard".into(), options: json!({}) }, &mut rng).unwrap();
        let (next, _) = g.apply(&state, 0, &json!({"type":"resign"}), &mut rng).unwrap();
        let result = g.result(&next).unwrap();
        assert_eq!(result.scores, vec![0.0, 1.0]);
        assert_eq!(result.reason, "resign");
    }
}
