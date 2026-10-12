#!/usr/bin/env node
// Two local agents play a scripted gomoku game over MCP, then sign.mjs verifies the log.
import { spawnSync } from "node:child_process";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const BASE = process.env.BASE_URL || "http://127.0.0.1:18480";
const SIGN = join(dirname(fileURLToPath(import.meta.url)), "../skill/arena/scripts/sign.mjs");
const MOVES = [
  ["black", "h8"],
  ["white", "i8"],
  ["black", "h9"],
  ["white", "i9"],
  ["black", "h10"],
  ["white", "i10"],
  ["black", "h11"],
  ["white", "i11"],
  ["black", "h12"],
];

function die(msg) {
  console.error(msg);
  process.exit(1);
}

async function http(path, { method = "GET", body, cookie, token } = {}) {
  const headers = {};
  if (body) headers["content-type"] = "application/json";
  if (cookie) headers.cookie = cookie;
  if (token) headers.authorization = `Bearer ${token}`;
  const res = await fetch(BASE + path, {
    method,
    headers,
    body: body ? JSON.stringify(body) : undefined,
  });
  const set = typeof res.headers.getSetCookie === "function" ? res.headers.getSetCookie() : [];
  const next = set.map((c) => c.split(";")[0]).join("; ") || cookie || "";
  const text = await res.text();
  const data = text ? JSON.parse(text) : null;
  if (!res.ok) die(`${method} ${path} ${res.status} ${text}`);
  return { data, cookie: next };
}

function signer(home) {
  return (args) => {
    const out = spawnSync("node", [SIGN, ...args], { env: { ...process.env, ARENA_HOME: home }, encoding: "utf8" });
    if (out.status !== 0) die(out.stderr || out.stdout);
    return JSON.parse(out.stdout);
  };
}

async function mcpCall(agent, name, args) {
  agent.rpc += 1;
  const headers = {
    "content-type": "application/json",
    accept: "application/json, text/event-stream",
    authorization: `Bearer ${agent.token}`,
  };
  if (agent.session) headers["mcp-session-id"] = agent.session;
  const res = await fetch(BASE + "/mcp", {
    method: "POST",
    headers,
    body: JSON.stringify({ jsonrpc: "2.0", id: agent.rpc, method: "tools/call", params: { name, arguments: args } }),
  });
  const sid = res.headers.get("mcp-session-id");
  if (sid) agent.session = sid;
  const data = await res.json();
  if (!res.ok || data.error) die(`mcp ${name}: ${JSON.stringify(data)}`);
  const body = data.result?.structuredContent ?? JSON.parse(data.result?.content?.[0]?.text || "{}");
  if (data.result?.isError) die(`tool ${name} failed: ${JSON.stringify(body)}`);
  return body;
}

async function initMcp(agent) {
  const res = await fetch(BASE + "/mcp", {
    method: "POST",
    headers: {
      "content-type": "application/json",
      accept: "application/json, text/event-stream",
      authorization: `Bearer ${agent.token}`,
    },
    body: JSON.stringify({
      jsonrpc: "2.0",
      id: 1,
      method: "initialize",
      params: { protocolVersion: "2025-03-26", capabilities: {}, clientInfo: { name: "e2e", version: "0.1.0" } },
    }),
  });
  agent.session = res.headers.get("mcp-session-id");
  const data = await res.json();
  if (!res.ok || data.error) die(`initialize: ${JSON.stringify(data)}`);
  agent.rpc = 1;
}

async function registerAgent(label) {
  const stamp = Date.now().toString(36);
  const reg = await http("/api/auth/register", {
    method: "POST",
    body: { email: `${label}.${stamp}@example.com`, password: "password123", handle: `${label}${stamp}`.slice(0, 20) },
  });
  const created = await http("/api/agents", {
    method: "POST",
    cookie: reg.cookie,
    body: { name: label, allow_comments: false },
  });
  const issued = await http(`/api/agents/${created.data.id}/tokens`, { method: "POST", cookie: reg.cookie });
  const home = mkdtempSync(join(tmpdir(), `arena-e2e-${label}-`));
  const sign = signer(home);
  sign(["keygen"]);
  const agent = { token: issued.data.token, session: null, rpc: 0, home, sign };
  await initMcp(agent);
  const challenge = await mcpCall(agent, "get_key_challenge", {});
  const req = join(home, "req.json");
  writeFileSync(req, JSON.stringify(challenge.signing_request));
  const envelope = sign(["sign", req]);
  const registered = await mcpCall(agent, "register_agent_key", { envelope, label: "e2e" });
  console.log(label, "kid", registered.kid);
  return agent;
}

async function play(agent, matchId, at) {
  for (let i = 0; i < 6; i++) {
    const turn = await mcpCall(agent, "wait_for_turn", { match_id: matchId });
    if (turn.status === "game_over") return turn;
    if (turn.status === "your_turn") break;
    if (i === 5) die(`${at} never became your_turn: ${JSON.stringify(turn)}`);
  }
  const state = await mcpCall(agent, "get_state", { match_id: matchId });
  if (!state.signing_request) die(`missing signing_request for ${at}`);
  const req = join(agent.home, "req.json");
  writeFileSync(req, JSON.stringify(state.signing_request));
  const envelope = agent.sign(["sign", req, "--action", JSON.stringify({ type: "place", data: { at } })]);
  const accepted = await mcpCall(agent, "make_move", { match_id: matchId, envelope });
  if (!accepted.accepted) die(`move ${at} rejected: ${JSON.stringify(accepted)}`);
  console.log("played", at, "seq", accepted.entry?.seq);
}

const black = await registerAgent("black");
const white = await registerAgent("white");
const room = await mcpCall(black, "create_room", { game: "gomoku", variant: "free", time_limit_sec: 60 });
console.log("room", room.room_code, room.match_id);
const joined = await mcpCall(white, "join_room", { code: room.room_code });
if (joined.status !== "running") die(`join: ${JSON.stringify(joined)}`);

for (const [side, at] of MOVES) {
  await play(side === "black" ? black : white, room.match_id, at);
}

const logRes = await fetch(`${BASE}/api/matches/${room.match_id}/log.jsonl`);
if (!logRes.ok) die(`log ${logRes.status}`);
const log = await logRes.text();
const keysRes = await fetch(`${BASE}/.well-known/arena-keys.json`);
const keys = await keysRes.json();
const dir = mkdtempSync(join(tmpdir(), "arena-e2e-log-"));
const logPath = join(dir, "match.jsonl");
const keyPath = join(dir, "keys.json");
writeFileSync(logPath, log);
writeFileSync(keyPath, JSON.stringify(keys));
const verified = spawnSync("node", [SIGN, "verify", logPath, "--server-keys", keyPath], { encoding: "utf8" });
console.log(verified.stdout);
if (verified.status !== 0) die(verified.stderr || "verify failed");
const report = JSON.parse(verified.stdout);
if (!report.ok || report.agent_sigs !== 9) die(`unexpected report ${verified.stdout}`);
console.log("e2e ok", room.match_id);
