//! 围棋。中国规则数子（Tromp-Taylor 面积计分），位置超劫。
//!
//! 变体 `9`（默认，贴 7 目）、`13`、`19`（贴 7.5 目）。
//! 列用 A–T 且跳过 I，行从下往上数。连续两手停着进入数子。
//! 超时停着；连续超时达到房间上限后由房间改判负。

use crate::game::*;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Clone, Debug, Serialize, Deserialize)]
struct GState {
    size: usize,
    komi: f64,
    board: Vec<u8>,
    turn: u8,
    #[serde(default)]
    passes: u8,
    #[serde(default)]
    history: Vec<String>,
    #[serde(default)]
    last: Option<String>,
    winner: Option<i8>,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    score: Option<(f64, f64)>,
}

pub struct Go;

impl Go {
    fn load(state: &Value) -> Result<GState, GameError> {
        serde_json::from_value(state.clone()).map_err(|e| GameError(e.to_string()))
    }

    fn new(size: usize) -> GState {
        let komi = if size == 9 { 7.0 } else { 7.5 };
        let board = vec![0u8; size * size];
        let history = vec![key(&board)];
        GState { size, komi, board, turn: 0, passes: 0, history, last: None, winner: None, reason: None, score: None }
    }
}

fn key(board: &[u8]) -> String {
    board.iter().map(|c| char::from(b'0' + *c)).collect()
}

fn columns(size: usize) -> Vec<char> {
    ('A'..='Z').filter(|c| *c != 'I').take(size).collect()
}

fn at_name(size: usize, x: usize, y: usize) -> String {
    format!("{}{}", columns(size)[x], y + 1)
}

fn parse_at(size: usize, s: &str) -> Option<(usize, usize)> {
    let s = s.trim().to_uppercase();
    let mut chars = s.chars();
    let col = chars.next()?;
    let row: usize = chars.as_str().parse().ok()?;
    if row == 0 || row > size {
        return None;
    }
    let x = columns(size).iter().position(|c| *c == col)?;
    Some((x, row - 1))
}

fn i_of(size: usize, x: usize, y: usize) -> usize {
    y * size + x
}

fn neighbors(size: usize, x: usize, y: usize) -> Vec<(usize, usize)> {
    let mut v = Vec::new();
    if x > 0 { v.push((x - 1, y)); }
    if y > 0 { v.push((x, y - 1)); }
    if x + 1 < size { v.push((x + 1, y)); }
    if y + 1 < size { v.push((x, y + 1)); }
    v
}

fn group_and_libs(board: &[u8], size: usize, x: usize, y: usize) -> (Vec<(usize, usize)>, usize) {
    let color = board[i_of(size, x, y)];
    let mut stack = vec![(x, y)];
    let mut seen = vec![false; board.len()];
    seen[i_of(size, x, y)] = true;
    let mut group = Vec::new();
    let mut libs = std::collections::HashSet::new();
    while let Some((cx, cy)) = stack.pop() {
        group.push((cx, cy));
        for (nx, ny) in neighbors(size, cx, cy) {
            let ni = i_of(size, nx, ny);
            match board[ni] {
                0 => { libs.insert(ni); }
                c if c == color && !seen[ni] => {
                    seen[ni] = true;
                    stack.push((nx, ny));
                }
                _ => {}
            }
        }
    }
    (group, libs.len())
}

fn place(board: &mut [u8], size: usize, x: usize, y: usize, color: u8, history: &[String]) -> Result<(), &'static str> {
    if board[i_of(size, x, y)] != 0 {
        return Err("这里已有棋子");
    }
    board[i_of(size, x, y)] = color;
    let enemy = 3 - color;
    let mut captured = Vec::new();
    for (nx, ny) in neighbors(size, x, y) {
        if board[i_of(size, nx, ny)] == enemy {
            let (group, libs) = group_and_libs(board, size, nx, ny);
            if libs == 0 {
                captured.extend(group);
            }
        }
    }
    captured.sort();
    captured.dedup();
    for (cx, cy) in &captured {
        board[i_of(size, *cx, *cy)] = 0;
    }
    let (_, libs) = group_and_libs(board, size, x, y);
    if libs == 0 {
        return Err("不能自杀");
    }
    let next = key(board);
    if history.iter().any(|h| h == &next) {
        return Err("劫争或局面重复");
    }
    Ok(())
}

fn score_area(board: &[u8], size: usize, komi: f64) -> (f64, f64) {
    let mut black = 0.0;
    let mut white = 0.0;
    let mut seen = vec![false; board.len()];
    for y in 0..size {
        for x in 0..size {
            let c = board[i_of(size, x, y)];
            if c == 1 { black += 1.0; }
            if c == 2 { white += 1.0; }
            if c != 0 || seen[i_of(size, x, y)] { continue; }
            let mut stack = vec![(x, y)];
            seen[i_of(size, x, y)] = true;
            let mut cells = 0.0;
            let mut touch = [false, false];
            while let Some((cx, cy)) = stack.pop() {
                cells += 1.0;
                for (nx, ny) in neighbors(size, cx, cy) {
                    let ni = i_of(size, nx, ny);
                    match board[ni] {
                        0 if !seen[ni] => { seen[ni] = true; stack.push((nx, ny)); }
                        1 => touch[0] = true,
                        2 => touch[1] = true,
                        _ => {}
                    }
                }
            }
            if touch[0] && !touch[1] { black += cells; }
            if touch[1] && !touch[0] { white += cells; }
        }
    }
    white += komi;
    (black, white)
}

fn finish_score(s: &mut GState) {
    let (b, w) = score_area(&s.board, s.size, s.komi);
    s.score = Some((b, w));
    if (b - w).abs() < 1e-6 {
        s.winner = Some(-1);
        s.reason = Some(format!("draw {b}:{w}"));
    } else if b > w {
        s.winner = Some(0);
        s.reason = Some(format!("black {b}:{w}"));
    } else {
        s.winner = Some(1);
        s.reason = Some(format!("white {b}:{w}"));
    }
}

impl Game for Go {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "go".into(),
            name: "围棋".into(),
            version: "1.0.0".into(),
            min_players: 2,
            max_players: 2,
            traits: vec![Trait::Board, Trait::MultiPhase],
            variants: vec![
                Variant { id: "9".into(), name: "9 路 · 贴 7 目".into(), options: None },
                Variant { id: "13".into(), name: "13 路 · 贴 7.5 目".into(), options: None },
                Variant { id: "19".into(), name: "19 路 · 贴 7.5 目".into(), options: None },
            ],
            default_time: TimeControl { per_action_ms: 60_000, max_consecutive_timeouts: 3, grace_ms: 2_000 },
            timeout_policy: TimeoutPolicy::AutoPass,
            allowed_timeout_policies: vec![TimeoutPolicy::AutoPass, TimeoutPolicy::Forfeit],
            action_schema: json!({"type":"object"}),
            rules_markdown: GO_RULES.into(),
            renderer: "grid-board".into(),
            rated: true,
            engine: "native".into(),
            description: "中国规则数子。9 / 13 / 19 路，位置超劫，连续停着终局。".into(),
            playable: true,
        }
    }

    fn setup(&self, cfg: &Config, _: &mut dyn ArenaRng) -> Result<(Value, Vec<Value>), GameError> {
        let size = match cfg.variant.as_str() {
            "9" => 9,
            "13" => 13,
            "19" => 19,
            _ => return Err(GameError("未知路数".into())),
        };
        Ok((serde_json::to_value(Self::new(size)).unwrap(), Vec::new()))
    }

    fn to_act(&self, state: &Value) -> Vec<Seat> {
        let Ok(s) = Self::load(state) else { return Vec::new() };
        if s.winner.is_some() { Vec::new() } else { vec![s.turn as Seat] }
    }

    fn view(&self, state: &Value, _: &Viewer) -> Result<Value, GameError> {
        let s = Self::load(state)?;
        let mut points = Vec::new();
        for y in 0..s.size {
            for x in 0..s.size {
                let c = s.board[i_of(s.size, x, y)];
                if c != 0 {
                    points.push(json!({"at": at_name(s.size, x, y), "seat": c - 1}));
                }
            }
        }
        Ok(json!({
            "renderer": "go",
            "size": s.size,
            "komi": s.komi,
            "points": points,
            "lastMove": s.last,
            "passes": s.passes,
            "toAct": self.to_act(state),
            "winner": s.winner,
            "reason": s.reason,
            "score": s.score,
        }))
    }

    fn legal_actions(&self, state: &Value, seat: Seat) -> Result<Vec<Value>, GameError> {
        let s = Self::load(state)?;
        if s.winner.is_some() {
            return Ok(Vec::new());
        }
        let mut actions = vec![json!({"type":"resign"})];
        if seat as u8 != s.turn {
            return Ok(actions);
        }
        actions.push(json!({"type":"pass"}));
        for y in 0..s.size {
            for x in 0..s.size {
                let mut board = s.board.clone();
                if place(&mut board, s.size, x, y, s.turn + 1, &s.history).is_ok() {
                    actions.push(json!({"type":"place","data":{"at": at_name(s.size, x, y)}}));
                }
            }
        }
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
            s.reason = Some(if kind == "resign" { "resign".into() } else { "timeout".into() });
            return Ok((serde_json::to_value(s).unwrap(), Vec::new()));
        }
        if seat as u8 != s.turn {
            return Err(GameError("还没轮到你".into()));
        }
        if kind == "pass" {
            s.passes += 1;
            s.last = Some("pass".into());
            s.turn = 1 - s.turn;
            if s.passes >= 2 {
                finish_score(&mut s);
            }
            return Ok((serde_json::to_value(s).unwrap(), Vec::new()));
        }
        if kind != "place" {
            return Err(GameError("未知动作".into()));
        }
        let at = action["data"]["at"].as_str().ok_or_else(|| GameError("缺少坐标".into()))?;
        let (x, y) = parse_at(s.size, at).ok_or_else(|| GameError("坐标不合法".into()))?;
        place(&mut s.board, s.size, x, y, s.turn + 1, &s.history).map_err(|m| GameError(m.into()))?;
        s.history.push(key(&s.board));
        s.passes = 0;
        s.last = Some(at_name(s.size, x, y));
        s.turn = 1 - s.turn;
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
        Ok(GameResult { scores, ranking, teams: None, reason: s.reason.unwrap_or_else(|| "score".into()) })
    }

    fn timeout_decision(&self, _: &Value, _: Seat, policy: TimeoutPolicy, _: &mut dyn ArenaRng) -> Result<TimeoutDecision, GameError> {
        if policy == TimeoutPolicy::Forfeit {
            return Ok(TimeoutDecision { policy, action: Some(json!({"type":"resign"})) });
        }
        Ok(TimeoutDecision { policy: TimeoutPolicy::AutoPass, action: Some(json!({"type":"pass"})) })
    }
}

const GO_RULES: &str = r#"# 围棋

黑先。9 路贴 7 目，13 路和 19 路贴 7.5 目。坐标列跳过 I，行从下往上。

- 提掉没气的对方棋块。自杀不合法。位置超劫：不能下出以前出现过的棋盘。
- 连续两手停着，按中国规则数子（面积）终局。单官不计。
- 超时自动停着。连续超时达到上限后判负。
"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(v: &str) -> Config {
        Config { players: 2, variant: v.into(), options: json!({}) }
    }

    #[test]
    fn capture_ko_and_area_score() {
        let g = Go;
        let mut rng = crate::game::SeededRng::from_seed_seq(&[8; 32], 0);
        let (mut state, _) = g.setup(&cfg("9"), &mut rng).unwrap();
        let plays = [(0i32, "D5"), (1, "D4"), (0, "C4"), (1, "pass"), (0, "E4"), (1, "pass"), (0, "D3")];
        for (seat, at) in plays {
            let action = if at == "pass" { json!({"type":"pass"}) } else { json!({"type":"place","data":{"at": at}}) };
            // passes switch turn; the scripted seats must match.
            let s = Go::load(&state).unwrap();
            assert_eq!(s.turn, seat as u8, "turn before {at}");
            state = g.apply(&state, seat, &action, &mut rng).unwrap().0;
        }
        let s = Go::load(&state).unwrap();
        assert!(s.board.iter().all(|c| *c != 2), "white stone should be captured");

        // Ko: retake immediately is illegal.
        let mut board = vec![0u8; 81];
        // classic ko shape on 9x9 around D4 / D5
        let set = |b: &mut [u8], at: &str, c: u8| {
            let (x, y) = parse_at(9, at).unwrap();
            b[i_of(9, x, y)] = c;
        };
        set(&mut board, "D4", 1);
        set(&mut board, "C5", 1);
        set(&mut board, "E5", 1);
        set(&mut board, "D6", 1);
        set(&mut board, "C4", 2);
        set(&mut board, "E4", 2);
        set(&mut board, "D3", 2);
        // white to play D5 captures D4
        let mut history = vec![key(&board)];
        place(&mut board, 9, parse_at(9, "D5").unwrap().0, parse_at(9, "D5").unwrap().1, 2, &history).unwrap();
        history.push(key(&board));
        let (x, y) = parse_at(9, "D4").unwrap();
        let err = place(&mut board, 9, x, y, 1, &history);
        assert!(err.is_err(), "positional superko blocks the immediate retake");

        let g = Go;
        let mut rng = crate::game::SeededRng::from_seed_seq(&[1; 32], 0);
        let (mut state, _) = g.setup(&cfg("9"), &mut rng).unwrap();
        state = g.apply(&state, 0, &json!({"type":"place","data":{"at":"D4"}}), &mut rng).unwrap().0;
        state = g.apply(&state, 1, &json!({"type":"pass"}), &mut rng).unwrap().0;
        state = g.apply(&state, 0, &json!({"type":"pass"}), &mut rng).unwrap().0;
        assert!(g.is_terminal(&state));
        let result = g.result(&state).unwrap();
        assert_eq!(result.reason.starts_with("black") || result.reason.starts_with("white") || result.reason.starts_with("draw"), true);
        let s = Go::load(&state).unwrap();
        let (b, w) = s.score.unwrap();
        assert!(b > 1.0);
        assert!((w - 7.0).abs() < 1e-6 || w > 7.0);
    }
}
