#!/usr/bin/env node
// Signed games over MCP for every built-in title, then sign.mjs verifies each log.
import { spawnSync } from "node:child_process";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const BASE = process.env.BASE_URL || "http://127.0.0.1:18480";
const SIGN = join(dirname(fileURLToPath(import.meta.url)), "../skills/arena/scripts/sign.mjs");
const GOMOKU = [
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
const JUNQI_SOUTH = [
  ["a1", "军旗"], ["b1", "司令"], ["c1", "军长"], ["d1", "师长"],
  ["a2", "地雷"], ["b2", "地雷"], ["c2", "地雷"], ["d2", "炸弹"], ["e2", "炸弹"],
  ["a3", "师长"], ["c3", "旅长"], ["e3", "旅长"],
  ["a4", "团长"], ["b4", "团长"], ["d4", "营长"], ["e4", "营长"],
  ["a5", "连长"], ["c5", "连长"], ["e5", "连长"], ["d5", "排长"],
  ["a6", "排长"], ["b6", "排长"], ["c6", "工兵"], ["d6", "工兵"], ["e6", "工兵"],
];

function die(msg) {
  console.error(msg);
  process.exit(1);
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
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
  const stamp = Date.now().toString(36) + Math.random().toString(36).slice(2, 6);
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
  const agent = { token: issued.data.token, session: null, rpc: 0, home, sign, id: created.data.id, handle: reg.data.user?.handle };
  await initMcp(agent);
  const challenge = await mcpCall(agent, "get_key_challenge", {});
  const req = join(home, "req.json");
  writeFileSync(req, JSON.stringify(challenge.signing_request));
  const envelope = sign(["sign", req]);
  const registered = await mcpCall(agent, "register_agent_key", { envelope, label: "e2e" });
  console.log(label, "kid", registered.kid);
  return agent;
}

async function snapshot(matchId) {
  const res = await fetch(`${BASE}/api/matches/${matchId}`);
  const text = await res.text();
  if (!res.ok) die(`snapshot ${res.status} ${text}`);
  return JSON.parse(text);
}

async function verifyMatch(matchId, { minAgent = 1, exactAgent, minReveals = 0 } = {}) {
  const pub = await snapshot(matchId);
  if (pub.status !== "finished") die(`${matchId} status ${pub.status}`);
  const logRes = await fetch(`${BASE}/api/matches/${matchId}/log.jsonl`);
  if (!logRes.ok) die(`log ${logRes.status}`);
  const keysRes = await fetch(`${BASE}/.well-known/arena-keys.json`);
  const dir = mkdtempSync(join(tmpdir(), "arena-e2e-log-"));
  const logPath = join(dir, "match.jsonl");
  const keyPath = join(dir, "keys.json");
  writeFileSync(logPath, await logRes.text());
  writeFileSync(keyPath, await keysRes.text());
  const verified = spawnSync("node", [SIGN, "verify", logPath, "--server-keys", keyPath], { encoding: "utf8" });
  console.log(verified.stdout);
  if (verified.status !== 0) die(verified.stderr || verified.stdout || "verify failed");
  const report = JSON.parse(verified.stdout);
  if (!report.ok) die(`verify errors ${verified.stdout}`);
  if (exactAgent != null && report.agent_sigs !== exactAgent) die(`agent sigs ${report.agent_sigs} != ${exactAgent}`);
  if (report.agent_sigs < minAgent) die(`agent sigs ${report.agent_sigs} < ${minAgent}`);
  if (report.reveals < minReveals) die(`reveals ${report.reveals} < ${minReveals}`);
  console.log("verified", matchId, "agents", report.agent_sigs, "reveals", report.reveals);
  return report;
}

async function signingState(agent, matchId) {
  for (let i = 0; i < 40; i++) {
    const state = await mcpCall(agent, "get_state", { match_id: matchId, format: "verbose" });
    if (state.status === "finished") return state;
    if (state.signing_request) return state;
    await sleep(150);
  }
  die(`${agent.id} never received a signing request in ${matchId}`);
}

async function submit(agent, matchId, action, data) {
  const state = await signingState(agent, matchId);
  if (state.status === "finished") return state;
  let commitment;
  let salt;
  let committed = data;
  if (data) {
    const file = join(agent.home, `data-${state.seat}-${Date.now()}.json`);
    writeFileSync(file, JSON.stringify(data));
    const made = agent.sign(["commit", file]);
    commitment = made.commitment;
    salt = made.salt;
    committed = made.data;
  }
  const req = join(agent.home, "req.json");
  writeFileSync(req, JSON.stringify(state.signing_request));
  const args = ["sign", req, "--action", JSON.stringify(action)];
  if (commitment) args.push("--commitment", commitment);
  const envelope = agent.sign(args);
  const body = { match_id: matchId, envelope };
  if (commitment) body.private = { data: committed, salt };
  const accepted = await mcpCall(agent, "make_move", body);
  if (!accepted.accepted) die(`rejected ${JSON.stringify(action)} ${JSON.stringify(accepted)}`);
  console.log(action.type, "seq", accepted.entry?.seq);
  return accepted;
}

async function openRoom(host, guests, game, extra = {}) {
  const room = await mcpCall(host, "create_room", { game, time_limit_sec: 60, ...extra });
  console.log("room", game, room.room_code, room.match_id);
  let last;
  for (const guest of guests) last = await mcpCall(guest, "join_room", { code: room.room_code });
  if (last?.status !== "running") die(`join ${game}: ${JSON.stringify(last)}`);
  return room.match_id;
}

async function playGomoku() {
  const black = await registerAgent("black");
  const white = await registerAgent("white");
  const id = await openRoom(black, [white], "gomoku", { variant: "free" });
  for (const [side, at] of GOMOKU) {
    await submit(side === "black" ? black : white, id, { type: "place", data: { at } });
  }
  await verifyMatch(id, { exactAgent: 9 });
}

async function playXiangqi() {
  const red = await registerAgent("red");
  const black = await registerAgent("blk");
  const id = await openRoom(red, [black], "xiangqi");
  await submit(red, id, { type: "move", data: { from: "b3", to: "b10" } });
  const resigned = await mcpCall(black, "resign", { match_id: id });
  if (!resigned.accepted) die(`xiangqi resign ${JSON.stringify(resigned)}`);
  await verifyMatch(id, { minAgent: 1 });
}

async function playGo() {
  const black = await registerAgent("gblack");
  const white = await registerAgent("gwhite");
  const id = await openRoom(black, [white], "go", { variant: "9" });
  await submit(black, id, { type: "place", data: { at: "D4" } });
  await submit(white, id, { type: "pass" });
  await submit(black, id, { type: "pass" });
  await verifyMatch(id, { exactAgent: 3 });
}

function junqiLayout(seat) {
  return {
    placements: JUNQI_SOUTH.map(([at, piece]) => ({
      at: seat === 0 ? at : `${at[0]}${13 - Number(at.slice(1))}`,
      piece,
    })),
  };
}

async function playJunqi() {
  const south = await registerAgent("south");
  const north = await registerAgent("nrth");
  const id = await openRoom(south, [north], "junqi");
  await submit(south, id, { type: "deploy_commit" }, junqiLayout(0));
  await submit(north, id, { type: "deploy_commit" }, junqiLayout(1));
  await submit(south, id, { type: "move", data: { from: "b4", to: "b3" } });
  const resigned = await mcpCall(north, "resign", { match_id: id });
  if (!resigned.accepted) die(`junqi resign ${JSON.stringify(resigned)}`);
  await verifyMatch(id, { minAgent: 3, minReveals: 2 });
}

function chooseDou(state) {
  const legal = state.legal_actions || [];
  if (state.view?.phase === "bid") return { type: "bid", data: { score: state.seat === 0 ? 1 : 0 } };
  const pass = legal.find((action) => action.type === "pass");
  if (pass) return pass;
  return legal.find((action) => action.type === "play" && action.data?.cards?.length === 1) || legal.find((action) => action.type === "play");
}

function chooseMahjong(state) {
  const phase = state.view?.phase;
  const hand = state.view?.hands?.[state.seat] || [];
  const legal = state.legal_actions || [];
  if (phase === "exchange") {
    const groups = { m: [], s: [], p: [] };
    for (const tile of hand) groups[tile[1]]?.push(tile);
    const tiles = Object.values(groups).sort((a, b) => b.length - a.length)[0].slice(0, 3);
    return { action: { type: "exchange_commit" }, private: { tiles } };
  }
  if (phase === "dingque") {
    const counts = { m: 0, s: 0, p: 0 };
    for (const tile of hand) counts[tile[1]] += 1;
    const suit = Object.entries(counts).sort((a, b) => a[1] - b[1])[0][0];
    return { action: { type: "dingque_commit" }, private: { suit } };
  }
  const hu = legal.find((action) => action.type === "hu");
  if (hu) return hu;
  if (phase === "claim") return legal.find((action) => action.type === "pass") || { type: "pass" };
  const miss = state.view?.dingque?.[state.seat];
  const discards = legal.filter((action) => action.type === "discard");
  return discards.find((action) => action.data?.tile?.[1] === miss) || discards.at(-1);
}

async function drive(players, matchId, choose) {
  for (let step = 0; step < 500; step++) {
    const pub = await snapshot(matchId);
    if (pub.status === "finished") return pub;
    const seats = pub.to_act || [];
    if (!seats.length) {
      await sleep(200);
      continue;
    }
    for (const seat of seats) {
      const state = await signingState(players[seat], matchId);
      if (state.status === "finished") return snapshot(matchId);
      const picked = choose(state);
      if (!picked) die(`no action at ${state.view?.phase} ${JSON.stringify(state.legal_actions?.slice?.(0, 4))}`);
      if (picked.private) await submit(players[seat], matchId, picked.action, picked.private);
      else await submit(players[seat], matchId, picked);
    }
  }
  die(`${matchId} did not finish`);
}

async function playDoudizhu() {
  const players = [await registerAgent("dza"), await registerAgent("dzb"), await registerAgent("dzc")];
  const id = await openRoom(players[0], players.slice(1), "doudizhu");
  await drive(players, id, chooseDou);
  await verifyMatch(id, { minAgent: 4 });
}

async function playMahjong() {
  const players = [await registerAgent("mje"), await registerAgent("mjs"), await registerAgent("mjw"), await registerAgent("mjn")];
  const id = await openRoom(players[0], players.slice(1), "mahjong-sichuan");
  await drive(players, id, chooseMahjong);
  await verifyMatch(id, { minAgent: 8, minReveals: 8 });
}

async function playRanked() {
  const winnerSide = await registerAgent("ranka");
  const loserSide = await registerAgent("rankb");
  const queued = await mcpCall(winnerSide, "join_queue", { game: "gomoku", variant: "free" });
  if (queued.status !== "queued") die(`queue ${JSON.stringify(queued)}`);
  const matched = await mcpCall(loserSide, "join_queue", { game: "gomoku", variant: "free" });
  if (matched.status !== "matched") die(`matchmake ${JSON.stringify(matched)}`);
  const seen = await mcpCall(winnerSide, "queue_status", {});
  if (seen.status !== "matched" || seen.match_id !== matched.match_id) die(`queue_status ${JSON.stringify(seen)}`);
  const resigned = await mcpCall(loserSide, "resign", { match_id: matched.match_id });
  if (!resigned.accepted) die(`ranked resign ${JSON.stringify(resigned)}`);
  await verifyMatch(matched.match_id, { minAgent: 0 });
  const pub = await snapshot(matched.match_id);
  const winSeat = (pub.result?.scores || []).findIndex((score) => score > 0);
  const win = pub.seats.find((seat) => seat.seat === winSeat);
  const lose = pub.seats.find((seat) => seat.seat !== winSeat);
  const winProfile = await (await fetch(`${BASE}/api/agents/${win.agent_id}/profile`)).json();
  const loseProfile = await (await fetch(`${BASE}/api/agents/${lose.agent_id}/profile`)).json();
  const winRating = winProfile.ratings?.find((row) => row.game_id === "gomoku")?.rating;
  const loseRating = loseProfile.ratings?.find((row) => row.game_id === "gomoku")?.rating;
  if (!(winRating > 1500 && loseRating < 1500)) die(`ratings ${winRating} ${loseRating}`);
  const board = await (await fetch(`${BASE}/api/leaderboard?game=gomoku`)).json();
  if (!board.rows?.some((row) => row.agent_id === win.agent_id)) die("winner missing from leaderboard");
  const user = await (await fetch(`${BASE}/api/users/${win.owner_handle}`)).json();
  if (!user.agents?.some((agent) => agent.id === win.agent_id)) die("user profile missing agent");
  console.log("ranked", winRating, loseRating);
}

async function playUploaded() {
  const stamp = Date.now().toString(36) + Math.random().toString(36).slice(2, 6);
  const handle = `up${stamp}`.slice(0, 20);
  const owner = await http("/api/auth/register", {
    method: "POST",
    body: { email: `${handle}@example.com`, password: "password123", handle },
  });
  const example = await fetch(`${BASE}/examples/tic-tac-toe.js`);
  if (!example.ok) die(`example ${example.status}`);
  const source = await example.text();
  const slug = `tt${stamp}`.replace(/[^a-z0-9]/g, "").slice(0, 24);
  const checked = await http("/api/uploads/check", { method: "POST", cookie: owner.cookie, body: { source } });
  if (!checked.data.ok) die(`upload check ${JSON.stringify(checked.data)}`);
  const published = await http("/api/uploads/publish", {
    method: "POST",
    cookie: owner.cookie,
    body: { source, slug, name: "井字棋" },
  });
  const gameId = published.data.id;
  const players = [await registerAgent("ttx"), await registerAgent("tto")];
  const id = await openRoom(players[0], [players[1]], gameId, { variant: "3" });
  const script = [
    [0, "a3"],
    [1, "a2"],
    [0, "b3"],
    [1, "b2"],
    [0, "c3"],
  ];
  for (const [seat, at] of script) {
    await submit(players[seat], id, { type: "place", data: { at } });
  }
  await verifyMatch(id, { exactAgent: 5 });
  const pub = await snapshot(id);
  if (pub.rules_hash !== published.data.rules_hash) die(`rules hash ${pub.rules_hash}`);
  const log = await (await fetch(`${BASE}/api/matches/${id}/log.jsonl`)).text();
  if (!log.includes(published.data.rules_hash)) die("signed log is missing the rules hash");
  writeFileSync("/tmp/arena-uploaded.json", JSON.stringify({ matchId: id, gameId }));
  console.log("uploaded", gameId, id);
}

await playGomoku();
await playXiangqi();
await playGo();
await playJunqi();
await playDoudizhu();
await playMahjong();
await playRanked();
await playUploaded();
console.log("e2e ok");
