import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { randomUUID } from "node:crypto";
import { once } from "node:events";
import { mkdtemp, rm } from "node:fs/promises";
import { createServer } from "node:http";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const root = fileURLToPath(new URL("../", import.meta.url));
const signer = path.join(root, "skills/arena/scripts/sign.mjs");
const token = "fixture_token_never_print";

async function run(args, env) {
  const child = spawn(process.execPath, args, { env, cwd: root });
  let stdout = "", stderr = "";
  child.stdout.on("data", chunk => stdout += chunk);
  child.stderr.on("data", chunk => stderr += chunk);
  const [code] = await once(child, "close");
  return { code, stdout, stderr };
}

for (const mode of ["explicit-home", "default-home", "unconfirmed-key"]) {
  test(`connect signer directory and ownership: ${mode}`, { timeout: 20000 }, async () => {
    const home = await mkdtemp(path.join(os.tmpdir(), "arena-connect-key-"));
    const defaultHome = path.join(home, ".arena");
    const customHome = path.join(home, "agent-two");
    const env = { ...process.env, HOME: home, USERPROFILE: home,
      ARENA_SKILL_SRC: path.join(root, "skills/arena") };
    delete env.ARENA_HOME;
    let original;
    if (mode === "explicit-home") {
      const generated = await run([signer, "keygen"], { ...env, ARENA_HOME: defaultHome });
      assert.equal(generated.code, 0);
      original = JSON.parse(generated.stdout);
      env.ARENA_HOME = customHome;
    }
    let registered;
    const server = createServer(async (req, res) => {
      assert.equal(req.headers.authorization, `Bearer ${token}`);
      let text = "";
      for await (const chunk of req) text += chunk;
      const message = JSON.parse(text);
      let result;
      if (message.method === "initialize") {
        result = { protocolVersion: "2025-03-26", capabilities: {} };
      } else {
        let value;
        let isError = false;
        switch (message.params.name) {
          case "get_key_challenge":
            value = { signing_request: {
              domain: { name: "AgentArena", version: "1", purpose: "register_key", origin: "agent.idou.bid" },
              payload: { challenge_id: randomUUID(), agent_id: "00000000-0000-4000-8000-000000000002",
                nonce: "fixture", expires_at: Date.now() + 60000 },
            } };
            break;
          case "register_agent_key": {
            const envelope = message.params.arguments.envelope;
            if (envelope.kid === original?.kid) {
              isError = true;
              value = { error: "key_bound_to_other_agent", message: "fixture key belongs to agent one" };
            } else {
              assert.equal(envelope.kid, envelope.payload.kid);
              assert.ok(envelope.signature);
              registered = { kid: envelope.kid, public_key: envelope.payload.public_key, status: "active" };
              value = { ok: true, kid: registered.kid };
            }
            break;
          }
          case "whoami":
            value = { agent_id: "00000000-0000-4000-8000-000000000002", name: "fixture", owner_handle: "owner",
              keys: mode === "unconfirmed-key" || !registered ? [] : [registered] };
            break;
          default:
            assert.fail(`unexpected tool: ${message.params.name}`);
        }
        result = { isError, content: [{ type: "text", text: JSON.stringify(value) }] };
        if (!isError) result.structuredContent = value;
      }
      res.setHeader("content-type", "application/json");
      res.end(JSON.stringify({ jsonrpc: "2.0", id: message.id, result }));
    });
    try {
      const listening = once(server, "listening");
      server.listen(0, "127.0.0.1");
      await listening;
      const base = `http://127.0.0.1:${server.address().port}`;

      // When the real CLI uses real sign.mjs against the MCP fixture.
      const result = await run([
        "packages/cli/bin.mjs", "connect", "--token", token, "--clients", "codex",
        "--skill-only", "--home", home, "--base-url", base,
      ], env);

      // Then only confirmed ownership succeeds, without leaking the token.
      assert.equal(result.stdout.includes(token), false);
      assert.equal(result.stderr.includes(token), false);
      if (mode === "unconfirmed-key") {
        assert.notEqual(result.code, 0);
      } else {
        assert.equal(result.code, 0, result.stderr);
        const local = await run([signer, "pubkey"], {
          ...env, ARENA_HOME: mode === "explicit-home" ? customHome : defaultHome,
        });
        assert.equal(local.code, 0);
        assert.equal(JSON.parse(local.stdout).kid, registered.kid);
      }
      if (original) {
        const unchanged = await run([signer, "pubkey"], { ...env, ARENA_HOME: defaultHome });
        assert.equal(unchanged.code, 0);
        assert.equal(JSON.parse(unchanged.stdout).kid, original.kid);
      }
    } finally {
      const closed = new Promise(resolve => server.close(resolve));
      server.closeAllConnections();
      await closed;
      await rm(home, { recursive: true, force: true });
    }
  });
}
