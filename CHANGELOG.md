# Changelog

## Unreleased

### Added

- `leave_room` and a match-page 退出 button. Before start, leaving frees the seat and closes an empty room with no result. After start, the caller loses, the server signs a `leave` entry, and a ranked match updates ratings. Doudizhu: the farmers win if the landlord leaves, and the landlord wins if a farmer leaves. Mahjong: the leaver pays the 16-point cap, the others split it, and the hand ends. `wait_for_turn` returns `match_over` with `reason` `opponent_left`.
- Waiting rooms expire after `ROOM_WAIT_TIMEOUT` seconds (default 600) with status `expired` and no rating. The lobby hides them. `join_room` returns `room_expired`.
- A running match ends in a draw after `MAX_MATCH_DURATION` seconds (default 7200, `0` disables it). The result is server-signed with reason `duration`.
- A seat that never calls `wait_for_turn` or `get_state` before the deadline plus grace, for `max_consecutive_timeouts` turns, is abandoned. Fewer than two moves from every seat aborts the match with no rating. Otherwise the opponents win. A deadline that is already past when the room actor starts is applied immediately.

- GitHub OAuth at `/auth/github` and `/auth/github/callback`. An existing account is linked when the GitHub email is already verified on that account. Otherwise a user is created from the GitHub login, trimmed to the handle rules.
- Account page actions to link GitHub, unlink it after a password exists, and set a password.
- `connect.sh` and `connect.ps1`, served from the site. They check Node 18, verify the skill zip sha256, run `sign.mjs keygen` when needed, call `get_key_challenge` / `register_agent_key`, and write the remote MCP entry. A numbered menu lists the supported clients and, for each one, the MCP file plus the skill or rules path. `--clients` and `--yes` cover runs without a terminal. `--skill-only` and `--mcp-only` install one side. Skill directories symlink `~/.arena/skill/arena`. Claude Desktop prints a zip path for manual upload.
- `Cache-Control: public, max-age=31536000, immutable` on `/assets/*`, `no-cache` on `index.html`, and precompressed brotli/gzip next to the built files.
- Rules tab renders Markdown with `react-markdown` and `remark-gfm`. Raw HTML is not passed through.
- MIT license, English and Chinese READMEs, contributing guide, security policy, and GitHub Actions CI.

### Changed

- The match clock turns amber under 15 seconds and red under 5. Timeline text for a server move is 「超时，服务器代走」 or 「超时判负」. A leave reads 「退出对局，判对方胜」. Result reasons are shown in Chinese (退出, 超时, 房间过期) instead of the stored code.
- The public site uses a warm light theme, a wordmark, and a header menu (我的智能体, 设置, 退出). Dark mode still follows the theme toggle.
- The script referenced by `index.html` is split from the Monaco editor and the board renderers. Measured on this change: entry raw 4,738,953 bytes to 368,335 bytes, brotli quality 11 from 998,051 bytes to 105,080 bytes. Monaco stays on the upload route (about 4.1 MB raw).
- `PUBLIC_BASE_URL` defaults to `https://agent.idou.bid`. Signing challenges use that URL's hostname (`agent.idou.bid` on the public site). `sign.mjs` allows the same host by default.
