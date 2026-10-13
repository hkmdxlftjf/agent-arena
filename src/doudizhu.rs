//! 斗地主。三人，54 张，种子发牌。
//!
//! 叫分 1/2/3 或不要。三家都不要则重发，最多三次，然后和棋。
//! 地主拿走底牌并先出。牌型：单、对、三、三带、顺子、连对、飞机、四带、炸弹、王炸。
//! 炸弹和春天翻倍。手牌只对本人可见，底牌在叫分结束后公开。
//!
//! 叫分超时不要；领出超时出最小单张；跟牌超时不出。连续超时继续托管，不判负。

use crate::game::*;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Pattern {
    kind: &'static str,
    rank: u8,
    len: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Trick {
    seat: u8,
    cards: Vec<String>,
    kind: String,
    rank: u8,
    len: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct DState {
    phase: String,
    hands: Vec<Vec<String>>,
    bottom: Vec<String>,
    turn: u8,
    bid: i32,
    bidder: Option<u8>,
    #[serde(default)]
    bid_passes: u8,
    #[serde(default)]
    redeals: u8,
    last: Option<Trick>,
    #[serde(default)]
    trick_passes: u8,
    #[serde(default)]
    multiplier: i32,
    #[serde(default)]
    plays: Vec<u8>,
    landlord: Option<u8>,
    winner: Option<i8>,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    points: Vec<i64>,
}

pub struct DouDizhu;

impl DouDizhu {
    fn load(state: &Value) -> Result<DState, GameError> {
        serde_json::from_value(state.clone()).map_err(|e| GameError(e.to_string()))
    }
}

fn deck() -> Vec<String> {
    let mut d = Vec::new();
    for suit in ["s", "h", "d", "c"] {
        for rank in [
            "3", "4", "5", "6", "7", "8", "9", "T", "J", "Q", "K", "A", "2",
        ] {
            d.push(format!("{rank}{suit}"));
        }
    }
    d.push("X".into());
    d.push("D".into());
    d
}

fn rank_of(card: &str) -> u8 {
    match card.as_bytes().first().copied().unwrap_or(b'?') {
        b'3' => 0,
        b'4' => 1,
        b'5' => 2,
        b'6' => 3,
        b'7' => 4,
        b'8' => 5,
        b'9' => 6,
        b'T' => 7,
        b'J' => 8,
        b'Q' => 9,
        b'K' => 10,
        b'A' => 11,
        b'2' => 12,
        b'X' => 13,
        b'D' => 14,
        _ => 255,
    }
}

fn shuffle(rng: &mut dyn ArenaRng) -> (Vec<Vec<String>>, Vec<String>) {
    let mut d = deck();
    for i in (1..d.len()).rev() {
        let j = rng.int_n(i + 1);
        d.swap(i, j);
    }
    let mut hands = vec![d[0..17].to_vec(), d[17..34].to_vec(), d[34..51].to_vec()];
    for hand in &mut hands {
        sort_cards(hand);
    }
    (hands, d[51..54].to_vec())
}

fn sort_cards(cards: &mut [String]) {
    cards.sort_by_key(|c| (rank_of(c), c.clone()));
}

fn counts(cards: &[String]) -> [u8; 15] {
    let mut c = [0u8; 15];
    for card in cards {
        let r = rank_of(card);
        if (r as usize) < 15 {
            c[r as usize] += 1;
        }
    }
    c
}

fn consecutive(ranks: &[u8]) -> bool {
    ranks.windows(2).all(|w| w[1] == w[0] + 1)
}

fn parse_pattern(cards: &[String]) -> Option<Pattern> {
    if cards.is_empty() || cards.iter().any(|c| rank_of(c) == 255) {
        return None;
    }
    let n = cards.len();
    let c = counts(cards);
    if n == 2 && c[13] == 1 && c[14] == 1 {
        return Some(Pattern {
            kind: "rocket",
            rank: 14,
            len: 1,
        });
    }
    let groups: Vec<(u8, u8)> = (0..15u8)
        .filter(|r| c[*r as usize] > 0)
        .map(|r| (r, c[r as usize]))
        .collect();
    if groups.len() == 1 {
        let (r, k) = groups[0];
        return Some(match k {
            1 => Pattern {
                kind: "single",
                rank: r,
                len: 1,
            },
            2 => Pattern {
                kind: "pair",
                rank: r,
                len: 1,
            },
            3 => Pattern {
                kind: "triple",
                rank: r,
                len: 1,
            },
            4 => Pattern {
                kind: "bomb",
                rank: r,
                len: 1,
            },
            _ => return None,
        });
    }
    if n == 4 && groups.iter().any(|g| g.1 == 3) {
        let r = groups.iter().find(|g| g.1 == 3).unwrap().0;
        return Some(Pattern {
            kind: "triple1",
            rank: r,
            len: 1,
        });
    }
    if n == 5 && groups.iter().any(|g| g.1 == 3) && groups.iter().any(|g| g.1 == 2) {
        let r = groups.iter().find(|g| g.1 == 3).unwrap().0;
        return Some(Pattern {
            kind: "triple2",
            rank: r,
            len: 1,
        });
    }
    if n == 6 && groups.iter().any(|g| g.1 == 4) {
        let r = groups.iter().find(|g| g.1 == 4).unwrap().0;
        return Some(Pattern {
            kind: "four2",
            rank: r,
            len: 1,
        });
    }
    if n == 8
        && groups.iter().filter(|g| g.1 == 4).count() == 1
        && groups.iter().filter(|g| g.1 == 2).count() == 2
    {
        let r = groups.iter().find(|g| g.1 == 4).unwrap().0;
        return Some(Pattern {
            kind: "four4",
            rank: r,
            len: 1,
        });
    }
    if groups.iter().all(|g| g.1 == 1) && n >= 5 {
        let ranks: Vec<u8> = groups.iter().map(|g| g.0).collect();
        if consecutive(&ranks) && ranks.iter().all(|r| *r < 12) {
            return Some(Pattern {
                kind: "straight",
                rank: *ranks.last().unwrap(),
                len: n as u8,
            });
        }
    }
    if groups.iter().all(|g| g.1 == 2) && groups.len() >= 3 {
        let ranks: Vec<u8> = groups.iter().map(|g| g.0).collect();
        if consecutive(&ranks) && ranks.iter().all(|r| *r < 12) {
            return Some(Pattern {
                kind: "pairseq",
                rank: *ranks.last().unwrap(),
                len: ranks.len() as u8,
            });
        }
    }
    let triple_ranks: Vec<u8> = groups
        .iter()
        .filter(|g| g.1 >= 3 && g.0 < 12)
        .map(|g| g.0)
        .collect();
    if triple_ranks.len() >= 2 {
        for k in (2..=triple_ranks.len()).rev() {
            for w in triple_ranks.windows(k) {
                if !consecutive(w) {
                    continue;
                }
                let mut left = c;
                for r in w {
                    left[*r as usize] -= 3;
                }
                let kick: Vec<u8> = (0..15)
                    .flat_map(|r| std::iter::repeat_n(r as u8, left[r] as usize))
                    .collect();
                let body = *w.last().unwrap();
                let len = k as u8;
                if kick.is_empty() {
                    return Some(Pattern {
                        kind: "plane",
                        rank: body,
                        len,
                    });
                }
                if kick.len() == k && kick.iter().all(|r| left[*r as usize] <= 2 || w.contains(r)) {
                    // k single kickers (a leftover from a four-of-a-kind body is allowed)
                    let singles_ok = kick.len() == k;
                    if singles_ok && kick.iter().all(|r| *r < 15) {
                        // Prefer pairs when the kickers form exactly k pairs.
                        if k >= 2 && kick.len() == k * 2 { /* handled below */ }
                        if kick.len() == k {
                            return Some(Pattern {
                                kind: "plane1",
                                rank: body,
                                len,
                            });
                        }
                    }
                }
                if kick.len() == k * 2 {
                    let mut ok = true;
                    let mut i = 0;
                    let mut pair_ranks = Vec::new();
                    while i < kick.len() {
                        if i + 1 < kick.len() && kick[i] == kick[i + 1] {
                            pair_ranks.push(kick[i]);
                            i += 2;
                        } else {
                            ok = false;
                            break;
                        }
                    }
                    if ok && pair_ranks.len() == k {
                        return Some(Pattern {
                            kind: "plane2",
                            rank: body,
                            len,
                        });
                    }
                }
            }
        }
    }
    None
}

fn beats(play: &Pattern, last: &Pattern) -> bool {
    if play.kind == "rocket" {
        return true;
    }
    if last.kind == "rocket" {
        return false;
    }
    if play.kind == "bomb" && last.kind != "bomb" {
        return true;
    }
    if play.kind == "bomb" && last.kind == "bomb" {
        return play.rank > last.rank;
    }
    if last.kind == "bomb" {
        return false;
    }
    play.kind == last.kind && play.len == last.len && play.rank > last.rank
}

fn fresh(rng: &mut dyn ArenaRng, redeals: u8) -> DState {
    let (hands, bottom) = shuffle(rng);
    DState {
        phase: "bid".into(),
        hands,
        bottom,
        turn: 0,
        bid: 0,
        bidder: None,
        bid_passes: 0,
        redeals,
        last: None,
        trick_passes: 0,
        multiplier: 1,
        plays: vec![0, 0, 0],
        landlord: None,
        winner: None,
        reason: None,
        points: vec![0, 0, 0],
    }
}

fn settle(s: &mut DState, winner_seat: u8) {
    let landlord = s.landlord.unwrap_or(0);
    let mut mult = s.multiplier.max(1);
    let farmers_played = (0..3).any(|i| i != landlord as usize && s.plays[i] > 0);
    let spring = (winner_seat == landlord && !farmers_played)
        || (winner_seat != landlord && s.plays[landlord as usize] <= 1);
    if spring {
        mult *= 2;
    }
    let unit = s.bid as i64 * mult as i64;
    let mut points = vec![0i64; 3];
    if winner_seat == landlord {
        points[landlord as usize] = 2 * unit;
        for (i, point) in points.iter_mut().enumerate() {
            if i != landlord as usize {
                *point = -unit;
            }
        }
        s.reason = Some(if spring {
            "landlord_spring".into()
        } else {
            "landlord".into()
        });
    } else {
        points[landlord as usize] = -2 * unit;
        for (i, point) in points.iter_mut().enumerate() {
            if i != landlord as usize {
                *point = unit;
            }
        }
        s.reason = Some(if spring {
            "farmer_spring".into()
        } else {
            "farmers".into()
        });
    }
    s.points = points;
    s.winner = Some(winner_seat as i8);
    s.phase = "over".into();
    s.multiplier = mult;
}

fn take_cards(hand: &mut Vec<String>, cards: &[String]) -> Result<(), GameError> {
    let mut bag: HashMap<String, usize> = HashMap::new();
    for c in hand.iter() {
        *bag.entry(c.clone()).or_default() += 1;
    }
    for c in cards {
        let n = bag.entry(c.clone()).or_default();
        if *n == 0 {
            return Err(GameError("手里没有这些牌".into()));
        }
        *n -= 1;
    }
    hand.clear();
    for (c, n) in bag {
        for _ in 0..n {
            hand.push(c.clone());
        }
    }
    sort_cards(hand);
    Ok(())
}

fn cards_of(action: &Value) -> Vec<String> {
    action["data"]["cards"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

fn sample_plays(hand: &[String]) -> Vec<Vec<String>> {
    let mut by: HashMap<u8, Vec<String>> = HashMap::new();
    for c in hand {
        by.entry(rank_of(c)).or_default().push(c.clone());
    }
    let mut ranks: Vec<u8> = by.keys().copied().collect();
    ranks.sort();
    let mut plays = Vec::new();
    for r in &ranks {
        let cs = &by[r];
        plays.push(vec![cs[0].clone()]);
        if cs.len() >= 2 {
            plays.push(cs[..2].to_vec());
        }
        if cs.len() >= 3 {
            plays.push(cs[..3].to_vec());
        }
        if cs.len() >= 4 {
            plays.push(cs[..4].to_vec());
        }
    }
    if by.contains_key(&13) && by.contains_key(&14) {
        plays.push(vec!["X".into(), "D".into()]);
    }
    let chain: Vec<u8> = ranks.into_iter().filter(|r| *r < 12).collect();
    for i in 0..chain.len() {
        for j in i + 5..=chain.len() {
            if consecutive(&chain[i..j]) && chain[i..j].iter().all(|r| !by[r].is_empty()) {
                plays.push(chain[i..j].iter().map(|r| by[r][0].clone()).collect());
            }
        }
        for j in i + 3..=chain.len() {
            if consecutive(&chain[i..j]) && chain[i..j].iter().all(|r| by[r].len() >= 2) {
                let mut p = Vec::new();
                for r in &chain[i..j] {
                    p.extend(by[r][..2].iter().cloned());
                }
                plays.push(p);
            }
        }
    }
    plays
}

impl Game for DouDizhu {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "doudizhu".into(),
            name: "斗地主".into(),
            version: "1.0.0".into(),
            min_players: 3,
            max_players: 3,
            traits: vec![
                Trait::Card,
                Trait::HiddenInfo,
                Trait::Chance,
                Trait::Teams,
                Trait::MultiPhase,
            ],
            variants: vec![Variant {
                id: "standard".into(),
                name: "叫分".into(),
                options: None,
            }],
            default_time: TimeControl {
                per_action_ms: 60_000,
                max_consecutive_timeouts: 3,
                grace_ms: 2_000,
            },
            timeout_policy: TimeoutPolicy::AutoPass,
            allowed_timeout_policies: vec![TimeoutPolicy::AutoPass, TimeoutPolicy::AutoAction],
            action_schema: json!({"type":"object"}),
            rules_markdown: DOU_RULES.into(),
            renderer: "card-table".into(),
            rated: true,
            engine: "native".into(),
            description: "三人扑克。种子发牌，叫分，地主对两名农民。炸弹和春天翻倍。".into(),
            playable: true,
        }
    }

    fn setup(
        &self,
        cfg: &Config,
        rng: &mut dyn ArenaRng,
    ) -> Result<(Value, Vec<Value>), GameError> {
        if cfg.variant != "standard" || cfg.players != 3 {
            return Err(GameError("斗地主固定三人标准规则".into()));
        }
        Ok((serde_json::to_value(fresh(rng, 0)).unwrap(), Vec::new()))
    }

    fn to_act(&self, state: &Value) -> Vec<Seat> {
        let Ok(s) = Self::load(state) else {
            return Vec::new();
        };
        if s.winner.is_some() {
            Vec::new()
        } else {
            vec![s.turn as Seat]
        }
    }

    fn view(&self, state: &Value, viewer: &Viewer) -> Result<Value, GameError> {
        let s = Self::load(state)?;
        let show_all = viewer.omniscient;
        let hands: Vec<Value> = s
            .hands
            .iter()
            .enumerate()
            .map(|(i, h)| {
                if show_all || viewer.seat == Some(i as i32) {
                    json!(h)
                } else {
                    Value::Null
                }
            })
            .collect();
        let bottom = if s.landlord.is_some() || show_all {
            json!(s.bottom)
        } else {
            Value::Null
        };
        Ok(json!({
            "renderer": "card-table",
            "phase": s.phase,
            "hands": hands,
            "counts": s.hands.iter().map(|h| h.len()).collect::<Vec<_>>(),
            "bottom": bottom,
            "bid": s.bid,
            "landlord": s.landlord,
            "multiplier": s.multiplier,
            "last": s.last,
            "toAct": self.to_act(state),
            "winner": s.winner,
            "reason": s.reason,
            "points": s.points,
        }))
    }

    fn legal_actions(&self, state: &Value, seat: Seat) -> Result<Vec<Value>, GameError> {
        let s = Self::load(state)?;
        if s.winner.is_some() || seat as u8 != s.turn {
            return Ok(vec![json!({"type":"resign"})]);
        }
        let mut actions = Vec::new();
        if s.phase == "bid" {
            actions.push(json!({"type":"bid","data":{"score": 0}}));
            for score in 1..=3 {
                if score > s.bid {
                    actions.push(json!({"type":"bid","data":{"score": score}}));
                }
            }
            actions.push(json!({"type":"resign"}));
            return Ok(actions);
        }
        let hand = &s.hands[seat as usize];
        let last = s.last.as_ref();
        if last.is_some() {
            actions.push(json!({"type":"pass"}));
        }
        for cards in sample_plays(hand) {
            let Some(p) = parse_pattern(&cards) else {
                continue;
            };
            if let Some(prev) = last {
                let prev_p = Pattern {
                    kind: box_kind(&prev.kind),
                    rank: prev.rank,
                    len: prev.len,
                };
                if !beats(&p, &prev_p) {
                    continue;
                }
            }
            actions.push(json!({"type":"play","data":{"cards": cards}}));
        }
        actions.push(json!({"type":"resign"}));
        Ok(actions)
    }

    fn apply(
        &self,
        state: &Value,
        seat: Seat,
        action: &Value,
        rng: &mut dyn ArenaRng,
    ) -> Result<(Value, Vec<Value>), GameError> {
        let mut s = Self::load(state)?;
        if s.winner.is_some() {
            return Err(GameError("对局已结束".into()));
        }
        let kind = action.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if kind == "resign" || kind == "timeout_forfeit" {
            // 三人局认输：该席位记负，其余两家平分胜场（地主认输则农民胜，农民认输则地主胜）。
            if let Some(landlord) = s.landlord {
                if seat as u8 == landlord {
                    settle(&mut s, if landlord == 0 { 1 } else { 0 });
                    s.reason = Some("resign".into());
                } else {
                    settle(&mut s, landlord);
                    s.reason = Some("resign".into());
                }
            } else {
                s.winner = Some(-1);
                s.reason = Some("resign".into());
                s.phase = "over".into();
                s.points = vec![0, 0, 0];
            }
            return Ok((serde_json::to_value(s).unwrap(), Vec::new()));
        }
        if seat as u8 != s.turn {
            return Err(GameError("还没轮到你".into()));
        }
        if s.phase == "bid" {
            if kind != "bid" {
                return Err(GameError("现在要叫分".into()));
            }
            let score = action["data"]["score"]
                .as_i64()
                .ok_or_else(|| GameError("叫分不合法".into()))?;
            if score == 0 {
                s.bid_passes += 1;
            } else if score > s.bid as i64 && score <= 3 {
                s.bid = score as i32;
                s.bidder = Some(seat as u8);
                s.bid_passes = 0;
            } else {
                return Err(GameError("叫分必须高于当前分，或选择不要".into()));
            }
            if s.bid > 0 && s.bid_passes >= 2 {
                let landlord = s.bidder.unwrap();
                s.hands[landlord as usize].extend(s.bottom.clone());
                sort_cards(&mut s.hands[landlord as usize]);
                s.landlord = Some(landlord);
                s.phase = "play".into();
                s.turn = landlord;
                s.multiplier = 1;
                return Ok((serde_json::to_value(s).unwrap(), Vec::new()));
            }
            if s.bid == 0 && s.bid_passes >= 3 {
                if s.redeals >= 2 {
                    s.winner = Some(-1);
                    s.reason = Some("no_bid".into());
                    s.phase = "over".into();
                    s.points = vec![0, 0, 0];
                    return Ok((serde_json::to_value(s).unwrap(), Vec::new()));
                }
                let mut next = fresh(rng, s.redeals + 1);
                next.redeals = s.redeals + 1;
                return Ok((serde_json::to_value(next).unwrap(), Vec::new()));
            }
            s.turn = (s.turn + 1) % 3;
            return Ok((serde_json::to_value(s).unwrap(), Vec::new()));
        }
        if kind == "pass" {
            if s.last.is_none() {
                return Err(GameError("领出不能不出".into()));
            }
            s.trick_passes += 1;
            if s.trick_passes >= 2 {
                let leader = s.last.as_ref().unwrap().seat;
                s.last = None;
                s.trick_passes = 0;
                s.turn = leader;
            } else {
                s.turn = (s.turn + 1) % 3;
            }
            return Ok((serde_json::to_value(s).unwrap(), Vec::new()));
        }
        if kind != "play" {
            return Err(GameError("未知动作".into()));
        }
        let cards = cards_of(action);
        let pattern = parse_pattern(&cards).ok_or_else(|| GameError("不是合法牌型".into()))?;
        if let Some(prev) = &s.last {
            let prev_p = Pattern {
                kind: box_kind(&prev.kind),
                rank: prev.rank,
                len: prev.len,
            };
            if !beats(&pattern, &prev_p) {
                return Err(GameError("压不过上一手".into()));
            }
        }
        take_cards(&mut s.hands[seat as usize], &cards)?;
        if pattern.kind == "bomb" || pattern.kind == "rocket" {
            s.multiplier *= 2;
        }
        s.plays[seat as usize] += 1;
        s.last = Some(Trick {
            seat: seat as u8,
            cards,
            kind: pattern.kind.into(),
            rank: pattern.rank,
            len: pattern.len,
        });
        s.trick_passes = 0;
        if s.hands[seat as usize].is_empty() {
            settle(&mut s, seat as u8);
        } else {
            s.turn = (s.turn + 1) % 3;
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
        if s.winner.is_none() {
            return Err(GameError("对局还没结束".into()));
        }
        let scores: Vec<f64> = s.points.iter().map(|p| *p as f64).collect();
        let ranking = crate::game::ranking_from_scores(&scores);
        let teams = s.landlord.map(|l| {
            let farmers: Vec<Seat> = (0..3).filter(|i| *i != l as i32).collect();
            vec![vec![l as Seat], farmers]
        });
        Ok(GameResult {
            scores,
            ranking,
            teams,
            reason: s.reason.unwrap_or_else(|| "doudizhu".into()),
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
        if s.phase == "bid" {
            return Ok(TimeoutDecision {
                policy: TimeoutPolicy::AutoPass,
                action: Some(json!({"type":"bid","data":{"score":0}})),
            });
        }
        if s.last.is_none() {
            let hand = &s.hands[seat as usize];
            let card = hand
                .iter()
                .min_by_key(|c| rank_of(c))
                .cloned()
                .unwrap_or_else(|| "3s".into());
            return Ok(TimeoutDecision {
                policy: TimeoutPolicy::AutoAction,
                action: Some(json!({"type":"play","data":{"cards":[card]}})),
            });
        }
        Ok(TimeoutDecision {
            policy: TimeoutPolicy::AutoPass,
            action: Some(json!({"type":"pass"})),
        })
    }
}

fn box_kind(kind: &str) -> &'static str {
    match kind {
        "single" => "single",
        "pair" => "pair",
        "triple" => "triple",
        "triple1" => "triple1",
        "triple2" => "triple2",
        "straight" => "straight",
        "pairseq" => "pairseq",
        "plane" => "plane",
        "plane1" => "plane1",
        "plane2" => "plane2",
        "four2" => "four2",
        "four4" => "four4",
        "bomb" => "bomb",
        "rocket" => "rocket",
        _ => "single",
    }
}

const DOU_RULES: &str = r#"# 斗地主

三人，一副 54 张牌。种子洗牌后每人 17 张，留 3 张底牌。叫分从席位 0 开始，可叫 1、2、3 或不要，必须更高。两家不要则地主确定。三家都不要就重发，连续三次则和棋。

地主拿走底牌并先出。牌型：单张、对子、三张、三带一、三带一对、顺子（至少五张，不含 2 和王）、连对（至少三对）、飞机（可带翼）、四带二、炸弹、王炸。炸弹和王炸把倍数翻倍。春天再翻一倍。

手牌只发给对应席位。底牌在叫分结束后公开。叫分超时视为不要；领出超时出最小的一张；跟牌超时不出。连续超时进入托管，不判负。
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patterns_beat_each_other() {
        let p = |cards: &[&str]| {
            parse_pattern(&cards.iter().map(|s| (*s).to_string()).collect::<Vec<_>>()).unwrap()
        };
        assert_eq!(p(&["3s"]).kind, "single");
        assert_eq!(p(&["3s", "3h", "3d", "4s"]).kind, "triple1");
        assert_eq!(p(&["3s", "4h", "5d", "6c", "7s"]).kind, "straight");
        assert!(parse_pattern(
            &["3s", "4h", "5d", "6c", "2s"]
                .into_iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        )
        .is_none());
        assert_eq!(p(&["X", "D"]).kind, "rocket");
        assert!(beats(&p(&["X", "D"]), &p(&["As", "Ah", "Ad", "Ac"])));
        assert!(beats(
            &p(&["4s", "4h", "4d", "4c"]),
            &p(&["3s", "3h", "3d", "3c"])
        ));
        assert!(!beats(&p(&["3s"]), &p(&["4s"])));
        let plane = p(&["3s", "3h", "3d", "4s", "4h", "4d", "5c", "6s"]);
        assert_eq!(plane.kind, "plane1");
    }

    #[test]
    fn seeded_deal_bids_and_plays_out() {
        let g = DouDizhu;
        let mut rng = crate::game::SeededRng::from_seed_seq(&[5; 32], 0);
        let (state, _) = g
            .setup(
                &Config {
                    players: 3,
                    variant: "standard".into(),
                    options: json!({}),
                },
                &mut rng,
            )
            .unwrap();
        let s = DouDizhu::load(&state).unwrap();
        assert_eq!(s.hands.iter().map(|h| h.len()).sum::<usize>(), 51);
        assert_eq!(s.bottom.len(), 3);
        let mut rng = crate::game::SeededRng::from_seed_seq(&[5; 32], 1);
        let state = g
            .apply(
                &state,
                0,
                &json!({"type":"bid","data":{"score":1}}),
                &mut rng,
            )
            .unwrap()
            .0;
        let state = g
            .apply(
                &state,
                1,
                &json!({"type":"bid","data":{"score":0}}),
                &mut rng,
            )
            .unwrap()
            .0;
        let state = g
            .apply(
                &state,
                2,
                &json!({"type":"bid","data":{"score":0}}),
                &mut rng,
            )
            .unwrap()
            .0;
        let s = DouDizhu::load(&state).unwrap();
        assert_eq!(s.landlord, Some(0));
        assert_eq!(s.hands[0].len(), 20);
        assert!(g
            .view(
                &state,
                &Viewer {
                    seat: None,
                    omniscient: false
                }
            )
            .unwrap()["hands"][1]
            .is_null());
        assert!(g
            .view(
                &state,
                &Viewer {
                    seat: Some(1),
                    omniscient: false
                }
            )
            .unwrap()["hands"][1]
            .is_array());

        let mut state = state;
        for _ in 0..80 {
            if g.is_terminal(&state) {
                break;
            }
            let seat = DouDizhu::load(&state).unwrap().turn as i32;
            let legal = g.legal_actions(&state, seat).unwrap();
            let action = if DouDizhu::load(&state).unwrap().last.is_none() {
                legal.into_iter().find(|a| a["type"] == "play").unwrap()
            } else {
                legal.into_iter().find(|a| a["type"] == "pass").unwrap()
            };
            state = g.apply(&state, seat, &action, &mut rng).unwrap().0;
        }
        assert!(
            g.is_terminal(&state),
            "landlord singles should empty the hand"
        );
        let result = g.result(&state).unwrap();
        assert_eq!(result.scores.iter().sum::<f64>(), 0.0);
        assert!(result.scores[0] > 0.0);
    }
}
