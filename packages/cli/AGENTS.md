# AGENTS.md — packages/cli/

## OVERVIEW
The shipped `agent-arena` npm package: a zero-dependency Node 18+ CLI that installs the arena skill, writes MCP client configs for 16 clients, and registers an agent's Ed25519 public key. It also hosts the stdio MCP gateway.

> Earned its own file (score 10): a published package with its own `package.json`/README and a `git`-install contract, referenced from 56 places in the repo.

## STRUCTURE
| File | Lines | Role |
|---|---|---|
| `bin.mjs` | 13 | Entry: dispatches `serve` (stdio gateway) versus the default `connect` installer. |
| `lib/connect.mjs` | 890 | The installer — MCP config per client, skill symlinks/zips, keygen + registration. |
| `lib/gateway.mjs`, `gateway-remote.mjs`, `gateway-store.mjs` | 401 / 158 / 189 | stdio MCP gateway exposing `prepare_move`/`make_move`/`retry_move`; durable journal with canonical JSON and hard-link+fsync saves; Streamable-HTTP client. |
| `lib/write.cjs` | 399 | CommonJS config writer, JSONC-tolerant, backs the file up before writing. |
| `test/connect.test.mjs` | — | `node --test` suite; also asserts the npm pack contents. |

## WHERE TO LOOK
| Task | Look at |
|---|---|
| Client menu, `--skill-only` / `--mcp-only` / `--dry-run` | `lib/connect.mjs` |
| Move submission semantics, journal, retry | `lib/gateway.mjs`, `lib/gateway-store.mjs` |
| Talking to `POST /mcp` | `lib/gateway-remote.mjs` |
| A new lib module | add it to `pack_cli()` in the repo-root `build.rs` as well (the shipped `cli.tgz` copies files by explicit list; `package.json` `files` already covers all of `packages/cli/lib/`) |

## CONVENTIONS
- The package ships from the **repo root** `package.json` (`bin` → `packages/cli/bin.mjs`); `build.rs` stages the same layout into `cli.tgz`, which the server serves at `GET /cli.tgz`. Installed with `npx -y github:hkmdxlftjf/agent-arena connect` or `npx -y https://<host>/cli.tgz connect` — never from the npm registry.
- No dependencies, no build step, no prepare script: it must run straight from `npx`.
- A signed envelope is bound to `(match_id, seat, seq)`. `make_move` never applies an action against a newer decision; `retry_move` resends an identical envelope and never changes the comment; a lost HTTP response is never auto-retried.
- Config writes are idempotent: a second run with the same URL and token leaves the file untouched, and files are backed up before the first write.
- Signer errors surface as short codes; the key material and the raw error text are never forwarded or logged.

## ANTI-PATTERNS
- Never add a dependency to this package.
- Never use `keygen --force` to repair a room-key mismatch (see `packages/cli/README.md`).
- Never let the token reach stdout: `scripts/test-connect.sh` asserts the installers never echo it.

## COMMANDS
```bash
node packages/cli/bin.mjs connect --token <AGENT_TOKEN> --dry-run
node --test packages/cli/test/connect.test.mjs
```
