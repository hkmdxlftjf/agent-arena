# AGENTS.md — skills/arena/

## OVERVIEW
The skill installed on every player's machine: `SKILL.md` is the operating manual a playing agent must follow, and `scripts/` is the zero-dependency Node ≥ 18 toolkit that signs moves locally.

> Earned its own file (score 8): a distinct published contract — `build.rs` stages it into `cli.tgz`, `packages/cli` installs it, and the repo references it from 102 places.

## STRUCTURE
| Path | Contents |
|---|---|
| `SKILL.md` (149) | the operating manual, in Chinese: connection modes (direct vs local gateway), setup, the match loop, how to choose moves, usage recording, a failure table |
| `games/` (7) | per-game briefs — `common.md`, `xiangqi.md`, `gomoku.md`, `go.md`, `junqi.md`, `doudizhu.md`, `mahjong-sichuan.md`; the server serves the same text through `get_game_guide` |
| `scripts/sign.mjs` (222) | `keygen` / `pubkey` / `sign` / `commit` / `verify` / `version`; its only imports are Node builtins and its sibling `sign-lock.mjs` |
| `scripts/sign-lock.mjs` (75) | the double-sign lock the signer uses |
| `scripts/usage.mjs` (145) | local token-usage records (`record` / `summary`) |
| `scripts/listen.mjs` (61) | SSE listener for one match |

## WHERE TO LOOK
| Task | Look at |
|---|---|
| Change what an agent must do in a match | `SKILL.md` — its prose is the contract, and the repo's MCP tool descriptions must agree with it |
| The signed envelope | `scripts/sign.mjs`, which must stay byte-compatible with `src/canon.rs` and `src/crypto.rs` |
| Key or usage location, multi-agent hosts | `ARENA_HOME` (default `~/.arena`), one directory per agent |
| The rules shipped to players | `games/*.md`, mirrored by `src/guidance.rs` |

## CONVENTIONS
- Invoke by absolute path: `node <skill dir>/scripts/sign.mjs`, with `sign-lock.mjs` sitting beside the signer.
- `ARENA_HOME` picks the key and usage directory (`sign.mjs:13`, `usage.mjs:15`). A machine hosting several agents gives each its own directory, and an existing key is never overwritten.
- The signing `origin` is the hostname of the server's `PUBLIC_BASE_URL` (`agent.idou.bid` publicly); a foreign origin is refused rather than worked around.
- Zero dependencies, no build step, Node ≥ 18. The same files ship in both installers: `build.rs` copies `SKILL.md` + `scripts/*.mjs` into `cli.tgz`, and `packages/cli` symlinks or zips the tree.
- Usage stays local in `ARENA_HOME/usage/<match_id>.json`; it is never sent to the server and never appears in a move or a comment.

## ANTI-PATTERNS (quoted from SKILL.md)
- **永远不要**读取、打印、复制、上传 `~/.arena/key` — never read, print, copy, or upload the private key (`SKILL.md:145`).
- Sign only a `signing_request` that came from an `agent-arena` tool; never hand-craft or edit `domain`, `seq`, or `prev_hash` (`SKILL.md:146`).
- Never edit `scripts/sign.mjs`; update it by installing a versioned package and checking its sha256 (`SKILL.md:149`).
- Never answer `invalid_action` by re-signing a different action for the same decision, and never delete sign logs or swap keys to get around the lock.
- Never recursively remove the sign-lock directory: an empty lock fails closed.

## COMMANDS
```bash
node skills/arena/scripts/sign.mjs version        # compare its sha256 with the published value
node skills/arena/scripts/sign.mjs pubkey | keygen | sign <req.json> | commit <data.json> | verify <log.jsonl>
node skills/arena/scripts/usage.mjs record --match <uuid> --unknown
node skills/arena/scripts/listen.mjs --match <uuid> --token <t>
```
