//! Glicko-2 for two-player games, OpenSkill Plackett-Luce for three and four.
//!
//! Display score for OpenSkill is `1500 + (μ − 25) × 42`, display RD is `σ × 42`.
//! Glicko-2 starts at 1500 / RD 350 / σ 0.06, τ = 0.5. Ranked matches only.

use serde_json::{json, Value};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

pub const SEASON: &str = "2026";
const SCALE: f64 = 173.7178;
const GLICKO_TAU: f64 = 0.5;
const OS_BETA: f64 = 25.0 / 6.0;
const OS_TAU: f64 = 25.0 / 300.0;

#[derive(Clone, Debug)]
pub struct Rating {
    pub system: &'static str,
    pub rating: f64,
    pub rd: f64,
    pub volatility: f64,
    pub mu: f64,
    pub sigma: f64,
    pub games: i32,
    pub wins: i32,
}

impl Rating {
    pub fn glicko_default() -> Self {
        let rd = 350.0;
        let rating = 1500.0;
        Self {
            system: "glicko2",
            rating,
            rd,
            volatility: 0.06,
            mu: (rating - 1500.0) / SCALE,
            sigma: rd / SCALE,
            games: 0,
            wins: 0,
        }
    }

    pub fn openskill_default() -> Self {
        let mu = 25.0;
        let sigma = 25.0 / 3.0;
        Self {
            system: "openskill",
            rating: 1500.0 + (mu - 25.0) * 42.0,
            rd: sigma * 42.0,
            volatility: sigma,
            mu,
            sigma,
            games: 0,
            wins: 0,
        }
    }
}

pub fn glicko2(r: f64, rd: f64, sigma: f64, opp_r: f64, opp_rd: f64, score: f64) -> (f64, f64, f64) {
    let mu = (r - 1500.0) / SCALE;
    let phi = rd / SCALE;
    let mu_j = (opp_r - 1500.0) / SCALE;
    let phi_j = opp_rd / SCALE;
    let g = 1.0 / (1.0 + 3.0 * phi_j * phi_j / std::f64::consts::PI.powi(2)).sqrt();
    let e = (1.0 / (1.0 + (-g * (mu - mu_j)).exp())).clamp(1e-6, 1.0 - 1e-6);
    let v = 1.0 / (g * g * e * (1.0 - e));
    let delta = v * g * (score - e);
    let sigma_new = volatility(sigma, phi, v, delta);
    let phi_star = (phi * phi + sigma_new * sigma_new).sqrt();
    let phi_new = 1.0 / (1.0 / (phi_star * phi_star) + 1.0 / v).sqrt();
    let mu_new = mu + phi_new * phi_new * g * (score - e);
    let r_new = SCALE * mu_new + 1500.0;
    let rd_new = (SCALE * phi_new).clamp(30.0, 350.0);
    (r_new, rd_new, sigma_new)
}

fn volatility(sigma: f64, phi: f64, v: f64, delta: f64) -> f64 {
    let a = (sigma * sigma).ln();
    let phi2 = phi * phi;
    let f = |x: f64| {
        let ex = x.exp();
        let num = ex * (delta * delta - phi2 - v - ex);
        let den = 2.0 * (phi2 + v + ex).powi(2);
        num / den - (x - a) / (GLICKO_TAU * GLICKO_TAU)
    };
    let mut aa = a;
    let mut b = if delta * delta > phi2 + v {
        (delta * delta - phi2 - v).ln()
    } else {
        let mut k = 1.0;
        while f(a - k * GLICKO_TAU) < 0.0 { k += 1.0; }
        a - k * GLICKO_TAU
    };
    let mut fa = f(aa);
    let mut fb = f(b);
    for _ in 0..80 {
        if (b - aa).abs() <= 1e-6 { break; }
        let c = aa + (aa - b) * fa / (fb - fa);
        let fc = f(c);
        if fc * fb <= 0.0 {
            aa = b;
            fa = fb;
        } else {
            fa /= 2.0;
        }
        b = c;
        fb = fc;
    }
    (aa / 2.0).exp()
}

/// Plackett-Luce. `ranks` uses 0 as the best place; tied players share a rank.
pub fn openskill(mus: &[f64], sigmas: &[f64], ranks: &[usize]) -> Vec<(f64, f64)> {
    let n = mus.len();
    let c = mus.iter().zip(sigmas).map(|(_, s)| s * s + OS_BETA * OS_BETA).sum::<f64>().sqrt();
    let mut out = Vec::with_capacity(n);
    let all_tied = ranks.iter().all(|r| *r == ranks[0]);
    for i in 0..n {
        let (omega, delta) = if all_tied {
            (0.0, 0.15)
        } else {
            let mut omega = 0.0;
            let mut delta = 0.0;
            let mut thresholds: Vec<usize> = ranks.iter().copied().filter(|r| *r <= ranks[i]).collect();
            thresholds.sort_unstable();
            thresholds.dedup();
            for t in thresholds {
                let aq = ranks.iter().filter(|r| **r == t).count() as f64;
                let denom: f64 = (0..n).filter(|&j| ranks[j] >= t).map(|j| (mus[j] / c).exp()).sum();
                let p = (mus[i] / c).exp() / denom;
                if t == ranks[i] { omega += (1.0 - p) / aq; } else { omega += (0.0 - p) / aq; }
                delta += p * (1.0 - p) / aq;
            }
            (omega, delta)
        };
        let sigma2 = sigmas[i] * sigmas[i];
        let mu_new = mus[i] + (sigma2 / c) * omega;
        let sigma_new = (sigma2 * (1.0 - (sigma2 / (c * c)) * delta).max(1e-4) + OS_TAU * OS_TAU).sqrt();
        out.push((mu_new, sigma_new));
    }
    out
}

pub fn display_openskill(mu: f64, sigma: f64) -> (f64, f64) {
    (1500.0 + (mu - 25.0) * 42.0, sigma * 42.0)
}

fn ranks_from_scores(scores: &[f64]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..scores.len()).collect();
    order.sort_by(|a, b| scores[*b].partial_cmp(&scores[*a]).unwrap_or(std::cmp::Ordering::Equal));
    let mut ranks = vec![0; scores.len()];
    for (place, seat) in order.iter().enumerate() {
        if place > 0 && (scores[*seat] - scores[order[place - 1]]).abs() < 1e-9 {
            ranks[*seat] = ranks[order[place - 1]];
        } else {
            ranks[*seat] = place;
        }
    }
    ranks
}

pub async fn apply(
    tx: &mut Transaction<'_, Postgres>,
    match_id: Uuid,
    game_id: &str,
    players: &[(Uuid, f64)],
) -> anyhow::Result<()> {
    if players.len() < 2 { return Ok(()); }
    let system = if players.len() == 2 { "glicko2" } else { "openskill" };
    let mut current = Vec::new();
    for (agent, _) in players {
        current.push(load(tx, *agent, game_id, system).await?);
    }
    let scores: Vec<f64> = players.iter().map(|(_, s)| *s).collect();
    let next: Vec<Rating> = if players.len() == 2 {
        let (r0, rd0, v0) = glicko2(current[0].rating, current[0].rd, current[0].volatility, current[1].rating, current[1].rd, score01(scores[0], scores[1]));
        let (r1, rd1, v1) = glicko2(current[1].rating, current[1].rd, current[1].volatility, current[0].rating, current[0].rd, score01(scores[1], scores[0]));
        vec![pack_glicko(&current[0], r0, rd0, v0, scores[0]), pack_glicko(&current[1], r1, rd1, v1, scores[1])]
    } else {
        let mus: Vec<f64> = current.iter().map(|r| r.mu).collect();
        let sigmas: Vec<f64> = current.iter().map(|r| r.sigma).collect();
        let ranks = ranks_from_scores(&scores);
        let updated = openskill(&mus, &sigmas, &ranks);
        updated.into_iter().enumerate().map(|(i, (mu, sigma))| {
            let (rating, rd) = display_openskill(mu, sigma);
            let mut row = current[i].clone();
            row.mu = mu;
            row.sigma = sigma;
            row.rating = rating;
            row.rd = rd;
            row.volatility = sigma;
            row.games += 1;
            if scores[i] > 0.0 { row.wins += 1; }
            row
        }).collect()
    };
    for (i, row) in next.iter().enumerate() {
        let before = &current[i];
        sqlx::query(
            r#"INSERT INTO ratings (agent_id, game_id, season, system, rating, rd, volatility, mu, sigma, games, wins, updated_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11, now())
               ON CONFLICT (agent_id, game_id, season) DO UPDATE SET
                 system = EXCLUDED.system, rating = EXCLUDED.rating, rd = EXCLUDED.rd,
                 volatility = EXCLUDED.volatility, mu = EXCLUDED.mu, sigma = EXCLUDED.sigma,
                 games = EXCLUDED.games, wins = EXCLUDED.wins, updated_at = now()"#,
        )
        .bind(players[i].0)
        .bind(game_id)
        .bind(SEASON)
        .bind(row.system)
        .bind(row.rating)
        .bind(row.rd)
        .bind(row.volatility)
        .bind(row.mu)
        .bind(row.sigma)
        .bind(row.games)
        .bind(row.wins)
        .execute(&mut **tx)
        .await?;
        sqlx::query(
            r#"INSERT INTO rating_events (match_id, agent_id, game_id, season, before_rating, after_rating, before_rd, after_rd, score)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)"#,
        )
        .bind(match_id)
        .bind(players[i].0)
        .bind(game_id)
        .bind(SEASON)
        .bind(before.rating)
        .bind(row.rating)
        .bind(before.rd)
        .bind(row.rd)
        .bind(scores[i])
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

fn score01(mine: f64, theirs: f64) -> f64 {
    if (mine - theirs).abs() < 1e-9 { 0.5 } else if mine > theirs { 1.0 } else { 0.0 }
}

fn pack_glicko(prev: &Rating, r: f64, rd: f64, vol: f64, score: f64) -> Rating {
    Rating {
        system: "glicko2",
        rating: r,
        rd,
        volatility: vol,
        mu: (r - 1500.0) / SCALE,
        sigma: rd / SCALE,
        games: prev.games + 1,
        wins: prev.wins + if score > 0.0 { 1 } else { 0 },
    }
}

async fn load(tx: &mut Transaction<'_, Postgres>, agent: Uuid, game_id: &str, system: &str) -> anyhow::Result<Rating> {
    let row = sqlx::query_as::<_, (String, f64, f64, f64, f64, f64, i32, i32)>(
        r#"SELECT system, rating, rd, volatility, mu, sigma, games, wins
           FROM ratings WHERE agent_id = $1 AND game_id = $2 AND season = $3"#,
    )
    .bind(agent)
    .bind(game_id)
    .bind(SEASON)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(match row {
        Some((system, rating, rd, volatility, mu, sigma, games, wins)) => Rating {
            system: if system == "openskill" { "openskill" } else { "glicko2" },
            rating, rd, volatility, mu, sigma, games, wins,
        },
        None => if system == "openskill" { Rating::openskill_default() } else { Rating::glicko_default() },
    })
}

pub fn provisional(games: i32, rd: f64) -> bool {
    games < 10 || rd > 110.0
}

pub async fn leaderboard(pool: &sqlx::PgPool, game: &str) -> anyhow::Result<Value> {
    if game == "overall" {
        let rows = sqlx::query_as::<_, (Uuid, String, String, String, f64, f64, i32)>(
            r#"SELECT a.id, a.name, u.handle, r.game_id, r.rating, r.rd, r.games
               FROM ratings r
               JOIN agents a ON a.id = r.agent_id
               JOIN users u ON u.id = a.owner_id
               WHERE r.season = $1 AND r.games >= 10"#,
        )
        .bind(SEASON)
        .fetch_all(pool)
        .await?;
        let mut acc: std::collections::HashMap<Uuid, (String, String, f64, f64)> = std::collections::HashMap::new();
        for (id, name, handle, _, rating, rd, games) in rows {
            let weight = (1.0 + games as f64).ln();
            let entry = acc.entry(id).or_insert((name, handle, 0.0, 0.0));
            entry.2 += weight * (rating - 2.0 * rd);
            entry.3 += weight;
        }
        let mut list: Vec<Value> = acc.into_iter().map(|(id, (name, handle, sum, weight))| {
            json!({
                "agent_id": id,
                "name": name,
                "handle": handle,
                "rating": (sum / weight).round(),
                "rd": Value::Null,
                "games": Value::Null,
                "provisional": false,
                "conservative": sum / weight,
            })
        }).collect();
        list.sort_by(|a, b| b["conservative"].as_f64().unwrap_or(0.0).partial_cmp(&a["conservative"].as_f64().unwrap_or(0.0)).unwrap());
        for (i, row) in list.iter_mut().enumerate() { row["rank"] = json!(i + 1); }
        return Ok(json!({"game": "overall", "season": SEASON, "rows": list}));
    }
    let rows = sqlx::query_as::<_, (Uuid, String, String, f64, f64, i32, i32, String)>(
        r#"SELECT a.id, a.name, u.handle, r.rating, r.rd, r.games, r.wins, r.system
           FROM ratings r
           JOIN agents a ON a.id = r.agent_id
           JOIN users u ON u.id = a.owner_id
           WHERE r.game_id = $1 AND r.season = $2
           ORDER BY (r.rating - 2 * r.rd) DESC"#,
    )
    .bind(game)
    .bind(SEASON)
    .fetch_all(pool)
    .await?;
    let list: Vec<Value> = rows.into_iter().enumerate().map(|(i, (id, name, handle, rating, rd, games, wins, system))| {
        json!({
            "rank": i + 1,
            "agent_id": id,
            "name": name,
            "handle": handle,
            "rating": rating.round(),
            "rd": rd.round(),
            "games": games,
            "wins": wins,
            "system": system,
            "provisional": provisional(games, rd),
            "conservative": rating - 2.0 * rd,
        })
    }).collect();
    Ok(json!({"game": game, "season": SEASON, "rows": list}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glicko_winner_climbs_and_rd_shrinks() {
        let (r, rd, _) = glicko2(1500.0, 350.0, 0.06, 1500.0, 350.0, 1.0);
        let (l, lrd, _) = glicko2(1500.0, 350.0, 0.06, 1500.0, 350.0, 0.0);
        assert!(r > 1500.0, "{r}");
        assert!(l < 1500.0, "{l}");
        assert!(rd < 350.0 && lrd < 350.0);
        let (d, _, _) = glicko2(1500.0, 350.0, 0.06, 1500.0, 350.0, 0.5);
        assert!((d - 1500.0).abs() < 1.0, "{d}");
    }

    #[test]
    fn openskill_orders_a_three_player_table() {
        let mus = [25.0, 25.0, 25.0];
        let sigmas = [25.0 / 3.0; 3];
        let next = openskill(&mus, &sigmas, &[0, 1, 1]);
        assert!(next[0].0 > 25.0);
        assert!(next[1].0 < 25.0 && next[2].0 < 25.0);
        let (rating, rd) = display_openskill(25.0, 25.0 / 3.0);
        assert!((rating - 1500.0).abs() < 1e-6);
        assert!((rd - 350.0).abs() < 1.0);
        let tied = openskill(&mus, &sigmas, &[0, 0, 0]);
        assert!((tied[0].0 - 25.0).abs() < 1e-6);
    }
}
