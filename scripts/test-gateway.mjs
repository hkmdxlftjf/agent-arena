import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createHash, createPublicKey, verify } from "node:crypto";
import { EventEmitter, once } from "node:events";
import { mkdtemp, mkdir, readFile, readdir, rm, stat, writeFile } from "node:fs/promises";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createInterface } from "node:readline";
import { fileURLToPath } from "node:url";
import test from "node:test";
import { createGateway, serve } from "../packages/cli/lib/gateway.mjs";
import { canonical, Journal } from "../packages/cli/lib/gateway-store.mjs";
import { RemoteClient } from "../packages/cli/lib/gateway-remote.mjs";

const root = fileURLToPath(new URL("../", import.meta.url));
const signer = join(root, "skills/arena/scripts/sign.mjs");
const entrypoint = join(root, "packages/cli/lib/gateway.mjs");
const authToken = "gateway-fixture-secret-not-for-logs";
const match = "00000000-0000-4000-8000-000000000009";
const agent = "00000000-0000-4000-8000-000000000001";
const action = { type: "place", x: 2, y: 3 };
const clone = value => JSON.parse(JSON.stringify(value));
const sha = value => createHash("sha256").update(value).digest();
const packed = value => ({ content: [{ type: "text", text: JSON.stringify(value) }],
  structuredContent: value, isError: Boolean(value.error || value.accepted === false) });
const valueOf = result => result.structuredContent;

function controlledDeadlines(t) {
  const deadlines = [];
  const original = AbortSignal.timeout;
  // Replace only deadline scheduling. Real fetch, bodies and AbortSignals remain
  // connected so each test aborts an actual in-flight HTTP request after a signal.
  AbortSignal.timeout = milliseconds => {
    const controller = new AbortController();
    deadlines.push({ milliseconds, controller });
    return controller.signal;
  };
  t.after(() => { AbortSignal.timeout = original; });
  return deadlines;
}

async function runSigner(env, args, input) {
  const child = spawn(process.execPath, [signer, ...args], { env, stdio: ["pipe", "pipe", "pipe"] });
  const closed = once(child, "close");
  let stdout = "", stderr = "";
  child.stdout.on("data", data => { stdout += data; });
  child.stderr.on("data", data => { stderr += data; });
  child.stdin.end(input === undefined ? "" : JSON.stringify(input));
  const [code] = await closed;
  assert.equal(code, 0, "The real fixture signer must succeed");
  assert.equal(stderr, "");
  return JSON.parse(stdout);
}

async function fixture(t) {
  const home = await mkdtemp(join(tmpdir(), "arena-gateway-test-"));
  const arenaHome = join(home, "agent-a");
  await mkdir(arenaHome, { mode: 0o700 });
  const env = { ...process.env, HOME: home, USERPROFILE: home, ARENA_HOME: arenaHome,
    ARENA_TOKEN: authToken, ARENA_SIGN_SCRIPT: signer };
  delete env.ARENA_KEY_PASSPHRASE;
  const publicKey = await runSigner(env, ["keygen"]);
  const events = new EventEmitter();
  const f = {
    home, env, publicKey, events, calls: [], submissions: [], entries: [],
    mode: "accept", validation: "valid", who: agent, keys: [publicKey],
    sessions: new Set(), errors: [], clients: new Set(), gates: [],
  };
  const server = createServer(async (request, response) => {
    try {
      let text = "";
      for await (const chunk of request) text += chunk;
      if (request.url === "/signer-barrier") {
        f.gates.push(response);
        events.emit("signer-barrier", JSON.parse(text));
        return;
      }
      assert.equal(request.url, "/mcp");
      assert.equal(request.headers.authorization, `Bearer ${authToken}`);
      const message = JSON.parse(text);
      const reply = result => {
        response.setHeader("content-type", "application/json");
        response.end(JSON.stringify({ jsonrpc: "2.0", id: message.id, result }));
      };
      if (message.method === "initialize") {
        const session = `fixture-session-${f.sessions.size + 1}`;
        f.sessions.add(session);
        response.setHeader("mcp-session-id", session);
        reply({ protocolVersion: "2025-03-26", capabilities: { tools: {} } });
        return;
      }
      assert.ok(f.sessions.has(request.headers["mcp-session-id"]), "Remote session IDs must be reused");
      if (message.method === "notifications/initialized") {
        response.writeHead(202).end();
        return;
      }
      if (message.method === "tools/list") {
        reply({ tools: ["whoami", "get_state", "validate_move", "make_move", "get_history",
          "get_match_log", "wait_for_turn", "wire_error", "wire_http"].map(name => ({
          name, inputSchema: { type: "object", properties: {} },
        })) });
        return;
      }
      assert.equal(message.method, "tools/call");
      const { name, arguments: args } = message.params;
      f.calls.push({ name, args: clone(args) });
      switch (name) {
        case "whoami":
          reply(packed({ agent_id: f.who, keys: f.keys.map(key => ({ ...key, status: "active" })) }));
          break;
        case "get_state":
          if (args.history === "full" && f.historyError) {
            reply(packed({ error: "history_unavailable" }));
          } else {
            reply(packed({ ...clone(f.state),
              ...(args.history === "full" && !f.historyMissing ? { entries: clone(f.entries) } : {}) }));
          }
          break;
        case "validate_move": {
          if (f.holdValidation) {
            const release = new Promise(resolve => { f.releaseValidation = resolve; });
            response.once("close", () => events.emit("validation-closed"));
            events.emit("validation-entered");
            await release;
          }
          if (f.validation === "absent") {
            reply({ isError: true, content: [{ type: "text", text: '{"error":"unknown_tool"}' }] });
          } else if (f.validation === "rpc-error") {
            response.end(JSON.stringify({ jsonrpc: "2.0", id: message.id,
              error: { code: -32601, message: "Unknown tool" } }));
          } else if (f.validation === "unrecognized") {
            reply(packed({ ok: true }));
          } else if (f.validation !== "valid") {
            reply(packed({ valid: false, error: f.validation }));
          } else {
            let valid = canonical(args.signing_request) === canonical(f.state.signing_request)
              && args.match_id === match && args.action?.type === "place"
              && Number.isInteger(args.action.x) && args.action.x >= 0 && args.action.x < 15
              && Number.isInteger(args.action.y) && args.action.y >= 0 && args.action.y < 15
              && (args.comment === undefined || [...args.comment].length <= 140);
            if (args.private !== undefined || args.commitment !== undefined) {
              valid &&= typeof args.private?.salt === "string" && /^[a-f0-9]{64}$/.test(args.private.salt);
              if (valid) valid &&= args.commitment === sha(Buffer.concat([
                Buffer.from("arena-commit-v1"), Buffer.from(canonical(args.private.data)),
                Buffer.from(args.private.salt, "hex"),
              ])).toString("hex");
            }
            reply(packed(valid ? { valid: true } : { valid: false, error: "illegal_action" }));
          }
          break;
        }
        case "make_move": {
          const { envelope } = args;
          assert.equal(envelope.payload.agent_id, f.who);
          assert.equal(envelope.kid, publicKey.kid);
          const pub = createPublicKey({ format: "jwk", key: {
            kty: "OKP", crv: "Ed25519", x: publicKey.public_key,
          } });
          const bytes = Buffer.concat([Buffer.from("\x19AgentArena Signed Message:\n"),
            sha(canonical(envelope.domain)), sha(canonical(envelope.payload))]);
          assert.equal(verify(null, bytes, pub, Buffer.from(envelope.signature, "base64url")), true);
          const directory = join(arenaHome, "gateway", "submissions");
          const records = await Promise.all((await readdir(directory))
            .filter(name => name.endsWith(".json")).map(async name => JSON.parse(await readFile(join(directory, name)))));
          assert.ok(records.some(record => canonical(record.envelope) === canonical(envelope)
            && canonical(record.intent.private) === canonical(args.private)), "Envelope/private must be durable before HTTP");
          f.submissions.push(clone(args));
          if (f.mode === "reject") {
            reply(packed({ accepted: false, error: "fixture_rejected", nack: { signature: "fixture-nack" } }));
            break;
          }
          if (f.mode !== "drop-before") {
            f.entries.push({ kind: "action", signer: "agent", seq: 900 + f.entries.length,
              hash: "ab".repeat(32), ...clone(envelope) });
          }
          if (f.mode === "drop-after" || f.mode === "drop-before") {
            response.destroy();
          } else if (f.mode === "hold-after") {
            response.setHeader("content-type", "application/json");
            response.write(`{"jsonrpc":"2.0","id":${JSON.stringify(message.id)},"result":`);
            response.once("close", () => events.emit("submission-closed"));
            events.emit("submission-entered");
          } else reply(packed({ accepted: true, entry: { seq: f.entries.at(-1).seq, hash: "ab".repeat(32) } }));
          break;
        }
        case "get_history": {
          const moves = f.entries.filter(entry => entry.seq >= args.from_move)
            .slice(0, args.limit).map(entry => ({ seq: entry.seq, seat: entry.payload.seat,
              action: entry.payload.action, kind: entry.kind }));
          reply(packed({ match_id: match, from_move: args.from_move, returned: moves.length, moves }));
          break;
        }
        case "get_match_log":
          reply(packed({ entries: f.entries.filter(entry => entry.seq >= args.from_seq) }));
          break;
        case "wait_for_turn": {
          response.setHeader("content-type", "text/event-stream");
          response.write(": open\r\n\r\n");
          const progress = { jsonrpc: "2.0", method: "notifications/progress",
            params: { progressToken: message.params._meta?.progressToken || "wait", progress: 1 } };
          response.write(`event: message\r\ndata: ${JSON.stringify(progress)}\r\n\r\n`);
          f.releaseWait = () => {
            const result = { jsonrpc: "2.0", id: message.id, result: packed({ status: "your_turn", text: "轮到你" }) };
            const wire = Buffer.from(`event: message\ndata: ${JSON.stringify(result)}\n\n`);
            response.write(wire.subarray(0, wire.length - 7));
            response.end(wire.subarray(wire.length - 7));
          };
          events.emit("wait-entered");
          if (!f.holdWait) f.releaseWait();
          break;
        }
        case "wire_error":
          response.end(JSON.stringify({ jsonrpc: "2.0", id: message.id,
            error: { code: -32042, message: "fixture RPC error", data: { reason: "fixture" } } }));
          break;
        case "wire_http":
          response.writeHead(429, { "content-type": "application/json" });
          response.end(JSON.stringify({ error: "rate_limited" }));
          break;
        default: reply(packed({ error: "unknown_tool" }));
      }
    } catch (error) {
      f.errors.push(error);
      response.destroy();
    }
  });
  const listening = once(server, "listening");
  server.listen(0, "127.0.0.1");
  await listening;
  f.base = `http://127.0.0.1:${server.address().port}`;
  env.ARENA_BASE_URL = f.base;
  env.ARENA_ALLOWED_ORIGINS = new URL(f.base).host;
  f.state = {
    match_id: match, status: "running", your_turn: true, seat: 0, board: { stones: [] },
    legal_actions: [{ type: "place", x: 0, y: 0 }], legal_truncated: true,
    deadline_at: 4102444800000,
    signing_request: {
      domain: { name: "AgentArena", version: "1", purpose: "move", origin: new URL(f.base).host },
      payload: { v: 1, match_id: match, game_id: "gomoku", game_version: "1", rules_hash: "12".repeat(32),
        seat: 0, agent_id: agent, kid: publicKey.kid, seq: 7, prev_hash: "00".repeat(32), deadline_at: 4102444800000 },
    },
  };
  f.gateway = () => createGateway({ env });
  f.local = async (gateway, name, args) => valueOf(await gateway.callTool(name, args));
  f.prepare = async gateway => f.local(gateway, "prepare_move", { match_id: match });
  f.make = async (gateway, decision_token, extra = {}) =>
    f.local(gateway, "make_move", { decision_token, action, ...extra });
  f.retry = async (gateway, submission_id) => f.local(gateway, "retry_move", { submission_id });
  f.noSigned = async () => {
    await assert.rejects(readFile(join(arenaHome, "signed.json")), { code: "ENOENT" });
    assert.deepEqual(await readdir(join(arenaHome, "gateway", "submissions")), []);
    assert.equal(f.submissions.length, 0);
  };
  f.stdio = (overrides = {}) => {
    const child = spawn(process.execPath, [entrypoint], {
      env: { ...env, ...overrides }, stdio: ["pipe", "pipe", "pipe"],
    });
    const closed = once(child, "close");
    const replies = new Map();
    const notifications = [];
    let id = 0, stdout = "", stderr = "";
    child.stdout.on("data", data => { stdout += data; });
    child.stderr.on("data", data => { stderr += data; });
    const lines = createInterface({ input: child.stdout });
    lines.on("line", line => {
      const message = JSON.parse(line);
      if (Object.hasOwn(message, "id")) {
        const resolve = replies.get(message.id);
        replies.delete(message.id);
        resolve?.(message);
      } else notifications.push(message);
    });
    const client = {
      child, notifications, closed,
      request(method, params = {}) {
        const next = ++id;
        const response = new Promise(resolve => replies.set(next, resolve));
        child.stdin.write(`${JSON.stringify({ jsonrpc: "2.0", id: next, method, params })}\n`);
        return response;
      },
      async tool(name, args) {
        return (await client.request("tools/call", { name, arguments: args })).result.structuredContent;
      },
      async stop(signal) {
        if (child.exitCode === null && child.signalCode === null) {
          if (signal) child.kill(signal);
          else child.stdin.end();
        }
        const result = await closed;
        assert.equal(stdout.includes(authToken), false, "Bearer tokens must never reach stdout");
        assert.equal(stderr.includes(authToken), false);
        assert.equal(stdout.includes("BEGIN PRIVATE KEY"), false);
        assert.equal(stderr.includes("BEGIN PRIVATE KEY"), false);
        f.clients.delete(client);
        return { code: result[0], signal: result[1], stdout, stderr };
      },
    };
    f.clients.add(client);
    return client;
  };
  t.after(async () => {
    f.releaseValidation?.();
    f.releaseWait?.();
    for (const response of f.gates) response.end("{}");
    for (const client of f.clients) await client.stop("SIGKILL");
    const closed = new Promise(resolve => server.close(resolve));
    server.closeAllConnections();
    await closed;
    await rm(home, { recursive: true, force: true });
    assert.deepEqual(f.errors, [], "HTTP fixture must complete without hidden assertion failures");
  });
  return f;
}

test("stdio exports, initialize, tool override, JSON/SSE/session/error forwarding", { timeout: 20000 }, async t => {
  assert.equal(typeof serve, "function");
  const f = await fixture(t);
  const client = f.stdio();
  const initialized = await client.request("initialize", { protocolVersion: "2025-03-26" });
  assert.deepEqual(initialized.result.capabilities, { tools: {} });
  client.child.stdin.write('{"jsonrpc":"2.0","method":"notifications/initialized"}\n');
  const listed = await client.request("tools/list");
  assert.equal(listed.result.tools.filter(tool => tool.name === "make_move").length, 1);
  assert.deepEqual(listed.result.tools.find(tool => tool.name === "make_move").inputSchema.required,
    ["decision_token", "action"]);
  assert.ok(listed.result.tools.some(tool => tool.name === "get_state"));
  f.holdWait = true;
  const entered = once(f.events, "wait-entered");
  const waiting = client.request("tools/call", { name: "wait_for_turn", arguments: { match_id: match },
    _meta: { progressToken: "client-progress" } });
  await entered;
  assert.deepEqual((await client.request("ping")).result, {});
  f.releaseWait();
  assert.equal((await waiting).result.structuredContent.text, "轮到你");
  assert.equal(client.notifications[0].params.progressToken, "client-progress");
  const remoteError = await client.request("tools/call", { name: "wire_error", arguments: {} });
  assert.deepEqual(remoteError.error, { code: -32042, message: "fixture RPC error", data: { reason: "fixture" } });
  const httpError = await client.request("tools/call", { name: "wire_http", arguments: {} });
  assert.equal(httpError.error.data.error, "rate_limited");
  const unknown = await client.request("resources/list");
  assert.equal(unknown.error.code, -32601);
  assert.equal((await client.stop()).code, 0);
});

test("prepare persists opaque original snapshot and valid parameterized move uses real signer", { timeout: 20000 }, async t => {
  const f = await fixture(t);
  const gateway = await f.gateway();
  const prepared = await f.prepare(gateway);
  assert.match(prepared.decision_token, /^[a-f0-9]{48}$/);
  assert.deepEqual(prepared.context, f.state);
  await f.noSigned();
  const result = await f.make(gateway, prepared.decision_token, { comment: "--literal-comment" });
  assert.equal(result.status, "accepted");
  assert.equal(f.submissions[0].envelope.payload.comment, "--literal-comment");
  assert.deepEqual(f.submissions[0].envelope.payload.action, action);
  assert.ok(f.calls.findIndex(call => call.name === "validate_move")
    < f.calls.findIndex(call => call.name === "make_move"));
  const replay = await f.make(gateway, prepared.decision_token, { comment: "--literal-comment" });
  assert.equal(replay.status, "accepted");
  assert.equal(f.submissions.length, 1);
  for (const extra of [{ action: { ...action, x: 4 } }, { comment: "changed" }, { commitment: "ab".repeat(32) }]) {
    assert.equal((await f.make(gateway, prepared.decision_token, extra)).status, "intent_locked");
  }
  for (const directory of ["gateway", "gateway/tokens", "gateway/submissions", "gateway/locks"]) {
    assert.equal((await stat(join(f.env.ARENA_HOME, directory))).mode & 0o777, 0o700);
    for (const filename of await readdir(join(f.env.ARENA_HOME, directory))) {
      const item = await stat(join(f.env.ARENA_HOME, directory, filename));
      if (item.isFile()) assert.equal(item.mode & 0o777, 0o600);
    }
  }
});

test("ordinary validation deadline aborts actual HTTP and releases the journal without signing", { timeout: 20000 }, async t => {
  const f = await fixture(t);
  const deadlines = controlledDeadlines(t);
  const gateway = await f.gateway();
  const token = (await f.prepare(gateway)).decision_token;
  f.holdValidation = true;
  const entered = once(f.events, "validation-entered");
  const moving = f.make(gateway, token);
  await entered;
  const closed = once(f.events, "validation-closed");
  assert.equal(deadlines.at(-1).milliseconds, 30_000);
  deadlines.at(-1).controller.abort(new DOMException("Request deadline", "TimeoutError"));
  assert.equal((await moving).status, "validation_unavailable");
  await closed;
  await f.noSigned();
  f.releaseValidation();
  f.holdValidation = false;
  assert.equal((await f.make(await f.gateway(), token)).status, "accepted");
});

test("submission body deadline stays unknown and restart recovers its original envelope", { timeout: 20000 }, async t => {
  const f = await fixture(t);
  const deadlines = controlledDeadlines(t);
  const gateway = await f.gateway();
  const token = (await f.prepare(gateway)).decision_token;
  f.mode = "hold-after";
  const entered = once(f.events, "submission-entered");
  const moving = f.make(gateway, token, { comment: "original" });
  await entered;
  const closed = once(f.events, "submission-closed");
  assert.equal(deadlines.at(-1).milliseconds, 30_000);
  deadlines.at(-1).controller.abort(new DOMException("Request deadline", "TimeoutError"));
  const result = await moving;
  assert.equal(result.status, "unknown");
  assert.equal(result.error, "response_unavailable");
  await closed;
  const record = JSON.parse(await readFile(join(f.env.ARENA_HOME, "gateway", "submissions",
    `${result.submission_id}.json`)));
  assert.equal(record.status, "unknown");
  assert.deepEqual(record.envelope, f.submissions[0].envelope);
  assert.equal((await f.retry(await f.gateway(), result.submission_id)).status, "accepted");
  assert.equal(f.submissions.length, 1);
});

test("remote wait receives a separate long SSE budget and can be explicitly aborted after progress", { timeout: 20000 }, async t => {
  const f = await fixture(t);
  const deadlines = controlledDeadlines(t);
  const remote = new RemoteClient({ baseUrl: f.base, token: authToken,
    onNotification: message => f.events.emit("progress-received", message) });
  await remote.initialize();
  assert.ok(deadlines.every(deadline => deadline.milliseconds === 30_000));
  f.holdWait = true;
  const progress = once(f.events, "progress-received");
  const rejected = assert.rejects(remote.tool("wait_for_turn", { match_id: match }),
    error => ["AbortError", "TimeoutError"].includes(error.name));
  assert.equal((await progress)[0].method, "notifications/progress");
  assert.equal(deadlines.at(-1).milliseconds, 660_000);
  deadlines.at(-1).controller.abort(new DOMException("Wait deadline", "TimeoutError"));
  await rejected;
});

test("explicit signing origins may differ from transport while unknown and changed origins fail closed", { timeout: 20000 }, async t => {
  const f = await fixture(t);
  const transportHost = new URL(f.base).host;
  const defaults = await createGateway({ env: { ...f.env, ARENA_ALLOWED_ORIGINS: undefined } });
  assert.equal((await f.prepare(defaults)).status, "your_turn");
  f.state.signing_request.domain.origin = "agent.idou.bid";
  assert.equal((await f.prepare(defaults)).error, "identity_mismatch");
  await f.noSigned();

  f.env.ARENA_ALLOWED_ORIGINS = " agent.idou.bid , approved.example ";
  const client = f.stdio();
  for (const origin of ["untrusted.invalid", transportHost]) {
    f.state.signing_request.domain.origin = origin;
    const rejected = await client.tool("prepare_move", { match_id: match });
    assert.equal(rejected.error, "identity_mismatch");
    assert.equal(rejected.decision_token, undefined);
  }
  await f.noSigned();
  f.state.signing_request.domain.origin = "agent.idou.bid";
  const prepared = await client.tool("prepare_move", { match_id: match });
  assert.equal(prepared.status, "your_turn");
  const token = JSON.parse(await readFile(join(f.env.ARENA_HOME, "gateway", "tokens",
    `${prepared.decision_token}.json`)));
  assert.equal(token.signing_request.domain.origin, "agent.idou.bid");
  f.state.signing_request.domain.origin = "approved.example";
  assert.equal((await client.tool("make_move", { decision_token: prepared.decision_token, action })).status,
    "stale_decision");
  await f.noSigned();
  assert.equal(f.calls.some(call => call.name === "validate_move"), false);

  f.state.signing_request.domain.origin = "agent.idou.bid";
  const accepted = await client.tool("make_move", { decision_token: prepared.decision_token, action });
  assert.equal(accepted.status, "accepted");
  assert.equal(f.submissions[0].envelope.domain.origin, "agent.idou.bid");
  const record = JSON.parse(await readFile(join(f.env.ARENA_HOME, "gateway", "submissions",
    `${accepted.submission_id}.json`)));
  assert.deepEqual(record.signing_request.domain, token.signing_request.domain);
  assert.deepEqual(record.envelope.domain, token.signing_request.domain);
  assert.equal((await client.stop()).code, 0);
});

test("stale complete requests and illegal parameters never reserve or sign", { timeout: 20000 }, async t => {
  const f = await fixture(t);
  const gateway = await f.gateway();
  const token = (await f.prepare(gateway)).decision_token;
  assert.equal((await f.make(gateway, token, { action: { ...action, x: 99 } })).status, "invalid_action");
  assert.equal((await f.make(gateway, token, { comment: "x".repeat(141) })).status, "invalid_action");
  await f.noSigned();
  for (const [field, value] of [["prev_hash", "11".repeat(32)], ["rules_hash", "22".repeat(32)],
    ["deadline_at", 4102444800001], ["seq", 8]]) {
    const original = f.state.signing_request.payload[field];
    f.state.signing_request.payload[field] = value;
    assert.equal((await f.make(gateway, token)).status, "stale_decision");
    f.state.signing_request.payload[field] = original;
  }
  f.state.your_turn = false;
  assert.equal((await f.make(gateway, token)).status, "stale_decision");
  assert.equal((await f.prepare(gateway)).decision_token, undefined);
  await f.noSigned();
});

test("absent, RPC-failing and unrecognized validation fail closed", { timeout: 20000 }, async t => {
  const f = await fixture(t);
  const gateway = await f.gateway();
  const token = (await f.prepare(gateway)).decision_token;
  for (const mode of ["absent", "rpc-error", "unrecognized"]) {
    f.validation = mode;
    assert.equal((await f.make(gateway, token)).status, "validation_unavailable");
    await f.noSigned();
  }
  for (const mode of ["validation_unavailable", "unknown_key", "key_revoked"]) {
    f.validation = mode;
    assert.equal((await f.make(gateway, token)).status, "validation_unavailable");
    await f.noSigned();
  }
  for (const mode of ["context_mismatch", "deadline_passed", "seq_mismatch", "game_over"]) {
    f.validation = mode;
    assert.equal((await f.make(gateway, token)).status, "stale_decision");
    await f.noSigned();
  }
});

test("same-process concurrent intents serialize and identical calls replay", { timeout: 20000 }, async t => {
  const f = await fixture(t);
  const first = await f.gateway(), second = await f.gateway();
  const token = (await f.prepare(first)).decision_token;
  f.holdValidation = true;
  const entered = once(f.events, "validation-entered");
  const initial = f.make(first, token);
  await entered;
  const competing = f.make(second, token, { action: { ...action, x: 1 } });
  const identical = f.make(second, token);
  f.releaseValidation();
  const results = await Promise.all([
    initial, competing, identical,
  ]);
  assert.deepEqual(results.map(result => result.status), ["accepted", "intent_locked", "accepted"]);
  assert.equal(f.submissions.length, 1);
});

test("identity, token ownership and separate home isolation", { timeout: 20000 }, async t => {
  const f = await fixture(t);
  const gateway = await f.gateway();
  const token = (await f.prepare(gateway)).decision_token;
  assert.equal((await f.make(gateway, "../identity")).error, "invalid_decision_token");
  const journalPath = join(f.env.ARENA_HOME, "gateway");
  const before = await readdir(join(journalPath, "locks"));
  const binding = await readFile(join(journalPath, "identity.json"), "utf8");
  f.who = "00000000-0000-4000-8000-000000000002";
  f.state.signing_request.payload.agent_id = f.who;
  const foreign = f.stdio();
  assert.equal((await foreign.tool("prepare_move", { match_id: match })).error, "identity_mismatch");
  assert.equal((await foreign.stop()).code, 0);
  assert.deepEqual(await readdir(join(journalPath, "locks")), before);
  assert.equal(await readFile(join(journalPath, "identity.json"), "utf8"), binding);
  assert.deepEqual(await readdir(join(journalPath, "tokens")), [`${token}.json`]);
  assert.deepEqual(await readdir(join(journalPath, "submissions")), []);
  const otherHome = join(f.home, "agent-b");
  await mkdir(otherHome, { mode: 0o700 });
  const otherEnv = { ...f.env, ARENA_HOME: otherHome };
  const otherKey = await runSigner(otherEnv, ["keygen"]);
  f.keys = [otherKey];
  f.state.signing_request.payload.kid = otherKey.kid;
  const other = await createGateway({ env: otherEnv });
  assert.equal((await f.make(other, token)).error, "invalid_decision_token");
  assert.equal((await f.prepare(other)).status, "your_turn");
  f.keys = [f.publicKey];
  assert.equal((await f.prepare(other)).error, "identity_mismatch");
  await f.noSigned();
  await assert.rejects(readFile(join(otherHome, "signed.json")), { code: "ENOENT" });
});

test("response loss and restart reconcile canonical signed payload from full state history", { timeout: 20000 }, async t => {
  const f = await fixture(t);
  const client = f.stdio();
  const prepared = await client.tool("prepare_move", { match_id: match });
  f.mode = "drop-after";
  const lost = await client.tool("make_move", { decision_token: prepared.decision_token, action });
  assert.equal(lost.status, "unknown");
  assert.equal(f.calls.some(call => call.args.history === "full"), false);
  assert.equal((await client.stop()).code, 0);
  const accepted = f.entries[0];
  accepted.domain = Object.fromEntries(Object.entries(accepted.domain).reverse());
  accepted.payload = Object.fromEntries(Object.entries(accepted.payload).reverse());
  accepted.payload.action = Object.fromEntries(Object.entries(accepted.payload.action).reverse());
  f.entries = Array.from({ length: 200 }, (_, i) => ({
    kind: "auto_action", signer: "server", seq: i + 1, payload: { seat: 1, action: { type: "pass" } },
  })).concat(accepted);
  f.state.your_turn = false;
  f.state.signing_request = null;
  const restarted = f.stdio();
  const recovered = await restarted.tool("retry_move", { submission_id: lost.submission_id });
  assert.equal(recovered.status, "accepted");
  assert.equal(recovered.receipt.recovered, true);
  assert.deepEqual(f.calls.filter(call => call.args.history === "full"),
    [{ name: "get_state", args: { match_id: match, history: "full" } }]);
  assert.equal(f.calls.some(call => ["get_history", "get_match_log"].includes(call.name)), false);
  assert.equal(f.submissions.length, 1);
  assert.equal((await restarted.stop()).code, 0);
});

test("retry resends exact original hidden data only while current", { timeout: 20000 }, async t => {
  const f = await fixture(t);
  let gateway = await f.gateway();
  const token = (await f.prepare(gateway)).decision_token;
  const hidden = { data: { layout: [3, 1, 4] }, salt: "ab".repeat(32) };
  const commitment = sha(Buffer.concat([Buffer.from("arena-commit-v1"),
    Buffer.from(canonical(hidden.data)), Buffer.from(hidden.salt, "hex")])).toString("hex");
  assert.equal((await f.make(gateway, token, { private: hidden, commitment: "00".repeat(32) })).status, "invalid_action");
  await f.noSigned();
  f.mode = "drop-before";
  const lost = await f.make(gateway, token, { private: hidden, commitment, comment: "fixed" });
  assert.equal(lost.status, "unknown");
  gateway = await f.gateway();
  f.historyError = true;
  assert.equal((await f.retry(gateway, lost.submission_id)).status, "unknown");
  f.historyError = false;
  f.historyMissing = true;
  assert.equal((await f.retry(gateway, lost.submission_id)).status, "unknown");
  f.historyMissing = false;
  assert.equal(f.submissions.length, 1);
  const changed = await f.make(gateway, token, { private: { ...hidden, salt: "cd".repeat(32) }, commitment, comment: "fixed" });
  assert.equal(changed.status, "intent_locked");
  f.mode = "accept";
  assert.equal((await f.retry(gateway, lost.submission_id)).status, "accepted");
  assert.deepEqual(f.submissions[0], f.submissions[1]);
  assert.equal(f.calls.filter(call => call.name === "validate_move").length, 2);
});

test("same board, action or decision sequence does not establish acceptance", { timeout: 20000 }, async t => {
  const f = await fixture(t);
  const gateway = await f.gateway();
  const token = (await f.prepare(gateway)).decision_token;
  f.mode = "drop-before";
  const lost = await f.make(gateway, token);
  const envelope = clone(f.submissions[0].envelope);
  envelope.payload.issued_at++;
  f.entries = [{ kind: "action", signer: "agent", seq: 900, ...envelope }];
  f.state.signing_request.payload.prev_hash = "aa".repeat(32);
  assert.equal((await f.retry(gateway, lost.submission_id)).status, "stale_decision");
  assert.equal(f.submissions.length, 1);
  const newToken = (await f.prepare(gateway)).decision_token;
  assert.equal((await f.make(gateway, newToken)).status, "stale_decision");
  assert.equal((await f.make(gateway, newToken, { action: { ...action, y: 4 } })).status, "intent_locked");
});

test("explicit rejection stays locked and unavailable history never triggers resend", { timeout: 20000 }, async t => {
  const f = await fixture(t);
  const gateway = await f.gateway();
  const token = (await f.prepare(gateway)).decision_token;
  f.mode = "reject";
  const rejected = await f.make(gateway, token);
  assert.equal(rejected.status, "rejected");
  f.mode = "accept";
  assert.equal((await f.retry(gateway, rejected.submission_id)).status, "rejected");
  assert.equal((await f.make(gateway, token, { action: { ...action, x: 1 } })).status, "intent_locked");
  f.historyError = true;
  assert.equal((await f.retry(gateway, rejected.submission_id)).status, "unknown");
  assert.equal(f.submissions.length, 1);
});

test("concurrent process intents cannot steal a live gateway lock", { timeout: 20000 }, async t => {
  const f = await fixture(t);
  const first = f.stdio(), second = f.stdio();
  const token = (await first.tool("prepare_move", { match_id: match })).decision_token;
  f.holdValidation = true;
  const entered = once(f.events, "validation-entered");
  const moving = first.tool("make_move", { decision_token: token, action });
  await entered;
  const competing = await second.tool("make_move", { decision_token: token, action: { ...action, x: 1 } });
  assert.deepEqual(competing, { status: "intent_locked", error: "gateway_busy" });
  f.releaseValidation();
  assert.equal((await moving).status, "accepted");
  assert.equal((await second.tool("make_move", { decision_token: token, action: { ...action, x: 1 } })).status, "intent_locked");
  assert.equal(f.submissions.length, 1);
  assert.equal((await first.stop()).code, 0);
  assert.equal((await second.stop()).code, 0);
});

test("simultaneous dead-owner contenders publish one successor without stealing the live winner", { timeout: 20000 }, async t => {
  const home = await mkdtemp(join(tmpdir(), "arena-gateway-lock-test-"));
  const workers = [];
  const moduleUrl = new URL("../packages/cli/lib/gateway-store.mjs", import.meta.url).href;
  const workerSource = `
    import { once } from "node:events";
    import { Journal } from ${JSON.stringify(moduleUrl)};
    const journal = new Journal(process.env.ARENA_HOME);
    await journal.initialize();
    const begin = once(process, "message");
    process.send({ event: "ready" });
    await begin;
    try {
      await journal.exclusive(async () => {
        const release = once(process, "message");
        process.send({ event: "acquired" });
        await release;
      });
      process.send({ event: "released" });
    } catch (error) {
      process.send({ event: "blocked", code: error.code });
    }
    process.disconnect();
  `;
  function startWorker() {
    const child = spawn(process.execPath, ["--input-type=module", "-e", workerSource], {
      env: { ...process.env, ARENA_HOME: home },
      stdio: ["ignore", "pipe", "pipe", "ipc"],
    });
    let output = "";
    child.stdout.on("data", data => { output += data; });
    child.stderr.on("data", data => { output += data; });
    const worker = { child, ready: once(child, "message"), closed: once(child, "close"),
      output: () => output };
    workers.push(worker);
    return worker;
  }
  t.after(async () => {
    for (const worker of workers) {
      if (worker.child.exitCode === null && worker.child.signalCode === null) worker.child.kill("SIGKILL");
      await worker.closed;
    }
    await rm(home, { recursive: true, force: true });
  });
  const dead = startWorker();
  assert.deepEqual((await dead.ready)[0], { event: "ready" });
  const acquired = once(dead.child, "message");
  dead.child.send("acquire");
  assert.deepEqual((await acquired)[0], { event: "acquired" });
  const headPath = join(home, "gateway", "locks", "head.json");
  const originalHead = await readFile(headPath, "utf8");
  dead.child.kill("SIGKILL");
  assert.equal((await dead.closed)[1], "SIGKILL");

  const contenders = [startWorker(), startWorker()];
  await Promise.all(contenders.map(worker => worker.ready));
  // Subscribe both outcomes before releasing either IPC start barrier.
  const claimed = contenders.map(worker => once(worker.child, "message"));
  for (const worker of contenders) worker.child.send("acquire");
  const results = (await Promise.all(claimed)).map(result => result[0]);
  assert.deepEqual(results.map(result => result.event).sort(), ["acquired", "blocked"]);
  const winner = contenders[results.findIndex(result => result.event === "acquired")];
  const loser = contenders[results.findIndex(result => result.event === "blocked")];
  assert.equal(results.find(result => result.event === "blocked").code, "gateway_busy");
  assert.equal((await loser.closed)[0], 0);
  assert.equal(await readFile(headPath, "utf8"), originalHead);

  const journal = new Journal(home);
  await journal.initialize();
  await assert.rejects(journal.exclusive(() => assert.fail("A live owner must retain its transaction")),
    { code: "gateway_busy" });
  const released = once(winner.child, "message");
  winner.child.send("release");
  assert.deepEqual((await released)[0], { event: "released" });
  assert.equal((await winner.closed)[0], 0);
  assert.equal(await journal.exclusive(async () => "next transaction"), "next transaction");
  for (const worker of workers) assert.equal(worker.output(), "");
});

for (const stage of ["before-sign", "after-sign"]) {
  test(`dead owner recovery preserves ${stage} reservation and signer replay`, { timeout: 20000 }, async t => {
    const f = await fixture(t);
    const wrapper = join(f.home, "signer-barrier.mjs");
    // This fixture only gates the real signer; it never implements signing.
    await writeFile(wrapper, `
      import { spawn } from "node:child_process";
      import { once } from "node:events";
      const args = process.argv.slice(2);
      const signing = args[0] === "sign";
      async function barrier() {
        await fetch(${JSON.stringify(f.base + "/signer-barrier")}, {
          method: "POST", body: JSON.stringify({ pid: process.pid })
        });
      }
      if (signing && ${JSON.stringify(stage)} === "before-sign") await barrier();
      const child = spawn(process.execPath, [${JSON.stringify(signer)}, ...args],
        { stdio: ["inherit", "pipe", "pipe"] });
      const closed = once(child, "close");
      let out = "";
      child.stdout.on("data", data => out += data);
      child.stderr.pipe(process.stderr);
      const [code] = await closed;
      if (code !== 0) process.exit(code);
      if (signing && ${JSON.stringify(stage)} === "after-sign") await barrier();
      process.stdout.write(out);
    `, { mode: 0o600 });
    const client = f.stdio({ ARENA_SIGN_SCRIPT: wrapper });
    const token = (await client.tool("prepare_move", { match_id: match })).decision_token;
    const reached = once(f.events, "signer-barrier");
    void client.tool("make_move", { decision_token: token, action });
    const [{ pid }] = await reached;
    const files = await readdir(join(f.env.ARENA_HOME, "gateway", "submissions"));
    assert.equal(files.length, 1);
    const reservation = JSON.parse(await readFile(join(f.env.ARENA_HOME, "gateway", "submissions", files[0])));
    assert.equal(reservation.status, "reserved");
    assert.equal(reservation.envelope, undefined);
    const stopped = await client.stop("SIGKILL");
    assert.equal(stopped.signal, "SIGKILL");
    process.kill(pid, "SIGKILL");
    const gateway = await f.gateway();
    assert.equal((await f.make(gateway, token, { action: { ...action, x: 8 } })).status, "intent_locked");
    let original;
    if (stage === "after-sign") {
      const replay = JSON.parse(await readFile(join(f.env.ARENA_HOME, "signed.json")));
      original = Object.values(replay)[0].envelope;
    }
    assert.equal((await f.retry(gateway, reservation.submission_id)).status, "accepted");
    assert.equal(f.submissions.length, 1);
    if (original) assert.deepEqual(f.submissions[0].envelope, original);
  });
}
