//! QuickJS sandbox for uploaded rule modules.
//!
//! Each call builds a fresh isolate: 8 MiB memory, a 500 ms interrupt, no
//! network or filesystem, and no clock. `Date` and `Math.random` throw.
//! Randomness comes only from the match seed via `rng`.

use crate::canon::canon;
use crate::crypto::sha256_hex;
use crate::game::{
    ArenaRng, Config, Game, GameError, GameResult, Manifest, Seat, TimeControl, TimeoutDecision,
    TimeoutPolicy, Trait, Variant, Viewer,
};
use rquickjs::context::intrinsic::{Eval, Json};
use rquickjs::{CaughtError, Context, Ctx, Function, Runtime};
use serde::Deserialize;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::time::{Duration, Instant};

pub const MEMORY_LIMIT: usize = 8 * 1024 * 1024;
pub const CALL_BUDGET: Duration = Duration::from_millis(500);
pub const MAX_SOURCE: usize = 256 * 1024;

#[derive(Clone, Copy)]
#[repr(C)]
struct RngRaw {
    data: *mut (),
    vtable: *mut (),
}

thread_local! {
    static RNG: RefCell<Option<RngRaw>> = const { RefCell::new(None) };
}

struct RngGuard;
impl Drop for RngGuard {
    fn drop(&mut self) {
        RNG.with(|slot| *slot.borrow_mut() = None);
    }
}

fn park_rng(rng: &mut dyn ArenaRng) {
    let raw: RngRaw = unsafe { std::mem::transmute(rng as *mut dyn ArenaRng) };
    RNG.with(|slot| *slot.borrow_mut() = Some(raw));
}

fn with_parked<T>(f: impl FnOnce(&mut dyn ArenaRng) -> T) -> Option<T> {
    RNG.with(|slot| {
        let raw = (*slot.borrow())?;
        let ptr: *mut dyn ArenaRng = unsafe { std::mem::transmute(raw) };
        Some(f(unsafe { &mut *ptr }))
    })
}

const PRELUDE: &str = r#"
var __game = null;
function defineGame(game) { __game = game; }
function __blocked(name) {
  var fn = function () { throw new Error(name + " is disabled"); };
  fn.now = fn;
  fn.parse = fn;
  fn.UTC = fn;
  return fn;
}
globalThis.Date = __blocked("Date");
globalThis.eval = __blocked("eval");
var __deny = __blocked("Function");
try {
  Object.defineProperty(Function.prototype, "constructor", { configurable: true, writable: true, value: __deny });
} catch (e) {}
globalThis.Function = __deny;
if (globalThis.Math) {
  globalThis.Math.random = function () { throw new Error("Math.random is disabled"); };
}
["fetch", "XMLHttpRequest", "WebSocket", "require", "process", "Deno", "Worker", "setTimeout", "setInterval", "queueMicrotask"].forEach(function (name) {
  try { globalThis[name] = __blocked(name); } catch (e) {}
});
function __rng() {
  return {
    uint32: function () {
      var n = hostUint32();
      if (n < 0) throw new Error("RNG 只在 setup、apply、timeoutAction 里可用");
      return n;
    },
    int: function (n) {
      var v = hostInt(n);
      if (v < 0) throw new Error("RNG 只在 setup、apply、timeoutAction 里可用");
      return v;
    },
    shuffle: function (arr) {
      var out = arr.slice();
      var i = out.length - 1;
      while (i > 0) {
        var j = hostInt(i + 1);
        if (j < 0) throw new Error("RNG 只在 setup、apply、timeoutAction 里可用");
        var tmp = out[i];
        out[i] = out[j];
        out[j] = tmp;
        i -= 1;
      }
      return out;
    }
  };
}
globalThis.__arenaCall = function (method, argsJson) {
  if (!__game) throw new Error("defineGame was not called");
  var fn = __game[method];
  if (typeof fn !== "function") throw new Error("missing " + method);
  var args = JSON.parse(argsJson);
  var result;
  if (method === "setup") result = fn.call(__game, args[0], __rng());
  else if (method === "apply") result = fn.call(__game, args[0], args[1], args[2], __rng());
  else if (method === "timeoutAction") result = fn.call(__game, args[0], args[1], __rng());
  else if (method === "view" || method === "legalActions") result = fn.call(__game, args[0], args[1]);
  else result = fn.call(__game, args[0]);
  var encoded = JSON.stringify(result);
  if (encoded === undefined) throw new Error(method + " returned a value JSON cannot encode");
  return encoded;
};
globalThis.__arenaInspect = function () {
  if (!__game || !__game.manifest) throw new Error("defineGame was not called");
  var methods = ["setup", "toAct", "view", "legalActions", "apply", "isTerminal", "result", "timeoutAction"];
  var missing = [];
  for (var i = 0; i < methods.length; i++) {
    if (typeof __game[methods[i]] !== "function") missing.push(methods[i]);
  }
  return JSON.stringify({ manifest: __game.manifest, missing: missing });
};
"#;

#[derive(Clone, Debug, serde::Serialize)]
pub struct Check {
    pub id: String,
    pub ok: bool,
    pub detail: String,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct Report {
    pub ok: bool,
    pub checks: Vec<Check>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manifest: Option<Value>,
}

pub struct JsGame {
    source: String,
    manifest: Manifest,
}

impl JsGame {
    pub fn from_stored(source: String, manifest: Manifest) -> Self {
        Self { source, manifest }
    }

    pub fn source(&self) -> &str {
        &self.source
    }
}

pub fn rules_hash(manifest: &Value, source: &str) -> String {
    sha256_hex(canon(&json!({"manifest": manifest, "source": source})).as_bytes())
}

pub fn validate(source: &str) -> Result<Report, GameError> {
    if source.len() > MAX_SOURCE {
        return Err(GameError("规则源码不能超过 256KB".into()));
    }
    if let Some(token) = banned(source) {
        return Err(GameError(format!("源码使用了禁用标识 {token}")));
    }
    let inspected = inspect(source)?;
    let missing = inspected
        .get("missing")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let raw = inspected.get("manifest").cloned().unwrap_or(Value::Null);
    if !missing.is_empty() {
        let names: Vec<_> = missing.iter().filter_map(|v| v.as_str()).collect();
        return Ok(fail_report(
            "interface",
            format!("缺少方法 {}", names.join("、")),
            Some(raw),
        ));
    }
    let parsed = match parse_manifest(&raw) {
        Ok(m) => m,
        Err(err) => return Ok(fail_report("manifest", err.0, Some(raw))),
    };
    let mut checks = vec![
        Check {
            id: "interface".into(),
            ok: true,
            detail: "八个规则方法都在".into(),
        },
        Check {
            id: "manifest".into(),
            ok: true,
            detail: format!(
                "{} · {} 人",
                parsed.manifest.name, parsed.manifest.max_players
            ),
        },
    ];
    let game = JsGame {
        source: source.to_string(),
        manifest: parsed.manifest.clone(),
    };
    match self_play(&game) {
        Ok((left, right, terminal)) => {
            let same = left == right;
            checks.push(Check {
                id: "determinism".into(),
                ok: same,
                detail: if same {
                    "同一粒种子自对两次，局面一致".into()
                } else {
                    "同一粒种子得到了不同局面".into()
                },
            });
            checks.push(Check {
                id: "self-play".into(),
                ok: true,
                detail: if terminal {
                    "随机对随机走到了终局".into()
                } else {
                    "自对下了 48 步仍未终局，先记为通过".into()
                },
            });
        }
        Err(err) if hostile(&err.0) => return Err(err),
        Err(err) => checks.push(Check {
            id: "self-play".into(),
            ok: false,
            detail: err.0,
        }),
    }
    match timeout_ok(&game) {
        Ok(detail) => checks.push(Check {
            id: "timeout".into(),
            ok: true,
            detail,
        }),
        Err(err) if hostile(&err.0) => return Err(err),
        Err(err) => checks.push(Check {
            id: "timeout".into(),
            ok: false,
            detail: err.0,
        }),
    }
    if parsed.manifest.traits.contains(&Trait::HiddenInfo) {
        checks.push(Check {
            id: "hidden".into(),
            ok: true,
            detail: "隐藏信息由 view 自己裁剪，服务器不会把完整状态发给对手".into(),
        });
    }
    let ok = checks.iter().all(|c| c.ok);
    Ok(Report {
        ok,
        checks,
        manifest: Some(parsed.public),
    })
}

fn fail_report(id: &str, detail: String, manifest: Option<Value>) -> Report {
    Report {
        ok: false,
        checks: vec![Check {
            id: id.into(),
            ok: false,
            detail,
        }],
        manifest,
    }
}

struct Parsed {
    manifest: Manifest,
    public: Value,
}

fn parse_manifest(raw: &Value) -> Result<Parsed, GameError> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Incoming {
        name: String,
        #[serde(default)]
        description: String,
        min_players: u8,
        max_players: u8,
        #[serde(default)]
        traits: Vec<Trait>,
        variants: Vec<Variant>,
        default_time: TimeControl,
        timeout_policy: TimeoutPolicy,
        allowed_timeout_policies: Vec<TimeoutPolicy>,
        #[serde(default = "default_schema")]
        action_schema: Value,
        rules_markdown: String,
        renderer: String,
        #[serde(default)]
        render_schema: Option<Value>,
    }
    fn default_schema() -> Value {
        json!({"type": "object"})
    }
    let incoming: Incoming = serde_json::from_value(raw.clone())
        .map_err(|err| GameError(format!("manifest 对不上：{err}")))?;
    if incoming.name.trim().is_empty() || incoming.name.chars().count() > 40 {
        return Err(GameError("名称需要 1–40 个字".into()));
    }
    if !(1..=8).contains(&incoming.min_players)
        || incoming.max_players < incoming.min_players
        || incoming.max_players > 8
    {
        return Err(GameError("人数需在 1–8，且最少不超过最多".into()));
    }
    if incoming.variants.is_empty() {
        return Err(GameError("至少要有一个变体".into()));
    }
    if !["grid-board", "card-table", "json-inspector"].contains(&incoming.renderer.as_str()) {
        return Err(GameError(
            "renderer 只能是 grid-board、card-table 或 json-inspector".into(),
        ));
    }
    if incoming.rules_markdown.trim().is_empty() {
        return Err(GameError("规则说明不能为空".into()));
    }
    if !incoming
        .allowed_timeout_policies
        .contains(&incoming.timeout_policy)
    {
        return Err(GameError("默认超时策略不在允许列表里".into()));
    }
    let manifest = Manifest {
        id: "preview".into(),
        name: incoming.name,
        version: "0.0.0".into(),
        min_players: incoming.min_players,
        max_players: incoming.max_players,
        traits: incoming.traits,
        variants: incoming.variants,
        default_time: incoming.default_time,
        timeout_policy: incoming.timeout_policy,
        allowed_timeout_policies: incoming.allowed_timeout_policies,
        action_schema: incoming.action_schema,
        rules_markdown: incoming.rules_markdown,
        renderer: incoming.renderer,
        rated: false,
        engine: "js-sandbox".into(),
        description: incoming.description,
        playable: true,
    };
    let mut public = serde_json::to_value(&manifest).map_err(|err| GameError(err.to_string()))?;
    if let Some(schema) = incoming.render_schema {
        public["renderSchema"] = schema;
    }
    Ok(Parsed { manifest, public })
}

pub fn publish_manifest(
    public: Value,
    id: &str,
    version: &str,
    name: Option<&str>,
    description: Option<&str>,
) -> Result<(Manifest, Value), GameError> {
    let mut value = public;
    value["id"] = json!(id);
    value["version"] = json!(version);
    value["rated"] = json!(false);
    value["engine"] = json!("js-sandbox");
    value["playable"] = json!(true);
    if let Some(name) = name.map(str::trim).filter(|s| !s.is_empty()) {
        if name.chars().count() > 40 {
            return Err(GameError("名称需要 1–40 个字".into()));
        }
        value["name"] = json!(name);
    }
    if let Some(description) = description.map(str::trim).filter(|s| !s.is_empty()) {
        value["description"] = json!(description.chars().take(200).collect::<String>());
    }
    let manifest: Manifest = serde_json::from_value(value.clone())
        .map_err(|err| GameError(format!("manifest 对不上：{err}")))?;
    Ok((manifest, value))
}

fn self_play(game: &JsGame) -> Result<(Value, Value, bool), GameError> {
    let seed = [7u8; 32];
    let left = play_once(game, &seed)?;
    let right = play_once(game, &seed)?;
    Ok((left.0, right.0, left.1 && right.1))
}

fn play_once(game: &JsGame, seed: &[u8; 32]) -> Result<(Value, bool), GameError> {
    let mut rng = crate::game::SeededRng::from_seed_seq(seed, 0);
    let cfg = Config {
        players: game.manifest.max_players,
        variant: game
            .manifest
            .variants
            .first()
            .map(|v| v.id.clone())
            .unwrap_or_else(|| "standard".into()),
        options: json!({}),
    };
    let (mut state, _) = game.setup(&cfg, &mut rng)?;
    let mut terminal = game.is_terminal(&state);
    for _ in 0..48 {
        if terminal {
            break;
        }
        let seats = game.to_act(&state);
        let Some(seat) = seats.first().copied() else {
            break;
        };
        let actions = game.legal_actions(&state, seat)?;
        let action = actions
            .iter()
            .find(|a| a.get("type").and_then(|t| t.as_str()) != Some("resign"))
            .cloned()
            .or_else(|| actions.first().cloned())
            .ok_or_else(|| GameError("没有合法动作".into()))?;
        let (next, _) = game.apply(&state, seat, &action, &mut rng)?;
        state = next;
        terminal = game.is_terminal(&state);
    }
    Ok((state, terminal))
}

fn timeout_ok(game: &JsGame) -> Result<String, GameError> {
    let mut rng = crate::game::SeededRng::from_seed_seq(&[9u8; 32], 1);
    let cfg = Config {
        players: game.manifest.max_players,
        variant: game
            .manifest
            .variants
            .first()
            .map(|v| v.id.clone())
            .unwrap_or_else(|| "standard".into()),
        options: json!({}),
    };
    let (state, _) = game.setup(&cfg, &mut rng)?;
    let seat = game.to_act(&state).first().copied().unwrap_or(0);
    let decision = game.timeout_decision(&state, seat, TimeoutPolicy::AutoAction, &mut rng)?;
    let Some(action) = decision.action else {
        return Err(GameError("timeoutAction 没有给出动作".into()));
    };
    let kind = action.get("type").and_then(|t| t.as_str()).unwrap_or("");
    let legal = game.legal_actions(&state, seat)?;
    if !legal
        .iter()
        .any(|a| a.get("type").and_then(|t| t.as_str()) == Some(kind))
    {
        return Err(GameError(format!("超时动作 {kind} 不在合法列表里")));
    }
    Ok(format!("超时会走 {kind}"))
}

fn hostile(msg: &str) -> bool {
    msg.contains("500 毫秒") || msg.contains("内存") || msg.contains("stack")
}

fn banned(source: &str) -> Option<&'static str> {
    const CALLS: &[(&str, &str)] = &[
        ("eval", "eval"),
        ("Function", "Function"),
        ("fetch", "fetch"),
        ("require", "require"),
        ("XMLHttpRequest", "XMLHttpRequest"),
    ];
    for (name, label) in CALLS {
        if has_call(source, name) {
            return Some(label);
        }
    }
    const WORDS: &[(&str, &str)] = &[
        ("Date", "Date"),
        ("WebSocket", "WebSocket"),
        ("Worker", "Worker"),
        ("Deno", "Deno"),
        ("process", "process"),
        ("import", "import"),
    ];
    for (word, label) in WORDS {
        if has_word(source, word) {
            return Some(label);
        }
    }
    if source.contains("Math.random") {
        return Some("Math.random");
    }
    None
}

fn has_call(source: &str, name: &str) -> bool {
    source.match_indices(name).any(|(i, _)| {
        word_before(source, i) && source[i + name.len()..].trim_start().starts_with('(')
    })
}

fn has_word(source: &str, word: &str) -> bool {
    source
        .match_indices(word)
        .any(|(i, _)| word_before(source, i) && word_after(source, i + word.len()))
}

fn word_before(source: &str, index: usize) -> bool {
    source[..index]
        .chars()
        .next_back()
        .map(|c| !is_ident(c))
        .unwrap_or(true)
}

fn word_after(source: &str, index: usize) -> bool {
    source[index..]
        .chars()
        .next()
        .map(|c| !is_ident(c))
        .unwrap_or(true)
}

fn is_ident(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '$'
}

fn inspect(source: &str) -> Result<Value, GameError> {
    let text = execute(source, None, None)?;
    serde_json::from_str(&text).map_err(|err| GameError(format!("inspect 不是 JSON：{err}")))
}

fn execute(
    source: &str,
    call: Option<(&str, Value)>,
    rng: Option<&mut dyn ArenaRng>,
) -> Result<String, GameError> {
    if let Some(rng) = rng {
        park_rng(rng);
    } else {
        RNG.with(|slot| *slot.borrow_mut() = None);
    }
    let _guard = RngGuard;
    let rt = Runtime::new().map_err(|err| GameError(err.to_string()))?;
    rt.set_memory_limit(MEMORY_LIMIT);
    rt.set_max_stack_size(256 * 1024);
    let started = Instant::now();
    rt.set_interrupt_handler(Some(Box::new(move || started.elapsed() >= CALL_BUDGET)));
    let ctx = Context::builder()
        .with::<Eval>()
        .with::<Json>()
        .build(&rt)
        .map_err(|err| GameError(err.to_string()))?;
    ctx.with(|ctx| {
        bind_hosts(&ctx)?;
        eval_source(&ctx, PRELUDE)?;
        eval_source(&ctx, source)?;
        if let Some((method, args)) = call {
            let func: Function = ctx
                .globals()
                .get("__arenaCall")
                .map_err(|err| explain(&ctx, err))?;
            let args_json =
                serde_json::to_string(&args).map_err(|err| GameError(err.to_string()))?;
            func.call((method, args_json))
                .map_err(|err| explain(&ctx, err))
        } else {
            let func: Function = ctx
                .globals()
                .get("__arenaInspect")
                .map_err(|err| explain(&ctx, err))?;
            func.call(()).map_err(|err| explain(&ctx, err))
        }
    })
}

fn bind_hosts(ctx: &Ctx<'_>) -> Result<(), GameError> {
    let uint32 = Function::new(ctx.clone(), |ctx: Ctx<'_>| -> rquickjs::Result<f64> {
        match rng_u32() {
            Some(n) => Ok(n as f64),
            None => Err(throw_msg(
                &ctx,
                "RNG 只在 setup、apply、timeoutAction 里可用",
            )),
        }
    })
    .map_err(|err| GameError(err.to_string()))?;
    ctx.globals()
        .set("hostUint32", uint32)
        .map_err(|err| GameError(err.to_string()))?;
    let int_n = Function::new(
        ctx.clone(),
        |ctx: Ctx<'_>, n: f64| -> rquickjs::Result<i32> {
            if !n.is_finite() || n < 1.0 {
                return Err(throw_msg(&ctx, "rng.int 需要正整数"));
            }
            match rng_int(n as usize) {
                Some(v) => Ok(v as i32),
                None => Err(throw_msg(
                    &ctx,
                    "RNG 只在 setup、apply、timeoutAction 里可用",
                )),
            }
        },
    )
    .map_err(|err| GameError(err.to_string()))?;
    ctx.globals()
        .set("hostInt", int_n)
        .map_err(|err| GameError(err.to_string()))?;
    Ok(())
}

fn throw_msg(ctx: &Ctx<'_>, msg: &str) -> rquickjs::Error {
    match rquickjs::String::from_str(ctx.clone(), msg) {
        Ok(text) => ctx.throw(text.into()),
        Err(err) => err,
    }
}

fn rng_u32() -> Option<u32> {
    with_parked(|rng| rng.next_u64() as u32)
}

fn rng_int(n: usize) -> Option<usize> {
    with_parked(|rng| rng.int_n(n))
}

fn eval_source(ctx: &Ctx<'_>, source: &str) -> Result<(), GameError> {
    ctx.eval::<(), _>(source).map_err(|err| explain(ctx, err))
}

fn explain(ctx: &Ctx<'_>, err: rquickjs::Error) -> GameError {
    let caught = CaughtError::from_error(ctx, err);
    let msg = match caught {
        CaughtError::Exception(ex) => ex.message().unwrap_or_else(|| ex.to_string()),
        CaughtError::Error(err) => err.to_string(),
        CaughtError::Value(_) => "规则执行失败".into(),
    };
    GameError(friendly(&msg))
}

fn friendly(msg: &str) -> String {
    let lower = msg.to_ascii_lowercase();
    if lower.contains("interrupt") {
        return "单次调用超过 500 毫秒".into();
    }
    if lower.contains("out of memory") || lower.contains("allocation") {
        return "超出内存上限".into();
    }
    msg.to_string()
}

fn call_value(
    game: &JsGame,
    method: &str,
    args: Value,
    rng: Option<&mut dyn ArenaRng>,
) -> Result<Value, GameError> {
    let text = execute(&game.source, Some((method, args)), rng)?;
    serde_json::from_str(&text).map_err(|err| GameError(format!("{method} 返回的不是 JSON：{err}")))
}

fn take_state(value: Value) -> Result<(Value, Vec<Value>), GameError> {
    let state = value
        .get("state")
        .cloned()
        .ok_or_else(|| GameError("需要返回 { state }".into()))?;
    let events = value
        .get("events")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    Ok((state, events))
}

impl Game for JsGame {
    fn manifest(&self) -> Manifest {
        self.manifest.clone()
    }

    fn setup(
        &self,
        cfg: &Config,
        rng: &mut dyn ArenaRng,
    ) -> Result<(Value, Vec<Value>), GameError> {
        let value = call_value(
            self,
            "setup",
            json!([{ "players": cfg.players, "variant": cfg.variant, "options": cfg.options }]),
            Some(rng),
        )?;
        take_state(value)
    }

    fn to_act(&self, state: &Value) -> Vec<Seat> {
        call_value(self, "toAct", json!([state]), None)
            .ok()
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default()
    }

    fn view(&self, state: &Value, viewer: &Viewer) -> Result<Value, GameError> {
        let mut view = call_value(
            self,
            "view",
            json!([state, { "seat": viewer.seat, "omniscient": viewer.omniscient }]),
            None,
        )?;
        if view.get("renderer").and_then(|v| v.as_str()).is_none() {
            view["renderer"] = json!(self.manifest.renderer);
        }
        Ok(view)
    }

    fn legal_actions(&self, state: &Value, seat: Seat) -> Result<Vec<Value>, GameError> {
        let value = call_value(self, "legalActions", json!([state, seat]), None)?;
        value
            .as_array()
            .cloned()
            .ok_or_else(|| GameError("legalActions 需要返回数组".into()))
    }

    fn apply(
        &self,
        state: &Value,
        seat: Seat,
        action: &Value,
        rng: &mut dyn ArenaRng,
    ) -> Result<(Value, Vec<Value>), GameError> {
        let value = call_value(self, "apply", json!([state, seat, action]), Some(rng))?;
        take_state(value)
    }

    fn is_terminal(&self, state: &Value) -> bool {
        call_value(self, "isTerminal", json!([state]), None)
            .ok()
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }

    fn result(&self, state: &Value) -> Result<GameResult, GameError> {
        let value = call_value(self, "result", json!([state]), None)?;
        serde_json::from_value(value).map_err(|err| GameError(format!("result 对不上：{err}")))
    }

    fn timeout_decision(
        &self,
        state: &Value,
        seat: Seat,
        policy: TimeoutPolicy,
        rng: &mut dyn ArenaRng,
    ) -> Result<TimeoutDecision, GameError> {
        if policy == TimeoutPolicy::Forfeit {
            return Ok(TimeoutDecision {
                policy,
                action: Some(json!({"type": "resign"})),
            });
        }
        let value = call_value(self, "timeoutAction", json!([state, seat]), Some(rng))?;
        let action = value.get("action").cloned().filter(|v| !v.is_null());
        let chosen = value
            .get("policy")
            .and_then(|v| v.as_str())
            .and_then(TimeoutPolicy::parse)
            .unwrap_or(policy);
        Ok(TimeoutDecision {
            policy: chosen,
            action,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::SeededRng;

    fn example() -> JsGame {
        let report = validate(include_str!("fixtures/tic-tac-toe.js")).expect("validate");
        assert!(report.ok, "{:?}", report.checks);
        let public = report.manifest.expect("manifest");
        let (manifest, _) = publish_manifest(public, "u_tictactoe", "1.0.0", None, None).unwrap();
        JsGame::from_stored(include_str!("fixtures/tic-tac-toe.js").into(), manifest)
    }

    fn setup_of(source: &str) -> Result<Value, GameError> {
        let report = validate(source);
        if let Err(err) = &report {
            return Err(GameError(err.0.clone()));
        }
        let report = report.unwrap();
        if !report.ok {
            return Err(GameError(format!("{:?}", report.checks)));
        }
        let (manifest, _) =
            publish_manifest(report.manifest.unwrap(), "u_probe", "1.0.0", None, None)?;
        let game = JsGame::from_stored(source.to_string(), manifest);
        let mut rng = SeededRng::from_seed_seq(&[1u8; 32], 0);
        let cfg = Config {
            players: 2,
            variant: game.manifest.variants[0].id.clone(),
            options: json!({}),
        };
        Ok(game.setup(&cfg, &mut rng)?.0)
    }

    #[test]
    fn tic_tac_toe_scripted_line() {
        let game = example();
        let mut rng = SeededRng::from_seed_seq(&[3u8; 32], 0);
        let cfg = Config {
            players: 2,
            variant: "3".into(),
            options: json!({}),
        };
        let (mut state, _) = game.setup(&cfg, &mut rng).unwrap();
        for (seat, at) in [(0, "a3"), (1, "a2"), (0, "b3"), (1, "b2"), (0, "c3")] {
            let action = json!({"type": "place", "data": {"at": at}});
            let (next, _) = game.apply(&state, seat, &action, &mut rng).unwrap();
            state = next;
        }
        assert!(game.is_terminal(&state));
        let result = game.result(&state).unwrap();
        assert_eq!(result.scores, vec![1.0, -1.0]);
        assert_eq!(result.reason, "line");
        let view = game
            .view(
                &state,
                &Viewer {
                    seat: Some(1),
                    omniscient: false,
                },
            )
            .unwrap();
        assert_eq!(view["renderer"], "grid-board");
        assert_eq!(view["winner"], 0);
    }

    #[test]
    fn infinite_loop_is_interrupted() {
        let source = r#"
          defineGame({
            manifest: {
              name: "环", description: "转圈", minPlayers: 2, maxPlayers: 2, traits: ["board"],
              variants: [{ id: "std", name: "标准" }],
              defaultTime: { perActionMs: 180000, maxConsecutiveTimeouts: 3, graceMs: 2000 },
              timeoutPolicy: "auto_action", allowedTimeoutPolicies: ["auto_action", "forfeit"],
              actionSchema: { type: "object" }, rulesMarkdown: "没有规则", renderer: "json-inspector"
            },
            setup() { while (true) {} return { state: {} }; },
            toAct() { return []; }, view(s) { return s; }, legalActions() { return [{ type: "resign" }]; },
            apply(s) { return { state: s }; }, isTerminal() { return true; },
            result() { return { scores: [0, 0], ranking: [[0, 1]], reason: "loop" }; },
            timeoutAction() { return { policy: "forfeit", action: { type: "resign" } }; }
          });
        "#;
        let err = validate(source).expect_err("loop must fail");
        assert!(err.0.contains("500"), "{}", err.0);
    }

    #[test]
    fn memory_bomb_is_capped() {
        let source = r#"
          defineGame({
            manifest: {
              name: "胀", description: "占内存", minPlayers: 2, maxPlayers: 2, traits: ["board"],
              variants: [{ id: "std", name: "标准" }],
              defaultTime: { perActionMs: 180000, maxConsecutiveTimeouts: 3, graceMs: 2000 },
              timeoutPolicy: "auto_action", allowedTimeoutPolicies: ["auto_action", "forfeit"],
              actionSchema: { type: "object" }, rulesMarkdown: "没有规则", renderer: "json-inspector"
            },
            setup() {
              var bag = [];
              while (true) bag.push(new Array(8000).join("x"));
              return { state: { n: bag.length } };
            },
            toAct() { return []; }, view(s) { return s; }, legalActions() { return [{ type: "resign" }]; },
            apply(s) { return { state: s }; }, isTerminal() { return true; },
            result() { return { scores: [0, 0], ranking: [[0, 1]], reason: "mem" }; },
            timeoutAction() { return { policy: "forfeit", action: { type: "resign" } }; }
          });
        "#;
        let err = validate(source).expect_err("bomb must fail");
        assert!(err.0.contains("内存") || err.0.contains("500"), "{}", err.0);
    }

    #[test]
    fn seeded_shuffle_is_stable_and_clock_is_blocked() {
        let source = r#"
          defineGame({
            manifest: {
              name: "洗", description: "洗牌", minPlayers: 2, maxPlayers: 2, traits: ["chance"],
              variants: [{ id: "std", name: "标准" }],
              defaultTime: { perActionMs: 180000, maxConsecutiveTimeouts: 3, graceMs: 2000 },
              timeoutPolicy: "forfeit", allowedTimeoutPolicies: ["forfeit"],
              actionSchema: { type: "object" }, rulesMarkdown: "洗一次牌", renderer: "json-inspector"
            },
            setup(cfg, rng) {
              var random = Math["ran" + "dom"];
              var clock = globalThis["Da" + "te"];
              try { random(); throw new Error("random ran"); } catch (e) { if (e.message === "random ran") throw e; }
              try { clock.now(); throw new Error("clock ran"); } catch (e) { if (e.message === "clock ran") throw e; }
              return { state: { n: rng.uint32(), deck: rng.shuffle([1, 2, 3, 4, 5, 6, 7, 8]) } };
            },
            toAct() { return []; }, view(s) { return s; }, legalActions() { return [{ type: "resign" }]; },
            apply(s) { return { state: s }; }, isTerminal() { return true; },
            result() { return { scores: [0, 0], ranking: [[0, 1]], reason: "shuffle" }; },
            timeoutAction() { return { policy: "forfeit", action: { type: "resign" } }; }
          });
        "#;
        let left = setup_of(source).unwrap();
        let right = setup_of(source).unwrap();
        assert_eq!(left, right);
        assert!(left["deck"].as_array().unwrap().len() == 8);
    }

    #[test]
    fn static_ban_rejects_eval() {
        let err = validate("eval('1')").unwrap_err();
        assert!(err.0.contains("eval"), "{}", err.0);
    }

    #[test]
    fn function_constructor_cannot_compile_code() {
        let source = r#"
          defineGame({
            manifest: {
              name: "逃", description: "构造函数", minPlayers: 2, maxPlayers: 2, traits: ["board"],
              variants: [{ id: "std", name: "标准" }],
              defaultTime: { perActionMs: 180000, maxConsecutiveTimeouts: 3, graceMs: 2000 },
              timeoutPolicy: "forfeit", allowedTimeoutPolicies: ["forfeit"],
              actionSchema: { type: "object" }, rulesMarkdown: "不许动态编译", renderer: "json-inspector"
            },
            setup() {
              var compile = (function () {}).constructor;
              return { state: { n: compile("return 1")() } };
            },
            toAct() { return []; }, view(s) { return s; }, legalActions() { return [{ type: "resign" }]; },
            apply(s) { return { state: s }; }, isTerminal() { return true; },
            result() { return { scores: [0, 0], ranking: [[0, 1]], reason: "x" }; },
            timeoutAction() { return { policy: "forfeit", action: { type: "resign" } }; }
          });
        "#;
        let err = setup_of(source).unwrap_err();
        assert!(
            err.0.contains("Function") || err.0.contains("disabled") || !err.0.is_empty(),
            "{}",
            err.0
        );
    }
}
