/* arena-connect-writer */
const fs = require("node:fs");
const path = require("node:path");

const client = process.argv[2];
const file = process.argv[3];
const url = process.argv[4];
const dry = process.argv[5] === "1";
const token = process.env.ARENA_CONNECT_TOKEN || "";
const local = process.env.ARENA_CONNECT_LOCAL ? JSON.parse(process.env.ARENA_CONNECT_LOCAL) : null;

if (!dry && !token) {
  console.error("缺少令牌");
  process.exit(1);
}

const auth = "Bearer " + token;

function readText(target) {
  if (!fs.existsSync(target)) return "";
  let text = fs.readFileSync(target, "utf8");
  if (text.charCodeAt(0) === 0xfeff) text = text.slice(1);
  return text;
}

function stripJsonc(input) {
  let out = "";
  let i = 0;
  let inStr = false;
  let esc = false;
  while (i < input.length) {
    const c = input[i];
    const n = input[i + 1];
    if (inStr) {
      out += c;
      if (esc) esc = false;
      else if (c === "\\") esc = true;
      else if (c === '"') inStr = false;
      i += 1;
      continue;
    }
    if (c === '"') {
      inStr = true;
      out += c;
      i += 1;
      continue;
    }
    if (c === "/" && n === "/") {
      i += 2;
      while (i < input.length && input[i] !== "\n") i += 1;
      continue;
    }
    if (c === "/" && n === "*") {
      i += 2;
      while (i < input.length && !(input[i] === "*" && input[i + 1] === "/")) i += 1;
      i += 2;
      continue;
    }
    out += c;
    i += 1;
  }
  let cleaned = "";
  i = 0;
  inStr = false;
  esc = false;
  while (i < out.length) {
    const c = out[i];
    if (inStr) {
      cleaned += c;
      if (esc) esc = false;
      else if (c === "\\") esc = true;
      else if (c === '"') inStr = false;
      i += 1;
      continue;
    }
    if (c === '"') {
      inStr = true;
      cleaned += c;
      i += 1;
      continue;
    }
    if (c === ",") {
      let j = i + 1;
      while (j < out.length && /\s/.test(out[j])) j += 1;
      if (out[j] === "}" || out[j] === "]") {
        i += 1;
        continue;
      }
    }
    cleaned += c;
    i += 1;
  }
  return cleaned;
}

function readJson(target) {
  const text = readText(target).trim();
  if (!text) return { doc: {}, jsonc: false };
  try {
    return { doc: JSON.parse(text), jsonc: false };
  } catch (err) {
    try {
      return { doc: JSON.parse(stripJsonc(text)), jsonc: true };
    } catch (err2) {
      console.error("配置文件不是 JSON 对象");
      process.exit(1);
    }
  }
}

function finish(target, text, prevNorm) {
  const next = text.endsWith("\n") ? text : text + "\n";
  if (!dry && prevNorm === next) {
    console.log("unchanged");
    return;
  }
  if (dry) {
    console.log("would-write");
    return;
  }
  fs.mkdirSync(path.dirname(target), { recursive: true });
  if (fs.existsSync(target)) {
    const stamp = new Date().toISOString().replace(/[:.]/g, "-");
    fs.copyFileSync(target, target + ".bak." + stamp);
  }
  fs.writeFileSync(target, next, { mode: 0o600 });
  try {
    fs.chmodSync(target, 0o600);
  } catch (err) {
    /* Windows may refuse the mode bit. */
  }
  console.log("updated");
}

function rootKey(name) {
  if (name === "opencode") return "mcp";
  if (name === "vscode") return "servers";
  if (name === "zed") return "context_servers";
  return "mcpServers";
}

function buildEntry(name, prev) {
  const base = prev && typeof prev === "object" && !Array.isArray(prev) ? Object.assign({}, prev) : {};
  if (local) {
    for (const key of ["url", "httpUrl", "serverUrl", "headers", "oauth", "command", "args", "env", "environment", "type"]) delete base[key];
    if (name === "opencode") {
      delete base.enabled;
      delete base.disabled;
      delete base.timeout;
      delete base.protocol;
      return Object.assign(base, {
        type: "local", command: [local.command, ...local.args], environment: local.env,
      });
    }
    return Object.assign(base, { command: local.command, args: local.args, env: local.env },
      name === "vscode" || name === "cline" || name === "roo" ? { type: "stdio" } : {},
      name === "cline" || name === "roo" ? { disabled: false } : {});
  }
  const headers = Object.assign({}, base.headers || {}, { Authorization: auth });
  if (name === "claude-desktop") {
    return {
      command: "npx",
      args: ["-y", "mcp-remote", url, "--header", "Authorization: " + auth],
    };
  }
  if (name === "vscode") return Object.assign(base, { type: "http", url, headers });
  if (name === "opencode") {
    return Object.assign(base, { type: "remote", url, enabled: true, headers, oauth: false });
  }
  if (name === "gemini" || name === "qwen") {
    const next = Object.assign(base, { httpUrl: url, headers });
    delete next.url;
    return next;
  }
  if (name === "windsurf") return Object.assign(base, { serverUrl: url, headers });
  if (name === "cline") {
    return Object.assign(base, { type: "streamableHttp", url, headers, disabled: false });
  }
  if (name === "roo") return Object.assign(base, { type: "streamable-http", url, headers });
  return Object.assign(base, { url, headers });
}

const OPENCODE_PERMISSIONS = {
  external_directory: { "~/.arena/**": "allow" },
  bash: { "node *sign.mjs*": "allow" },
};

function mergeRules(existing, ours) {
  if (typeof existing === "string") return Object.assign({ "*": existing }, ours);
  if (existing && typeof existing === "object" && !Array.isArray(existing)) return Object.assign({}, existing, ours);
  return Object.assign({}, ours);
}

function mergeOpenCodePermission(perm) {
  const merged = perm && typeof perm === "object" && !Array.isArray(perm) ? Object.assign({}, perm) : {};
  for (const [name, rules] of Object.entries(OPENCODE_PERMISSIONS)) {
    merged[name] = mergeRules(merged[name], rules);
  }
  return merged;
}

function openCodePermissionOk(perm) {
  if (!perm || typeof perm !== "object" || Array.isArray(perm)) return false;
  for (const [name, rules] of Object.entries(OPENCODE_PERMISSIONS)) {
    const cur = perm[name];
    if (!cur || typeof cur !== "object" || Array.isArray(cur)) return false;
    for (const [pattern, effect] of Object.entries(rules)) {
      if (cur[pattern] !== effect) return false;
    }
  }
  return true;
}

function writeJson() {
  const parsed = readJson(file);
  const doc = parsed.doc;
  if (doc === null || typeof doc !== "object" || Array.isArray(doc)) {
    console.error("配置文件不是 JSON 对象");
    process.exit(1);
  }
  const key = rootKey(client);
  if (!doc[key] || typeof doc[key] !== "object" || Array.isArray(doc[key])) doc[key] = {};
  let servers = doc[key];
  let v2 = false;
  if (local && client === "opencode") {
    // Explicit layout wins over the installed binary. An empty config follows
    // its installed major, defaulting to the current V2 layout if unavailable.
    const mcp = doc.mcp;
    const direct = Object.values(mcp).some((entry) => entry && typeof entry === "object" && ["local", "remote"].includes(entry.type));
    const major = (process.env.ARENA_CONNECT_OPENCODE_VERSION || "").match(/^(?:v)?(\d+)\./);
    v2 = Object.hasOwn(mcp, "servers") || (!direct && (!major || Number(major[1]) >= 2));
    if (v2) {
      if (!mcp.servers || typeof mcp.servers !== "object" || Array.isArray(mcp.servers)) mcp.servers = {};
      servers = mcp.servers;
    }
  }
  const prev = servers["agent-arena"] && typeof servers["agent-arena"] === "object" ? servers["agent-arena"] : {};
  const next = buildEntry(client, prev);
  if (local && client === "opencode") {
    Object.assign(next, v2 ? {
      disabled: false, protocol: "legacy",
      timeout: { startup: 30000, catalog: 30000, execution: 660000 },
    } : { enabled: true, timeout: 660000 });
  }
  let permOk = true;
  if (client === "opencode" && !local) {
    if (typeof doc.permission !== "string") {
      permOk = openCodePermissionOk(doc.permission);
      doc.permission = mergeOpenCodePermission(doc.permission);
    }
  }
  if (!dry && JSON.stringify(prev) === JSON.stringify(next) && permOk) {
    console.log("unchanged");
    return;
  }
  if (dry) {
    console.log("would-write");
    return;
  }
  servers["agent-arena"] = next;
  const prevNorm = readText(file).replace(/\r\n/g, "\n");
  const nextText = JSON.stringify(doc, null, 2) + "\n";
  if (parsed.jsonc && nextText !== prevNorm) {
    console.error("提示: 配置文件含注释或尾随逗号，重写为严格 JSON；原始内容保留在 .bak 备份中");
  }
  finish(file, nextText, prevNorm);
}

function tomlQuote(value) {
  return '"' + String(value).replace(/\\/g, "\\\\").replace(/"/g, '\\"') + '"';
}

function desiredToml() {
  if (local) return [
    "[mcp_servers.agent-arena]",
    "command = " + tomlQuote(local.command),
    "args = [" + local.args.map(tomlQuote).join(", ") + "]",
    "env = { " + Object.entries(local.env).map(([key, value]) => key + " = " + tomlQuote(value)).join(", ") + " }",
    "",
  ].join("\n");
  return [
    "[mcp_servers.agent-arena]",
    "url = " + tomlQuote(url),
    "http_headers = { Authorization = " + tomlQuote(auth) + " }",
    "",
  ].join("\n");
}

function stripToml(src) {
  const lines = src.split(/\r?\n/);
  const out = [];
  let skipping = false;
  let ml = null;
  for (const line of lines) {
    if (ml) {
      if (!skipping) out.push(line);
      const parts = line.split(ml);
      if ((parts.length - 1) % 2 === 1) ml = null;
      continue;
    }
    if (line.includes('"""') && (line.split('"""').length - 1) % 2 === 1) ml = '"""';
    else if (line.includes("'''") && (line.split("'''").length - 1) % 2 === 1) ml = "'''";
    const table = line.match(/^\s*\[([^\]]+)\]\s*$/);
    if (table) {
      const name = table[1].trim();
      skipping = name === "mcp_servers.agent-arena" || name.startsWith("mcp_servers.agent-arena.");
      if (skipping) continue;
    }
    if (skipping) continue;
    if (/^\s*mcp_servers\.agent-arena\./.test(line)) continue;
    out.push(line);
  }
  return out.join("\n").replace(/\n+$/, "");
}

function writeToml() {
  const prev = readText(file).replace(/\r\n/g, "\n");
  const base = stripToml(prev);
  const next = (base ? base + "\n\n" : "") + desiredToml();
  finish(file, next, prev.endsWith("\n") || prev === "" ? prev : prev + "\n");
}

function gooseBlock() {
  return [
    "  agent-arena:",
    "    type: streamable_http",
    "    name: agent-arena",
    "    enabled: true",
    "    uri: " + JSON.stringify(url),
    "    headers:",
    "      Authorization: " + JSON.stringify(auth),
  ];
}

function writeGoose() {
  const prev = readText(file).replace(/\r\n/g, "\n");
  const lines = prev.length ? prev.split("\n") : [];
  if (lines.length && lines[lines.length - 1] === "") lines.pop();
  let ext = lines.findIndex((line) => /^extensions:\s*(#.*)?$/.test(line));
  if (ext === -1) {
    if (lines.length && lines[lines.length - 1] !== "") lines.push("");
    lines.push("extensions:");
    ext = lines.length - 1;
  }
  let start = -1;
  let end = lines.length;
  for (let i = ext + 1; i < lines.length; i += 1) {
    const line = lines[i];
    if (line.trim() === "" || /^\s*#/.test(line)) continue;
    const indent = line.match(/^ */)[0].length;
    if (indent === 0) {
      end = i;
      break;
    }
    if (indent === 2 && /^  agent-arena:\s*(#.*)?$/.test(line)) {
      start = i;
      let j = i + 1;
      for (; j < lines.length; j += 1) {
        if (lines[j].trim() === "") continue;
        if (lines[j].match(/^ */)[0].length <= 2) break;
      }
      end = j;
      break;
    }
  }
  const block = gooseBlock();
  if (start === -1) lines.splice(ext + 1, 0, ...block);
  else lines.splice(start, end - start, ...block);
  const next = lines.join("\n").replace(/\n*$/, "\n");
  const prevNorm = prev.length === 0 ? "" : prev.endsWith("\n") ? prev : prev + "\n";
  finish(file, next, prevNorm);
}

function continueText() {
  return [
    "name: agent-arena",
    "version: 0.0.1",
    "schema: v1",
    "mcpServers:",
    "  - name: agent-arena",
    "    type: streamable-http",
    "    url: " + JSON.stringify(url),
    "    requestOptions:",
    "      headers:",
    "        Authorization: " + JSON.stringify(auth),
    "",
  ].join("\n");
}

function writeContinue() {
  const prev = readText(file).replace(/\r\n/g, "\n");
  const prevNorm = prev.length === 0 ? "" : prev.endsWith("\n") ? prev : prev + "\n";
  finish(file, continueText(), prevNorm);
}

if (client === "codex") writeToml();
else if (client === "goose") writeGoose();
else if (client === "continue") writeContinue();
else writeJson();
