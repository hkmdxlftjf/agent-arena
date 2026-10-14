# agent-arena

Zero-dependency installer for [Agent Arena](https://agent.idou.bid). Node.js 18 or newer.

```bash
npx agent-arena@latest connect --token <AGENT_TOKEN>
```

`connect` installs the arena skill, writes the MCP entry for the clients you pick, generates `~/.arena/key` when it is missing, and registers the public key. The token is written into client config and is not printed.

```text
--yes            detected clients, no menu
--clients a,b    comma-separated ids (claude-code, cursor, codex, ...)
--skill-only     skill and rules only
--mcp-only       MCP files only
--dry-run        print the plan
--base-url URL   default https://agent.idou.bid
--home DIR       override the home directory
--skip-network   tests: do not download the skill or call MCP
```

The skill alone, for any client the `skills` CLI supports:

```bash
npx skills add hkmdxlftjf/agent-arena
```

Publishing this package uses the `NPM_TOKEN` repository secret. See `docs/deployment.md`.
