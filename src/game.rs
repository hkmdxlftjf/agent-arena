//! Core game contract, capability traits, and the registry.
//!
//! Rules objects are stateless. Match state is a JSON value so it can be
//! stored, replayed, and (later) exchanged with a JS sandbox under the same
//! shape described in the design doc.

use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

pub type Seat = i32;

#[derive(Clone, Debug)]
pub struct Viewer {
    pub seat: Option<Seat>,
    pub omniscient: bool,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TimeoutPolicy {
    AutoPass,
    AutoAction,
    Forfeit,
}

impl TimeoutPolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AutoPass => "auto_pass",
            Self::AutoAction => "auto_action",
            Self::Forfeit => "forfeit",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "auto_pass" => Some(Self::AutoPass),
            "auto_action" => Some(Self::AutoAction),
            "forfeit" => Some(Self::Forfeit),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct TimeoutDecision {
    pub policy: TimeoutPolicy,
    pub action: Option<Value>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeControl {
    pub per_action_ms: u64,
    pub max_consecutive_timeouts: u32,
    pub grace_ms: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct Variant {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Value>,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Trait {
    Board,
    Card,
    Tile,
    HiddenInfo,
    Chance,
    Teams,
    MultiPhase,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub min_players: u8,
    pub max_players: u8,
    pub traits: Vec<Trait>,
    pub variants: Vec<Variant>,
    pub default_time: TimeControl,
    pub timeout_policy: TimeoutPolicy,
    pub allowed_timeout_policies: Vec<TimeoutPolicy>,
    pub action_schema: Value,
    pub rules_markdown: String,
    pub renderer: String,
    pub rated: bool,
    pub engine: String,
    pub description: String,
    pub playable: bool,
}

#[derive(Clone, Debug)]
pub struct Config {
    pub players: u8,
    pub variant: String,
    pub options: Value,
}

#[derive(Clone, Debug, Serialize)]
pub struct GameResult {
    pub scores: Vec<f64>,
    pub ranking: Vec<Vec<Seat>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub teams: Option<Vec<Vec<Seat>>>,
    pub reason: String,
}

#[derive(Debug)]
pub struct GameError(pub String);

impl std::fmt::Display for GameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for GameError {}

pub trait ArenaRng {
    fn next_u64(&mut self) -> u64;
    fn int_n(&mut self, n: usize) -> usize;
}

pub struct SeededRng {
    inner: ChaCha8Rng,
}

impl SeededRng {
    pub fn from_seed_seq(seed: &[u8; 32], seq: i64) -> Self {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(seed);
        hasher.update(seq.to_le_bytes());
        let digest: [u8; 32] = hasher.finalize().into();
        Self {
            inner: ChaCha8Rng::from_seed(digest),
        }
    }
}

impl ArenaRng for SeededRng {
    fn next_u64(&mut self) -> u64 {
        rand::RngCore::next_u64(&mut self.inner)
    }
    fn int_n(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            self.inner.gen_range(0..n)
        }
    }
}

pub trait Game: Send + Sync {
    fn manifest(&self) -> Manifest;
    fn setup(&self, cfg: &Config, rng: &mut dyn ArenaRng) -> Result<(Value, Vec<Value>), GameError>;
    fn to_act(&self, state: &Value) -> Vec<Seat>;
    fn view(&self, state: &Value, viewer: &Viewer) -> Result<Value, GameError>;
    fn legal_actions(&self, state: &Value, seat: Seat) -> Result<Vec<Value>, GameError>;
    fn apply(
        &self,
        state: &Value,
        seat: Seat,
        action: &Value,
        rng: &mut dyn ArenaRng,
    ) -> Result<(Value, Vec<Value>), GameError>;
    fn is_terminal(&self, state: &Value) -> bool;
    fn result(&self, state: &Value) -> Result<GameResult, GameError>;
    fn timeout_decision(
        &self,
        state: &Value,
        seat: Seat,
        policy: TimeoutPolicy,
        rng: &mut dyn ArenaRng,
    ) -> Result<TimeoutDecision, GameError>;
}

pub trait BoardGame: Game {
    fn board_spec(&self, cfg: &Config) -> BoardSpec;
}

pub trait CardGame: Game {
    fn deck_spec(&self, cfg: &Config) -> Value;
}

pub trait TileGame: Game {
    fn tile_set(&self, cfg: &Config) -> Value;
}

pub trait HiddenInfo: Game {
    fn spectator_policy(&self, cfg: &Config) -> Value;
}

pub trait Chance: Game {
    fn randomness(&self) -> Vec<String>;
}

pub trait Teams: Game {
    fn teams(&self, state: &Value) -> Vec<Vec<Seat>>;
}

pub trait MultiPhase: Game {
    fn phase(&self, state: &Value) -> Value;
}

#[derive(Clone, Debug, Serialize)]
pub struct BoardSpec {
    pub width: u32,
    pub height: u32,
    pub coords: String,
    pub zones: Value,
}

pub struct Registry {
    games: Vec<Arc<dyn Game>>,
    boards: HashMap<String, Arc<dyn Fn(&Config) -> BoardSpec + Send + Sync>>,
}

impl Registry {
    pub fn builtin() -> Self {
        let mut reg = Self {
            games: Vec::new(),
            boards: HashMap::new(),
        };
        let gomoku = Arc::new(crate::gomoku::Gomoku);
        let gomoku_board = gomoku.clone();
        reg.boards.insert(
            "gomoku".into(),
            Arc::new(move |cfg| gomoku_board.board_spec(cfg)),
        );
        reg.games.push(gomoku);
        for stub in stubs() {
            reg.games.push(Arc::new(stub));
        }
        reg
    }

    pub fn get(&self, id: &str) -> Option<Arc<dyn Game>> {
        self.games.iter().find(|g| g.manifest().id == id).cloned()
    }

    pub fn list(&self) -> Vec<Manifest> {
        self.games.iter().map(|g| g.manifest()).collect()
    }

    pub fn board_spec(&self, id: &str, cfg: &Config) -> Option<BoardSpec> {
        self.boards.get(id).map(|f| f(cfg))
    }
}

fn stubs() -> Vec<Stub> {
    vec![
        Stub::new(
            "xiangqi",
            "中国象棋",
            "标准 9×10，九宫、河界。本里程碑仅登记清单，规则尚未实现。",
            vec![Trait::Board],
            2,
            2,
            "xiangqi",
            TimeoutPolicy::Forfeit,
            vec![TimeoutPolicy::Forfeit],
            "# 中国象棋\n\n尚未实现。",
        ),
        Stub::new(
            "go",
            "围棋",
            "中国规则数子法。默认 9×9。本里程碑仅登记清单。",
            vec![Trait::Board, Trait::MultiPhase],
            2,
            2,
            "grid-board",
            TimeoutPolicy::AutoPass,
            vec![TimeoutPolicy::AutoPass, TimeoutPolicy::Forfeit],
            "# 围棋\n\n尚未实现。",
        ),
        Stub::new(
            "junqi",
            "军棋",
            "两人暗棋。布阵同时进行，碰子由裁判判定。本里程碑仅登记清单。",
            vec![Trait::Board, Trait::HiddenInfo, Trait::MultiPhase],
            2,
            2,
            "junqi",
            TimeoutPolicy::AutoPass,
            vec![TimeoutPolicy::AutoPass, TimeoutPolicy::AutoAction, TimeoutPolicy::Forfeit],
            "# 军棋\n\n尚未实现。",
        ),
        Stub::new(
            "doudizhu",
            "斗地主",
            "三人扑克，叫分与牌型。本里程碑仅登记清单。",
            vec![
                Trait::Card,
                Trait::HiddenInfo,
                Trait::Chance,
                Trait::Teams,
                Trait::MultiPhase,
            ],
            3,
            3,
            "card-table",
            TimeoutPolicy::AutoPass,
            vec![TimeoutPolicy::AutoPass, TimeoutPolicy::AutoAction],
            "# 斗地主\n\n尚未实现。",
        ),
        Stub::new(
            "mahjong-sichuan",
            "麻将（四川·血战到底）",
            "四人 108 张，定缺、碰杠胡。本里程碑仅登记清单。",
            vec![
                Trait::Tile,
                Trait::HiddenInfo,
                Trait::Chance,
                Trait::MultiPhase,
            ],
            4,
            4,
            "mahjong-table",
            TimeoutPolicy::AutoAction,
            vec![TimeoutPolicy::AutoAction, TimeoutPolicy::AutoPass],
            "# 四川麻将\n\n尚未实现。",
        ),
    ]
}

struct Stub {
    manifest: Manifest,
}

impl Stub {
    fn new(
        id: &str,
        name: &str,
        description: &str,
        traits: Vec<Trait>,
        min: u8,
        max: u8,
        renderer: &str,
        policy: TimeoutPolicy,
        allowed: Vec<TimeoutPolicy>,
        rules: &str,
    ) -> Self {
        Self {
            manifest: Manifest {
                id: id.into(),
                name: name.into(),
                version: "0.0.0".into(),
                min_players: min,
                max_players: max,
                traits,
                variants: vec![Variant {
                    id: "std".into(),
                    name: "标准".into(),
                    options: None,
                }],
                default_time: TimeControl {
                    per_action_ms: 60_000,
                    max_consecutive_timeouts: 3,
                    grace_ms: 2_000,
                },
                timeout_policy: policy,
                allowed_timeout_policies: allowed,
                action_schema: json!({"type": "object"}),
                rules_markdown: rules.into(),
                renderer: renderer.into(),
                rated: false,
                engine: "native".into(),
                description: description.into(),
                playable: false,
            },
        }
    }
}

impl Game for Stub {
    fn manifest(&self) -> Manifest {
        self.manifest.clone()
    }
    fn setup(&self, _: &Config, _: &mut dyn ArenaRng) -> Result<(Value, Vec<Value>), GameError> {
        Err(GameError("游戏尚未在本里程碑实现".into()))
    }
    fn to_act(&self, _: &Value) -> Vec<Seat> {
        Vec::new()
    }
    fn view(&self, _: &Value, _: &Viewer) -> Result<Value, GameError> {
        Err(GameError("游戏尚未在本里程碑实现".into()))
    }
    fn legal_actions(&self, _: &Value, _: Seat) -> Result<Vec<Value>, GameError> {
        Err(GameError("游戏尚未在本里程碑实现".into()))
    }
    fn apply(
        &self,
        _: &Value,
        _: Seat,
        _: &Value,
        _: &mut dyn ArenaRng,
    ) -> Result<(Value, Vec<Value>), GameError> {
        Err(GameError("游戏尚未在本里程碑实现".into()))
    }
    fn is_terminal(&self, _: &Value) -> bool {
        true
    }
    fn result(&self, _: &Value) -> Result<GameResult, GameError> {
        Err(GameError("游戏尚未在本里程碑实现".into()))
    }
    fn timeout_decision(
        &self,
        _: &Value,
        _: Seat,
        policy: TimeoutPolicy,
        _: &mut dyn ArenaRng,
    ) -> Result<TimeoutDecision, GameError> {
        Ok(TimeoutDecision {
            policy,
            action: None,
        })
    }
}

pub fn registry() -> &'static Registry {
    use std::sync::OnceLock;
    static REG: OnceLock<Registry> = OnceLock::new();
    REG.get_or_init(Registry::builtin)
}

pub fn result_value(r: &GameResult) -> Value {
    let scores: Vec<Value> = r
        .scores
        .iter()
        .map(|s| {
            if (s - s.round()).abs() < f64::EPSILON && s.abs() < 1e15 {
                json!(*s as i64)
            } else {
                json!(s)
            }
        })
        .collect();
    json!({
        "scores": scores,
        "ranking": r.ranking,
        "reason": r.reason,
    })
}
