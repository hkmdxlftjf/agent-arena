//! 四川麻将 · 血战到底。
//!
//! 108 张（万筒条，无字），四人。默认换三张，然后定缺。不能吃。
//! 胡、杠优先于碰。一家胡后其余继续，直到三家胡或牌墙摸完，再查叫、查花猪。
//! 换三张和定缺走承诺-揭示。观众在终局前只能看公开信息。
//!
//! 番型：平胡 1、对对胡 2、清一色 +4、七对 4、龙七对 8、金钩钓 4，
//! 杠上花 / 杠上炮 / 抢杠 / 海底各 +2，自摸 +1，封顶 16。

use crate::game::*;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Meld {
    kind: String,
    tile: u8,
    from: Option<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Pending {
    tile: u8,
    from: u8,
    kind: String,
    waiting: Vec<u8>,
    answers: Vec<Option<String>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct MState {
    phase: String,
    exchange: bool,
    wall: Vec<u8>,
    hands: Vec<Vec<u8>>,
    melds: Vec<Vec<Meld>>,
    discards: Vec<Vec<u8>>,
    given: Vec<Option<Vec<u8>>>,
    dingque: Vec<Option<u8>>,
    turn: u8,
    last_draw: Option<u8>,
    pending: Option<Pending>,
    hu: Vec<bool>,
    points: Vec<i64>,
    #[serde(default)]
    kong_draw: bool,
    #[serde(default)]
    reason: Option<String>,
    terminal: bool,
}

pub struct Mahjong;

impl Mahjong {
    fn load(state: &Value) -> Result<MState, GameError> {
        serde_json::from_value(state.clone()).map_err(|e| GameError(e.to_string()))
    }
}

fn tile_str(t: u8) -> String {
    format!("{}{}", t % 9 + 1, ['m', 's', 'p'][(t / 9) as usize])
}

fn parse_tile(s: &str) -> Option<u8> {
    let b = s.as_bytes();
    if b.len() != 2 {
        return None;
    }
    let r = b[0].wrapping_sub(b'1');
    let suit = match b[1] {
        b'm' => 0,
        b's' => 1,
        b'p' => 2,
        _ => return None,
    };
    if r < 9 {
        Some(suit * 9 + r)
    } else {
        None
    }
}

fn parse_suit(s: &str) -> Option<u8> {
    match s {
        "m" => Some(0),
        "s" => Some(1),
        "p" => Some(2),
        _ => None,
    }
}

fn suit_name(s: u8) -> &'static str {
    match s {
        0 => "m",
        1 => "s",
        _ => "p",
    }
}

fn sort_hand(h: &mut [u8]) {
    h.sort_unstable();
}

fn counts_of(tiles: &[u8]) -> [[u8; 9]; 3] {
    let mut c = [[0u8; 9]; 3];
    for t in tiles {
        c[(t / 9) as usize][(t % 9) as usize] += 1;
    }
    c
}

fn first_tile(c: &[[u8; 9]; 3]) -> Option<(usize, usize)> {
    for (s, row) in c.iter().enumerate() {
        for (r, n) in row.iter().enumerate() {
            if *n > 0 {
                return Some((s, r));
            }
        }
    }
    None
}

fn seven_pairs(c: &[[u8; 9]; 3]) -> bool {
    let mut pairs = 0;
    for row in c {
        for n in row {
            match *n {
                0 => {}
                2 => pairs += 1,
                4 => pairs += 2,
                _ => return false,
            }
        }
    }
    pairs == 7
}

fn has_quad(c: &[[u8; 9]; 3]) -> bool {
    c.iter().flatten().any(|n| *n == 4)
}

fn search(c: &mut [[u8; 9]; 3], sets: i32, need_pair: bool, pungs_only: bool) -> bool {
    if sets == 0 && !need_pair {
        return first_tile(c).is_none();
    }
    let Some((s, r)) = first_tile(c) else {
        return sets == 0 && !need_pair;
    };
    if need_pair && c[s][r] >= 2 {
        c[s][r] -= 2;
        if search(c, sets, false, pungs_only) {
            return true;
        }
        c[s][r] += 2;
    }
    if sets > 0 && c[s][r] >= 3 {
        c[s][r] -= 3;
        if search(c, sets - 1, need_pair, pungs_only) {
            return true;
        }
        c[s][r] += 3;
    }
    if sets > 0 && !pungs_only && r <= 6 && c[s][r] > 0 && c[s][r + 1] > 0 && c[s][r + 2] > 0 {
        c[s][r] -= 1;
        c[s][r + 1] -= 1;
        c[s][r + 2] -= 1;
        if search(c, sets - 1, need_pair, pungs_only) {
            return true;
        }
        c[s][r] += 1;
        c[s][r + 1] += 1;
        c[s][r + 2] += 1;
    }
    false
}

fn can_win(hand: &[u8], melds: &[Meld]) -> bool {
    if hand.len() + melds.len() * 3 != 14 && !(melds.is_empty() && hand.len() == 14) {
        // gangs still occupy one meld slot but four tiles; concealed count is 14 - 3*melds.
        if hand.len() != 14 - melds.len() * 3 {
            return false;
        }
    }
    if hand.len() != 14 - melds.len() * 3 {
        return false;
    }
    let mut c = counts_of(hand);
    if melds.is_empty() && seven_pairs(&c) {
        return true;
    }
    search(&mut c, (4 - melds.len() as i32).max(0), true, false)
}

fn all_pungs(hand: &[u8], melds: &[Meld]) -> bool {
    if melds.iter().any(|m| m.kind == "chi") {
        return false;
    }
    if !melds.is_empty()
        && melds
            .iter()
            .any(|m| m.kind != "peng" && m.kind != "gang" && m.kind != "angang")
    {
        return false;
    }
    let mut c = counts_of(hand);
    search(&mut c, (4 - melds.len() as i32).max(0), true, true)
}

fn one_suit(hand: &[u8], melds: &[Meld]) -> bool {
    let mut suits = [false; 3];
    for t in hand {
        suits[(t / 9) as usize] = true;
    }
    for m in melds {
        suits[(m.tile / 9) as usize] = true;
    }
    suits.iter().filter(|s| **s).count() == 1
}

fn fan(hand: &[u8], melds: &[Meld], zimo: bool, extra: i64) -> (i64, String) {
    let mut c = counts_of(hand);
    let (mut pts, name) = if melds.is_empty() && seven_pairs(&c) {
        if has_quad(&c) {
            (8, "龙七对")
        } else {
            (4, "七对")
        }
    } else if all_pungs(hand, melds) {
        if melds.len() == 4 {
            (4, "金钩钓")
        } else {
            (2, "对对胡")
        }
    } else {
        let _ = search(&mut c, (4 - melds.len() as i32).max(0), true, false);
        (1, "平胡")
    };
    let mut label = name.to_string();
    if one_suit(hand, melds) {
        pts += 4;
        label = format!("清一色{label}");
    }
    if zimo {
        pts += 1;
        label.push_str(" 自摸");
    }
    pts += extra;
    (pts.min(16), label)
}

fn holds_suit(hand: &[u8], melds: &[Meld], suit: u8) -> bool {
    hand.iter().any(|t| t / 9 == suit) || melds.iter().any(|m| m.tile / 9 == suit)
}

fn tenpai(hand: &[u8], melds: &[Meld], miss: Option<u8>) -> bool {
    let open = melds.len() * 3;
    if hand.len() != 13 - open && hand.len() != 14 - open - 1 && hand.len() + open != 13 {
        return false;
    }
    if let Some(miss) = miss {
        if holds_suit(hand, melds, miss) {
            return false;
        }
    }
    for t in 0..27u8 {
        if miss == Some(t / 9) {
            continue;
        }
        let mut h = hand.to_vec();
        h.push(t);
        if can_win(&h, melds) {
            return true;
        }
    }
    false
}

fn live_after(from: u8, hu: &[bool]) -> Option<u8> {
    (1..4).map(|i| (from + i) % 4).find(|s| !hu[*s as usize])
}

fn deal(rng: &mut dyn ArenaRng, exchange: bool) -> MState {
    let mut wall = Vec::new();
    for t in 0..27u8 {
        for _ in 0..4 {
            wall.push(t);
        }
    }
    for i in (1..wall.len()).rev() {
        wall.swap(i, rng.int_n(i + 1));
    }
    let mut hands = vec![Vec::new(); 4];
    for i in 0..52 {
        hands[i % 4].push(wall.pop().unwrap());
    }
    for h in &mut hands {
        sort_hand(h);
    }
    MState {
        phase: if exchange {
            "exchange".into()
        } else {
            "dingque".into()
        },
        exchange,
        wall,
        hands,
        melds: vec![Vec::new(); 4],
        discards: vec![Vec::new(); 4],
        given: vec![None, None, None, None],
        dingque: vec![None, None, None, None],
        turn: 0,
        last_draw: None,
        pending: None,
        hu: vec![false; 4],
        points: vec![0; 4],
        kong_draw: false,
        reason: None,
        terminal: false,
    }
}

fn private_data(action: &Value) -> Option<&Value> {
    action
        .get("_private")
        .and_then(|p| p.get("data"))
        .or_else(|| action.get("data"))
}

fn most_three(hand: &[u8]) -> Vec<u8> {
    let mut best = 0u8;
    let mut nbest = 0;
    for s in 0..3u8 {
        let n = hand.iter().filter(|t| *t / 9 == s).count();
        if n > nbest {
            nbest = n;
            best = s;
        }
    }
    hand.iter()
        .copied()
        .filter(|t| t / 9 == best)
        .take(3)
        .collect()
}

fn fewest_suit(hand: &[u8]) -> u8 {
    let mut best = 0u8;
    let mut nbest = usize::MAX;
    for s in 0..3u8 {
        let n = hand.iter().filter(|t| *t / 9 == s).count();
        if n < nbest {
            nbest = n;
            best = s;
        }
    }
    best
}

fn remove_tiles(hand: &mut Vec<u8>, tiles: &[u8]) -> Result<(), GameError> {
    for t in tiles {
        let pos = hand
            .iter()
            .position(|x| x == t)
            .ok_or_else(|| GameError("手里没有这些牌".into()))?;
        hand.remove(pos);
    }
    sort_hand(hand);
    Ok(())
}

fn draw_tile(s: &mut MState, seat: u8, back: bool) -> bool {
    if s.wall.is_empty() {
        settle_wall(s);
        return false;
    }
    let tile = if back {
        s.wall.remove(0)
    } else {
        s.wall.pop().unwrap()
    };
    // pop is the back of the vec which we treated as the front of the wall (deal popped).
    // Keep pop as the live end. Replacement kongs take from the other end.
    s.hands[seat as usize].push(tile);
    sort_hand(&mut s.hands[seat as usize]);
    s.last_draw = Some(tile);
    s.kong_draw = back;
    s.turn = seat;
    s.phase = "play".into();
    true
}

fn start_play(s: &mut MState) {
    s.phase = "play".into();
    if !draw_tile(s, 0, false) {
        return;
    }
    s.kong_draw = false;
}

fn open_claim(s: &mut MState, tile: u8, from: u8, kind: &str) {
    let mut waiting = Vec::new();
    for seat in 0..4u8 {
        if seat == from || s.hu[seat as usize] {
            continue;
        }
        if can_claim(s, seat, tile, kind) {
            waiting.push(seat);
        }
    }
    if waiting.is_empty() {
        if kind == "bugang" {
            let _ = draw_tile(s, from, true);
        } else if let Some(next) = live_after(from, &s.hu) {
            let _ = draw_tile(s, next, false);
            s.kong_draw = false;
        } else {
            s.terminal = true;
            s.phase = "over".into();
        }
        return;
    }
    s.pending = Some(Pending {
        tile,
        from,
        kind: kind.into(),
        waiting: waiting.clone(),
        answers: vec![None; 4],
    });
    s.phase = "claim".into();
}

fn can_claim(s: &MState, seat: u8, tile: u8, kind: &str) -> bool {
    if s.dingque[seat as usize] == Some(tile / 9) {
        return false;
    }
    let hand = &s.hands[seat as usize];
    let n = hand.iter().filter(|t| **t == tile).count();
    let mut trial = hand.clone();
    trial.push(tile);
    let win = can_win(&trial, &s.melds[seat as usize])
        && !holds_suit(
            hand,
            &s.melds[seat as usize],
            s.dingque[seat as usize].unwrap_or(9),
        );
    if kind == "bugang" {
        return win;
    }
    win || n >= 3 || n >= 2
}

fn resolve_claim(s: &mut MState) {
    let pending = s.pending.clone().unwrap();
    let answers: Vec<(u8, String)> = pending
        .waiting
        .iter()
        .filter_map(|seat| {
            s.pending.as_ref().unwrap().answers[*seat as usize]
                .clone()
                .map(|a| (*seat, a))
        })
        .collect();
    let hus: Vec<u8> = answers
        .iter()
        .filter(|(_, a)| a == "hu")
        .map(|(s, _)| *s)
        .collect();
    s.pending = None;
    if !hus.is_empty() {
        let extra = match pending.kind.as_str() {
            "bugang" | "gangpao" => 2,
            _ if s.wall.is_empty() => 2,
            _ => 0,
        };
        for seat in hus {
            let mut hand = s.hands[seat as usize].clone();
            hand.push(pending.tile);
            let (pts, _) = fan(&hand, &s.melds[seat as usize], false, extra);
            s.points[seat as usize] += pts;
            s.points[pending.from as usize] -= pts;
            s.hu[seat as usize] = true;
        }
        if pending.kind == "bugang" {
            if let Some(meld) = s.melds[pending.from as usize]
                .iter_mut()
                .find(|m| m.kind == "gang" && m.tile == pending.tile)
            {
                meld.kind = "peng".into();
            }
        }
        if s.hu.iter().filter(|h| **h).count() >= 3 {
            s.terminal = true;
            s.phase = "over".into();
            s.reason = Some("xuezhan".into());
            return;
        }
        let next = live_after(pending.from, &s.hu);
        if let Some(next) = next {
            let _ = draw_tile(s, next, false);
            s.kong_draw = false;
        }
        return;
    }
    if let Some((seat, _)) = answers.iter().find(|(_, a)| a == "gang") {
        remove_tiles(&mut s.hands[*seat as usize], &[pending.tile; 3]).ok();
        s.melds[*seat as usize].push(Meld {
            kind: "gang".into(),
            tile: pending.tile,
            from: Some(pending.from),
        });
        let _ = draw_tile(s, *seat, true);
        return;
    }
    if let Some((seat, _)) = answers.iter().find(|(_, a)| a == "peng") {
        remove_tiles(&mut s.hands[*seat as usize], &[pending.tile; 2]).ok();
        s.melds[*seat as usize].push(Meld {
            kind: "peng".into(),
            tile: pending.tile,
            from: Some(pending.from),
        });
        s.turn = *seat;
        s.phase = "play".into();
        s.last_draw = None;
        s.kong_draw = false;
        return;
    }
    if pending.kind == "bugang" {
        let _ = draw_tile(s, pending.from, true);
    } else if let Some(next) = live_after(pending.from, &s.hu) {
        let _ = draw_tile(s, next, false);
        s.kong_draw = false;
    }
}

fn settle_wall(s: &mut MState) {
    for seat in 0..4 {
        if s.hu[seat] {
            continue;
        }
        let miss = s.dingque[seat].unwrap_or(0);
        if holds_suit(&s.hands[seat], &s.melds[seat], miss) {
            for other in 0..4 {
                if other != seat
                    && !s.hu[other]
                    && !holds_suit(
                        &s.hands[other],
                        &s.melds[other],
                        s.dingque[other].unwrap_or(0),
                    )
                {
                    s.points[seat] -= 16;
                    s.points[other] += 16;
                }
            }
        }
    }
    for seat in 0..4 {
        if s.hu[seat] {
            continue;
        }
        let miss = s.dingque[seat];
        if holds_suit(&s.hands[seat], &s.melds[seat], miss.unwrap_or(9)) {
            continue;
        }
        if !tenpai(&s.hands[seat], &s.melds[seat], miss) {
            continue;
        }
        for other in 0..4 {
            if other == seat || s.hu[other] {
                continue;
            }
            let omiss = s.dingque[other];
            if holds_suit(&s.hands[other], &s.melds[other], omiss.unwrap_or(9)) {
                continue;
            }
            if tenpai(&s.hands[other], &s.melds[other], omiss) {
                continue;
            }
            s.points[other] -= 1;
            s.points[seat] += 1;
        }
    }
    s.terminal = true;
    s.phase = "over".into();
    s.reason = Some("wall".into());
}

fn show_hand(tiles: &[u8], visible: bool) -> Value {
    if visible {
        json!(tiles.iter().map(|t| tile_str(*t)).collect::<Vec<_>>())
    } else {
        Value::Null
    }
}

impl Game for Mahjong {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "mahjong-sichuan".into(),
            name: "麻将（四川·血战到底）".into(),
            version: "1.0.0".into(),
            min_players: 4,
            max_players: 4,
            traits: vec![
                Trait::Tile,
                Trait::HiddenInfo,
                Trait::Chance,
                Trait::MultiPhase,
            ],
            variants: vec![
                Variant {
                    id: "xuezhan".into(),
                    name: "血战到底 · 换三张".into(),
                    options: None,
                },
                Variant {
                    id: "no-huan".into(),
                    name: "血战到底 · 不换牌".into(),
                    options: None,
                },
            ],
            default_time: TimeControl {
                per_action_ms: 60_000,
                max_consecutive_timeouts: 3,
                grace_ms: 2_000,
            },
            timeout_policy: TimeoutPolicy::AutoAction,
            allowed_timeout_policies: vec![TimeoutPolicy::AutoAction, TimeoutPolicy::AutoPass],
            action_schema: json!({"type":"object"}),
            rules_markdown: MJ_RULES.into(),
            renderer: "mahjong-table".into(),
            rated: true,
            engine: "native".into(),
            description: "四人 108 张。换三张、定缺、碰杠胡，血战到底，查叫查花猪。".into(),
            playable: true,
        }
    }

    fn setup(
        &self,
        cfg: &Config,
        rng: &mut dyn ArenaRng,
    ) -> Result<(Value, Vec<Value>), GameError> {
        let exchange = match cfg.variant.as_str() {
            "xuezhan" => true,
            "no-huan" => false,
            _ => return Err(GameError("未知变体".into())),
        };
        if cfg.players != 4 {
            return Err(GameError("四川麻将固定四人".into()));
        }
        Ok((
            serde_json::to_value(deal(rng, exchange)).unwrap(),
            Vec::new(),
        ))
    }

    fn to_act(&self, state: &Value) -> Vec<Seat> {
        let Ok(s) = Self::load(state) else {
            return Vec::new();
        };
        if s.terminal {
            return Vec::new();
        }
        match s.phase.as_str() {
            "exchange" => s
                .given
                .iter()
                .enumerate()
                .filter(|(_, g)| g.is_none())
                .map(|(i, _)| i as Seat)
                .collect(),
            "dingque" => s
                .dingque
                .iter()
                .enumerate()
                .filter(|(_, g)| g.is_none())
                .map(|(i, _)| i as Seat)
                .collect(),
            "claim" => s
                .pending
                .as_ref()
                .map(|p| {
                    p.waiting
                        .iter()
                        .filter(|seat| p.answers[**seat as usize].is_none())
                        .map(|seat| *seat as Seat)
                        .collect()
                })
                .unwrap_or_default(),
            "play" => vec![s.turn as Seat],
            _ => Vec::new(),
        }
    }

    fn view(&self, state: &Value, viewer: &Viewer) -> Result<Value, GameError> {
        let s = Self::load(state)?;
        let hands: Vec<Value> = (0..4)
            .map(|i| {
                let see = viewer.omniscient || viewer.seat == Some(i);
                show_hand(&s.hands[i as usize], see)
            })
            .collect();
        let melds: Vec<Value> = s
            .melds
            .iter()
            .map(|ms| {
                json!(ms
                    .iter()
                    .map(|m| json!({"kind": m.kind, "tile": tile_str(m.tile)}))
                    .collect::<Vec<_>>())
            })
            .collect();
        let discards: Vec<Value> = s
            .discards
            .iter()
            .map(|d| json!(d.iter().map(|t| tile_str(*t)).collect::<Vec<_>>()))
            .collect();
        let dingque: Vec<Value> = s
            .dingque
            .iter()
            .map(|d| d.map(|x| json!(suit_name(x))).unwrap_or(Value::Null))
            .collect();
        let own = viewer
            .seat
            .filter(|seat| !viewer.omniscient && s.phase == "play" && *seat as u8 == s.turn);
        Ok(json!({
            "renderer": "mahjong-table",
            "phase": s.phase,
            "wall": s.wall.len(),
            "hands": hands,
            "counts": s.hands.iter().map(|h| h.len()).collect::<Vec<_>>(),
            "melds": melds,
            "discards": discards,
            "dingque": dingque,
            "lastDraw": own.and_then(|_| s.last_draw.map(tile_str)),
            "pending": s.pending.as_ref().map(|p| json!({"tile": tile_str(p.tile), "from": p.from, "kind": p.kind})),
            "hu": s.hu,
            "points": s.points,
            "toAct": self.to_act(state),
            "reason": s.reason,
        }))
    }

    fn legal_actions(&self, state: &Value, seat: Seat) -> Result<Vec<Value>, GameError> {
        let s = Self::load(state)?;
        if s.terminal || seat < 0 {
            return Ok(Vec::new());
        }
        let seat_u = seat as u8;
        let mut actions = Vec::new();
        match s.phase.as_str() {
            "exchange" if s.given[seat as usize].is_none() => {
                actions.push(json!({"type":"exchange_commit"}))
            }
            "dingque" if s.dingque[seat as usize].is_none() => {
                actions.push(json!({"type":"dingque_commit"}))
            }
            "claim" => {
                let Some(p) = &s.pending else {
                    return Ok(actions);
                };
                if !p.waiting.contains(&seat_u) || p.answers[seat as usize].is_some() {
                    return Ok(actions);
                }
                actions.push(json!({"type":"pass"}));
                let n = s.hands[seat as usize]
                    .iter()
                    .filter(|t| **t == p.tile)
                    .count();
                let mut trial = s.hands[seat as usize].clone();
                trial.push(p.tile);
                if can_win(&trial, &s.melds[seat as usize]) {
                    actions.push(json!({"type":"hu"}));
                }
                if p.kind != "bugang" && n >= 3 {
                    actions.push(json!({"type":"gang"}));
                }
                if p.kind != "bugang" && n >= 2 {
                    actions.push(json!({"type":"peng"}));
                }
            }
            "play" if s.turn == seat_u && !s.hu[seat as usize] => {
                let hand = &s.hands[seat as usize];
                let miss = s.dingque[seat as usize];
                if s.last_draw.is_some()
                    && can_win(hand, &s.melds[seat as usize])
                    && miss
                        .map(|m| !holds_suit(hand, &s.melds[seat as usize], m))
                        .unwrap_or(true)
                {
                    actions.push(json!({"type":"hu"}));
                }
                let mut seen = std::collections::HashSet::new();
                for t in hand {
                    if seen.insert(*t) && hand.iter().filter(|x| *x == t).count() == 4 {
                        actions.push(json!({"type":"angang","data":{"tile": tile_str(*t)}}));
                    }
                }
                for meld in &s.melds[seat as usize] {
                    if meld.kind == "peng" && hand.contains(&meld.tile) {
                        actions.push(json!({"type":"bugang","data":{"tile": tile_str(meld.tile)}}));
                    }
                }
                let mut seen = std::collections::HashSet::new();
                for t in hand {
                    if seen.insert(*t) {
                        actions.push(json!({"type":"discard","data":{"tile": tile_str(*t)}}));
                    }
                }
            }
            _ => {}
        }
        if !actions.is_empty() {
            actions.push(json!({"type":"resign"}));
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
        if s.terminal {
            return Err(GameError("对局已结束".into()));
        }
        let kind = action.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let seat_u = seat as u8;
        if kind == "resign" || kind == "timeout_forfeit" {
            s.hu[seat as usize] = true;
            s.points[seat as usize] -= 16;
            for i in 0..4 {
                if i != seat as usize && !s.hu[i] {
                    s.points[i] += 16 / 3;
                }
            }
            if s.hu.iter().filter(|h| **h).count() >= 3 || live_after(seat_u, &s.hu).is_none() {
                s.terminal = true;
                s.phase = "over".into();
                s.reason = Some("resign".into());
            }
            return Ok((serde_json::to_value(s).unwrap(), Vec::new()));
        }
        match s.phase.as_str() {
            "exchange" => {
                if kind != "exchange_commit" && kind != "exchange" {
                    return Err(GameError("现在要换三张".into()));
                }
                if s.given[seat as usize].is_some() {
                    return Err(GameError("你已经换过".into()));
                }
                let tiles_v = private_data(action)
                    .and_then(|d| d.get("tiles"))
                    .ok_or_else(|| GameError("缺少三张牌".into()))?;
                let tiles: Vec<u8> = tiles_v
                    .as_array()
                    .unwrap_or(&Vec::new())
                    .iter()
                    .filter_map(|v| v.as_str().and_then(parse_tile))
                    .collect();
                if tiles.len() != 3 || tiles.iter().any(|t| t / 9 != tiles[0] / 9) {
                    return Err(GameError("必须换同花色的三张".into()));
                }
                remove_tiles(&mut s.hands[seat as usize], &tiles)?;
                s.given[seat as usize] = Some(tiles);
                if s.given.iter().all(|g| g.is_some()) {
                    let given = s.given.clone();
                    for i in 0..4 {
                        let from = (i + 3) % 4;
                        let mut got = given[from].clone().unwrap();
                        s.hands[i].append(&mut got);
                        sort_hand(&mut s.hands[i]);
                    }
                    s.phase = "dingque".into();
                }
            }
            "dingque" => {
                if kind != "dingque_commit" && kind != "dingque" {
                    return Err(GameError("现在要定缺".into()));
                }
                if s.dingque[seat as usize].is_some() {
                    return Err(GameError("你已经定缺".into()));
                }
                let suit = private_data(action)
                    .and_then(|d| d.get("suit"))
                    .and_then(|v| v.as_str())
                    .and_then(parse_suit)
                    .ok_or_else(|| GameError("缺门不合法".into()))?;
                s.dingque[seat as usize] = Some(suit);
                if s.dingque.iter().all(|d| d.is_some()) {
                    start_play(&mut s);
                }
            }
            "claim" => {
                if !matches!(kind, "pass" | "peng" | "gang" | "hu") {
                    return Err(GameError("现在只能碰、杠、胡或过".into()));
                }
                let waiting = s
                    .pending
                    .as_ref()
                    .map(|p| p.waiting.clone())
                    .unwrap_or_default();
                if !waiting.contains(&seat_u) {
                    return Err(GameError("你不能应这张牌".into()));
                }
                if s.pending.as_ref().unwrap().answers[seat as usize].is_some() {
                    return Err(GameError("你已经表过态".into()));
                }
                if kind != "pass" {
                    let legal = self.legal_actions(state, seat)?;
                    if !legal.iter().any(|a| a["type"] == kind) {
                        return Err(GameError("这个应牌不合法".into()));
                    }
                }
                s.pending.as_mut().unwrap().answers[seat as usize] = Some(kind.into());
                let done = {
                    let p = s.pending.as_ref().unwrap();
                    p.waiting
                        .iter()
                        .all(|seat| p.answers[*seat as usize].is_some())
                };
                if done {
                    resolve_claim(&mut s);
                }
            }
            "play" => {
                if s.turn != seat_u {
                    return Err(GameError("还没轮到你".into()));
                }
                match kind {
                    "discard" => {
                        let tile = action["data"]["tile"]
                            .as_str()
                            .and_then(parse_tile)
                            .ok_or_else(|| GameError("没有这张牌".into()))?;
                        remove_tiles(&mut s.hands[seat as usize], &[tile])?;
                        s.discards[seat as usize].push(tile);
                        s.last_draw = None;
                        let gangpao = s.kong_draw;
                        s.kong_draw = false;
                        open_claim(
                            &mut s,
                            tile,
                            seat_u,
                            if gangpao { "gangpao" } else { "discard" },
                        );
                    }
                    "hu" => {
                        let miss = s.dingque[seat as usize];
                        if holds_suit(
                            &s.hands[seat as usize],
                            &s.melds[seat as usize],
                            miss.unwrap_or(9),
                        ) || !can_win(&s.hands[seat as usize], &s.melds[seat as usize])
                        {
                            return Err(GameError("现在不能胡".into()));
                        }
                        let extra = (s.kong_draw as i64) * 2 + (s.wall.is_empty() as i64) * 2;
                        let (pts, _) = fan(
                            &s.hands[seat as usize],
                            &s.melds[seat as usize],
                            true,
                            extra,
                        );
                        for i in 0..4 {
                            if i != seat as usize && !s.hu[i] {
                                s.points[i] -= pts;
                                s.points[seat as usize] += pts;
                            }
                        }
                        s.hu[seat as usize] = true;
                        if s.hu.iter().filter(|h| **h).count() >= 3 {
                            s.terminal = true;
                            s.phase = "over".into();
                            s.reason = Some("xuezhan".into());
                        } else if let Some(next) = live_after(seat_u, &s.hu) {
                            let _ = draw_tile(&mut s, next, false);
                            s.kong_draw = false;
                        }
                    }
                    "angang" => {
                        let tile = action["data"]["tile"]
                            .as_str()
                            .and_then(parse_tile)
                            .ok_or_else(|| GameError("没有这张牌".into()))?;
                        if s.hands[seat as usize]
                            .iter()
                            .filter(|t| **t == tile)
                            .count()
                            < 4
                        {
                            return Err(GameError("不能暗杠".into()));
                        }
                        remove_tiles(&mut s.hands[seat as usize], &[tile; 4])?;
                        s.melds[seat as usize].push(Meld {
                            kind: "angang".into(),
                            tile,
                            from: None,
                        });
                        if !draw_tile(&mut s, seat_u, true) { /* wall ended */ }
                    }
                    "bugang" => {
                        let tile = action["data"]["tile"]
                            .as_str()
                            .and_then(parse_tile)
                            .ok_or_else(|| GameError("没有这张牌".into()))?;
                        if !s.melds[seat as usize]
                            .iter()
                            .any(|m| m.kind == "peng" && m.tile == tile)
                            || !s.hands[seat as usize].contains(&tile)
                        {
                            return Err(GameError("不能补杠".into()));
                        }
                        remove_tiles(&mut s.hands[seat as usize], &[tile])?;
                        if let Some(m) = s.melds[seat as usize]
                            .iter_mut()
                            .find(|m| m.kind == "peng" && m.tile == tile)
                        {
                            m.kind = "gang".into();
                        }
                        open_claim(&mut s, tile, seat_u, "bugang");
                    }
                    _ => return Err(GameError("未知动作".into())),
                }
            }
            _ => return Err(GameError("这个阶段不能出牌".into())),
        }
        Ok((serde_json::to_value(s).unwrap(), Vec::new()))
    }

    fn is_terminal(&self, state: &Value) -> bool {
        Self::load(state).map(|s| s.terminal).unwrap_or(true)
    }

    fn result(&self, state: &Value) -> Result<GameResult, GameError> {
        let s = Self::load(state)?;
        if !s.terminal {
            return Err(GameError("对局还没结束".into()));
        }
        let scores: Vec<f64> = s.points.iter().map(|p| *p as f64).collect();
        Ok(GameResult {
            scores: scores.clone(),
            ranking: ranking_from_scores(&scores),
            teams: None,
            reason: s.reason.unwrap_or_else(|| "mahjong".into()),
        })
    }

    fn timeout_decision(
        &self,
        state: &Value,
        seat: Seat,
        _: TimeoutPolicy,
        _: &mut dyn ArenaRng,
    ) -> Result<TimeoutDecision, GameError> {
        let s = Self::load(state)?;
        let legal = self.legal_actions(state, seat).unwrap_or_default();
        if legal.iter().any(|a| a["type"] == "hu") {
            return Ok(TimeoutDecision {
                policy: TimeoutPolicy::AutoAction,
                action: Some(json!({"type":"hu"})),
            });
        }
        match s.phase.as_str() {
            "exchange" => {
                let tiles: Vec<String> = most_three(&s.hands[seat as usize])
                    .into_iter()
                    .map(tile_str)
                    .collect();
                Ok(TimeoutDecision {
                    policy: TimeoutPolicy::AutoAction,
                    action: Some(json!({"type":"exchange","data":{"tiles": tiles}})),
                })
            }
            "dingque" => {
                let suit = suit_name(fewest_suit(&s.hands[seat as usize]));
                Ok(TimeoutDecision {
                    policy: TimeoutPolicy::AutoAction,
                    action: Some(json!({"type":"dingque","data":{"suit": suit}})),
                })
            }
            "claim" => Ok(TimeoutDecision {
                policy: TimeoutPolicy::AutoPass,
                action: Some(json!({"type":"pass"})),
            }),
            _ => {
                let hand = &s.hands[seat as usize];
                let miss = s.dingque[seat as usize];
                let ready = tenpai(hand, &s.melds[seat as usize], miss);
                let tile = if ready {
                    s.last_draw.or_else(|| hand.last().copied())
                } else if let Some(miss) = miss {
                    hand.iter()
                        .copied()
                        .find(|t| t / 9 == miss)
                        .or(s.last_draw)
                        .or_else(|| hand.last().copied())
                } else {
                    s.last_draw.or_else(|| hand.last().copied())
                };
                let tile = tile.ok_or_else(|| GameError("没有可打的牌".into()))?;
                Ok(TimeoutDecision {
                    policy: TimeoutPolicy::AutoAction,
                    action: Some(json!({"type":"discard","data":{"tile": tile_str(tile)}})),
                })
            }
        }
    }
}

const MJ_RULES: &str = r#"# 四川麻将 · 血战到底

108 张，只有万、条、筒。四人各 13 张。默认先换三张（顺时针交给下家），再定缺。这两步都是承诺-揭示，同时进行。

不能吃。可碰、明杠、暗杠、补杠。点炮和自摸都算。一家胡牌后退出，其余继续，直到三家胡或牌墙摸完。摸完后查叫、查花猪。

番：平胡 1，对对胡 2，七对 4，龙七对 8，金钩钓 4，清一色再加 4。杠上花、杠上炮、抢杠、海底各加 2，自摸加 1，封顶 16。

换牌超时交出数量最多的一门三张。定缺超时缺最少的一门。打牌超时：听牌则打出刚摸的牌，否则优先打定缺。应牌超时视为过。连续超时继续托管。
"#;

pub fn ranking_from_scores(scores: &[f64]) -> Vec<Vec<Seat>> {
    crate::game::ranking_from_scores(scores)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiles(s: &str) -> Vec<u8> {
        s.split_whitespace()
            .map(|t| parse_tile(t).unwrap())
            .collect()
    }

    #[test]
    fn standard_seven_pairs_and_dingque_block() {
        let hand = tiles("1m 1m 2m 2m 3m 3m 4m 4m 5m 5m 6m 6m 7s 7s");
        assert!(can_win(&hand, &[]));
        assert_eq!(fan(&hand, &[], false, 0).1, "七对");
        let pure = tiles("1m 1m 2m 2m 3m 3m 4m 4m 5m 5m 6m 6m 7m 7m");
        assert_eq!(fan(&pure, &[], false, 0).1, "清一色七对");
        let dragon = tiles("1m 1m 1m 1m 2m 2m 3m 3m 4m 4m 5s 5s 6s 6s");
        assert_eq!(fan(&dragon, &[], false, 0).0, 8);
        let mixed = tiles("1m 2m 3m 4m 5m 6m 7m 8m 9m 1s 1s 1s 9p 9p");
        assert!(can_win(&mixed, &[]));
        assert!(!can_win(
            &tiles("1m 1m 1m 2m 3m 4m 5m 6m 7m 8m 9m 9m 9s 9s"),
            &[]
        ));
        assert!(tenpai(
            &tiles("1m 1m 2m 2m 3m 3m 4m 4m 5m 5m 6m 6m 7m"),
            &[],
            Some(2)
        ));
        assert!(!tenpai(
            &tiles("1m 1m 2m 2m 3m 3m 4m 4m 5m 5m 6m 6m 1s"),
            &[],
            Some(1)
        ));
    }

    #[test]
    fn exchange_dingque_and_discard_reaches_a_decision() {
        let g = Mahjong;
        let mut rng = crate::game::SeededRng::from_seed_seq(&[6; 32], 0);
        let mut state = g
            .setup(
                &Config {
                    players: 4,
                    variant: "xuezhan".into(),
                    options: json!({}),
                },
                &mut rng,
            )
            .unwrap()
            .0;
        assert_eq!(
            Mahjong::load(&state)
                .unwrap()
                .hands
                .iter()
                .map(|h| h.len())
                .sum::<usize>(),
            52
        );
        for seat in 0..4 {
            let hand = Mahjong::load(&state).unwrap().hands[seat].clone();
            let tiles: Vec<String> = most_three(&hand).into_iter().map(tile_str).collect();
            assert_eq!(tiles.len(), 3, "opening hand always has a suit of three");
            state = g
                .apply(
                    &state,
                    seat as i32,
                    &json!({"type":"exchange","data":{"tiles": tiles}}),
                    &mut rng,
                )
                .unwrap()
                .0;
        }
        assert_eq!(Mahjong::load(&state).unwrap().phase, "dingque");
        for seat in 0..4 {
            let hand = Mahjong::load(&state).unwrap().hands[seat].clone();
            let suit = suit_name(fewest_suit(&hand));
            state = g
                .apply(
                    &state,
                    seat as i32,
                    &json!({"type":"dingque","data":{"suit": suit}}),
                    &mut rng,
                )
                .unwrap()
                .0;
        }
        let s = Mahjong::load(&state).unwrap();
        assert_eq!(s.phase, "play");
        assert_eq!(s.hands[0].len(), 14);
        let view = g
            .view(
                &state,
                &Viewer {
                    seat: Some(1),
                    omniscient: false,
                },
            )
            .unwrap();
        assert!(view["hands"][1].is_array());
        assert!(view["hands"][0].is_null());
        let tile = tile_str(s.hands[0][0]);
        let state = g
            .apply(
                &state,
                0,
                &json!({"type":"discard","data":{"tile": tile}}),
                &mut rng,
            )
            .unwrap()
            .0;
        assert!(
            !Mahjong::load(&state).unwrap().terminal
                || Mahjong::load(&state).unwrap().phase == "claim"
                || Mahjong::load(&state).unwrap().phase == "play"
        );
    }
}
