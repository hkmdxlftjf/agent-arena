# AGENTS.md — src/

## OVERVIEW
The entire Rust backend (`arena`, edition 2021): one axum process that referees matches, relays them over HTTP/MCP/SSE, and persists to PostgreSQL 16 through sqlx. Six built-in games sit behind the `Game` trait.

> Earned its own file (score 15): 27 Rust files plus one JS fixture, `lib.rs` as the module boundary, and by far the highest symbol density and reference centrality in the repo (`CallError` alone has ~313 references).

## STRUCTURE
| File | Lines | Role |
|---|---|---|
| `app.rs` | 3227 | Process-wide `App` handle (Arc<Inner>): accounts, auth, GitHub OAuth, queue/matchmaking, ratings, quotas. Injected as axum `State`. |
| `room.rs` | 2341 | Live-match runtime: one tokio task per match; `Engine` / `Hub` / `Slot` / `Cmd`. |
| `http.rs` | 1193 | Browser API, WebSocket spectating, embedded SPA (`rust-embed` of `web/dist`), hashed-asset caching. |
| `mcp.rs` | 868 | The MCP endpoint (`POST /mcp`); JSON-RPC per request, bearer token. |
| `game.rs` | 317 | Core contract: `Game`, capability traits, `Registry`, `Manifest`, `SeededRng`. |
| `sandbox.rs` | 971 | QuickJS (`rquickjs`) host for uploaded JS rules; `JsGame`. |
| `crypto.rs` / `chain.rs` / `canon.rs` | 280 / 230 / 78 | Ed25519 envelopes, hash-chained log, JCS canonical JSON that must byte-match `sign.mjs`. |
| `rating.rs` / `compact.rs` | 565 / 511 | Glicko-2 + OpenSkill; per-game state projection for SSE. |
| `ai.rs` + `ai/prompt.rs` | 513 / 25 | Built-in LLM opponent (OpenAI-compatible endpoint, forced `choose_move` tool call). |
| `xiangqi.rs` `junqi.rs` `gomoku.rs` `go.rs` `doudizhu.rs` `mahjong.rs` | 545–1277 | The six built-in rule sets, each an `impl Game`. |
| `cosmetics.rs` | 1109 | Avatars, titles, skins; awarded after matches. |
| `config.rs` | 139 | All configuration from env; `DATABASE_URL` is required. |
| `error.rs` | 32 | `CallError` — the error type for every handler. |

## WHERE TO LOOK
| Task | Look at |
|---|---|
| Add a built-in game | new module + `impl Game` + register in `Registry::builtin()` (`game.rs:237`) + a `compact.rs` branch + a `guidance.rs` entry + a `skills/arena/games/*.md` brief |
| Add an HTTP route | `router()` in `http.rs` |
| Add an MCP tool | `mcp.rs` |
| Change turns, verification, or timeouts | `room.rs` |
| Change an error's shape | `error.rs` |
| Boot order | `main.rs`: `Config::from_env` → `ServerKey::load_or_create` → pool + `sqlx::migrate!` → `App::boot` → `http::router(app)` |

## CONVENTIONS (this crate)
- Handler failures are `CallError { status, code, message }`, serialised by `IntoResponse` as `{"error", "message"}`; rule failures are `GameError(pub String)`. `anyhow` appears only at the boot boundary.
- Rule objects are **stateless**; match state is a `serde_json::Value` stored, replayed, and exchanged with the sandbox verbatim. Keep typed structs inside a rule module and convert at its edges.
- Randomness is deterministic: `SeededRng::from_seed_seq(seed, seq)` (ChaCha8). Never reach for `thread_rng` in rules — the sandbox replays the same seed to prove determinism.
- `canon()` must stay byte-identical to `sign.mjs` (UTF-16 key order, `JSON.stringify` number/string forms); `tests/cross_sign.rs` locks this.
- Exactly one tokio task owns a live match. Everything else talks to it through `Cmd` over `Hub`/`Slot`, never by touching match state directly.
- `#[allow(dead_code)]` on DB-row structs (e.g. `GameInfo`, `QueueRow`) is deliberate: sqlx selects more columns than every caller reads.

## ANTI-PATTERNS (THIS CRATE)
- Never put cosmetics fields into a signed entry (`cosmetics.rs:4`).
- Never spawn a match actor spontaneously from request handling — startup is where overdue decisions get executed (`room.rs:356`).
- Never add `Date`, `Math.random`, `eval`, or the `Function` constructor to the sandbox prelude or to an uploaded rule.
- Never let a rejected move bypass timeout handling.

## COMMANDS
```bash
DATABASE_URL=postgres://arena:arena@127.0.0.1:5432/arena cargo run   # dev server, port 18480
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```
