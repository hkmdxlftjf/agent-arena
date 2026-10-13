//! 五子棋, 15×15.
//!
//! Files `a`–`o` run left to right, ranks `1`–`15` run bottom to top.
//! The center star point is `h8`. Seat 0 plays black and moves first.
//!
//! Variant `free` (default): five or more in a row wins, no forbidden points.
//! Variant `renju`: black may not play an overline (6+), two open threes, or
//! two fours (straight runs only; jump threes/fours are not detected). Black
//! wins with exactly five. White wins with five or more.

use crate::game::*;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const N: usize = 15;

pub struct Gomoku;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GState {
    size: u8,
    board: Vec<u8>,
    turn: u8,
    #[serde(default)]
    last: Option<String>,
    winner: Option<i8>,
    #[serde(default)]
    win_line: Vec<String>,
    variant: String,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    moves: u32,
}

impl Gomoku {
    fn manifest_inner() -> Manifest {
        Manifest {
            id: "gomoku".into(),
            name: "五子棋".into(),
            version: "1.0.0".into(),
            min_players: 2,
            max_players: 2,
            traits: vec![Trait::Board],
            variants: vec![
                Variant {
                    id: "free".into(),
                    name: "无禁手".into(),
                    options: None,
                },
                Variant {
                    id: "renju".into(),
                    name: "连珠".into(),
                    options: None,
                },
            ],
            default_time: TimeControl {
                per_action_ms: 60_000,
                max_consecutive_timeouts: 3,
                grace_ms: 2_000,
            },
            timeout_policy: TimeoutPolicy::Forfeit,
            allowed_timeout_policies: vec![TimeoutPolicy::Forfeit, TimeoutPolicy::AutoAction],
            action_schema: json!({
                "type": "object",
                "required": ["type", "data"],
                "properties": {
                    "type": {"const": "place"},
                    "data": {
                        "type": "object",
                        "required": ["at"],
                        "properties": {"at": {"type": "string", "pattern": "^[a-oA-O](?:[1-9]|1[0-5])$"}}
                    }
                }
            }),
            rules_markdown: GOMOKU_RULES.into(),
            renderer: "grid-board".into(),
            rated: true,
            engine: "native".into(),
            description: "15×15，黑先。五子连珠即胜。默认无禁手，可选连珠禁手。".into(),
            playable: true,
        }
    }
}

const GOMOKU_RULES: &str = r#"# 五子棋

15×15 的交叉点棋盘。黑方（席位 0）先行，双方轮流落一子。率先在横、竖、斜任意方向形成连续五子（无禁手变体中，五子及以上）者获胜。棋盘下满无人达成则为和棋。

## 坐标

列 `a`–`o` 从左到右，行 `1`–`15` 从下到上。天元是 `h8`。

## 动作

```json
{"type":"place","data":{"at":"h8"}}
```

## 计时

默认每步 60 秒，宽限 2 秒。默认超时判负。休闲房可以把超时策略改成 `auto_action`：在离最近一手最近的空点落子（用本局种子决定，可复现）。

## 变体

- `free`（默认，无禁手）：任意方向连续五子或更长即获胜。
- `renju`（连珠）：黑方禁止长连（六子及以上）、双活三、双四（按直线连续子计算）。黑方恰好五子获胜；白方五子及以上获胜。
"#;

impl Game for Gomoku {
    fn manifest(&self) -> Manifest {
        Self::manifest_inner()
    }

    fn setup(&self, cfg: &Config, _: &mut dyn ArenaRng) -> Result<(Value, Vec<Value>), GameError> {
        if cfg.players != 2 {
            return Err(GameError("五子棋需要 2 人".into()));
        }
        let variant = if cfg.variant.is_empty() {
            "free"
        } else {
            cfg.variant.as_str()
        };
        if variant != "free" && variant != "renju" {
            return Err(GameError(format!("未知变体 {variant}")));
        }
        let state = GState {
            size: 15,
            board: vec![0; N * N],
            turn: 0,
            last: None,
            winner: None,
            win_line: Vec::new(),
            variant: variant.into(),
            reason: None,
            moves: 0,
        };
        Ok((
            to_value(&state),
            vec![json!({"type":"start","visibility":{"public":true}})],
        ))
    }

    fn to_act(&self, state: &Value) -> Vec<Seat> {
        let st = parse(state);
        if st.winner.is_some() {
            Vec::new()
        } else {
            vec![st.turn as Seat]
        }
    }

    fn view(&self, state: &Value, _: &Viewer) -> Result<Value, GameError> {
        let st = parse(state);
        let mut stones = Vec::new();
        let mut rows = Vec::new();
        for r in 0..N {
            let mut row = Vec::with_capacity(N);
            for c in 0..N {
                let v = st.board[r * N + c];
                row.push(v);
                if v != 0 {
                    stones.push(json!({"at": format_at(c, r), "seat": v as i32 - 1}));
                }
            }
            rows.push(row);
        }
        let to_act = if st.winner.is_some() {
            Vec::<i32>::new()
        } else {
            vec![st.turn as i32]
        };
        Ok(json!({
            "size": 15,
            "files": "a-o",
            "ranks": "1-15",
            "rank1": "bottom",
            "variant": st.variant,
            "toAct": to_act,
            "lastMove": st.last,
            "winLine": st.win_line,
            "moves": st.moves,
            "reason": st.reason,
            "winner": st.winner,
            "stones": stones,
            "board": rows,
        }))
    }

    fn legal_actions(&self, state: &Value, seat: Seat) -> Result<Vec<Value>, GameError> {
        let st = parse(state);
        if st.winner.is_some() || st.turn as Seat != seat {
            return Ok(Vec::new());
        }
        let mut actions = Vec::new();
        for r in 0..N {
            for c in 0..N {
                if st.board[r * N + c] != 0 {
                    continue;
                }
                let at = format_at(c, r);
                if st.variant == "renju" && seat == 0 && forbidden(&st, c, r) {
                    continue;
                }
                actions.push(place_action(&at));
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
        let mut st = parse(state);
        if st.winner.is_some() {
            return Err(GameError("对局已结束".into()));
        }
        if action.get("type").and_then(|v| v.as_str()) == Some("resign") {
            let winner = 1 - seat;
            st.winner = Some(winner as i8);
            st.reason = Some("resign".into());
            let event = json!({
                "type": "resign",
                "data": {"seat": seat},
                "visibility": {"public": true}
            });
            return Ok((to_value(&st), vec![event]));
        }
        if st.turn as Seat != seat {
            return Err(GameError("还没轮到该席位".into()));
        }
        let at = action
            .get("data")
            .and_then(|d| d.get("at"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| GameError("动作需要 data.at".into()))?;
        if action.get("type").and_then(|v| v.as_str()) != Some("place") {
            return Err(GameError("动作类型必须是 place".into()));
        }
        let (c, r) = parse_at(at).ok_or_else(|| GameError(format!("无法解析坐标 {at}")))?;
        if st.board[r * N + c] != 0 {
            return Err(GameError(format!("{at} 已有棋子")));
        }
        if st.variant == "renju" && seat == 0 && forbidden(&st, c, r) {
            return Err(GameError(format!("{at} 是禁手")));
        }
        st.board[r * N + c] = seat as u8 + 1;
        st.moves += 1;
        let at_norm = format_at(c, r);
        st.last = Some(at_norm.clone());
        if let Some(line) = winning_line(&st.board, c, r, seat as u8 + 1, &st.variant) {
            st.winner = Some(seat as i8);
            st.win_line = line;
            st.reason = Some("five_in_row".into());
        } else if st.board.iter().all(|v| *v != 0) {
            st.winner = Some(-1);
            st.reason = Some("draw".into());
        } else {
            st.turn = 1 - st.turn;
        }
        let event = json!({
            "type": "place",
            "data": {"at": at_norm, "seat": seat},
            "visibility": {"public": true}
        });
        Ok((to_value(&st), vec![event]))
    }

    fn is_terminal(&self, state: &Value) -> bool {
        parse(state).winner.is_some()
    }

    fn result(&self, state: &Value) -> Result<GameResult, GameError> {
        let st = parse(state);
        match st.winner {
            Some(0) => Ok(win(0, st.reason.unwrap_or_else(|| "five_in_row".into()))),
            Some(1) => Ok(win(1, st.reason.unwrap_or_else(|| "five_in_row".into()))),
            Some(-1) => Ok(GameResult {
                scores: vec![0.5, 0.5],
                ranking: vec![vec![0, 1]],
                teams: None,
                reason: "draw".into(),
            }),
            _ => Err(GameError("对局尚未结束".into())),
        }
    }

    fn timeout_decision(
        &self,
        state: &Value,
        _seat: Seat,
        policy: TimeoutPolicy,
        rng: &mut dyn ArenaRng,
    ) -> Result<TimeoutDecision, GameError> {
        match policy {
            TimeoutPolicy::Forfeit => Ok(TimeoutDecision {
                policy,
                action: None,
            }),
            TimeoutPolicy::AutoAction => {
                let action = pick_near(state, rng)?;
                Ok(TimeoutDecision {
                    policy,
                    action: Some(action),
                })
            }
            TimeoutPolicy::AutoPass => Err(GameError("五子棋没有过牌".into())),
        }
    }
}

impl BoardGame for Gomoku {
    fn board_spec(&self, _: &Config) -> BoardSpec {
        BoardSpec {
            width: 15,
            height: 15,
            coords: "intersection".into(),
            zones: json!({}),
        }
    }
}

fn win(seat: Seat, reason: String) -> GameResult {
    let mut scores = vec![0.0, 0.0];
    scores[seat as usize] = 1.0;
    let loser = 1 - seat;
    GameResult {
        scores,
        ranking: vec![vec![seat], vec![loser]],
        teams: None,
        reason,
    }
}

pub fn force_terminal(state: &Value, winner: Seat, reason: &str) -> Value {
    let mut st = parse(state);
    st.winner = Some(winner as i8);
    st.reason = Some(reason.into());
    to_value(&st)
}

fn pick_near(state: &Value, rng: &mut dyn ArenaRng) -> Result<Value, GameError> {
    let st = parse(state);
    let (lx, ly) = st
        .last
        .as_deref()
        .and_then(parse_at)
        .unwrap_or((7usize, 7usize));
    let mut best = i32::MAX;
    let mut cands: Vec<(usize, usize)> = Vec::new();
    for r in 0..N {
        for c in 0..N {
            if st.board[r * N + c] != 0 {
                continue;
            }
            if st.variant == "renju" && st.turn == 0 && forbidden(&st, c, r) {
                continue;
            }
            let d = (c as i32 - lx as i32)
                .abs()
                .max((r as i32 - ly as i32).abs());
            if d < best {
                best = d;
                cands.clear();
            }
            if d == best {
                cands.push((c, r));
            }
        }
    }
    if cands.is_empty() {
        return Err(GameError("没有可落子的点".into()));
    }
    cands.sort();
    let (c, r) = cands[rng.int_n(cands.len())];
    Ok(place_action(&format_at(c, r)))
}

fn place_action(at: &str) -> Value {
    json!({"type": "place", "data": {"at": at}})
}

fn to_value(st: &GState) -> Value {
    serde_json::to_value(st).expect("gomoku state serializes")
}

fn parse(state: &Value) -> GState {
    serde_json::from_value(state.clone()).expect("gomoku state shape")
}

pub fn format_at(col: usize, row: usize) -> String {
    format!("{}{}", (b'a' + col as u8) as char, row + 1)
}

pub fn parse_at(s: &str) -> Option<(usize, usize)> {
    let s = s.trim().to_ascii_lowercase();
    let b = s.as_bytes();
    if b.len() < 2 || b.len() > 3 {
        return None;
    }
    let col = b[0];
    if !(b'a'..=b'o').contains(&col) {
        return None;
    }
    let row: usize = s[1..].parse().ok()?;
    if !(1..=15).contains(&row) {
        return None;
    }
    Some(((col - b'a') as usize, row - 1))
}

fn winning_line(
    board: &[u8],
    col: usize,
    row: usize,
    color: u8,
    variant: &str,
) -> Option<Vec<String>> {
    let dirs = [(1, 0), (0, 1), (1, 1), (1, -1)];
    let mut best: Option<Vec<String>> = None;
    for (dx, dy) in dirs {
        let (count, cells) = run(board, col, row, dx, dy, color);
        let win = if variant == "renju" && color == 1 {
            count == 5
        } else {
            count >= 5
        };
        if win && best.as_ref().map(|b| cells.len() > b.len()).unwrap_or(true) {
            best = Some(cells);
        }
    }
    best
}

fn run(board: &[u8], col: usize, row: usize, dx: i32, dy: i32, color: u8) -> (usize, Vec<String>) {
    let mut cells = vec![format_at(col, row)];
    for sign in [-1, 1] {
        let mut c = col as i32 + dx * sign;
        let mut r = row as i32 + dy * sign;
        while (0..N as i32).contains(&c) && (0..N as i32).contains(&r) {
            if board[r as usize * N + c as usize] != color {
                break;
            }
            let at = format_at(c as usize, r as usize);
            if sign < 0 {
                cells.insert(0, at);
            } else {
                cells.push(at);
            }
            c += dx * sign;
            r += dy * sign;
        }
    }
    let n = cells.len();
    (n, cells)
}

/// Straight-line renju forbidden check for a black stone at (col, row) on the
/// board *before* placement.
fn forbidden(st: &GState, col: usize, row: usize) -> bool {
    let mut board = st.board.clone();
    board[row * N + col] = 1;
    let mut fours = 0;
    let mut open_threes = 0;
    let mut exact_five = false;
    let mut overline = false;
    for (dx, dy) in [(1, 0), (0, 1), (1, 1), (1, -1)] {
        let (count, _) = run(&board, col, row, dx, dy, 1);
        let (open_neg, open_pos) = open_ends(&board, col, row, dx, dy, 1);
        if count >= 6 {
            overline = true;
        } else if count == 5 {
            exact_five = true;
        } else if count == 4 && (open_neg || open_pos) {
            fours += 1;
        } else if count == 3 && open_neg && open_pos {
            open_threes += 1;
        }
    }
    if overline {
        return true;
    }
    if exact_five {
        return false;
    }
    fours >= 2 || open_threes >= 2
}

fn open_ends(board: &[u8], col: usize, row: usize, dx: i32, dy: i32, color: u8) -> (bool, bool) {
    let neg = end_open(board, col, row, -dx, -dy, color);
    let pos = end_open(board, col, row, dx, dy, color);
    (neg, pos)
}

fn end_open(board: &[u8], col: usize, row: usize, dx: i32, dy: i32, color: u8) -> bool {
    let mut c = col as i32 + dx;
    let mut r = row as i32 + dy;
    while (0..N as i32).contains(&c)
        && (0..N as i32).contains(&r)
        && board[r as usize * N + c as usize] == color
    {
        c += dx;
        r += dy;
    }
    (0..N as i32).contains(&c)
        && (0..N as i32).contains(&r)
        && board[r as usize * N + c as usize] == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NopRng;
    impl ArenaRng for NopRng {
        fn next_u64(&mut self) -> u64 {
            0
        }
        fn int_n(&mut self, _n: usize) -> usize {
            0
        }
    }

    fn game() -> (Gomoku, Value) {
        let g = Gomoku;
        let (state, _) = g
            .setup(
                &Config {
                    players: 2,
                    variant: "free".into(),
                    options: json!({}),
                },
                &mut NopRng,
            )
            .unwrap();
        (g, state)
    }

    fn play(g: &Gomoku, state: &mut Value, seat: Seat, at: &str) {
        let (next, _) = g
            .apply(state, seat, &place_action(at), &mut NopRng)
            .unwrap();
        *state = next;
    }

    #[test]
    fn h8_is_center() {
        assert_eq!(parse_at("h8"), Some((7, 7)));
        assert_eq!(parse_at("a1"), Some((0, 0)));
        assert_eq!(parse_at("o15"), Some((14, 14)));
        assert_eq!(format_at(7, 7), "h8");
    }

    #[test]
    fn vertical_five_wins_for_black() {
        let (g, mut state) = game();
        let seq = [
            (0, "h8"),
            (1, "i8"),
            (0, "h9"),
            (1, "i9"),
            (0, "h10"),
            (1, "i10"),
            (0, "h11"),
            (1, "i11"),
            (0, "h12"),
        ];
        for (seat, at) in seq {
            play(&g, &mut state, seat, at);
        }
        assert!(g.is_terminal(&state));
        let result = g.result(&state).unwrap();
        assert_eq!(result.reason, "five_in_row");
        assert_eq!(result.scores, vec![1.0, 0.0]);
        let view = g
            .view(
                &state,
                &Viewer {
                    seat: None,
                    omniscient: false,
                },
            )
            .unwrap();
        assert_eq!(view["winLine"].as_array().unwrap().len(), 5);
        assert!(g.to_act(&state).is_empty());
    }

    #[test]
    fn horizontal_and_diagonal() {
        let (g, mut state) = game();
        // black a1 b2 c3 d4 e5 — but white has to move in between, off the line
        let seq = [
            (0, "a1"),
            (1, "a2"),
            (0, "b2"),
            (1, "a3"),
            (0, "c3"),
            (1, "a4"),
            (0, "d4"),
            (1, "a5"),
            (0, "e5"),
        ];
        for (seat, at) in seq {
            play(&g, &mut state, seat, at);
        }
        assert!(g.is_terminal(&state));
        assert_eq!(g.result(&state).unwrap().ranking[0], vec![0]);
    }

    #[test]
    fn occupied_and_wrong_turn() {
        let (g, mut state) = game();
        play(&g, &mut state, 0, "h8");
        let err = g
            .apply(&state, 0, &place_action("h9"), &mut NopRng)
            .unwrap_err();
        assert!(err.0.contains("轮到"));
        let err = g
            .apply(&state, 1, &place_action("h8"), &mut NopRng)
            .unwrap_err();
        assert!(err.0.contains("已有"));
    }

    #[test]
    fn free_overline_wins() {
        let (g, mut state) = game();
        // a1–d1 and f1 are filled first (no five yet). e1 closes a six.
        let blacks = ["a1", "b1", "c1", "d1", "f1"];
        let whites = ["a2", "b2", "c2", "d2", "f2"];
        for i in 0..5 {
            play(&g, &mut state, 0, blacks[i]);
            play(&g, &mut state, 1, whites[i]);
        }
        play(&g, &mut state, 0, "e1");
        assert!(g.is_terminal(&state));
        let result = g.result(&state).unwrap();
        assert_eq!(result.scores[0], 1.0);
        assert!(
            g.view(
                &state,
                &Viewer {
                    seat: None,
                    omniscient: false
                }
            )
            .unwrap()["winLine"]
                .as_array()
                .unwrap()
                .len()
                >= 6
        );
    }

    #[test]
    fn renju_overline_is_forbidden() {
        let g = Gomoku;
        let (mut state, _) = g
            .setup(
                &Config {
                    players: 2,
                    variant: "renju".into(),
                    options: json!({}),
                },
                &mut NopRng,
            )
            .unwrap();
        // Five black stones with a gap at e8: a8–d8 and f8. Filling e8 makes six.
        let blacks = ["a8", "b8", "c8", "d8", "f8"];
        let whites = ["a9", "b9", "c9", "d9", "f9"];
        for i in 0..5 {
            play(&g, &mut state, 0, blacks[i]);
            play(&g, &mut state, 1, whites[i]);
        }
        let err = g
            .apply(&state, 0, &place_action("e8"), &mut NopRng)
            .unwrap_err();
        assert!(err.0.contains("禁手"), "{}", err.0);
        let legal = g.legal_actions(&state, 0).unwrap();
        assert!(legal.iter().all(|a| a["data"]["at"] != "e8"));
    }

    #[test]
    fn renju_double_three_forbidden() {
        let g = Gomoku;
        let (mut state, _) = g
            .setup(
                &Config {
                    players: 2,
                    variant: "renju".into(),
                    options: json!({}),
                },
                &mut NopRng,
            )
            .unwrap();
        // Build two open threes that meet at h8.
        // Horizontal: g8 and i8 exist, f8 and j8 empty, h8 is the candidate.
        // Vertical: h7 and h9 exist, h6 and h10 empty.
        // White answers far away.
        let blacks = ["g8", "h7", "i8", "h9"];
        let whites = ["a1", "a2", "a3", "c1"];
        for i in 0..4 {
            play(&g, &mut state, 0, blacks[i]);
            play(&g, &mut state, 1, whites[i]);
        }
        let err = g
            .apply(&state, 0, &place_action("h8"), &mut NopRng)
            .unwrap_err();
        assert!(err.0.contains("禁手"), "{}", err.0);
    }

    #[test]
    fn full_board_draw() {
        let g = Gomoku;
        // (c + 2r) mod 4, black on 0 and 1. a1 is empty; filling it does not make five.
        let mut board = vec![0u8; 225];
        for r in 0..15 {
            for c in 0..15 {
                if r == 0 && c == 0 {
                    continue;
                }
                board[r * 15 + c] = if (c + 2 * r) % 4 < 2 { 1 } else { 2 };
            }
        }
        let st = GState {
            size: 15,
            board,
            turn: 0,
            last: Some("b1".into()),
            winner: None,
            win_line: vec![],
            variant: "free".into(),
            reason: None,
            moves: 224,
        };
        let state = to_value(&st);
        let (next, _) = g
            .apply(&state, 0, &place_action("a1"), &mut NopRng)
            .unwrap();
        assert!(g.is_terminal(&next));
        assert_eq!(g.result(&next).unwrap().reason, "draw");
    }

    #[test]
    fn auto_action_is_deterministic_and_legal() {
        let (g, mut state) = game();
        play(&g, &mut state, 0, "h8");
        let mut rng_a = SeededRng::from_seed_seq(&[9u8; 32], 1);
        let mut rng_b = SeededRng::from_seed_seq(&[9u8; 32], 1);
        let a = g
            .timeout_decision(&state, 1, TimeoutPolicy::AutoAction, &mut rng_a)
            .unwrap();
        let b = g
            .timeout_decision(&state, 1, TimeoutPolicy::AutoAction, &mut rng_b)
            .unwrap();
        assert_eq!(a.action, b.action);
        let at = a.action.unwrap()["data"]["at"]
            .as_str()
            .unwrap()
            .to_string();
        let (c, r) = parse_at(&at).unwrap();
        let dist = (c as i32 - 7).abs().max((r as i32 - 7).abs());
        assert_eq!(dist, 1);
        let forfeit = g
            .timeout_decision(&state, 1, TimeoutPolicy::Forfeit, &mut NopRng)
            .unwrap();
        assert!(forfeit.action.is_none());
        assert_eq!(forfeit.policy, TimeoutPolicy::Forfeit);
    }

    #[test]
    fn board_trait_reports_intersections() {
        let g = Gomoku;
        let spec = g.board_spec(&Config {
            players: 2,
            variant: "free".into(),
            options: json!({}),
        });
        assert_eq!(spec.width, 15);
        assert_eq!(spec.coords, "intersection");
        let reg = Registry::builtin();
        assert!(reg
            .board_spec(
                "gomoku",
                &Config {
                    players: 2,
                    variant: "free".into(),
                    options: json!({}),
                }
            )
            .is_some());
        assert!(reg.get("xiangqi").unwrap().manifest().playable);
        assert!(reg.get("mahjong-sichuan").unwrap().manifest().playable);
        assert_eq!(reg.list().len(), 6);
    }
}
