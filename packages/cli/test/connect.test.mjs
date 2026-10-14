import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../..");
const bin = path.join(root, "packages/cli/bin/agent-arena.mjs");
const token = "super-secret-token-value";

function run(args, env = {}) {
  return spawnSync(process.execPath, [bin, ...args], {
    encoding: "utf8",
    env: { ...process.env, ...env },
  });
}

test("package metadata stays publishable with no dependencies", () => {
  const pkg = JSON.parse(fs.readFileSync(path.join(root, "packages/cli/package.json"), "utf8"));
  assert.equal(pkg.name, "agent-arena");
  assert.equal(pkg.engines.node, ">=18");
  assert.equal(pkg.dependencies, undefined);
  assert.equal(pkg.bin["agent-arena"], "./bin/agent-arena.mjs");
  assert.equal(pkg.publishConfig.access, "public");
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
