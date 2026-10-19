import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { existsSync } from "node:fs";
import path from "node:path";
import { createInterface } from "node:readline";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../", import.meta.url));
const scenario = process.argv[2];
let matchId = process.env.ARENA_TEST_MATCH;

function openGateway(overrides = {}) {
  const entry = process.env.ARENA_TEST_GATEWAY_ENTRY;
  const args = entry ? [entry] : [path.join(root, "packages/cli/bin.mjs"), "serve"];
  const env = { ...process.env, ...overrides };
  const child = spawn(process.execPath, args, {
    env,
    stdio: ["pipe", "pipe", "pipe"],
  });
  const closed = once(child, "close");
  const pending = new Map();
  let serial = 0;
  let stderr = "";
  child.stderr.on("data", chunk => stderr += chunk);
  const lines = createInterface({ input: child.stdout });
  lines.on("line", line => {
    const message = JSON.parse(line);
    const request = pending.get(message.id);
    if (!request) return;
    pending.delete(message.id);
    if (message.error) request.reject(new Error(JSON.stringify(message.error)));
    else request.resolve(message.result);
  });
  child.on("close", () => {
    for (const request of pending.values()) request.reject(new Error("gateway exited"));
    pending.clear();
  });
  return {
    async rpc(method, params = {}) {
      const id = ++serial;
      const signal = AbortSignal.timeout(15000);
      const response = new Promise((resolve, reject) => {
        pending.set(id, { resolve, reject });
        signal.addEventListener("abort", () => {
          pending.delete(id);
          reject(new Error(`gateway request timed out: ${method}`));
        }, { once: true });
      });
      child.stdin.write(JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n");
      return response;
    },
    async stop(signal = "SIGTERM") {
      child.kill(signal);
      await closed;
      lines.close();
      assert.equal(stderr.includes(env.ARENA_TOKEN), false);
    },
  };
}

function toolValue(result) {
  if (result.structuredContent) return result.structuredContent;
  const text = result.content?.find(item => item.type === "text")?.text;
  assert.ok(text, "tool must return structured JSON");
  return JSON.parse(text);
}

async function initialize(gateway) {
  await gateway.rpc("initialize", {
    protocolVersion: "2025-03-26", capabilities: {},
    clientInfo: { name: "arena-local-e2e", version: "1" },
  });
}

async function call(gateway, name, args) {
  return toolValue(await gateway.rpc("tools/call", { name, arguments: args }));
}

let gateway = openGateway();
try {
  // Given a real stdio gateway backed by the real HTTP referee.
  await initialize(gateway);
  const listed = await gateway.rpc("tools/list");
  const moveTools = listed.tools.filter(tool => tool.name === "make_move");
  assert.equal(moveTools.length, 1);
  assert.ok(moveTools[0].inputSchema.properties.action);
  assert.equal(moveTools[0].inputSchema.properties.envelope, undefined);
  if (scenario === "admission") {
    const room = await call(gateway, "create_room", { game: "gomoku", variant: "free" });
    matchId = room.match_id;
    const opponent = openGateway({
      ARENA_TOKEN: process.env.ARENA_TEST_OPPONENT_TOKEN,
      ARENA_HOME: process.env.ARENA_TEST_OPPONENT_HOME,
    });
    try {
      await initialize(opponent);
      await call(opponent, "join_room", { code: room.room_code });
    } finally {
      await opponent.stop();
    }
  }
  const prepared = await call(gateway, "prepare_move", { match_id: matchId });
  assert.equal(typeof prepared.decision_token, "string");
  const action = scenario === "invalid"
    ? { type: "place", data: { at: "z99" } }
    : { type: "place", data: { at: "h8" } };

  // When the chosen move is submitted without a request file or envelope.
  const submitted = await call(gateway, "make_move", {
    decision_token: prepared.decision_token, action,
  });

  // Then rejection precedes signing, or a real signed move is accepted/recovered.
  if (scenario === "invalid") {
    assert.equal(submitted.status, "invalid_action");
    assert.equal(existsSync(path.join(process.env.ARENA_HOME, "signed.json")), false);
  } else if (scenario === "lost-response") {
    assert.equal(submitted.status, "unknown");
    assert.equal(typeof submitted.submission_id, "string");
    await gateway.stop("SIGKILL");
    gateway = openGateway();
    await initialize(gateway);
    const recovered = await call(gateway, "retry_move", {
      submission_id: submitted.submission_id,
    });
    assert.equal(recovered.status, "accepted");
  } else {
    assert.ok(["accepted", "admission"].includes(scenario));
    assert.equal(submitted.status, "accepted");
  }
  assert.equal(existsSync(path.join(process.env.ARENA_HOME, "req.json")), false);
  console.log(JSON.stringify({ scenario, match_id: matchId, passed: true }));
} finally {
  await gateway.stop();
}
