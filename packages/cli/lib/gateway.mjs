#!/usr/bin/env node
import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { homedir } from "node:os";
import { isAbsolute, join, resolve } from "node:path";
import { createInterface } from "node:readline";
import { fileURLToPath } from "node:url";
import { GatewayError, Journal, canonical, equal, readJSON } from "./gateway-store.mjs";
import { RemoteClient, RemoteError } from "./gateway-remote.mjs";

const object = value => value !== null && typeof value === "object" && !Array.isArray(value);
const outcome = (status, error, record) => ({
  ...(record ? { submission_id: record.submission_id } : {}),
  status, ...(error ? { error } : {}),
});
const localNames = new Set(["prepare_move", "make_move", "retry_move"]);
const schema = (properties, required) => ({ type: "object", properties, required, additionalProperties: false });
export const LOCAL_TOOLS = [
  {
    name: "prepare_move",
    description: "Snapshot the current decision without signing. Choose an action from this context.",
    inputSchema: schema({ match_id: { type: "string" } }, ["match_id"]),
  },
  {
    name: "make_move",
    description: "Validate and sign the prepared decision locally; never applies an action to a newer decision.",
    inputSchema: schema({
      decision_token: { type: "string" }, action: { type: "object" },
      comment: { type: "string" }, commitment: { type: "string" },
      private: schema({ data: {}, salt: { type: "string" } }, ["data", "salt"]),
    }, ["decision_token", "action"]),
  },
  {
    name: "retry_move",
    description: "Resolve or resend only the original durable submission. Never creates a different intent.",
    inputSchema: schema({ submission_id: { type: "string" } }, ["submission_id"]),
  },
];

function runSigner(script, home, env, args, input) {
  return new Promise((resolveResult, reject) => {
    const signerEnv = { ...env, ARENA_HOME: home };
    delete signerEnv.ARENA_TOKEN;
    const child = spawn(process.execPath, [script, ...args], {
      env: signerEnv, stdio: ["pipe", "pipe", "pipe"],
    });
    let stdout = "";
    child.stdout.setEncoding("utf8");
    child.stdout.on("data", data => { stdout += data; });
    // Signer errors can include intent details. Never forward or log them.
    child.stderr.resume();
    child.on("error", () => reject(new GatewayError("signer_failed")));
    child.stdin.on("error", () => {});
    child.on("close", code => {
      if (code !== 0) return reject(new GatewayError("signer_failed"));
      try { resolveResult(JSON.parse(stdout)); }
      catch { reject(new GatewayError("signer_invalid_response")); }
    });
    child.stdin.end(input === undefined ? "" : JSON.stringify(input));
  });
}

export async function createGateway({ env = process.env, onNotification } = {}) {
  if (!env.ARENA_TOKEN) throw new GatewayError("missing_arena_token");
  const base = new URL(env.ARENA_BASE_URL || "https://agent.idou.bid");
  if (!["http:", "https:"].includes(base.protocol) || base.username || base.password
      || base.search || base.hash) throw new GatewayError("invalid_base_url");
  const baseUrl = base.href.replace(/\/+$/, "");
  const allowedOrigins = (env.ARENA_ALLOWED_ORIGINS ?? base.host)
    .split(",").map(origin => origin.trim()).filter(Boolean);
  const home = resolve(env.ARENA_HOME || join(homedir(), ".arena"));
  const script = env.ARENA_SIGN_SCRIPT || join(homedir(), ".arena", "skill", "arena", "scripts", "sign.mjs");
  if (!isAbsolute(script)) throw new GatewayError("sign_script_not_absolute");
  const journal = new Journal(home);
  await journal.initialize();
  const remote = new RemoteClient({ baseUrl, token: env.ARENA_TOKEN, onNotification });
  const sign = (args, input) => runSigner(script, journal.home, env, args, input);

  async function identity() {
    const who = await remote.tool("whoami", {});
    const local = await sign(["pubkey"]);
    if (!who?.agent_id || !local?.kid || !local?.public_key || !who.keys?.some(key =>
      key.status === "active" && key.kid === local.kid && key.public_key === local.public_key)) {
      throw new GatewayError("identity_mismatch");
    }
    const bound = { agent_id: who.agent_id, kid: local.kid, public_key: local.public_key, base_url: baseUrl };
    const existing = await readJSON(join(journal.directory, "identity.json"));
    if (existing && !equal(existing, bound)) throw new GatewayError("identity_mismatch");
    return bound;
  }

  function requestBound(request, bound, match) {
    const payload = request?.payload;
    return request?.domain?.name === "AgentArena" && request.domain.version === "1"
      && request.domain.purpose === "move" && allowedOrigins.includes(request.domain.origin)
      && object(payload) && payload.agent_id === bound.agent_id && payload.kid === bound.kid
      && payload.match_id === match && Number.isSafeInteger(payload.seq) && payload.seq >= 0
      && Number.isSafeInteger(payload.seat) && payload.seat >= 0
      && typeof payload.prev_hash === "string" && typeof payload.rules_hash === "string"
      && typeof payload.deadline_at === "number";
  }

  async function current(record, bound) {
    const state = await remote.tool("get_state", { match_id: record.match_id, include_legal_moves: true });
    return state?.your_turn === true && requestBound(state.signing_request, bound, record.match_id)
      && equal(state.signing_request, record.signing_request)
      && Date.now() <= record.signing_request.payload.deadline_at
      && (record.signing_request.payload.expires_at === undefined
        || Date.now() <= record.signing_request.payload.expires_at);
  }

  function intendedPayload(record) {
    return { ...record.signing_request.payload, action: record.intent.action,
      ...(record.intent.comment === undefined ? {} : { comment: record.intent.comment }),
      ...(record.intent.commitment === undefined ? {} : { commitment: record.intent.commitment }) };
  }

  function originalEnvelope(record, envelope) {
    if (!object(envelope?.payload) || typeof envelope.payload.issued_at !== "number"
        || typeof envelope.signature !== "string" || envelope.alg !== "ed25519"
        || envelope.kid !== record.identity.kid || !equal(envelope.domain, record.signing_request.domain)) return false;
    const { issued_at, ...payload } = envelope.payload;
    return equal(payload, intendedPayload(record));
  }

  async function recoverSignature(record) {
    if (record.envelope) return;
    // Read only the signer's public-envelope replay cache, never its private key.
    const replay = await readJSON(join(journal.home, "signed.json"));
    const p = record.signing_request.payload;
    const envelope = replay?.[`${p.match_id}:${p.seat}:${p.seq}`]?.envelope;
    if (!envelope) return;
    if (!originalEnvelope(record, envelope)) throw new GatewayError("intent_locked");
    record.envelope = envelope;
    record.status = "unknown";
    await journal.save(record);
  }

  async function validate(record, bound) {
    let stillCurrent;
    try { stillCurrent = await current(record, bound); }
    catch { return outcome("validation_unavailable", "state_unavailable", record); }
    if (!stillCurrent) return outcome("stale_decision", "decision_changed", record);
    let result;
    try {
      result = await remote.tool("validate_move", {
        match_id: record.match_id, signing_request: record.signing_request, ...record.intent,
      });
    } catch {
      return outcome("validation_unavailable", "validation_unavailable", record);
    }
    if (result?.valid === true && !result.error) return;
    if (result?.valid === false) {
      if (["stale_decision", "request_changed", "expired", "deadline", "context_mismatch",
        "seq_mismatch", "deadline_passed", "game_over", "not_seated"].includes(result.error)) {
        return outcome("stale_decision", "decision_changed", record);
      }
      if (["validation_unavailable", "unknown_key", "key_revoked"].includes(result.error)) {
        return outcome("validation_unavailable", "validation_unavailable", record);
      }
      return outcome("invalid_action", "invalid_action", record);
    }
    return outcome("validation_unavailable", "validation_unavailable", record);
  }

  async function signReserved(record) {
    let envelope;
    // Fill only the chosen signing fields; the persisted remote template stays
    // untouched. Stdin also handles comments beginning "--" without argv parsing.
    try {
      envelope = await sign(["sign", "-"], {
        ...record.signing_request, payload: intendedPayload(record),
      });
    }
    catch { return outcome("unknown", "signer_failed", record); }
    if (!originalEnvelope(record, envelope)) return outcome("intent_locked", "signer_replay_mismatch", record);
    record.envelope = envelope;
    record.status = "unknown";
    await journal.save(record); // Durable exact envelope/private precedes the HTTP request.
    return send(record);
  }

  async function send(record) {
    let receipt;
    try {
      receipt = await remote.tool("make_move", {
        match_id: record.match_id, envelope: record.envelope,
        ...(record.intent.private === undefined ? {} : { private: record.intent.private }),
      });
    } catch {
      // The server may have committed before either HTTP or JSON-RPC failed.
      return outcome("unknown", "response_unavailable", record);
    }
    if (receipt?.accepted === true) {
      record.status = "accepted";
      record.receipt = receipt;
    } else if (receipt?.accepted === false || receipt?.error) {
      record.status = "rejected";
      record.receipt = receipt;
    } else {
      return outcome("unknown", "response_unrecognized", record);
    }
    await journal.save(record);
    return { ...outcome(record.status, undefined, record), receipt };
  }

  function matches(entry, envelope) {
    return entry?.kind === "action" && entry.signer === "agent"
      && entry.kid === envelope.kid && entry.signature === envelope.signature
      && equal(entry.domain, envelope.domain) && equal(entry.payload, envelope.payload)
      && (entry.alg === undefined || entry.alg === envelope.alg);
  }

  async function acceptance(record) {
    // Full entries are needed only to reconcile a previously signed submission.
    // Compact get_history action summaries cannot establish payload equality.
    const state = await remote.tool("get_state", { match_id: record.match_id, history: "full" });
    if (state?.error || !Array.isArray(state?.entries)) {
      throw new GatewayError("history_unavailable");
    }
    const found = state.entries.find(entry => matches(entry, record.envelope));
    if (found) return {
      accepted: true, recovered: true,
      entry: { seq: found.seq, hash: found.hash, prev_hash: found.prev_hash },
      ...(found.server_ack ? { ack: found.server_ack } : {}),
    };
  }

  async function retry(record, bound) {
    if (!equal(record.identity, bound)) throw new GatewayError("identity_mismatch");
    if (record.status === "accepted") return { ...outcome("accepted", undefined, record), receipt: record.receipt };
    await recoverSignature(record);
    if (record.envelope) {
      let receipt;
      try { receipt = await acceptance(record); }
      catch { return outcome("unknown", "history_unavailable", record); }
      if (receipt) {
        record.status = "accepted";
        record.receipt = receipt;
        await journal.save(record);
        return { ...outcome("accepted", undefined, record), receipt };
      }
      if (record.status === "rejected") return { ...outcome("rejected", undefined, record), receipt: record.receipt };
      let stillCurrent;
      try { stillCurrent = await current(record, bound); }
      catch { return outcome("unknown", "state_unavailable", record); }
      if (!stillCurrent) return outcome("stale_decision", "decision_changed", record);
      return send(record);
    }
    const failure = await validate(record, bound);
    return failure || signReserved(record);
  }

  async function prepare(args, bound) {
    if (typeof args.match_id !== "string" || !args.match_id) throw new GatewayError("invalid_arguments");
    const context = await remote.tool("get_state", { match_id: args.match_id, include_legal_moves: true });
    if (context?.error) return { status: "rejected", error: context.error };
    const result = { status: context.your_turn ? "your_turn" : context.status, context,
      legal_actions: context.legal_actions, legal_truncated: context.legal_truncated,
      deadline_at: context.deadline_at };
    if (context.your_turn !== true) return result;
    if (!requestBound(context.signing_request, bound, args.match_id)) throw new GatewayError("identity_mismatch");
    if (Date.now() > context.signing_request.payload.deadline_at) return { ...result, status: "stale_decision" };
    result.decision_token = await journal.token({
      identity: bound, match_id: args.match_id, signing_request: context.signing_request, context,
    });
    return result;
  }

  async function make(args, bound) {
    const token = await journal.getToken(args.decision_token);
    if (!token) return outcome("stale_decision", "invalid_decision_token");
    if (!equal(token.identity, bound)) throw new GatewayError("identity_mismatch");
    if (!object(args.action) || typeof args.action.type !== "string"
        || (args.comment !== undefined && typeof args.comment !== "string")
        || (args.commitment !== undefined && typeof args.commitment !== "string")
        || (args.private !== undefined && !object(args.private))) return outcome("invalid_action", "invalid_arguments");
    const intent = { action: args.action,
      ...(args.comment === undefined ? {} : { comment: args.comment }),
      ...(args.commitment === undefined ? {} : { commitment: args.commitment }),
      ...(args.private === undefined ? {} : { private: args.private }) };
    const p = token.signing_request.payload;
    const submission_id = `sub_${createHash("sha256").update(canonical([p.match_id, p.seat, p.seq])).digest("hex")}`;
    const existing = await journal.submission(submission_id);
    if (existing) {
      if (!equal(existing.intent, intent)) return outcome("intent_locked", "intent_locked", existing);
      if (!equal(existing.signing_request, token.signing_request)) return outcome("stale_decision", "decision_changed", existing);
      return retry(existing, bound);
    }
    const record = { submission_id, identity: bound, match_id: token.match_id,
      signing_request: token.signing_request, intent, status: "reserved" };
    const failure = await validate(record, bound);
    if (failure) {
      delete failure.submission_id; // No durable submission exists before successful validation.
      return failure;
    }
    if (!await journal.save(record, true)) throw new GatewayError("intent_locked");
    return signReserved(record);
  }

  return {
    async listTools(params = {}) {
      const result = await remote.call("tools/list", params);
      return { ...result, tools: [...result.tools.filter(item => !localNames.has(item.name)), ...LOCAL_TOOLS] };
    },
    async callTool(name, args = {}, meta) {
      if (!localNames.has(name)) {
        let forwarded = args;
        if (["create_room", "join_room", "join_queue"].includes(name)) {
          const bound = await identity();
          if (args.kid !== undefined && args.kid !== bound.kid) {
            const value = outcome("rejected", "key_mismatch");
            return {
              content: [{ type: "text", text: JSON.stringify(value) }],
              structuredContent: value,
              isError: true,
            };
          }
          forwarded = { ...args, kid: bound.kid };
        }
        return remote.call("tools/call", { name, arguments: forwarded, ...(meta ? { _meta: meta } : {}) });
      }
      let value;
      try {
        // A foreign identity must not even append a lock record in a bound home.
        const bound = await identity();
        value = await journal.exclusive(async () => {
          // Recheck/bind inside the transaction for concurrent first use.
          await journal.bind(bound);
          if (name === "prepare_move") return prepare(args, bound);
          if (name === "make_move") return make(args, bound);
          const record = await journal.submission(args.submission_id);
          return record ? retry(record, bound) : outcome("unknown", "submission_not_found");
        });
      } catch (error) {
        const code = error instanceof GatewayError ? error.code : "gateway_unavailable";
        value = outcome(["intent_locked", "gateway_busy"].includes(code) ? "intent_locked" : "rejected", code);
      }
      return { content: [{ type: "text", text: JSON.stringify(value) }], structuredContent: value,
        isError: !["accepted", "your_turn", "waiting", "running", "finished", "not_your_turn", "game_over", "match_over"].includes(value.status) };
    },
  };
}

// Integration entrypoint: agent-arena serve calls serve(argv).
export async function serve(argv = []) {
  if (argv.length) throw new GatewayError("unexpected_serve_arguments");
  const send = message => process.stdout.write(`${JSON.stringify(message)}\n`);
  const gateway = await createGateway({ onNotification: send });
  const lines = createInterface({ input: process.stdin, crlfDelay: Infinity });
  const pending = new Set();
  async function dispatch(line) {
    let message;
    try { message = JSON.parse(line); }
    catch { send({ jsonrpc: "2.0", id: null, error: { code: -32700, message: "Parse error" } }); return; }
    if (!object(message) || message.jsonrpc !== "2.0" || typeof message.method !== "string") {
      send({ jsonrpc: "2.0", id: message?.id ?? null, error: { code: -32600, message: "Invalid request" } });
      return;
    }
    if (!Object.hasOwn(message, "id")) return;
    try {
      let result;
      switch (message.method) {
        case "initialize":
          result = { protocolVersion: message.params?.protocolVersion || "2025-03-26",
            capabilities: { tools: {} }, serverInfo: { name: "agent-arena-local", version: "1.0.0" } };
          break;
        case "ping": result = {}; break;
        case "tools/list": result = await gateway.listTools(message.params); break;
        case "tools/call":
          if (typeof message.params?.name !== "string" || (message.params.arguments !== undefined
              && !object(message.params.arguments))) throw new RemoteError(-32602, "Invalid tool arguments");
          result = await gateway.callTool(message.params.name, message.params.arguments, message.params._meta);
          break;
        default: throw new RemoteError(-32601, "Method not found");
      }
      send({ jsonrpc: "2.0", id: message.id, result });
    } catch (error) {
      send({ jsonrpc: "2.0", id: message.id, error: error instanceof RemoteError
        ? { code: error.code, message: error.message, ...(error.data === undefined ? {} : { data: error.data }) }
        : { code: -32000, message: "Gateway unavailable" } });
    }
  }
  // A long remote wait must not block independent stdio requests or notifications.
  for await (const line of lines) {
    if (!line.trim()) continue;
    const request = dispatch(line);
    pending.add(request);
    request.finally(() => pending.delete(request));
  }
  await Promise.all(pending);
  return 0;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try { process.exitCode = await serve(process.argv.slice(2)); }
  catch (error) {
    process.stderr.write(`${error instanceof GatewayError ? error.code : "gateway_unavailable"}\n`);
    process.exitCode = 1;
  }
}
