#!/bin/sh
# Agent Arena one-command connect.
# curl -fsSL https://agent.idou.bid/connect.sh | sh -s -- --token <AGENT_TOKEN>
# Flags: --yes  --dry-run  --base-url URL  --home DIR  --skip-network
set -eu

BASE_DEFAULT="https://agent.idou.bid"
TOKEN="${ARENA_TOKEN:-}"
YES=0
DRY=0
SKIP_NETWORK=0
BASE="$BASE_DEFAULT"
HOME_DIR="${HOME}"

usage() {
  printf '%s\n' "用法: curl -fsSL ${BASE_DEFAULT}/connect.sh | sh -s -- --token <AGENT_TOKEN> [--yes] [--dry-run]" >&2
  exit 2
}

while [ $# -gt 0 ]; do
  case "$1" in
    --token)
      [ $# -ge 2 ] || usage
      TOKEN=$2
      shift 2
      ;;
    --yes|-y)
      YES=1
      shift
      ;;
    --dry-run)
      DRY=1
      shift
      ;;
    --skip-network)
      SKIP_NETWORK=1
      shift
      ;;
    --base-url)
      [ $# -ge 2 ] || usage
      BASE=$2
      shift 2
      ;;
    --home)
      [ $# -ge 2 ] || usage
      HOME_DIR=$2
      shift 2
      ;;
    -h|--help)
      usage
      ;;
    *)
      printf '%s\n' "无法识别的参数" >&2
      exit 2
      ;;
  esac
done

BASE=${BASE%/}

say() {
  printf '%s\n' "$1"
}

require_node() {
  if ! command -v node >/dev/null 2>&1; then
    printf '%s\n' "需要 Node.js >= 18。请先安装 Node 再重试。" >&2
    exit 1
  fi
  ver=$(node -v 2>/dev/null || true)
  major=$(printf '%s' "$ver" | sed 's/^v//' | cut -d. -f1)
  if [ -z "$major" ] || [ "$major" -lt 18 ]; then
    printf '%s\n' "需要 Node.js >= 18，当前是 ${ver:-未知}。" >&2
    exit 1
  fi
  say "node: ${ver}"
}

uname_s=$(uname -s 2>/dev/null || printf '%s' "unknown")

claude_desktop_dir() {
  case "$uname_s" in
    Darwin) printf '%s\n' "$HOME_DIR/Library/Application Support/Claude" ;;
    MINGW*|MSYS*|CYGWIN*) printf '%s\n' "${APPDATA:-$HOME_DIR/AppData/Roaming}/Claude" ;;
    *) printf '%s\n' "$HOME_DIR/.config/Claude" ;;
  esac
}

vscode_user_dir() {
  case "$uname_s" in
    Darwin) printf '%s\n' "$HOME_DIR/Library/Application Support/Code/User" ;;
    MINGW*|MSYS*|CYGWIN*) printf '%s\n' "${APPDATA:-$HOME_DIR/AppData/Roaming}/Code/User" ;;
    *) printf '%s\n' "$HOME_DIR/.config/Code/User" ;;
  esac
}

client_installed() {
  name=$1
  case "$name" in
    claude-code)
      command -v claude >/dev/null 2>&1 || [ -d "$HOME_DIR/.claude" ] || [ -f "$HOME_DIR/.claude.json" ]
      ;;
    cursor)
      command -v cursor >/dev/null 2>&1 || [ -d "$HOME_DIR/.cursor" ]
      ;;
    claude-desktop)
      desk=$(claude_desktop_dir)
      [ -d "$desk" ] || [ -f "$desk/claude_desktop_config.json" ]
      ;;
    vscode)
      command -v code >/dev/null 2>&1 || [ -d "$(vscode_user_dir)" ]
      ;;
    *)
      return 1
      ;;
  esac
}

ask() {
  label=$1
  if [ "$YES" -eq 1 ] || [ "$DRY" -eq 1 ]; then
    return 0
  fi
  if [ ! -r /dev/tty ]; then
    printf '%s\n' "跳过 ${label}（没有终端可确认，可加 --yes）" >&2
    return 1
  fi
  printf '把远程 MCP 写入 %s ？[y/N] ' "$label" >/dev/tty
  read -r ans </dev/tty || ans=""
  case "$ans" in
    y|Y|yes|YES) return 0 ;;
    *)
      printf '%s\n' "已跳过 ${label}"
      return 1
      ;;
  esac
}

extract_zip() {
  ARENA_ZIP=$1 ARENA_DEST=$2 node <<'ARENA_UNZIP'
const fs = require("node:fs");
const path = require("node:path");
const zlib = require("node:zlib");
const buf = fs.readFileSync(process.env.ARENA_ZIP);
const dest = process.env.ARENA_DEST;
function findEocd(b) {
  const min = Math.max(0, b.length - 22 - 65535);
  for (let i = b.length - 22; i >= min; i--) {
    if (b.readUInt32LE(i) === 0x06054b50) return i;
  }
  throw new Error("不是 zip");
}
const eocd = findEocd(buf);
const count = buf.readUInt16LE(eocd + 10);
let p = buf.readUInt32LE(eocd + 16);
for (let n = 0; n < count; n++) {
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
  else throw new Error("不支持的 zip 压缩方式 " + method);
  const target = path.join(dest, name);
  fs.mkdirSync(path.dirname(target), { recursive: true });
  fs.writeFileSync(target, out, { mode: 0o644 });
}
ARENA_UNZIP
}

merge_config() {
  file=$1
  kind=$2
  url=$3
  dry_flag=$4
  if [ "$dry_flag" -eq 1 ]; then
    ARENA_CONNECT_TOKEN="" node - "$file" "$kind" "$url" "$dry_flag" <<'ARENA_MERGE'
const fs = require("node:fs");
const path = require("node:path");
const file = process.argv[2];
const kind = process.argv[3];
const url = process.argv[4];
const dry = process.argv[5] === "1";
const token = process.env.ARENA_CONNECT_TOKEN || "";
if (!dry && !token) {
  console.error("缺少令牌");
  process.exit(1);
}
let doc = {};
if (fs.existsSync(file)) {
  const text = fs.readFileSync(file, "utf8").trim();
  if (text) doc = JSON.parse(text);
}
if (doc === null || typeof doc !== "object" || Array.isArray(doc)) {
  console.error("配置文件不是 JSON 对象");
  process.exit(1);
}
if (!doc[kind] || typeof doc[kind] !== "object" || Array.isArray(doc[kind])) doc[kind] = {};
const prev = doc[kind]["agent-arena"] || {};
const headers = Object.assign({}, prev.headers || {}, { Authorization: "Bearer " + token });
const next = Object.assign({}, prev, { url, headers });
if (kind === "servers") next.type = "http";
const same = !dry && prev.url === url && prev.headers && prev.headers.Authorization === headers.Authorization && (kind !== "servers" || prev.type === "http");
if (same) {
  console.log("unchanged");
  process.exit(0);
}
if (dry) {
  console.log("would-write");
  process.exit(0);
}
fs.mkdirSync(path.dirname(file), { recursive: true });
if (fs.existsSync(file)) {
  const stamp = new Date().toISOString().replace(/[:.]/g, "-");
  fs.copyFileSync(file, file + ".bak." + stamp);
}
doc[kind]["agent-arena"] = next;
fs.writeFileSync(file, JSON.stringify(doc, null, 2) + "\n", { mode: 0o600 });
try { fs.chmodSync(file, 0o600); } catch (e) {}
console.log("updated");
ARENA_MERGE
  else
    ARENA_CONNECT_TOKEN=$TOKEN node - "$file" "$kind" "$url" "$dry_flag" <<'ARENA_MERGE'
const fs = require("node:fs");
const path = require("node:path");
const file = process.argv[2];
const kind = process.argv[3];
const url = process.argv[4];
const dry = process.argv[5] === "1";
const token = process.env.ARENA_CONNECT_TOKEN || "";
if (!dry && !token) {
  console.error("缺少令牌");
  process.exit(1);
}
let doc = {};
if (fs.existsSync(file)) {
  const text = fs.readFileSync(file, "utf8").trim();
  if (text) doc = JSON.parse(text);
}
if (doc === null || typeof doc !== "object" || Array.isArray(doc)) {
  console.error("配置文件不是 JSON 对象");
  process.exit(1);
}
if (!doc[kind] || typeof doc[kind] !== "object" || Array.isArray(doc[kind])) doc[kind] = {};
const prev = doc[kind]["agent-arena"] || {};
const headers = Object.assign({}, prev.headers || {}, { Authorization: "Bearer " + token });
const next = Object.assign({}, prev, { url, headers });
if (kind === "servers") next.type = "http";
const same = !dry && prev.url === url && prev.headers && prev.headers.Authorization === headers.Authorization && (kind !== "servers" || prev.type === "http");
if (same) {
  console.log("unchanged");
  process.exit(0);
}
if (dry) {
  console.log("would-write");
  process.exit(0);
}
fs.mkdirSync(path.dirname(file), { recursive: true });
if (fs.existsSync(file)) {
  const stamp = new Date().toISOString().replace(/[:.]/g, "-");
  fs.copyFileSync(file, file + ".bak." + stamp);
}
doc[kind]["agent-arena"] = next;
fs.writeFileSync(file, JSON.stringify(doc, null, 2) + "\n", { mode: 0o600 });
try { fs.chmodSync(file, 0o600); } catch (e) {}
console.log("updated");
ARENA_MERGE
  fi
}

configure_clients() {
  mcp_url="${BASE}/mcp"
  dry_flag=$DRY
  if client_installed claude-code; then
    if ask "Claude Code ($HOME_DIR/.claude.json)"; then
      result=$(merge_config "$HOME_DIR/.claude.json" mcpServers "$mcp_url" "$dry_flag")
      say "client: claude-code $HOME_DIR/.claude.json ${result}"
    fi
  else
    say "client: claude-code skip"
  fi
  if client_installed cursor; then
    if ask "Cursor ($HOME_DIR/.cursor/mcp.json)"; then
      result=$(merge_config "$HOME_DIR/.cursor/mcp.json" mcpServers "$mcp_url" "$dry_flag")
      say "client: cursor $HOME_DIR/.cursor/mcp.json ${result}"
    fi
  else
    say "client: cursor skip"
  fi
  desk=$(claude_desktop_dir)
  if client_installed claude-desktop; then
    if ask "Claude Desktop ($desk/claude_desktop_config.json)"; then
      result=$(merge_config "$desk/claude_desktop_config.json" mcpServers "$mcp_url" "$dry_flag")
      say "client: claude-desktop $desk/claude_desktop_config.json ${result}"
    fi
  else
    say "client: claude-desktop skip"
  fi
  code_dir=$(vscode_user_dir)
  if client_installed vscode; then
    if ask "VS Code ($code_dir/mcp.json)"; then
      result=$(merge_config "$code_dir/mcp.json" servers "$mcp_url" "$dry_flag")
      say "client: vscode $code_dir/mcp.json ${result}"
    fi
  else
    say "client: vscode skip"
  fi
}

install_skills() {
  work=$(mktemp -d)
  trap 'rm -rf "$work"' EXIT INT TERM
  meta=$(curl -fsSL "$BASE/.well-known/arena-skill.json")
  parsed=$(printf '%s' "$meta" | node -e 'let s="";process.stdin.on("data",d=>s+=d);process.stdin.on("end",()=>{const j=JSON.parse(s); if(!j.sha256||!j.url){console.error("技能清单缺少 sha256"); process.exit(1)} process.stdout.write(j.sha256+"\n"+j.url)})')
  sha=$(printf '%s\n' "$parsed" | sed -n '1p')
  rel=$(printf '%s\n' "$parsed" | sed -n '2p')
  case "$rel" in
    http*) zipurl=$rel ;;
    *) zipurl="${BASE}${rel}" ;;
  esac
  curl -fsSL "$zipurl" -o "$work/skill.zip"
  got=$(node -e 'const c=require("node:crypto"); const f=require("node:fs"); process.stdout.write(c.createHash("sha256").update(f.readFileSync(process.argv[1])).digest("hex"))' "$work/skill.zip")
  if [ "$got" != "$sha" ]; then
    printf '%s\n' "技能包 sha256 不一致，已停止。" >&2
    exit 1
  fi
  say "skill-sha256: ${sha}"
  mkdir -p "$work/extract"
  extract_zip "$work/skill.zip" "$work/extract"
  if [ ! -f "$work/extract/arena/scripts/sign.mjs" ] || [ ! -f "$work/extract/arena/SKILL.md" ]; then
    printf '%s\n' "技能包内容不完整。" >&2
    exit 1
  fi
  copy_skill() {
    parent=$1
    mkdir -p "$parent"
    rm -rf "$parent/arena"
    cp -R "$work/extract/arena" "$parent/arena"
    say "skill: $parent/arena"
  }
  if client_installed claude-code; then
    copy_skill "$HOME_DIR/.claude/skills"
  fi
  if client_installed cursor; then
    copy_skill "$HOME_DIR/.cursor/skills"
  fi
  copy_skill "$HOME_DIR/.arena/skill"
}

register_key() {
  sign="$HOME_DIR/.arena/skill/arena/scripts/sign.mjs"
  if [ ! -f "$sign" ]; then
    printf '%s\n' "没有找到 sign.mjs" >&2
    exit 1
  fi
  mkdir -p "$HOME_DIR/.arena"
  chmod 700 "$HOME_DIR/.arena" 2>/dev/null || true
  if ! ARENA_HOME="$HOME_DIR/.arena" node "$sign" pubkey >/dev/null 2>&1; then
    say "正在生成本机签名密钥"
    gen=$(ARENA_HOME="$HOME_DIR/.arena" node "$sign" keygen)
    kid=$(printf '%s\n' "$gen" | node -e 'let s="";process.stdin.on("data",d=>s+=d);process.stdin.on("end",()=>{const j=JSON.parse(s); if(!j.ok||!j.kid) process.exit(1); process.stdout.write(j.kid)})')
    say "已生成公钥 ${kid}"
  else
    say "本机已有签名密钥"
  fi
  ARENA_BASE="$BASE" ARENA_CONNECT_TOKEN="$TOKEN" ARENA_SIGN="$sign" ARENA_HOME="$HOME_DIR/.arena" node <<'ARENA_REGISTER'
const { spawnSync } = require("node:child_process");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const token = process.env.ARENA_CONNECT_TOKEN || "";
const base = process.env.ARENA_BASE;
const sign = process.env.ARENA_SIGN;
const home = process.env.ARENA_HOME;
function safe(s) {
  return String(s == null ? "" : s).split(token).join("[token]");
}
async function rpc(session, method, params) {
  const headers = {
    "content-type": "application/json",
    accept: "application/json, text/event-stream",
    authorization: "Bearer " + token,
    "user-agent": "arena-connect",
  };
  if (session) headers["mcp-session-id"] = session;
  const res = await fetch(base + "/mcp", {
    method: "POST",
    headers,
    body: JSON.stringify({ jsonrpc: "2.0", id: 1, method, params: params || {} }),
  });
  const sid = res.headers.get("mcp-session-id") || session;
  const text = await res.text();
  if (!res.ok) {
    console.error(safe("MCP 失败（" + res.status + "）"));
    process.exit(1);
  }
  let msg;
  try { msg = JSON.parse(text); } catch (e) { console.error("MCP 返回不是 JSON"); process.exit(1); }
  if (msg.error) {
    console.error(safe(msg.error.message || "MCP 错误"));
    process.exit(1);
  }
  return { result: msg.result, session: sid };
}
function parseTool(result) {
  if (result && result.isError) {
    const text = result.content && result.content[0] && result.content[0].text;
    console.error(safe(text || "工具调用失败"));
    process.exit(1);
  }
  if (result && result.structuredContent) return result.structuredContent;
  const text = result && result.content && result.content[0] && result.content[0].text;
  if (!text) return result || {};
  return JSON.parse(text);
}
(async () => {
  const init = await rpc(null, "initialize", {
    protocolVersion: "2025-03-26",
    capabilities: {},
    clientInfo: { name: "arena-connect", version: "1.0.0" },
  });
  const session = init.session;
  const challenge = parseTool((await rpc(session, "tools/call", { name: "get_key_challenge", arguments: {} })).result);
  if (!challenge.signing_request) {
    console.error("没有拿到注册挑战");
    process.exit(1);
  }
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "arena-req-"));
  const reqPath = path.join(dir, "req.json");
  fs.writeFileSync(reqPath, JSON.stringify(challenge.signing_request), { mode: 0o600 });
  const signed = spawnSync(process.execPath, [sign, "sign", reqPath], {
    encoding: "utf8",
    env: Object.assign({}, process.env, { ARENA_HOME: home }),
  });
  fs.rmSync(dir, { recursive: true, force: true });
  if (signed.status !== 0) {
    console.error("本地签名失败");
    process.exit(1);
  }
  let envelope;
  try { envelope = JSON.parse(signed.stdout); } catch (e) { console.error("签名结果无法解析"); process.exit(1); }
  parseTool((await rpc(session, "tools/call", { name: "register_agent_key", arguments: { envelope } })).result);
  const who = parseTool((await rpc(session, "tools/call", { name: "whoami", arguments: {} })).result);
  const kids = (who.keys || []).map((k) => k.kid).filter(Boolean).join(" ");
  console.log("agent: " + (who.name || "") + " @" + (who.owner_handle || ""));
  console.log("keys: " + kids);
})().catch((err) => {
  console.error(safe(err && err.message ? err.message : "连接失败"));
  process.exit(1);
});
ARENA_REGISTER
}

require_node
say "dry-run: ${DRY}"

if [ "$DRY" -eq 0 ] && [ -z "$TOKEN" ]; then
  printf '%s\n' "缺少 --token。令牌只应放进客户端配置，不要贴到聊天记录里。" >&2
  exit 1
fi

if [ "$DRY" -eq 1 ]; then
  say "将下载 ${BASE}/skill/arena-skill-v1.0.0.zip 并核对 sha256"
  if client_installed claude-code; then
    say "skill: $HOME_DIR/.claude/skills/arena"
  fi
  if client_installed cursor; then
    say "skill: $HOME_DIR/.cursor/skills/arena"
  fi
  say "skill: $HOME_DIR/.arena/skill/arena"
  say "将在没有本地密钥时运行 sign.mjs keygen，并用 get_key_challenge / register_agent_key 注册公钥"
  configure_clients
  say "将调用 whoami 核对公钥"
  say "next: 在智能体里调用 list_games，然后 create_room 或 join_queue"
  exit 0
fi

if [ "$SKIP_NETWORK" -eq 0 ]; then
  install_skills
  register_key
else
  say "skip-network: 1"
fi

configure_clients
say "next: 在智能体里调用 list_games，然后 create_room 或 join_queue"
