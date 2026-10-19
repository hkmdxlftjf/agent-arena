import { spawnSync } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import zlib from "node:zlib";

const BASE_DEFAULT = "https://agent.idou.bid";
const WRITER = fileURLToPath(new URL("./write.cjs", import.meta.url));
const LOCAL_CLIENTS = new Set([
  "claude-code", "claude-desktop", "cursor", "vscode", "opencode", "codex",
  "gemini", "windsurf", "cline", "roo", "qwen", "kiro", "trae",
]);
const CLIENT_IDS = [
  "claude-code",
  "claude-desktop",
  "cursor",
  "vscode",
  "opencode",
  "codex",
  "gemini",
  "windsurf",
  "cline",
  "roo",
  "zed",
  "qwen",
  "kiro",
  "trae",
  "continue",
  "goose",
];
const ALIASES = {
  claude: "claude-code",
  "claude-code": "claude-code",
  desktop: "claude-desktop",
  "claude-desktop": "claude-desktop",
  cursor: "cursor",
  code: "vscode",
  copilot: "vscode",
  vscode: "vscode",
  opencode: "opencode",
  codex: "codex",
  gemini: "gemini",
  "gemini-cli": "gemini",
  windsurf: "windsurf",
  cline: "cline",
  roo: "roo",
  "roo-code": "roo",
  zed: "zed",
  qwen: "qwen",
  "qwen-code": "qwen",
  kiro: "kiro",
  trae: "trae",
  continue: "continue",
  goose: "goose",
};

function say(line) {
  process.stdout.write(`${line}\n`);
}

function fail(message, code) {
  process.stderr.write(`${message}\n`);
  return code;
}

function usage() {
  return fail(
    "用法: agent-arena serve | connect [--local] [--token <AGENT_TOKEN>] [--yes] [--clients claude-code,cursor] [--skill-only] [--mcp-only] [--dry-run]",
    2,
  );
}

function commandExists(name) {
  if (process.platform === "win32") {
    const found = spawnSync("where", [name], { encoding: "utf8" });
    return found.status === 0;
  }
  const found = spawnSync("sh", ["-c", `command -v ${name}`], { encoding: "utf8" });
  return found.status === 0 && found.stdout.trim().length > 0;
}

function appData(home, ...rest) {
  const base = process.env.APPDATA || path.join(home, "AppData", "Roaming");
  return path.join(base, ...rest);
}

function claudeDesktopDir(home) {
  if (process.platform === "darwin") return path.join(home, "Library", "Application Support", "Claude");
  if (process.platform === "win32") return appData(home, "Claude");
  return path.join(home, ".config", "Claude");
}

function vscodeUserDir(home) {
  if (process.platform === "darwin") return path.join(home, "Library", "Application Support", "Code", "User");
  if (process.platform === "win32") return appData(home, "Code", "User");
  return path.join(home, ".config", "Code", "User");
}

function traeDir(home) {
  if (process.platform === "darwin") return path.join(home, "Library", "Application Support", "Trae", "User");
  if (process.platform === "win32") return appData(home, "Trae", "User");
  return path.join(home, ".config", "Trae", "User");
}

function zedFile(home) {
  if (process.platform === "win32") return appData(home, "Zed", "settings.json");
  return path.join(home, ".config", "zed", "settings.json");
}

function gooseFile(home) {
  if (process.platform === "win32") return appData(home, "Block", "goose", "config", "config.yaml");
  return path.join(home, ".config", "goose", "config.yaml");
}

function clientConfigPath(home, id) {
  switch (id) {
    case "claude-code":
      return path.join(home, ".claude.json");
    case "claude-desktop":
      return path.join(claudeDesktopDir(home), "claude_desktop_config.json");
    case "cursor":
      return path.join(home, ".cursor", "mcp.json");
    case "vscode":
      return path.join(vscodeUserDir(home), "mcp.json");
    case "opencode": {
      const dir = path.join(home, ".config", "opencode");
      const jsonc = path.join(dir, "opencode.jsonc");
      return fs.existsSync(jsonc) ? jsonc : path.join(dir, "opencode.json");
    }
    case "codex":
      return path.join(home, ".codex", "config.toml");
    case "gemini":
      return path.join(home, ".gemini", "settings.json");
    case "windsurf":
      return path.join(home, ".codeium", "windsurf", "mcp_config.json");
    case "cline":
      return path.join(vscodeUserDir(home), "globalStorage", "saoudrizwan.claude-dev", "settings", "cline_mcp_settings.json");
    case "roo":
      return path.join(vscodeUserDir(home), "globalStorage", "rooveterinaryinc.roo-cline", "settings", "mcp_settings.json");
    case "zed":
      return zedFile(home);
    case "qwen":
      return path.join(home, ".qwen", "settings.json");
    case "kiro":
      return path.join(home, ".kiro", "settings", "mcp.json");
    case "trae":
      return path.join(traeDir(home), "mcp.json");
    case "continue":
      return path.join(home, ".continue", "mcpServers", "agent-arena.yaml");
    case "goose":
      return gooseFile(home);
    default:
      return "";
  }
}

function clientInstalled(home, id) {
  switch (id) {
    case "claude-code":
      return commandExists("claude") || fs.existsSync(path.join(home, ".claude")) || fs.existsSync(path.join(home, ".claude.json"));
    case "claude-desktop": {
      const desk = claudeDesktopDir(home);
      return fs.existsSync(desk) || fs.existsSync(path.join(desk, "claude_desktop_config.json"));
    }
    case "cursor":
      return commandExists("cursor") || fs.existsSync(path.join(home, ".cursor"));
    case "vscode":
      return commandExists("code") || fs.existsSync(vscodeUserDir(home));
    case "opencode":
      return commandExists("opencode") || fs.existsSync(path.join(home, ".config", "opencode"));
    case "codex":
      return commandExists("codex") || fs.existsSync(path.join(home, ".codex"));
    case "gemini":
      return commandExists("gemini") || fs.existsSync(path.join(home, ".gemini"));
    case "windsurf":
      return commandExists("windsurf") || fs.existsSync(path.join(home, ".codeium", "windsurf"));
    case "cline":
      return commandExists("cline") || fs.existsSync(path.join(vscodeUserDir(home), "globalStorage", "saoudrizwan.claude-dev"));
    case "roo":
      return fs.existsSync(path.join(vscodeUserDir(home), "globalStorage", "rooveterinaryinc.roo-cline"));
    case "zed":
      return commandExists("zed") || fs.existsSync(path.join(home, ".config", "zed")) || fs.existsSync(zedFile(home));
    case "qwen":
      return commandExists("qwen") || fs.existsSync(path.join(home, ".qwen"));
    case "kiro":
      return commandExists("kiro") || fs.existsSync(path.join(home, ".kiro"));
    case "trae":
      return commandExists("trae") || fs.existsSync(traeDir(home));
    case "continue":
      return fs.existsSync(path.join(home, ".continue"));
    case "goose":
      return fs.existsSync(path.join(home, ".config", "goose")) || fs.existsSync(gooseFile(home));
    default:
      return false;
  }
}

function skillSpec(home, id) {
  switch (id) {
    case "claude-code":
      return ["link", path.join(home, ".claude", "skills", "arena")];
    case "claude-desktop":
      return ["zip", path.join(home, ".arena", "skill", "arena-skill.zip")];
    case "cursor":
      return ["link", path.join(home, ".cursor", "skills", "arena")];
    case "vscode":
      return ["rules", path.join(home, ".copilot", "instructions", "arena.instructions.md")];
    case "opencode":
      return ["link", path.join(home, ".config", "opencode", "skills", "arena")];
    case "codex":
    case "zed":
    case "goose":
      return ["link", path.join(home, ".agents", "skills", "arena")];
    case "gemini":
      return ["link", path.join(home, ".gemini", "skills", "arena")];
    case "windsurf":
      return ["link", path.join(home, ".codeium", "windsurf", "skills", "arena")];
    case "cline":
      return ["rules", path.join(home, ".cline", "rules", "arena.md")];
    case "roo":
      return ["rules", path.join(home, ".roo", "rules", "arena.md")];
    case "qwen":
      return ["link", path.join(home, ".qwen", "skills", "arena")];
    case "kiro":
      return ["link", path.join(home, ".kiro", "skills", "arena")];
    case "trae":
      return ["link", path.join(home, ".trae", "skills", "arena")];
    case "continue":
      return ["rules", path.join(home, ".continue", "rules", "arena.md")];
    default:
      return ["", ""];
  }
}

function centralSkill(home) {
  return path.join(home, ".arena", "skill", "arena");
}

function openTty() {
  try {
    return fs.openSync("/dev/tty", "r+");
  } catch {
    return null;
  }
}

function ttyWrite(fd, text) {
  fs.writeSync(fd, Buffer.from(text));
}

function ttyReadLine(fd) {
  let out = "";
  const chunk = Buffer.alloc(1);
  while (true) {
    let n = 0;
    try {
      n = fs.readSync(fd, chunk, 0, 1, null);
    } catch {
      break;
    }
    if (n === 0) break;
    const ch = chunk.toString("utf8", 0, n);
    if (ch === "\n") break;
    if (ch !== "\r") out += ch;
  }
  return out;
}

function menuLine(home, mark, n, id, skillOnly, mcpOnly) {
  const [kind, dest] = skillSpec(home, id);
  const parts = [];
  if (!skillOnly) parts.push(`MCP ${clientConfigPath(home, id)}`);
  if (!mcpOnly) {
    if (kind === "link") parts.push(`技能 ${dest}`);
    else if (kind === "rules") parts.push(`规则 ${dest}`);
    else parts.push("技能需手动上传 zip");
  }
  return `[${mark}] ${n} ${id}  ${parts.join(" + ")}`;
}

function normalizeClients(raw) {
  const out = [];
  for (const item of raw.split(/[,\s]+/).filter(Boolean)) {
    const id = ALIASES[item];
    if (!id) return fail(`未知客户端: ${item}`, 2);
    if (!out.includes(id)) out.push(id);
  }
  if (out.length === 0) return fail("客户端列表是空的", 2);
  return out;
}

function mapNumbers(raw) {
  const out = [];
  for (const n of raw.split(/[,\s]+/).filter(Boolean)) {
    if (!/^\d+$/.test(n)) return fail(`无法识别的编号: ${n}`, 2);
    const num = Number(n);
    if (num < 1 || num > CLIENT_IDS.length) return fail(`编号超出范围: ${n}`, 2);
    const id = CLIENT_IDS[num - 1];
    if (!out.includes(id)) out.push(id);
  }
  return out;
}

function chooseClients(home, opts) {
  if (opts.clientsArg) return normalizeClients(opts.clientsArg);
  const detected = CLIENT_IDS.filter((id) => clientInstalled(home, id));
  if (opts.yes) return detected;
  const fd = openTty();
  if (fd === null) return fail("没有终端。请加 --clients 或 --yes。", 1);
  try {
    CLIENT_IDS.forEach((id, index) => {
      const mark = detected.includes(id) ? "*" : " ";
      ttyWrite(fd, `${menuLine(home, mark, index + 1, id, opts.skillOnly, opts.mcpOnly)}\n`);
    });
    ttyWrite(fd, "输入编号，逗号分隔，回车=全部已检测到的，a=全部，n=跳过\n");
    const ans = ttyReadLine(fd).trim();
    if (ans === "") return detected;
    if (ans === "a" || ans === "A") return CLIENT_IDS.slice();
    if (ans === "n" || ans === "N") return [];
    return mapNumbers(ans);
  } finally {
    fs.closeSync(fd);
  }
}

function redact(text, token) {
  if (!token) return String(text ?? "");
  return String(text ?? "").split(token).join("[token]");
}

function writeClient(opts, id) {
  const file = clientConfigPath(opts.home, id);
  const env = { ...process.env, ARENA_CONNECT_TOKEN: opts.dry ? "" : opts.token };
  if (opts.local) {
    env.ARENA_CONNECT_LOCAL = JSON.stringify({
      command: process.execPath,
      args: [path.join(opts.arenaHome, "runtime", "bin.mjs"), "serve"],
      env: {
        ARENA_TOKEN: opts.dry ? "" : opts.token,
        ARENA_HOME: opts.arenaHome,
        ARENA_BASE_URL: opts.base,
        ARENA_SIGN_SCRIPT: path.join(centralSkill(opts.home), "scripts", "sign.mjs"),
        ...(process.env.ARENA_ALLOWED_ORIGINS === undefined ? {} : { ARENA_ALLOWED_ORIGINS: process.env.ARENA_ALLOWED_ORIGINS }),
      },
    });
    if (id === "opencode") {
      const version = spawnSync("opencode", ["--version"], { encoding: "utf8" });
      if (version.status === 0) env.ARENA_CONNECT_OPENCODE_VERSION = version.stdout.trim();
    }
  } else {
    delete env.ARENA_CONNECT_LOCAL;
  }
  const result = spawnSync(process.execPath, [WRITER, id, file, `${opts.base}/mcp`, opts.dry ? "1" : "0"], {
    encoding: "utf8",
    env,
  });
  if (result.status !== 0) {
    const err = (result.stderr || result.stdout || "写入失败").trim();
    return fail(redact(err, opts.token), result.status || 1);
  }
  const line = (result.stdout || "").trim().split("\n").pop();
  say(`client: ${id} ${file} ${line}`);
  if (!opts.local && id === "claude-code" && opts.home === opts.realHome && commandExists("claude")) {
    if (opts.dry) {
      say("client: claude-code cli would-run");
    } else {
      spawnSync(
        "claude",
        ["mcp", "add", "--transport", "http", "agent-arena", `${opts.base}/mcp`, "--header", `Authorization: Bearer ${opts.token}`],
        { stdio: "ignore" },
      );
      say("client: claude-code cli");
    }
  }
  return 0;
}

function configureClients(opts, selected) {
  for (const id of CLIENT_IDS) {
    if (!selected.includes(id)) {
      say(`client: ${id} skip`);
      continue;
    }
    const code = writeClient(opts, id);
    if (code !== 0) return code;
  }
  return 0;
}

function planSkills(home, selected) {
  for (const id of CLIENT_IDS) {
    if (!selected.includes(id)) continue;
    const [kind, dest] = skillSpec(home, id);
    say(`skill: ${id} ${kind} ${dest}`);
  }
  say(`skill: ${centralSkill(home)}`);
}

function placeCentral(src, home) {
  const dest = centralSkill(home);
  if (!["SKILL.md", "scripts/sign.mjs", "scripts/sign-lock.mjs"].every((name) => fs.existsSync(path.join(src, name)))) {
    return fail("技能包内容不完整。", 1);
  }
  fs.mkdirSync(path.dirname(dest), { recursive: true });
  fs.rmSync(dest, { recursive: true, force: true });
  fs.cpSync(src, dest, { recursive: true });
  say(`skill: ${dest}`);
  return 0;
}

function linkSkill(home, dest) {
  const target = centralSkill(home);
  fs.mkdirSync(path.dirname(dest), { recursive: true });
  if (fs.lstatSync(dest, { throwIfNoEntry: false })?.isSymbolicLink()) {
    if (fs.readlinkSync(dest) === target) {
      say(`skill: ${dest} unchanged`);
      return;
    }
    fs.unlinkSync(dest);
  } else if (fs.existsSync(dest)) {
    fs.rmSync(dest, { recursive: true, force: true });
  }
  fs.symlinkSync(target, dest);
  say(`skill: ${dest} -> ${target}`);
}

function rulesBody(home, id) {
  const skill = path.join(centralSkill(home), "SKILL.md");
  const sign = path.join(centralSkill(home), "scripts", "sign.mjs");
  if (id === "vscode") {
    return `---
name: arena
description: 用户提到 agent-arena、对局、落子或房间时，先读技能文件再行动。
---

玩 Agent Arena 时先读下面两个文件，按技能区分远程信封或本地网关流程。不要改 sign.mjs。

- 技能：${skill}
- 签名：${sign}
`;
  }
  if (id === "continue") {
    return `---
name: arena
description: 用户提到 agent-arena、对局、落子或房间时读取技能。
alwaysApply: false
---

玩 Agent Arena 时先读下面两个文件，按技能区分远程信封或本地网关流程。不要改 sign.mjs。

- 技能：${skill}
- 签名：${sign}
`;
  }
  return `# Agent Arena

用户提到 agent-arena、对局、落子或房间时，先读技能文件，按其中的远程信封或本地网关流程行动。不要改 sign.mjs。

- 技能：${skill}
- 签名：${sign}
`;
}

function sameBody(current, next) {
  const norm = (text) => text.replace(/\r\n/g, "\n").replace(/\n+$/, "");
  return norm(current) === norm(next);
}

function writeRules(home, id, dest) {
  const body = rulesBody(home, id);
  fs.mkdirSync(path.dirname(dest), { recursive: true });
  if (fs.existsSync(dest) && sameBody(fs.readFileSync(dest, "utf8"), body)) {
    say(`rules: ${dest} unchanged`);
    return;
  }
  fs.writeFileSync(dest, body.endsWith("\n") ? body : `${body}\n`);
  say(`rules: ${dest}`);
}

function crc32(buf) {
  const table = crc32.table || (crc32.table = (() => {
    const t = new Uint32Array(256);
    for (let n = 0; n < 256; n += 1) {
      let c = n;
      for (let k = 0; k < 8; k += 1) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
      t[n] = c >>> 0;
    }
    return t;
  })());
  let c = 0xffffffff;
  for (let i = 0; i < buf.length; i += 1) c = table[(c ^ buf[i]) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

function skillZip(src) {
  const files = [];
  function walk(dir, base) {
    for (const name of fs.readdirSync(dir)) {
      const abs = path.join(dir, name);
      const rel = base ? `${base}/${name}` : name;
      if (fs.statSync(abs).isDirectory()) walk(abs, rel);
      else files.push({ rel: `arena/${rel}`, data: fs.readFileSync(abs) });
    }
  }
  walk(src, "");
  files.sort((a, b) => (a.rel < b.rel ? -1 : a.rel > b.rel ? 1 : 0));
  const parts = [];
  const central = [];
  let offset = 0;
  for (const file of files) {
    const name = Buffer.from(file.rel);
    const crc = crc32(file.data);
    const local = Buffer.alloc(30 + name.length);
    local.writeUInt32LE(0x04034b50, 0);
    local.writeUInt16LE(20, 4);
    local.writeUInt16LE(name.length, 26);
    local.writeUInt32LE(crc, 14);
    local.writeUInt32LE(file.data.length, 18);
    local.writeUInt32LE(file.data.length, 22);
    name.copy(local, 30);
    const cen = Buffer.alloc(46 + name.length);
    cen.writeUInt32LE(0x02014b50, 0);
    cen.writeUInt16LE(20, 4);
    cen.writeUInt16LE(20, 6);
    cen.writeUInt16LE(name.length, 28);
    cen.writeUInt32LE(crc, 16);
    cen.writeUInt32LE(file.data.length, 20);
    cen.writeUInt32LE(file.data.length, 24);
    cen.writeUInt32LE(offset, 42);
    name.copy(cen, 46);
    parts.push(local, file.data);
    central.push(cen);
    offset += local.length + file.data.length;
  }
  const cenBuf = Buffer.concat(central);
  const eocd = Buffer.alloc(22);
  eocd.writeUInt32LE(0x06054b50, 0);
  eocd.writeUInt16LE(files.length, 8);
  eocd.writeUInt16LE(files.length, 10);
  eocd.writeUInt32LE(cenBuf.length, 12);
  eocd.writeUInt32LE(offset, 16);
  return Buffer.concat([...parts, cenBuf, eocd]);
}

function writeSkillZip(home, dest) {
  const out = skillZip(centralSkill(home));
  const prev = fs.existsSync(dest) ? fs.readFileSync(dest) : null;
  if (prev && prev.equals(out)) return "unchanged";
  fs.mkdirSync(path.dirname(dest), { recursive: true });
  fs.writeFileSync(dest, out);
  return "updated";
}

function installClientSkills(home, selected) {
  const linked = new Set();
  for (const id of CLIENT_IDS) {
    if (!selected.includes(id)) continue;
    const [kind, dest] = skillSpec(home, id);
    if (kind === "link") {
      if (linked.has(dest)) say(`skill: ${id} ${dest} unchanged`);
      else {
        linkSkill(home, dest);
        linked.add(dest);
      }
    } else if (kind === "rules") writeRules(home, id, dest);
    else if (kind === "zip") {
      const status = writeSkillZip(home, dest);
      say(`skill: ${dest} ${status}`);
      say(`claude-desktop 技能要手动上传。压缩包：${dest}`);
      say("打开 Claude Desktop 的 Customize，进入 Skills，点 +，选 Upload a skill，上传这个 zip。压缩包顶层必须是 arena/SKILL.md。");
    }
  }
}

function installFromTree(src, home, selected) {
  const code = placeCentral(src, home);
  if (code !== 0) return code;
  installClientSkills(home, selected);
  return 0;
}

function findEocd(buf) {
  const min = Math.max(0, buf.length - 22 - 65535);
  for (let i = buf.length - 22; i >= min; i -= 1) {
    if (buf.readUInt32LE(i) === 0x06054b50) return i;
  }
  throw new Error("不是 zip");
}

function extractZip(zipPath, dest) {
  const buf = fs.readFileSync(zipPath);
  const eocd = findEocd(buf);
  const count = buf.readUInt16LE(eocd + 10);
  let p = buf.readUInt32LE(eocd + 16);
  for (let n = 0; n < count; n += 1) {
    if (buf.readUInt32LE(p) !== 0x02014b50) throw new Error("zip 目录损坏");
    const method = buf.readUInt16LE(p + 10);
    const compSize = buf.readUInt32LE(p + 20);
    const nameLen = buf.readUInt16LE(p + 28);
    const extraLen = buf.readUInt16LE(p + 30);
    const commentLen = buf.readUInt16LE(p + 32);
    const localOff = buf.readUInt32LE(p + 42);
    const name = buf.slice(p + 46, p + 46 + nameLen).toString("utf8");
    p += 46 + nameLen + extraLen + commentLen;
    if (!name || name.endsWith("/")) continue;
    if (name.includes("..") || name.startsWith("/") || name.includes("\\")) throw new Error("zip 路径不安全");
    const ln = buf.readUInt16LE(localOff + 26);
    const le = buf.readUInt16LE(localOff + 28);
    const dataStart = localOff + 30 + ln + le;
    const data = buf.slice(dataStart, dataStart + compSize);
    let out;
    if (method === 0) out = data;
    else if (method === 8) out = zlib.inflateRawSync(data);
    else throw new Error(`不支持的 zip 压缩方式 ${method}`);
    const target = path.join(dest, name);
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.writeFileSync(target, out, { mode: 0o644 });
  }
}

async function installSkills(opts, selected) {
  const work = fs.mkdtempSync(path.join(os.tmpdir(), "arena-skill-"));
  try {
    const metaRes = await fetch(`${opts.base}/.well-known/arena-skill.json`);
    if (!metaRes.ok) return fail("技能清单缺少 sha256", 1);
    const meta = await metaRes.json();
    if (!meta.sha256 || !meta.url) return fail("技能清单缺少 sha256", 1);
    const zipUrl = String(meta.url).startsWith("http") ? meta.url : `${opts.base}${meta.url}`;
    const zipRes = await fetch(zipUrl);
    if (!zipRes.ok) return fail("技能包下载失败", 1);
    const bytes = Buffer.from(await zipRes.arrayBuffer());
    const got = crypto.createHash("sha256").update(bytes).digest("hex");
    if (got !== meta.sha256) return fail("技能包 sha256 不一致，已停止。", 1);
    say(`skill-sha256: ${meta.sha256}`);
    const zipPath = path.join(work, "skill.zip");
    fs.writeFileSync(zipPath, bytes);
    const extract = path.join(work, "extract");
    fs.mkdirSync(extract, { recursive: true });
    extractZip(zipPath, extract);
    return installFromTree(path.join(extract, "arena"), opts.home, selected);
  } finally {
    fs.rmSync(work, { recursive: true, force: true });
  }
}

async function rpc(opts, session, method, params) {
  const headers = {
    "content-type": "application/json",
    accept: "application/json, text/event-stream",
    authorization: `Bearer ${opts.token}`,
    "user-agent": "arena-connect",
  };
  if (session) headers["mcp-session-id"] = session;
  const res = await fetch(`${opts.base}/mcp`, {
    method: "POST",
    headers,
    body: JSON.stringify({ jsonrpc: "2.0", id: 1, method, params: params || {} }),
  });
  const sid = res.headers.get("mcp-session-id") || session;
  const text = await res.text();
  if (!res.ok) throw new Error(`MCP 失败（${res.status}）`);
  let msg;
  try {
    msg = JSON.parse(text);
  } catch {
    throw new Error("MCP 返回不是 JSON");
  }
  if (msg.error) throw new Error(msg.error.message || "MCP 错误");
  return { result: msg.result, session: sid };
}

function parseTool(result) {
  if (result && result.isError) {
    const text = result.content && result.content[0] && result.content[0].text;
    throw new Error(text || "工具调用失败");
  }
  if (result && result.structuredContent) return result.structuredContent;
  const text = result && result.content && result.content[0] && result.content[0].text;
  if (!text) return result || {};
  return JSON.parse(text);
}

async function registerKey(opts) {
  const sign = path.join(centralSkill(opts.home), "scripts", "sign.mjs");
  if (!fs.existsSync(sign)) return fail("没有找到 sign.mjs", 1);
  const arenaHome = path.resolve(process.env.ARENA_HOME || path.join(opts.home, ".arena"));
  say(`signer-home: ${redact(arenaHome, opts.token)}`);
  fs.mkdirSync(arenaHome, { recursive: true });
  try {
    fs.chmodSync(arenaHome, 0o700);
  } catch {
    /* Windows may refuse the mode bit. */
  }
  const env = { ...process.env, ARENA_HOME: arenaHome };
  let pub = spawnSync(process.execPath, [sign, "pubkey"], { encoding: "utf8", env });
  if (pub.status !== 0) {
    say("正在生成本机签名密钥");
    const gen = spawnSync(process.execPath, [sign, "keygen"], { encoding: "utf8", env });
    if (gen.status !== 0) return fail("本地密钥生成失败", 1);
    let kid = "";
    try {
      const parsed = JSON.parse(gen.stdout);
      if (!parsed.ok || !parsed.kid) return fail("本地密钥生成失败", 1);
      kid = parsed.kid;
    } catch {
      return fail("本地密钥生成失败", 1);
    }
    pub = gen;
    say(`已生成公钥 ${kid}`);
  } else {
    say("本机已有签名密钥");
  }
  try {
    const localKey = JSON.parse(pub.stdout);
    if (!localKey.ok || !localKey.kid || !localKey.public_key) return fail("本地公钥无法解析", 1);
    const init = await rpc(opts, null, "initialize", {
      protocolVersion: "2025-03-26",
      capabilities: {},
      clientInfo: { name: "arena-connect", version: "1.0.0" },
    });
    const challenge = parseTool((await rpc(opts, init.session, "tools/call", { name: "get_key_challenge", arguments: {} })).result);
    if (!challenge.signing_request) return fail("没有拿到注册挑战", 1);
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), "arena-req-"));
    const reqPath = path.join(dir, "req.json");
    try {
      fs.writeFileSync(reqPath, JSON.stringify(challenge.signing_request), { mode: 0o600 });
      const signed = spawnSync(process.execPath, [sign, "sign", reqPath], { encoding: "utf8", env });
      if (signed.status !== 0) return fail("本地签名失败", 1);
      let envelope;
      try {
        envelope = JSON.parse(signed.stdout);
      } catch {
        return fail("签名结果无法解析", 1);
      }
      parseTool((await rpc(opts, init.session, "tools/call", { name: "register_agent_key", arguments: { envelope } })).result);
      const who = parseTool((await rpc(opts, init.session, "tools/call", { name: "whoami", arguments: {} })).result);
      const confirmed = (who.keys || []).some((key) =>
        key.kid === localKey.kid && key.public_key === localKey.public_key && key.status === "active");
      if (!confirmed) return fail("公钥校验失败：当前智能体没有本机对应的有效公钥，请确认令牌与 ARENA_HOME", 1);
      const kids = (who.keys || []).map((k) => k.kid).filter(Boolean).join(" ");
      say(`agent: ${who.name || ""} @${who.owner_handle || ""}`);
      say(`keys: ${kids}`);
    } finally {
      fs.rmSync(dir, { recursive: true, force: true });
    }
  } catch (err) {
    return fail(redact(err && err.message ? err.message : "连接失败", opts.token), 1);
  }
  return 0;
}

function parseArgs(argv) {
  const opts = {
    token: process.env.ARENA_TOKEN || "",
    yes: false,
    dry: false,
    skipNetwork: false,
    base: BASE_DEFAULT,
    home: process.env.HOME || process.env.USERPROFILE || os.homedir(),
    realHome: process.env.HOME || process.env.USERPROFILE || os.homedir(),
    clientsArg: "",
    skillOnly: false,
    mcpOnly: false,
    local: false,
  };
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    const need = () => {
      if (i + 1 >= argv.length) return null;
      i += 1;
      return argv[i];
    };
    if (arg === "--token") {
      const value = need();
      if (value === null) return usage();
      opts.token = value;
    } else if (arg === "--yes" || arg === "-y") opts.yes = true;
    else if (arg === "--dry-run") opts.dry = true;
    else if (arg === "--skip-network") opts.skipNetwork = true;
    else if (arg === "--local") opts.local = true;
    else if (arg === "--base-url") {
      const value = need();
      if (value === null) return usage();
      opts.base = value.replace(/\/+$/, "");
    } else if (arg === "--home") {
      const value = need();
      if (value === null) return usage();
      opts.home = value;
    } else if (arg === "--clients") {
      const value = need();
      if (value === null) return usage();
      opts.clientsArg = value;
    } else if (arg === "--skill-only") opts.skillOnly = true;
    else if (arg === "--mcp-only") opts.mcpOnly = true;
    else if (arg === "-h" || arg === "--help") return usage();
    else return fail("无法识别的参数", 2);
  }
  return opts;
}

function openCodeHint(selected, dry) {
  if (!selected.includes("opencode")) return;
  say(dry
    ? "opencode: 将预授权 ~/.arena 目录与 sign.mjs 命令，避免 external_directory 授权弹窗"
    : "opencode: 已预授权 ~/.arena 目录与 sign.mjs 命令；若仍弹授权，选 Always allow 即可");
}

export async function main(argv) {
  const cmd = argv[0];
  if (!cmd || cmd === "-h" || cmd === "--help") return usage();
  if (cmd !== "connect") return fail("无法识别的参数", 2);
  const major = Number(process.versions.node.split(".")[0]);
  if (!Number.isFinite(major) || major < 18) {
    return fail(`需要 Node.js >= 18，当前是 v${process.versions.node}。`, 1);
  }
  const opts = parseArgs(argv.slice(1));
  if (typeof opts === "number") return opts;
  if (opts.skillOnly && opts.mcpOnly) return fail("--skill-only 和 --mcp-only 不能一起用。", 2);
  say(`node: v${process.versions.node}`);
  say(`dry-run: ${opts.dry ? 1 : 0}`);
  if (!opts.dry && !opts.token) return fail("缺少 --token。令牌只应放进客户端配置，不要贴到聊天记录里。", 1);
  const selected = chooseClients(opts.home, opts);
  if (typeof selected === "number") return selected;
  opts.home = path.resolve(opts.home);
  opts.arenaHome = path.resolve(process.env.ARENA_HOME || path.join(opts.home, ".arena"));
  if (opts.local) {
    if (opts.skillOnly) return fail("--local 不能与 --skill-only 一起使用。", 2);
    const unsupported = selected.filter((id) => !LOCAL_CLIENTS.has(id));
    if (unsupported.length) return fail(`不支持本地 stdio 客户端: ${unsupported.join(", ")}`, 2);
  }
  if (opts.dry) {
    if (!opts.mcpOnly) {
      say(`将下载 ${opts.base}/skill/arena-skill-v1.0.0.zip 并核对 sha256`);
      planSkills(opts.home, selected);
      say("将在没有本地密钥时运行 sign.mjs keygen，并用 get_key_challenge / register_agent_key 注册公钥");
    }
    if (!opts.skillOnly) {
      const code = configureClients(opts, selected);
      if (code !== 0) return code;
      if (!opts.local) openCodeHint(selected, true);
      say("将调用 whoami 核对公钥");
    }
    say("next: 在智能体里用自然语言说：用 agent-arena 列出可以玩的游戏");
    return 0;
  }
  if (!opts.mcpOnly && process.env.ARENA_SKILL_SRC) {
    const code = installFromTree(process.env.ARENA_SKILL_SRC, opts.home, selected);
    if (code !== 0) return code;
  } else if (!opts.skipNetwork && !opts.mcpOnly) {
    const code = await installSkills(opts, selected);
    if (code !== 0) return code;
  } else if (opts.skipNetwork) {
    say("skip-network: 1");
  }
  if (opts.local) {
    const signDir = path.join(centralSkill(opts.home), "scripts");
    if (!["sign.mjs", "sign-lock.mjs"].every((name) => fs.existsSync(path.join(signDir, name)))) {
      return fail("--local 需要完整的已安装技能（sign.mjs 与 sign-lock.mjs）。", 1);
    }
    // Copy out of npx's disposable cache before publishing any client command.
    const runtime = path.join(opts.arenaHome, "runtime");
    const source = fileURLToPath(new URL("../", import.meta.url));
    fs.mkdirSync(path.join(runtime, "lib"), { recursive: true, mode: 0o700 });
    fs.chmodSync(opts.arenaHome, 0o700);
    fs.chmodSync(runtime, 0o700);
    for (const name of ["bin.mjs", "lib/connect.mjs", "lib/write.cjs",
      "lib/gateway.mjs", "lib/gateway-remote.mjs", "lib/gateway-store.mjs"]) {
      const dest = path.join(runtime, name);
      if (path.resolve(source, name) !== dest) fs.copyFileSync(path.join(source, name), dest);
      fs.chmodSync(dest, 0o600);
    }
    say(`runtime: ${redact(runtime, opts.token)}`);
  }
  const sign = path.join(centralSkill(opts.home), "scripts", "sign.mjs");
  if (!opts.skipNetwork && fs.existsSync(sign)) {
    const code = await registerKey(opts);
    if (code !== 0) return code;
  } else if (!opts.skipNetwork && opts.mcpOnly) {
    say("mcp-only: 未安装技能，跳过公钥注册");
  }
  if (!opts.skillOnly) {
    const code = configureClients(opts, selected);
    if (code !== 0) return code;
    if (!opts.local) openCodeHint(selected, false);
  }
  say("next: 在智能体里用自然语言说：用 agent-arena 列出可以玩的游戏");
  return 0;
}
