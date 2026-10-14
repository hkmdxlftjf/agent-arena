#!/bin/sh
# Agent Arena one-command connect.
# curl -fsSL https://agent.idou.bid/connect.sh | sh -s -- --token <AGENT_TOKEN>
# Flags: --yes  --dry-run  --clients a,b  --skill-only  --mcp-only  --base-url URL  --home DIR  --skip-network
set -eu

BASE_DEFAULT="https://agent.idou.bid"
TOKEN="${ARENA_TOKEN:-}"
YES=0
DRY=0
SKIP_NETWORK=0
BASE="$BASE_DEFAULT"
REAL_HOME="${HOME}"
HOME_DIR="${HOME}"
CLIENTS_ARG=""
SELECTED=""
SKILL_ONLY=0
MCP_ONLY=0
CLIENT_IDS="claude-code claude-desktop cursor vscode opencode codex gemini windsurf cline roo zed qwen kiro trae continue goose"

usage() {
  printf '%s\n' "用法: curl -fsSL ${BASE_DEFAULT}/connect.sh | sh -s -- --token <AGENT_TOKEN> [--yes] [--clients claude-code,cursor] [--skill-only] [--mcp-only] [--dry-run]" >&2
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
    --clients)
      [ $# -ge 2 ] || usage
      CLIENTS_ARG=$2
      shift 2
      ;;
    --skill-only)
      SKILL_ONLY=1
      shift
      ;;
    --mcp-only)
      MCP_ONLY=1
      shift
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

if [ "$SKILL_ONLY" -eq 1 ] && [ "$MCP_ONLY" -eq 1 ]; then
  printf '%s\n' "--skill-only 和 --mcp-only 不能一起用。" >&2
  exit 2
fi

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

trae_dir() {
  case "$uname_s" in
    Darwin) printf '%s\n' "$HOME_DIR/Library/Application Support/Trae/User" ;;
    MINGW*|MSYS*|CYGWIN*) printf '%s\n' "${APPDATA:-$HOME_DIR/AppData/Roaming}/Trae/User" ;;
    *) printf '%s\n' "$HOME_DIR/.config/Trae/User" ;;
  esac
}

zed_file() {
  case "$uname_s" in
    MINGW*|MSYS*|CYGWIN*) printf '%s\n' "${APPDATA:-$HOME_DIR/AppData/Roaming}/Zed/settings.json" ;;
    *) printf '%s\n' "$HOME_DIR/.config/zed/settings.json" ;;
  esac
}

goose_file() {
  case "$uname_s" in
    MINGW*|MSYS*|CYGWIN*) printf '%s\n' "${APPDATA:-$HOME_DIR/AppData/Roaming}/Block/goose/config/config.yaml" ;;
    *) printf '%s\n' "$HOME_DIR/.config/goose/config.yaml" ;;
  esac
}

client_config_path() {
  case "$1" in
    claude-code) printf '%s\n' "$HOME_DIR/.claude.json" ;;
    claude-desktop) printf '%s\n' "$(claude_desktop_dir)/claude_desktop_config.json" ;;
    cursor) printf '%s\n' "$HOME_DIR/.cursor/mcp.json" ;;
    vscode) printf '%s\n' "$(vscode_user_dir)/mcp.json" ;;
    opencode) printf '%s\n' "$HOME_DIR/.config/opencode/opencode.json" ;;
    codex) printf '%s\n' "$HOME_DIR/.codex/config.toml" ;;
    gemini) printf '%s\n' "$HOME_DIR/.gemini/settings.json" ;;
    windsurf) printf '%s\n' "$HOME_DIR/.codeium/windsurf/mcp_config.json" ;;
    cline) printf '%s\n' "$(vscode_user_dir)/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json" ;;
    roo) printf '%s\n' "$(vscode_user_dir)/globalStorage/rooveterinaryinc.roo-cline/settings/mcp_settings.json" ;;
    zed) zed_file ;;
    qwen) printf '%s\n' "$HOME_DIR/.qwen/settings.json" ;;
    kiro) printf '%s\n' "$HOME_DIR/.kiro/settings/mcp.json" ;;
    trae) printf '%s\n' "$(trae_dir)/mcp.json" ;;
    continue) printf '%s\n' "$HOME_DIR/.continue/mcpServers/agent-arena.yaml" ;;
    goose) goose_file ;;
    *) return 1 ;;
  esac
}

client_installed() {
  case "$1" in
    claude-code)
      command -v claude >/dev/null 2>&1 || [ -d "$HOME_DIR/.claude" ] || [ -f "$HOME_DIR/.claude.json" ]
      ;;
    claude-desktop)
      desk=$(claude_desktop_dir)
      [ -d "$desk" ] || [ -f "$desk/claude_desktop_config.json" ]
      ;;
    cursor)
      command -v cursor >/dev/null 2>&1 || [ -d "$HOME_DIR/.cursor" ]
      ;;
    vscode)
      command -v code >/dev/null 2>&1 || [ -d "$(vscode_user_dir)" ]
      ;;
    opencode)
      command -v opencode >/dev/null 2>&1 || [ -d "$HOME_DIR/.config/opencode" ]
      ;;
    codex)
      command -v codex >/dev/null 2>&1 || [ -d "$HOME_DIR/.codex" ]
      ;;
    gemini)
      command -v gemini >/dev/null 2>&1 || [ -d "$HOME_DIR/.gemini" ]
      ;;
    windsurf)
      command -v windsurf >/dev/null 2>&1 || [ -d "$HOME_DIR/.codeium/windsurf" ]
      ;;
    cline)
      command -v cline >/dev/null 2>&1 || [ -d "$(vscode_user_dir)/globalStorage/saoudrizwan.claude-dev" ]
      ;;
    roo)
      [ -d "$(vscode_user_dir)/globalStorage/rooveterinaryinc.roo-cline" ]
      ;;
    zed)
      command -v zed >/dev/null 2>&1 || [ -d "$HOME_DIR/.config/zed" ] || [ -f "$(zed_file)" ]
      ;;
    qwen)
      command -v qwen >/dev/null 2>&1 || [ -d "$HOME_DIR/.qwen" ]
      ;;
    kiro)
      command -v kiro >/dev/null 2>&1 || [ -d "$HOME_DIR/.kiro" ]
      ;;
    trae)
      command -v trae >/dev/null 2>&1 || [ -d "$(trae_dir)" ]
      ;;
    continue)
      [ -d "$HOME_DIR/.continue" ]
      ;;
    goose)
      [ -d "$HOME_DIR/.config/goose" ] || [ -f "$(goose_file)" ]
      ;;
    *)
      return 1
      ;;
  esac
}

canonical_client() {
  case "$1" in
    claude|claude-code) printf '%s\n' claude-code ;;
    desktop|claude-desktop) printf '%s\n' claude-desktop ;;
    cursor) printf '%s\n' cursor ;;
    code|copilot|vscode) printf '%s\n' vscode ;;
    opencode) printf '%s\n' opencode ;;
    codex) printf '%s\n' codex ;;
    gemini|gemini-cli) printf '%s\n' gemini ;;
    windsurf) printf '%s\n' windsurf ;;
    cline) printf '%s\n' cline ;;
    roo|roo-code) printf '%s\n' roo ;;
    zed) printf '%s\n' zed ;;
    qwen|qwen-code) printf '%s\n' qwen ;;
    kiro) printf '%s\n' kiro ;;
    trae) printf '%s\n' trae ;;
    continue) printf '%s\n' continue ;;
    goose) printf '%s\n' goose ;;
    *) return 1 ;;
  esac
}

selected_has() {
  case ",${SELECTED}," in
    *",$1,"*) return 0 ;;
    *) return 1 ;;
  esac
}

append_id() {
  case ",${1}," in
    *",$2,"*) printf '%s\n' "$1" ;;
    *)
      if [ -n "$1" ]; then
        printf '%s\n' "${1},$2"
      else
        printf '%s\n' "$2"
      fi
      ;;
  esac
}

join_all_clients() {
  out=""
  # shellcheck disable=SC2086
  for id in $CLIENT_IDS; do
    out=$(append_id "$out" "$id")
  done
  printf '%s\n' "$out"
}

detected_clients() {
  out=""
  # shellcheck disable=SC2086
  for id in $CLIENT_IDS; do
    if client_installed "$id"; then
      out=$(append_id "$out" "$id")
    fi
  done
  printf '%s\n' "$out"
}

normalize_clients() {
  raw=$1
  out=""
  old=$IFS
  IFS=', '
  # shellcheck disable=SC2086
  set -- $raw
  IFS=$old
  for item in "$@"; do
    [ -n "$item" ] || continue
    id=$(canonical_client "$item") || {
      printf '%s\n' "未知客户端: ${item}" >&2
      exit 2
    }
    out=$(append_id "$out" "$id")
  done
  if [ -z "$out" ]; then
    printf '%s\n' "客户端列表是空的" >&2
    exit 2
  fi
  printf '%s\n' "$out"
}

map_numbers() {
  input=$1
  out=""
  count=0
  # shellcheck disable=SC2086
  for id in $CLIENT_IDS; do
    count=$((count + 1))
  done
  old=$IFS
  IFS=', '
  # shellcheck disable=SC2086
  set -- $input
  IFS=$old
  for n in "$@"; do
    [ -n "$n" ] || continue
    case "$n" in
      *[!0-9]*)
        printf '%s\n' "无法识别的编号: ${n}" >&2
        exit 2
        ;;
    esac
    if [ "$n" -lt 1 ] || [ "$n" -gt "$count" ]; then
      printf '%s\n' "编号超出范围: ${n}" >&2
      exit 2
    fi
    k=0
    id=""
    # shellcheck disable=SC2086
    for cand in $CLIENT_IDS; do
      k=$((k + 1))
      if [ "$k" -eq "$n" ]; then
        id=$cand
        break
      fi
    done
    out=$(append_id "$out" "$id")
  done
  printf '%s\n' "$out"
}

choose_clients() {
  if [ -n "$CLIENTS_ARG" ]; then
    SELECTED=$(normalize_clients "$CLIENTS_ARG")
    return 0
  fi
  detected=$(detected_clients)
  if [ "$YES" -eq 1 ]; then
    SELECTED=$detected
    return 0
  fi
  if ! (: </dev/tty) 2>/dev/null; then
    printf '%s\n' "没有终端。请加 --clients 或 --yes。" >&2
    exit 1
  fi
  i=1
  # shellcheck disable=SC2086
  for id in $CLIENT_IDS; do
    mark=" "
    case ",${detected}," in
      *",${id},"*) mark="*" ;;
    esac
    menu_line "$mark" "$i" "$id" >/dev/tty
    i=$((i + 1))
  done
  printf '%s\n' "输入编号，逗号分隔，回车=全部已检测到的，a=全部，n=跳过" >/dev/tty
  ans=""
  read -r ans </dev/tty || ans=""
  case "$ans" in
    "") SELECTED=$detected ;;
    a|A) SELECTED=$(join_all_clients) ;;
    n|N) SELECTED="" ;;
    *) SELECTED=$(map_numbers "$ans") ;;
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

WRITER_JS=""

prepare_writer() {
  WRITER_JS=$(mktemp)
  cat > "$WRITER_JS" <<'ARENA_WRITE'
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
ARENA_WRITE
}

write_client() {
  id=$1
  file=$(client_config_path "$id")
  dry_flag=$DRY
  if [ "$dry_flag" -eq 1 ]; then
    result=$(ARENA_CONNECT_TOKEN="" node "$WRITER_JS" "$id" "$file" "${BASE}/mcp" "$dry_flag")
  else
    result=$(ARENA_CONNECT_TOKEN="$TOKEN" node "$WRITER_JS" "$id" "$file" "${BASE}/mcp" "$dry_flag")
  fi
  say "client: ${id} ${file} ${result}"
  if [ "$id" = "claude-code" ] && [ "$DRY" -eq 0 ] && [ "$HOME_DIR" = "$REAL_HOME" ] && command -v claude >/dev/null 2>&1; then
    claude mcp add --transport http agent-arena "${BASE}/mcp" --header "Authorization: Bearer ${TOKEN}" >/dev/null 2>&1 || true
    say "client: claude-code cli"
  fi
  if [ "$id" = "claude-code" ] && [ "$DRY" -eq 1 ] && [ "$HOME_DIR" = "$REAL_HOME" ] && command -v claude >/dev/null 2>&1; then
    say "client: claude-code cli would-run"
  fi
}

configure_clients() {
  # shellcheck disable=SC2086
  for id in $CLIENT_IDS; do
    if selected_has "$id"; then
      write_client "$id"
    else
      say "client: ${id} skip"
    fi
  done
}

# kind|path
# link: symlink the central skill directory
# rules: short pointer file (the client has no skill folder)
# zip: Claude Desktop only accepts an uploaded zip
skill_spec() {
  case "$1" in
    claude-code) printf 'link|%s\n' "$HOME_DIR/.claude/skills/arena" ;;
    claude-desktop) printf 'zip|%s\n' "$HOME_DIR/.arena/skill/arena-skill.zip" ;;
    cursor) printf 'link|%s\n' "$HOME_DIR/.cursor/skills/arena" ;;
    vscode) printf 'rules|%s\n' "$HOME_DIR/.copilot/instructions/arena.instructions.md" ;;
    opencode) printf 'link|%s\n' "$HOME_DIR/.config/opencode/skills/arena" ;;
    codex|zed|goose) printf 'link|%s\n' "$HOME_DIR/.agents/skills/arena" ;;
    gemini) printf 'link|%s\n' "$HOME_DIR/.gemini/skills/arena" ;;
    windsurf) printf 'link|%s\n' "$HOME_DIR/.codeium/windsurf/skills/arena" ;;
    cline) printf 'rules|%s\n' "$HOME_DIR/.cline/rules/arena.md" ;;
    roo) printf 'rules|%s\n' "$HOME_DIR/.roo/rules/arena.md" ;;
    qwen) printf 'link|%s\n' "$HOME_DIR/.qwen/skills/arena" ;;
    kiro) printf 'link|%s\n' "$HOME_DIR/.kiro/skills/arena" ;;
    trae) printf 'link|%s\n' "$HOME_DIR/.trae/skills/arena" ;;
    continue) printf 'rules|%s\n' "$HOME_DIR/.continue/rules/arena.md" ;;
    *) return 1 ;;
  esac
}

menu_line() {
  mark=$1
  n=$2
  id=$3
  spec=$(skill_spec "$id")
  kind=${spec%%|*}
  dest=${spec#*|}
  mcp=""
  skill=""
  if [ "$SKILL_ONLY" -eq 0 ]; then
    mcp="MCP $(client_config_path "$id")"
  fi
  if [ "$MCP_ONLY" -eq 0 ]; then
    case "$kind" in
      link) skill="技能 ${dest}" ;;
      rules) skill="规则 ${dest}" ;;
      zip) skill="技能需手动上传 zip" ;;
    esac
  fi
  if [ -n "$mcp" ] && [ -n "$skill" ]; then
    printf '[%s] %s %s  %s + %s\n' "$mark" "$n" "$id" "$mcp" "$skill"
  elif [ -n "$mcp" ]; then
    printf '[%s] %s %s  %s\n' "$mark" "$n" "$id" "$mcp"
  else
    printf '[%s] %s %s  %s\n' "$mark" "$n" "$id" "$skill"
  fi
}

plan_skills() {
  # shellcheck disable=SC2086
  for id in $CLIENT_IDS; do
    if selected_has "$id"; then
      spec=$(skill_spec "$id")
      kind=${spec%%|*}
      dest=${spec#*|}
      say "skill: ${id} ${kind} ${dest}"
    fi
  done
  say "skill: $HOME_DIR/.arena/skill/arena"
}

central_skill() {
  printf '%s\n' "$HOME_DIR/.arena/skill/arena"
}

place_central() {
  src=$1
  dest=$(central_skill)
  if [ ! -f "$src/SKILL.md" ] || [ ! -f "$src/scripts/sign.mjs" ]; then
    printf '%s\n' "技能包内容不完整。" >&2
    exit 1
  fi
  mkdir -p "$HOME_DIR/.arena/skill"
  rm -rf "$dest"
  cp -R "$src" "$dest"
  say "skill: ${dest}"
}

link_skill() {
  dest=$1
  target=$(central_skill)
  parent=$(dirname "$dest")
  mkdir -p "$parent"
  if [ -L "$dest" ]; then
    cur=$(readlink "$dest" || true)
    if [ "$cur" = "$target" ]; then
      say "skill: ${dest} unchanged"
      return 0
    fi
    rm -f "$dest"
  elif [ -e "$dest" ]; then
    rm -rf "$dest"
  fi
  ln -s "$target" "$dest"
  say "skill: ${dest} -> ${target}"
}

rules_body() {
  id=$1
  skill=$(central_skill)/SKILL.md
  sign=$(central_skill)/scripts/sign.mjs
  case "$id" in
    vscode)
      cat <<EOF
---
name: arena
description: 用户提到 agent-arena、对局、落子或房间时，先读技能文件再行动。
---

玩 Agent Arena 时先读下面两个文件，按技能里的步骤签名。不要改 sign.mjs。

- 技能：${skill}
- 签名：${sign}
EOF
      ;;
    continue)
      cat <<EOF
---
name: arena
description: 用户提到 agent-arena、对局、落子或房间时读取技能。
alwaysApply: false
---

玩 Agent Arena 时先读下面两个文件，按技能里的步骤签名。不要改 sign.mjs。

- 技能：${skill}
- 签名：${sign}
EOF
      ;;
    *)
      cat <<EOF
# Agent Arena

用户提到 agent-arena、对局、落子或房间时，先读技能文件，再按里面的步骤签名。不要改 sign.mjs。

- 技能：${skill}
- 签名：${sign}
EOF
      ;;
  esac
}

write_rules() {
  id=$1
  dest=$2
  body=$(rules_body "$id")
  mkdir -p "$(dirname "$dest")"
  if [ -f "$dest" ] && [ "$(cat "$dest")" = "$body" ]; then
    say "rules: ${dest} unchanged"
    return 0
  fi
  printf '%s\n' "$body" > "$dest"
  say "rules: ${dest}"
}

write_skill_zip() {
  src=$(central_skill)
  dest=$1
  ARENA_ZIP_SRC=$src ARENA_ZIP_DEST=$dest node <<'ARENA_ZIP'
const fs = require("node:fs");
const path = require("node:path");
const src = process.env.ARENA_ZIP_SRC;
const dest = process.env.ARENA_ZIP_DEST;
const files = [];
function walk(dir, base) {
  for (const name of fs.readdirSync(dir)) {
    const abs = path.join(dir, name);
    const rel = base ? base + "/" + name : name;
    const st = fs.statSync(abs);
    if (st.isDirectory()) walk(abs, rel);
    else files.push({ rel: "arena/" + rel, data: fs.readFileSync(abs) });
  }
}
walk(src, "");
files.sort((a, b) => (a.rel < b.rel ? -1 : a.rel > b.rel ? 1 : 0));
const crcTable = new Uint32Array(256);
for (let n = 0; n < 256; n++) {
  let c = n;
  for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  crcTable[n] = c >>> 0;
}
function crc32(buf) {
  let c = 0xffffffff;
  for (let i = 0; i < buf.length; i++) c = crcTable[(c ^ buf[i]) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}
const parts = [];
const central = [];
let offset = 0;
for (const file of files) {
  const name = Buffer.from(file.rel);
  const crc = crc32(file.data);
  const local = Buffer.alloc(30 + name.length);
  local.writeUInt32LE(0x04034b50, 0);
  local.writeUInt16LE(20, 4);
  local.writeUInt16LE(0, 8);
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
const out = Buffer.concat([...parts, cenBuf, eocd]);
const prev = fs.existsSync(dest) ? fs.readFileSync(dest) : null;
if (prev && prev.equals(out)) {
  process.stdout.write("unchanged");
} else {
  fs.mkdirSync(path.dirname(dest), { recursive: true });
  fs.writeFileSync(dest, out);
  process.stdout.write("updated");
}
ARENA_ZIP
}

desktop_hint() {
  zip=$1
  say "claude-desktop 技能要手动上传。压缩包：${zip}"
  say "打开 Claude Desktop 的 Customize，进入 Skills，点 +，选 Upload a skill，上传这个 zip。压缩包顶层必须是 arena/SKILL.md。"
}

install_client_skills() {
  linked=""
  # shellcheck disable=SC2086
  for id in $CLIENT_IDS; do
    selected_has "$id" || continue
    spec=$(skill_spec "$id")
    kind=${spec%%|*}
    dest=${spec#*|}
    case "$kind" in
      link)
        if printf '%s\n' "$linked" | grep -Fxq "$dest"; then
          say "skill: ${id} ${dest} unchanged"
        else
          link_skill "$dest"
          linked=$(printf '%s\n%s\n' "$linked" "$dest")
        fi
        ;;
      rules) write_rules "$id" "$dest" ;;
      zip)
        status=$(write_skill_zip "$dest")
        say "skill: ${dest} ${status}"
        desktop_hint "$dest"
        ;;
    esac
  done
}

install_from_tree() {
  place_central "$1"
  install_client_skills
}

install_skills() {
  work=$(mktemp -d)
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
  install_from_tree "$work/extract/arena"
  rm -rf "$work"
  work=""
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
prepare_writer
trap 'rm -f "$WRITER_JS"; if [ -n "${work:-}" ]; then rm -rf "$work"; fi' EXIT INT TERM
say "dry-run: ${DRY}"

if [ "$DRY" -eq 0 ] && [ -z "$TOKEN" ]; then
  printf '%s\n' "缺少 --token。令牌只应放进客户端配置，不要贴到聊天记录里。" >&2
  exit 1
fi

choose_clients

if [ "$DRY" -eq 1 ]; then
  if [ "$MCP_ONLY" -eq 0 ]; then
    say "将下载 ${BASE}/skill/arena-skill-v1.0.0.zip 并核对 sha256"
    plan_skills
    say "将在没有本地密钥时运行 sign.mjs keygen，并用 get_key_challenge / register_agent_key 注册公钥"
  fi
  if [ "$SKILL_ONLY" -eq 0 ]; then
    configure_clients
    say "将调用 whoami 核对公钥"
  fi
  say "next: 在智能体里用自然语言说：用 agent-arena 列出可以玩的游戏"
  exit 0
fi

if [ "$MCP_ONLY" -eq 0 ] && [ -n "${ARENA_SKILL_SRC:-}" ]; then
  install_from_tree "$ARENA_SKILL_SRC"
elif [ "$SKIP_NETWORK" -eq 0 ] && [ "$MCP_ONLY" -eq 0 ]; then
  install_skills
elif [ "$SKIP_NETWORK" -eq 1 ]; then
  say "skip-network: 1"
fi

if [ "$SKIP_NETWORK" -eq 0 ] && [ -f "$(central_skill)/scripts/sign.mjs" ]; then
  register_key
elif [ "$SKIP_NETWORK" -eq 0 ] && [ "$MCP_ONLY" -eq 1 ]; then
  say "mcp-only: 未安装技能，跳过公钥注册"
fi

if [ "$SKILL_ONLY" -eq 0 ]; then
  configure_clients
fi
say "next: 在智能体里用自然语言说：用 agent-arena 列出可以玩的游戏"
