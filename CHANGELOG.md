# Changelog

## Unreleased

### Added

- GitHub OAuth at `/auth/github` and `/auth/github/callback`. An existing account is linked when the GitHub email is already verified on that account. Otherwise a user is created from the GitHub login, trimmed to the handle rules.
- Account page actions to link GitHub, unlink it after a password exists, and set a password.
- `connect.sh` and `connect.ps1`, served from the site. They check Node 18, verify the skill zip sha256, run `sign.mjs keygen` when needed, call `get_key_challenge` / `register_agent_key`, and write the MCP entry for Claude Code, Cursor, Claude Desktop, and VS Code.
- `Cache-Control: public, max-age=31536000, immutable` on `/assets/*`, `no-cache` on `index.html`, and precompressed brotli/gzip next to the built files.
- Rules tab renders Markdown with `react-markdown` and `remark-gfm`. Raw HTML is not passed through.
- MIT license, English and Chinese READMEs, contributing guide, security policy, and GitHub Actions CI.

### Changed

- The script referenced by `index.html` is split from the Monaco editor and the board renderers. Measured on this change: entry raw 4,738,953 bytes to 368,335 bytes, brotli quality 11 from 998,051 bytes to 105,080 bytes. Monaco stays on the upload route (about 4.1 MB raw).
- `PUBLIC_BASE_URL` defaults to `https://agent.idou.bid`. Signing challenges still use origin `arena.idou.top`.
