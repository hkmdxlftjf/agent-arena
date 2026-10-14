/* arena-connect-writer */
const fs = require("node:fs");
const path = require("node:path");

const client = process.argv[2];
const file = process.argv[3];
const url = process.argv[4];
const dry = process.argv[5] === "1";
const token = process.env.ARENA_CONNECT_TOKEN || "";

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
  if (!text) return {};
  try {
    return JSON.parse(text);
  } catch (err) {
    try {
      return JSON.parse(stripJsonc(text));
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

function writeJson() {
  const doc = readJson(file);
  if (doc === null || typeof doc !== "object" || Array.isArray(doc)) {
    console.error("配置文件不是 JSON 对象");
    process.exit(1);
  }
  const key = rootKey(client);
  if (!doc[key] || typeof doc[key] !== "object" || Array.isArray(doc[key])) doc[key] = {};
  const prev = doc[key]["agent-arena"] && typeof doc[key]["agent-arena"] === "object" ? doc[key]["agent-arena"] : {};
  const next = buildEntry(client, prev);
  if (!dry && JSON.stringify(prev) === JSON.stringify(next)) {
    console.log("unchanged");
    return;
  }
  if (dry) {
    console.log("would-write");
    return;
  }
  doc[key]["agent-arena"] = next;
  const prevNorm = readText(file).replace(/\r\n/g, "\n");
  finish(file, JSON.stringify(doc, null, 2) + "\n", prevNorm);
}

function tomlQuote(value) {
  return '"' + String(value).replace(/\\/g, "\\\\").replace(/"/g, '\\"') + '"';
}

function desiredToml() {
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
