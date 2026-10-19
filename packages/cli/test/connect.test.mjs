import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import { once } from "node:events";
import http from "node:http";
import { createInterface } from "node:readline";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../..");
const bin = path.join(root, "packages/cli/bin.mjs");
const token = "super-secret-token-value";

function run(args, env = {}) {
  return spawnSync(process.execPath, [bin, ...args], {
    encoding: "utf8",
    env: { ...process.env, ...env },
  });
}

test("root package is a zero-dep bin with no install step", () => {
  const pkg = JSON.parse(fs.readFileSync(path.join(root, "package.json"), "utf8"));
  assert.equal(pkg.name, "agent-arena");
  assert.equal(pkg.engines.node, ">=18");
  assert.equal(pkg.dependencies, undefined);
  assert.equal(pkg.devDependencies, undefined);
  assert.equal(pkg.scripts, undefined);
  assert.equal(pkg.bin["agent-arena"], "packages/cli/bin.mjs");
  const sub = JSON.parse(fs.readFileSync(path.join(root, "packages/cli/package.json"), "utf8"));
  assert.equal(sub.dependencies, undefined);
  assert.equal(sub.bin["agent-arena"], "./bin.mjs");
});

test("npm pack stays on the cli files", () => {
  const res = spawnSync("npm", ["pack", "--dry-run", "--json"], { cwd: root, encoding: "utf8" });
  assert.equal(res.status, 0, res.stderr);
  const packed = JSON.parse(res.stdout)[0];
  const names = packed.files.map((file) => file.path);
  assert.ok(names.includes("packages/cli/bin.mjs"));
  assert.ok(names.includes("packages/cli/lib/connect.mjs"));
  assert.ok(names.includes("packages/cli/lib/write.cjs"));
  for (const name of ["gateway.mjs", "gateway-remote.mjs", "gateway-store.mjs"]) {
    assert.ok(names.includes(`packages/cli/lib/${name}`));
  }
  assert.ok(names.includes("skills/arena/scripts/sign-lock.mjs"));
  assert.ok(names.includes("skills/arena/scripts/sign.mjs"));
  for (const name of names) {
    assert.equal(name.startsWith("src/"), false, name);
    assert.equal(name.startsWith("web/"), false, name);
    assert.equal(name.startsWith("target/"), false, name);
  }
  assert.ok(packed.size < 200_000, `tarball ${packed.size} bytes`);
});

test("remote OpenCode persists permission-only updates and is then idempotent", () => {
  const home = fs.mkdtempSync(path.join(os.tmpdir(), "arena-remote-permissions-"));
  try {
    const file = path.join(home, "opencode.json");
    const url = "https://agent.idou.bid/mcp";
    const entry = { type: "remote", url, enabled: true, headers: { Authorization: `Bearer ${token}` }, oauth: false };
    fs.writeFileSync(file, JSON.stringify({
      mcp: { "agent-arena": entry },
      permission: { webextension: { "*": "ask" } },
    }));
    const invoke = () => spawnSync(process.execPath, [
      path.join(root, "packages/cli/lib/write.cjs"), "opencode", file, url, "0",
    ], { encoding: "utf8", env: { ...process.env, ARENA_CONNECT_TOKEN: token, ARENA_CONNECT_LOCAL: "" } });
    const first = invoke();
    assert.equal(first.status, 0, first.stderr);
    assert.equal(first.stdout.trim(), "updated");
    const text = fs.readFileSync(file, "utf8");
    const doc = JSON.parse(text);
    assert.deepEqual(doc.mcp["agent-arena"], entry);
    assert.deepEqual(doc.permission, {
      webextension: { "*": "ask" },
      external_directory: { "~/.arena/**": "allow" },
      bash: { "node *sign.mjs*": "allow" },
    });
    const backups = fs.readdirSync(home).filter((name) => name.includes(".bak."));
    assert.equal(backups.length, 1);
    const second = invoke();
    assert.equal(second.status, 0, second.stderr);
    assert.equal(second.stdout.trim(), "unchanged");
    assert.equal(fs.readFileSync(file, "utf8"), text);
    assert.deepEqual(fs.readdirSync(home).filter((name) => name.includes(".bak.")), backups);
  } finally { fs.rmSync(home, { recursive: true, force: true }); }
});

function localInstall(home, clients, extra = [], env = {}) {
  return run(["connect", "--local", "--skip-network", "--clients", clients, "--home", home, ...extra], {
    ARENA_TOKEN: token,
    ARENA_HOME: path.join(home, "agent home"),
    ARENA_SKILL_SRC: path.join(root, "skills/arena"),
    ...env,
  });
}

test("unsupported local clients fail before any partial writes", () => {
  const home = fs.mkdtempSync(path.join(os.tmpdir(), "arena-unsupported-"));
  try {
    const result = localInstall(home, "cursor,goose");
    assert.equal(result.status, 2, result.stderr);
    assert.deepEqual(fs.readdirSync(home), []);
    assert.equal((result.stdout + result.stderr).includes(token), false);
  } finally { fs.rmSync(home, { recursive: true, force: true }); }
});

test("local mcp-only requires both installed signer modules before config writes", () => {
  const home = fs.mkdtempSync(path.join(os.tmpdir(), "arena-missing-signer-"));
  try {
    const result = localInstall(home, "cursor", ["--mcp-only"]);
    assert.equal(result.status, 1, result.stderr);
    assert.equal(fs.existsSync(path.join(home, ".cursor/mcp.json")), false);
  } finally { fs.rmSync(home, { recursive: true, force: true }); }
});

test("local configs persist absolute runtime, env credentials and supported stdio shapes", () => {
  const home = fs.mkdtempSync(path.join(os.tmpdir(), "arena-local-config-"));
  try {
    const files = {
      "claude-code": ".claude.json",
      "claude-desktop": ".config/Claude/claude_desktop_config.json",
      cursor: ".cursor/mcp.json",
      vscode: ".config/Code/User/mcp.json",
      gemini: ".gemini/settings.json",
      windsurf: ".codeium/windsurf/mcp_config.json",
      cline: ".config/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json",
      roo: ".config/Code/User/globalStorage/rooveterinaryinc.roo-cline/settings/mcp_settings.json",
      qwen: ".qwen/settings.json",
      kiro: ".kiro/settings/mcp.json",
      trae: ".config/Trae/User/mcp.json",
    };
    const result = localInstall(home, [...Object.keys(files), "codex"].join(","));
    assert.equal(result.status, 0, result.stderr);
    assert.equal((result.stdout + result.stderr).includes(token), false);
    for (const [client, rel] of Object.entries(files)) {
      const doc = JSON.parse(fs.readFileSync(path.join(home, rel)));
      const entry = doc[client === "vscode" ? "servers" : "mcpServers"]["agent-arena"];
      assert.equal(entry.command, process.execPath, client);
      assert.deepEqual(entry.args, [path.join(home, "agent home/runtime/bin.mjs"), "serve"], client);
      assert.equal(entry.env.ARENA_TOKEN, token);
      assert.equal(entry.env.ARENA_HOME, path.join(home, "agent home"));
      assert.equal(entry.env.ARENA_BASE_URL, "https://agent.idou.bid");
      assert.equal(fs.existsSync(entry.env.ARENA_SIGN_SCRIPT), true);
      assert.equal(fs.existsSync(path.join(path.dirname(entry.env.ARENA_SIGN_SCRIPT), "sign-lock.mjs")), true);
      assert.equal(entry.args.some((arg) => arg.includes(token)), false);
      assert.equal(entry.url, undefined);
      if (client === "vscode" || client === "cline" || client === "roo") assert.equal(entry.type, "stdio");
    }
    const toml = fs.readFileSync(path.join(home, ".codex/config.toml"), "utf8");
    assert.ok(toml.includes(`command = ${JSON.stringify(process.execPath)}`));
    assert.ok(toml.includes(`args = [${JSON.stringify(path.join(home, "agent home/runtime/bin.mjs"))}, "serve"]`));
    assert.ok(toml.includes(`ARENA_TOKEN = "${token}"`));
    assert.equal(toml.includes("http_headers"), false);
  } finally { fs.rmSync(home, { recursive: true, force: true }); }
});

for (const format of ["json", "jsonc"]) {
  for (const version of [1, 2]) {
    test(`local OpenCode V${version} ${format} retains nesting, agents and permissions`, () => {
      const home = fs.mkdtempSync(path.join(os.tmpdir(), "arena-local-opencode-"));
      try {
        const other = { type: "local", command: ["echo", "keep"] };
        const doc = {
          mcp: version === 2 ? { servers: { other } } : { other },
          agent: { reviewer: { permission: { bash: "deny" } } },
          permission: { external_directory: "deny", bash: { "*": "ask" } },
        };
        const file = path.join(home, `.config/opencode/opencode.${format}`);
        fs.mkdirSync(path.dirname(file), { recursive: true });
        fs.writeFileSync(file, format === "jsonc" ? `// retain in backup\n${JSON.stringify(doc).replace(/}$/, ",}")}` : JSON.stringify(doc));
        const result = localInstall(home, "opencode");
        assert.equal(result.status, 0, result.stderr);
        const written = JSON.parse(fs.readFileSync(file));
        const servers = version === 2 ? written.mcp.servers : written.mcp;
        assert.deepEqual(servers.other, other);
        assert.deepEqual(written.agent, doc.agent);
        assert.deepEqual(written.permission, doc.permission);
        const entry = servers["agent-arena"];
        assert.equal(entry.type, "local");
        assert.deepEqual(entry.command, [process.execPath, path.join(home, "agent home/runtime/bin.mjs"), "serve"]);
        assert.equal(entry.environment.ARENA_TOKEN, token);
        assert.equal(entry.environment.ARENA_HOME, path.join(home, "agent home"));
        if (version === 2) {
          assert.equal(written.mcp["agent-arena"], undefined);
          assert.equal(entry.disabled, false);
          assert.equal(entry.enabled, undefined);
          assert.equal(entry.protocol, "legacy");
          assert.deepEqual(entry.timeout, { startup: 30000, catalog: 30000, execution: 660000 });
        } else {
          assert.equal(written.mcp.servers, undefined);
          assert.equal(entry.enabled, true);
        }
        const again = localInstall(home, "opencode");
        assert.equal(again.status, 0, again.stderr);
        assert.match(again.stdout, /unchanged/);
        assert.equal(fs.readdirSync(path.dirname(file)).filter((name) => name.includes(".bak.")).length, 1);
        if (format === "jsonc") assert.equal(fs.existsSync(file.replace(/jsonc$/, "json")), false);
      } finally { fs.rmSync(home, { recursive: true, force: true }); }
    });
  }
}

test("fresh local OpenCode follows installed major or current V2 when unavailable", () => {
  for (const major of [1, 2, null]) {
    const home = fs.mkdtempSync(path.join(os.tmpdir(), "arena-opencode-version-"));
    try {
      const dir = path.join(home, "bin");
      fs.mkdirSync(dir);
      if (major) fs.writeFileSync(path.join(dir, "opencode"), `#!/bin/sh\nprintf '%s\\n' '${major}.0.0'\n`, { mode: 0o755 });
      const result = localInstall(home, "opencode", [], { PATH: dir });
      assert.equal(result.status, 0, result.stderr);
      const doc = JSON.parse(fs.readFileSync(path.join(home, ".config/opencode/opencode.json")));
      assert.equal(Boolean(doc.mcp.servers), major !== 1);
    } finally { fs.rmSync(home, { recursive: true, force: true }); }
  }
});

test("installed Node config starts without source tree and exposes real gateway tools", { timeout: 15000 }, async (t) => {
  const home = fs.mkdtempSync(path.join(os.tmpdir(), "arena-installed-startup-"));
  t.after(() => fs.rmSync(home, { recursive: true, force: true }));
  const server = http.createServer(async (request, response) => {
    assert.equal(request.headers.authorization, `Bearer ${token}`);
    let body = "";
    for await (const chunk of request) body += chunk;
    const msg = JSON.parse(body);
    if (msg.id === undefined) { response.writeHead(202).end(); return; }
    const result = msg.method === "initialize"
      ? { protocolVersion: "2025-03-26", capabilities: {}, serverInfo: { name: "fixture", version: "1" } }
      : msg.method === "tools/list"
        ? { tools: [{ name: "make_move", inputSchema: { type: "object" } }, { name: "list_games", inputSchema: { type: "object" } }] }
        : { structuredContent: { games: ["fixture-game"] }, content: [{ type: "text", text: '{"games":["fixture-game"]}' }] };
    response.writeHead(200, { "content-type": "application/json" });
    response.end(JSON.stringify({ jsonrpc: "2.0", id: msg.id, result }));
  });
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  t.after(() => { server.closeAllConnections(); server.close(); });
  const source = path.join(home, "disposable-cli");
  fs.cpSync(path.join(root, "packages/cli"), source, { recursive: true });
  const installed = spawnSync(process.execPath, [path.join(source, "bin.mjs"), "connect", "--local",
    "--skip-network", "--clients", "cursor", "--home", home, "--base-url", `http://127.0.0.1:${server.address().port}`], {
    encoding: "utf8", env: { ...process.env, ARENA_TOKEN: token, ARENA_HOME: path.join(home, "agent home"),
      ARENA_SKILL_SRC: path.join(root, "skills/arena") },
  });
  assert.equal(installed.status, 0, installed.stderr);
  fs.rmSync(source, { recursive: true });
  const entry = JSON.parse(fs.readFileSync(path.join(home, ".cursor/mcp.json"))).mcpServers["agent-arena"];
  const version = spawnSync(entry.command, [entry.env.ARENA_SIGN_SCRIPT, "version"], { encoding: "utf8", env: { ...process.env, ...entry.env } });
  assert.equal(version.status, 0, version.stderr);
  const child = spawn(entry.command, entry.args, { env: { ...process.env, ...entry.env }, stdio: ["pipe", "pipe", "pipe"] });
  const closed = once(child, "close");
  t.after(() => { if (child.exitCode === null) child.kill(); });
  let stderr = "";
  child.stderr.on("data", (chunk) => { stderr += chunk; });
  const lines = createInterface({ input: child.stdout });
  const waiting = new Map();
  lines.on("line", (line) => {
    const msg = JSON.parse(line);
    const finish = waiting.get(msg.id);
    assert.ok(finish, `unexpected protocol response ${line}`);
    waiting.delete(msg.id);
    finish(msg);
  });
  let id = 0;
  const rpc = (method, params = {}) => new Promise((resolve) => {
    const next = ++id;
    waiting.set(next, resolve);
    child.stdin.write(`${JSON.stringify({ jsonrpc: "2.0", id: next, method, params })}\n`);
  });
  const initialized = await rpc("initialize");
  assert.equal(initialized.result.serverInfo.name, "agent-arena-local");
  const tools = (await rpc("tools/list")).result.tools;
  for (const name of ["prepare_move", "retry_move", "make_move", "list_games"]) assert.ok(tools.some((tool) => tool.name === name));
  const move = tools.find((tool) => tool.name === "make_move");
  assert.deepEqual(move.inputSchema.required, ["decision_token", "action"]);
  assert.equal(move.inputSchema.properties.envelope, undefined);
  const games = await rpc("tools/call", { name: "list_games", arguments: {} });
  assert.deepEqual(games.result.structuredContent, { games: ["fixture-game"] });
  child.stdin.end();
  const [code] = await closed;
  assert.equal(code, 0, stderr);
  assert.equal(stderr.includes(token), false);
});

test("skill-only and mcp-only together exit 2 and hide the token", () => {
  const res = run(["connect", "--skill-only", "--mcp-only", "--dry-run", "--yes", "--token", token]);
  assert.equal(res.status, 2);
  assert.match(res.stderr, /不能一起用/);
  assert.equal(res.stdout.includes(token), false);
  assert.equal(res.stderr.includes(token), false);
});

test("dry-run plans cursor and does not write the token", () => {
  const home = fs.mkdtempSync(path.join(os.tmpdir(), "arena-cli-"));
  fs.mkdirSync(path.join(home, ".cursor"));
  const res = run([
    "connect",
    "--dry-run",
    "--yes",
    "--token",
    token,
    "--home",
    home,
    "--base-url",
    "https://agent.idou.bid",
  ]);
  assert.equal(res.status, 0, res.stderr);
  assert.match(res.stdout, /dry-run: 1/);
  assert.match(res.stdout, /client: cursor/);
  assert.equal(res.stdout.includes(token), false);
  assert.equal(fs.existsSync(path.join(home, ".cursor", "mcp.json")), false);
});

test("missing token fails closed", () => {
  const res = run(["connect", "--yes", "--home", fs.mkdtempSync(path.join(os.tmpdir(), "arena-cli-"))]);
  assert.equal(res.status, 1);
  assert.match(res.stderr, /缺少 --token/);
});

test("skill-only links the skill and skips MCP config", () => {
  const home = fs.mkdtempSync(path.join(os.tmpdir(), "arena-cli-skill-"));
  const res = run(
    [
      "connect",
      "--skip-network",
      "--skill-only",
      "--clients",
      "cursor,vscode",
      "--token",
      token,
      "--home",
      home,
      "--base-url",
      "https://agent.idou.bid",
    ],
    { ARENA_SKILL_SRC: path.join(root, "skills/arena") },
  );
  assert.equal(res.status, 0, `${res.stderr}\n${res.stdout}`);
  const central = path.join(home, ".arena", "skill", "arena");
  assert.equal(fs.readlinkSync(path.join(home, ".cursor", "skills", "arena")), central);
  const rules = fs.readFileSync(path.join(home, ".copilot", "instructions", "arena.instructions.md"), "utf8");
  assert.ok(rules.includes(path.join(central, "SKILL.md")));
  assert.ok(rules.includes(path.join(central, "scripts", "sign.mjs")));
  assert.equal(fs.existsSync(path.join(home, ".cursor", "mcp.json")), false);
  assert.equal(res.stdout.includes(token), false);
});
