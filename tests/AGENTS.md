# AGENTS.md — tests/

## OVERVIEW
Integration suites: 17 `tests/*.rs` files that drive the real app (and, in a few cases, real child processes) instead of unit-testing modules.

> Earned its own file: 49 `#[sqlx::test]` suites plus a shared signer harness — the conventions here differ sharply from the rest of the crate.

## WHERE TO LOOK
| Suite | Covers |
|---|---|
| `db_game.rs`, `validate_move.rs` | a scripted signed gomoku game verified against Postgres; read-only move preflight over real MCP HTTP |
| `timeout_leave.rs`, `clock_sse.rs`, `queue_lease.rs` | per-action clocks, expiry, restart with an overdue deadline, compact projection, SSE framing, queue lease cleanup |
| `cross_sign.rs` | `canon.rs` output vs `skills/arena/scripts/sign.mjs`; no database |
| `local_gateway.rs`, `cli_tgz.rs`, `connect_script.rs` | real stdio gateway → HTTP MCP → match actor; the server-built npm tarball; `scripts/test-connect.sh` |
| `github_oauth.rs`, `key_registration.rs`, `delete_agent.rs`, `cosmetics.rs`, `uploads.rs`, `static_cache.rs`, `game_guidance.rs`, `guidance_positions.rs` | tokens/identities and the remaining feature surfaces |

## CONVENTIONS (this directory)
- A DB-backed test is `#[sqlx::test] async fn name(pool: PgPool)`: sqlx creates one temporary database per test and applies `migrations/`. The role in `DATABASE_URL` needs `CREATEDB`.
- Boot the app the way production does: `App::boot(pool, ServerKey::load_or_create(&dir), Config::for_test(dir))`; bind `127.0.0.1:0`.
- `tests/common/mod.rs` (pulled in with `mod common;`) drives the real signer rather than a mock: `sign_mjs()`, `fresh_home(label)`, `keygen(home)`, `sign(home, request, action, comment)`, `verify(home, log, keys)` shell out to `node skills/arena/scripts/sign.mjs` with `ARENA_HOME` set.
- Isolation is per-test (scratch database + temp dirs), so `cargo test` runs in parallel; plain `#[test]` suites (`cross_sign`, `connect_script`, `cli_tgz`, `guidance_positions`) never touch the database.
- Clocks are exercised with short but real sleeps, and restart tests inject an overdue deadline through SQL rather than waiting. Add no sleep-polling to tests that do not test time.

## COMMANDS
```bash
docker compose up -d postgres
export DATABASE_URL=postgres://arena:arena@127.0.0.1:5432/arena
cargo test                 # whole suite; needs CREATEDB on that role
cargo test --test db_game  # one suite
```
