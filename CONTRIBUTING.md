# Contributing

Questions about how a game should behave belong in an issue. Small fixes can go straight to a pull request.

## Setup

You need PostgreSQL 16, Rust 1.99 (see `rust-toolchain.toml`), and Node.js 22.

```bash
docker compose up -d postgres
export DATABASE_URL=postgres://arena:arena@127.0.0.1:5432/arena
export BIND_ADDR=0.0.0.0:18480
export COOKIE_SECURE=false
export DATA_DIR=./data

cd web && npm ci && npm run build && cd ..
cargo run
```

Open `http://127.0.0.1:18480`. The first registered account is an admin. `cd web && npm run dev` proxies `/api`, `/ws`, and `/mcp` to port 18480.

Copy `.env.example` if you want the variable names in one place. Do not commit a filled-in `.env`.

## Tests

```bash
export DATABASE_URL=postgres://arena:arena@127.0.0.1:5432/arena
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

The database role needs `CREATEDB`. `sqlx::test` creates a temporary database per test and applies `migrations/`.

`tests/cross_sign.rs` shells out to `skills/arena/scripts/sign.mjs`. `sh scripts/test-connect.sh` runs shellcheck and a dry run of `connect.sh`. `node --test packages/cli/test/connect.test.mjs` covers the npm installer. With the server already on 18480, `node scripts/e2e.mjs` plays a signed game of each built-in title.

Frontend:

```bash
cd web && npm run lint && npm run build
```

## Commits

Use an imperative subject, 72 characters or fewer: `Add GitHub link on the account card`. Put the reason in the body when the diff is not obvious. Mention the area (`auth`, `web`, `mcp`, `gomoku`) when it helps the log. Do not commit `target/`, `web/dist`, `pg/`, `data/`, `.env`, or `*.ed25519`.

## Add a built-in game

Built-in games implement `Game` in `src/game.rs`:

```rust
fn manifest(&self) -> Manifest;
fn setup(&self, cfg: &Config, rng: &mut dyn ArenaRng) -> Result<(Value, Vec<Value>), GameError>;
fn to_act(&self, state: &Value) -> Vec<Seat>;
fn view(&self, state: &Value, viewer: &Viewer) -> Result<Value, GameError>;
fn legal_actions(&self, state: &Value, seat: Seat) -> Result<Vec<Value>, GameError>;
fn apply(&self, state: &Value, seat: Seat, action: &Value, rng: &mut dyn ArenaRng) -> Result<(Value, Vec<Value>), GameError>;
fn is_terminal(&self, state: &Value) -> bool;
fn result(&self, state: &Value) -> Result<GameResult, GameError>;
fn timeout_decision(&self, state: &Value, seat: Seat, policy: TimeoutPolicy, rng: &mut dyn ArenaRng) -> Result<TimeoutDecision, GameError>;
```

`gomoku.rs` is the shortest reference. State is JSON. `setup` returns the initial state and public events. `view` must hide information the viewer should not see. `legal_actions` is what the agent is offered. `apply` is what actually changes the state. Same seed and same actions must produce the same state. Do not read the clock. Timeouts are decided by `timeout_decision`.

Register the game in `Registry::builtin` in `src/game.rs`. The id in `manifest()` is the MCP `game` string.

If the board is not a stone list, set `renderer` on the spectator view to a name the web client already has (`xiangqi`, `go`, `junqi`, `card-table`, `mahjong-table`, `grid-board`) or add a component under `web/src/components/` and a branch in `ArenaView.tsx`. Load it with `React.lazy` so it stays out of the entry chunk.

Add a test next to the module, or extend `scripts/e2e.mjs` if the game needs a signed match against `sign.mjs`.

## Upload a JavaScript ruleset

Authors do this from the game hall at `/games/new`. The server does not compile TypeScript. The type hints are `sdk/game.ts`. The module must call `defineGame({...})`. Limits and the method list are in [docs/rules-api.md](docs/rules-api.md).

Uploaded games are not rated. A published version is not edited in place. The rules hash covers the manifest and the source and is stored on the match log.

## Conduct

See [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md). Security reports go through [SECURITY.md](SECURITY.md), not a public issue.
