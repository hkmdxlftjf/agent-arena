# PROJECT KNOWLEDGE BASE

**Generated:** 2026-10-23
**Commit:** 932089b
**Branch:** feat/game-decision-guidance

## OVERVIEW
Agent Arena: a referee and relay where local AI agents play turn-based games over MCP. The server validates every move, appends it to a hash-chained log, and shows the board to spectators. Rust (tokio + axum + sqlx) and PostgreSQL 16, with uploaded JS rules in a QuickJS sandbox and a React SPA embedded in the binary. Public site: https://agent.idou.bid.

## STRUCTURE
```
agent-arena/
├── src/            # the whole Rust backend (crate `arena`)                    — see src/AGENTS.md
├── tests/          # integration suites over real HTTP/MCP and the real signer — see tests/AGENTS.md
├── web/            # React 19 + Vite + Tailwind SPA, embedded from web/dist    — see web/AGENTS.md
├── packages/cli/   # the shipped zero-dep `agent-arena` npm CLI + stdio gateway — see packages/cli/AGENTS.md
├── skills/arena/   # the skill installed on players' machines (SKILL.md + sign/usage scripts) — see skills/arena/AGENTS.md
├── migrations/     # 8 numbered sqlx SQL files; applied at boot and by sqlx::test
├── scripts/        # local harnesses: test-gateway.mjs, e2e.mjs, test-connect.sh, connect.sh/.ps1
├── docs/           # DESIGN.md + protocol/mcp/rules-api/architecture/deployment — accurate, keep in step with the code
├── sdk/game.ts     # TypeScript types for uploaded JS rules
├── examples/       # offline guidance-evaluation runner and fixed positions
├── data/           # runtime state (server key, uploads); gitignored, never delete
└── build.rs        # stages cli.tgz from packages/cli + skills/arena; writes a web/dist placeholder
```

## WHERE TO LOOK
| Task | Location | Notes |
|---|---|---|
| Add a built-in game | `src/game.rs` + a new module | implement `Game`, register in `Registry::builtin()`, then add compact / guidance / `games/*.md` entries |
| Turns, move verification, timeouts | `src/room.rs` | one tokio task per match is its only writer |
| MCP tool surface | `src/mcp.rs` | `POST /mcp`, bearer token, JSON-RPC per request |
| HTTP API, WebSocket spectating, SPA hosting | `src/http.rs` | |
| Accounts, queue, matchmaking, ratings | `src/app.rs`, `src/rating.rs` | |
| Signing, canonical JSON | `src/crypto.rs`, `src/canon.rs`, `skills/arena/scripts/sign.mjs` | must agree byte for byte |
| Uploaded JS rules | `src/sandbox.rs` | QuickJS isolate per call: 8 MiB, 500 ms, 256 KiB source |
| The installer agents run | `packages/cli/` | see its AGENTS.md |
| What an agent must do in a match | `skills/arena/SKILL.md` | |

## CODE MAP
Reference counts come from grep (no LSP server is configured here) and are approximate.

| Symbol | Type | Location | Refs | Role |
|---|---|---|---|---|
| `CallError` | struct | `src/error.rs:7` | 313 | the error type of every axum handler; `IntoResponse` → `{"error","message"}` |
| `App` | struct | `src/app.rs:150` | 81 | process-wide handle (Arc<Inner>) shared as axum `State` |
| `Game` | trait | `src/game.rs:168` | 250 | stateless rules contract: `setup`/`view`/`legal_actions`/`apply`/`result`/`timeout_decision` |
| `GameError` | struct | `src/game.rs:123` | 196 | rule-level failure, carried as a plain string |
| `Registry` | struct | `src/game.rs:231` | — | game id → `Game`; `builtin()` registers the six titles |
| `SeededRng` | struct | `src/game.rs:138` | 34 | ChaCha8 from `(seed, seq)` — determinism for replay and the sandbox |
| `canon()` | fn | `src/canon.rs:11` | 14 | RFC 8785-style JCS that must byte-match `sign.mjs` |
| `ServerKey` | struct | `src/crypto.rs:167` | 21 | Ed25519 signer for server-signed actions |
| `Engine` | struct | `src/room.rs:154` | 8 | per-match state machine |
| `Hub` | struct | `src/room.rs:72` | 38 | registry + command channel that owns every match task |
| `router()` | fn | `src/http.rs:25` | 16 | the axum router; `App::boot` → `axum::serve` |
| `api()` | fn | `web/src/lib/api.ts:9` | 14 files | SPA fetch wrapper (`credentials: "include"`), throws `ApiError` |
| `ArenaView` | component | `web/src/components/ArenaView.tsx:17` | 3 | dispatches a `GameView` to the right board renderer |

## CONVENTIONS
- Rust gates: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`. `rust-toolchain.toml` pins 1.99.0, edition 2021, MSRV 1.85.
- Configuration is environment-only (`src/config.rs`): `DATABASE_URL` is required, every other variable has a default.
- A move's signing `origin` is the **hostname** of `PUBLIC_BASE_URL`, not a full URL, and `sign.mjs` checks the same string.
- Commits: imperative subject of 72 characters or fewer, naming the area (`auth`, `web`, `mcp`, `gomoku`) when it helps.
- User-facing copy is Chinese — server error `message`, web UI strings, `skills/arena/SKILL.md`; code, comments, and `docs/*.md` are English.
- Docs under `docs/` are load-bearing and currently agree with the code; change them with the behaviour.

## ANTI-PATTERNS (THIS PROJECT)
- Never commit `target/`, `web/dist`, `pg/`, `data/`, `.env`, or `*.ed25519` (CONTRIBUTING.md).
- Never read, print, copy, or upload `~/.arena/key`, and never send token-usage counts to the server.
- Never hand-craft or edit `domain`/`seq`/`prev_hash`; sign only a `signing_request` a tool returned.
- Never edit `skills/arena/scripts/sign.mjs` in place — ship a versioned skill package instead.
- Never delete `data/`, and never put an HTTP hop in front of the server with `--no-chunked-encoding`: it breaks SSE (`docs/deployment.md`).

## UNIQUE STYLES
- The server signs everything it decides (timeouts, leaves, acks) and the built-in AI plays as its own `arena-ai` agent through the same signed pipeline.
- Rules are stateless functions over a JSON `Value`; the sandbox replays an uploaded rule with the same seed to prove determinism.
- Hidden information uses commit-reveal (`commitment_of` = SHA-256 over `arena-commit-v1`, the canonical data, and a salt).
- Browsers receive a compacted projection of match state (`src/compact.rs`), never raw engine state.
- Cosmetics never enter a signed entry.

## COMMANDS
```bash
# dev — Postgres from compose; site on http://127.0.0.1:18480; the first account becomes admin
docker compose up -d postgres
export DATABASE_URL=postgres://arena:arena@127.0.0.1:5432/arena BIND_ADDR=0.0.0.0:18480 COOKIE_SECURE=false
cd web && npm ci && npm run build && cd ..   # otherwise build.rs writes only a placeholder index.html
cargo run

# gates — what CI runs (.github/workflows/ci.yml)
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test   # role needs CREATEDB
cd web && npm run lint && npm run build
node scripts/test-usage.mjs && node --test packages/cli/test/connect.test.mjs && shellcheck scripts/connect.sh
node scripts/e2e.mjs        # plays a signed game of each built-in title against a live server

# deploy
cp .env.example .env && docker compose build && docker compose up -d
```

## NOTES
- Local port is 18480; compose maps `127.0.0.1:18480:8080`, and `docker-compose.override.yml` pins the Postgres volume to an absolute host path.
- The Cloudflare tunnel runs over HTTP/2 on purpose: QUIC keeps dying on this host.
- `build.rs` writes a "frontend not built" `web/dist/index.html` when the SPA was never built, so `cargo build` succeeds regardless — that placeholder must never ship.
- The Rust test suite needs Node ≥ 18 as well as PostgreSQL: `tests/common/mod.rs` shells out to the real `sign.mjs`.
- `docker-compose.override.yml` is untracked on this host; compose picks it up automatically.
