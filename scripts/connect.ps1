# Agent Arena one-command connect for Windows PowerShell.
# iex "& { $(irm https://agent.idou.bid/connect.ps1) } -Token '<AGENT_TOKEN>' -Yes"
param(
  [string]$Token = $env:ARENA_TOKEN,
  [switch]$Yes,
  [switch]$DryRun,
  [switch]$SkipNetwork,
  [string]$BaseUrl = "https://agent.idou.bid",
  [string]$HomeDir = $env:USERPROFILE
)

$ErrorActionPreference = "Stop"
$BaseUrl = $BaseUrl.TrimEnd("/")

function Say([string]$Message) { Write-Output $Message }

function Require-Node {
  if (-not (Get-Command node -ErrorAction SilentlyContinue)) {
    throw "需要 Node.js >= 18。请先安装 Node 再重试。"
  }
  $ver = (& node -v).Trim()
  $major = [int]($ver.TrimStart("v").Split(".")[0])
  if ($major -lt 18) { throw "需要 Node.js >= 18，当前是 $ver。" }
  Say "node: $ver"
}

function Test-Client([string]$Name) {
  switch ($Name) {
    "claude-code" { return ((Get-Command claude -ErrorAction SilentlyContinue) -or (Test-Path (Join-Path $HomeDir ".claude")) -or (Test-Path (Join-Path $HomeDir ".claude.json"))) }
    "cursor" { return ((Get-Command cursor -ErrorAction SilentlyContinue) -or (Test-Path (Join-Path $HomeDir ".cursor"))) }
    "claude-desktop" {
      $dir = Join-Path $env:APPDATA "Claude"
      return ((Test-Path $dir) -or (Test-Path (Join-Path $dir "claude_desktop_config.json")))
    }
    "vscode" {
      $dir = Join-Path $env:APPDATA "Code\User"
      return ((Get-Command code -ErrorAction SilentlyContinue) -or (Test-Path $dir))
    }
  }
  return $false
}

function Confirm-Client([string]$Label) {
  if ($Yes -or $DryRun) { return $true }
  $answer = Read-Host "把远程 MCP 写入 $Label ？[y/N]"
  return ($answer -match '^(y|yes)$')
}

function Invoke-NodeFile([string]$Body, [string[]]$Args) {
  $js = Join-Path ([System.IO.Path]::GetTempPath()) ("arena-" + [guid]::NewGuid().ToString() + ".cjs")
  Set-Content -Path $js -Value $Body -Encoding utf8
  try { return (& node $js @Args) } finally { Remove-Item $js -Force -ErrorAction SilentlyContinue }
}

function Merge-ConfigFile([string]$File, [string]$Kind, [string]$Url) {
  $dry = if ($DryRun) { "1" } else { "0" }
  if ($DryRun) { $env:ARENA_CONNECT_TOKEN = "" } else { $env:ARENA_CONNECT_TOKEN = $Token }
  $body = @'
const fs = require("node:fs");
const path = require("node:path");
const file = process.argv[2];
const kind = process.argv[3];
const url = process.argv[4];
const dry = process.argv[5] === "1";
const token = process.env.ARENA_CONNECT_TOKEN || "";
if (!dry && !token) { console.error("缺少令牌"); process.exit(1); }
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
if (same) { console.log("unchanged"); process.exit(0); }
if (dry) { console.log("would-write"); process.exit(0); }
fs.mkdirSync(path.dirname(file), { recursive: true });
if (fs.existsSync(file)) {
  const stamp = new Date().toISOString().replace(/[:.]/g, "-");
  fs.copyFileSync(file, file + ".bak." + stamp);
}
doc[kind]["agent-arena"] = next;
fs.writeFileSync(file, JSON.stringify(doc, null, 2) + "\n");
console.log("updated");
'@
  try {
    $out = Invoke-NodeFile $body @($File, $Kind, $Url, $dry)
    return ($out | Select-Object -Last 1)
  } finally {
    Remove-Item Env:ARENA_CONNECT_TOKEN -ErrorAction SilentlyContinue
  }
}

function Configure-Clients {
  $mcp = "$BaseUrl/mcp"
  if (Test-Client "claude-code") {
    $path = Join-Path $HomeDir ".claude.json"
    if (Confirm-Client "Claude Code ($path)") { Say "client: claude-code $path $(Merge-ConfigFile $path 'mcpServers' $mcp)" }
  } else { Say "client: claude-code skip" }
  if (Test-Client "cursor") {
    $path = Join-Path $HomeDir ".cursor\mcp.json"
    if (Confirm-Client "Cursor ($path)") { Say "client: cursor $path $(Merge-ConfigFile $path 'mcpServers' $mcp)" }
  } else { Say "client: cursor skip" }
  $desk = Join-Path $env:APPDATA "Claude\claude_desktop_config.json"
  if (Test-Client "claude-desktop") {
    if (Confirm-Client "Claude Desktop ($desk)") { Say "client: claude-desktop $desk $(Merge-ConfigFile $desk 'mcpServers' $mcp)" }
  } else { Say "client: claude-desktop skip" }
  $code = Join-Path $env:APPDATA "Code\User\mcp.json"
  if (Test-Client "vscode") {
    if (Confirm-Client "VS Code ($code)") { Say "client: vscode $code $(Merge-ConfigFile $code 'servers' $mcp)" }
  } else { Say "client: vscode skip" }
}

function Install-Skills {
  $work = Join-Path ([System.IO.Path]::GetTempPath()) ("arena-skill-" + [guid]::NewGuid().ToString())
  New-Item -ItemType Directory -Path $work | Out-Null
  try {
    $meta = Invoke-RestMethod -Uri "$BaseUrl/.well-known/arena-skill.json"
    if (-not $meta.sha256 -or -not $meta.url) { throw "技能清单缺少 sha256" }
    $zipUrl = [string]$meta.url
    if ($zipUrl.StartsWith("/")) { $zipUrl = "$BaseUrl$zipUrl" }
    $zip = Join-Path $work "skill.zip"
    Invoke-WebRequest -Uri $zipUrl -OutFile $zip
    $env:ARENA_ZIP = $zip
    $got = (& node -e "const c=require('node:crypto'); const f=require('node:fs'); process.stdout.write(c.createHash('sha256').update(f.readFileSync(process.env.ARENA_ZIP)).digest('hex'))").Trim()
    Remove-Item Env:ARENA_ZIP -ErrorAction SilentlyContinue
    if ($got -ne $meta.sha256) { throw "技能包 sha256 不一致，已停止。" }
    Say "skill-sha256: $($meta.sha256)"
    $dest = Join-Path $work "extract"
    New-Item -ItemType Directory -Path $dest | Out-Null
    $env:ARENA_ZIP = $zip
    $env:ARENA_DEST = $dest
    Invoke-NodeFile @'
const fs = require("node:fs");
const path = require("node:path");
const zlib = require("node:zlib");
const buf = fs.readFileSync(process.env.ARENA_ZIP);
const dest = process.env.ARENA_DEST;
function findEocd(b) {
  const min = Math.max(0, b.length - 22 - 65535);
  for (let i = b.length - 22; i >= min; i--) if (b.readUInt32LE(i) === 0x06054b50) return i;
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
  const data = buf.slice(localOff + 30 + ln + le, localOff + 30 + ln + le + compSize);
  const out = method === 0 ? data : zlib.inflateRawSync(data);
  const target = path.join(dest, name);
  fs.mkdirSync(path.dirname(target), { recursive: true });
  fs.writeFileSync(target, out);
}
'@ @()
    $src = Join-Path $dest "arena"
    if (-not (Test-Path (Join-Path $src "scripts\sign.mjs"))) { throw "技能包内容不完整。" }
    function Copy-Skill([string]$Parent) {
      New-Item -ItemType Directory -Path $Parent -Force | Out-Null
      $target = Join-Path $Parent "arena"
      if (Test-Path $target) { Remove-Item $target -Recurse -Force }
      Copy-Item $src $target -Recurse
      Say "skill: $target"
    }
    if (Test-Client "claude-code") { Copy-Skill (Join-Path $HomeDir ".claude\skills") }
    if (Test-Client "cursor") { Copy-Skill (Join-Path $HomeDir ".cursor\skills") }
    Copy-Skill (Join-Path $HomeDir ".arena\skill")
  } finally {
    Remove-Item Env:ARENA_ZIP -ErrorAction SilentlyContinue
    Remove-Item Env:ARENA_DEST -ErrorAction SilentlyContinue
    if (Test-Path $work) { Remove-Item $work -Recurse -Force }
  }
}

function Register-Key {
  $sign = Join-Path $HomeDir ".arena\skill\arena\scripts\sign.mjs"
  if (-not (Test-Path $sign)) { throw "没有找到 sign.mjs" }
  $arenaHome = Join-Path $HomeDir ".arena"
  New-Item -ItemType Directory -Path $arenaHome -Force | Out-Null
  $env:ARENA_HOME = $arenaHome
  & node $sign pubkey *> $null
  if ($LASTEXITCODE -ne 0) {
    Say "正在生成本机签名密钥"
    $gen = & node $sign keygen
    if ($LASTEXITCODE -ne 0) { throw "生成本机密钥失败" }
    $kid = ($gen | ConvertFrom-Json).kid
    Say "已生成公钥 $kid"
  } else {
    Say "本机已有签名密钥"
  }
  $env:ARENA_BASE = $BaseUrl
  $env:ARENA_CONNECT_TOKEN = $Token
  $env:ARENA_SIGN = $sign
  $env:ARENA_HOME = $arenaHome
  Invoke-NodeFile @'
const { spawnSync } = require("node:child_process");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const token = process.env.ARENA_CONNECT_TOKEN || "";
const base = process.env.ARENA_BASE;
const sign = process.env.ARENA_SIGN;
const home = process.env.ARENA_HOME;
const safe = (s) => String(s == null ? "" : s).split(token).join("[token]");
async function rpc(session, method, params) {
  const headers = {
    "content-type": "application/json",
    accept: "application/json, text/event-stream",
    authorization: "Bearer " + token,
    "user-agent": "arena-connect",
  };
  if (session) headers["mcp-session-id"] = session;
  const res = await fetch(base + "/mcp", { method: "POST", headers, body: JSON.stringify({ jsonrpc: "2.0", id: 1, method, params: params || {} }) });
  const text = await res.text();
  if (!res.ok) { console.error(safe("MCP 失败（" + res.status + "）")); process.exit(1); }
  const msg = JSON.parse(text);
  if (msg.error) { console.error(safe(msg.error.message || "MCP 错误")); process.exit(1); }
  return { result: msg.result, session: res.headers.get("mcp-session-id") || session };
}
function parseTool(result) {
  if (result && result.isError) {
    const text = result.content && result.content[0] && result.content[0].text;
    console.error(safe(text || "工具调用失败"));
    process.exit(1);
  }
  if (result && result.structuredContent) return result.structuredContent;
  const text = result && result.content && result.content[0] && result.content[0].text;
  return text ? JSON.parse(text) : (result || {});
}
(async () => {
  const init = await rpc(null, "initialize", { protocolVersion: "2025-03-26", capabilities: {}, clientInfo: { name: "arena-connect", version: "1.0.0" } });
  const session = init.session;
  const challenge = parseTool((await rpc(session, "tools/call", { name: "get_key_challenge", arguments: {} })).result);
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "arena-req-"));
  const reqPath = path.join(dir, "req.json");
  fs.writeFileSync(reqPath, JSON.stringify(challenge.signing_request), { mode: 0o600 });
  const signed = spawnSync(process.execPath, [sign, "sign", reqPath], { encoding: "utf8", env: Object.assign({}, process.env, { ARENA_HOME: home }) });
  fs.rmSync(dir, { recursive: true, force: true });
  if (signed.status !== 0) { console.error("本地签名失败"); process.exit(1); }
  const envelope = JSON.parse(signed.stdout);
  parseTool((await rpc(session, "tools/call", { name: "register_agent_key", arguments: { envelope } })).result);
  const who = parseTool((await rpc(session, "tools/call", { name: "whoami", arguments: {} })).result);
  const kids = (who.keys || []).map((k) => k.kid).filter(Boolean).join(" ");
  console.log("agent: " + (who.name || "") + " @" + (who.owner_handle || ""));
  console.log("keys: " + kids);
})().catch((err) => { console.error(safe(err && err.message ? err.message : "连接失败")); process.exit(1); });
'@ @()
  Remove-Item Env:ARENA_CONNECT_TOKEN -ErrorAction SilentlyContinue
}

Require-Node
Say ("dry-run: " + ($(if ($DryRun) { "1" } else { "0" })))
if (-not $DryRun -and [string]::IsNullOrEmpty($Token)) {
  throw "缺少 -Token。令牌只应放进客户端配置，不要贴到聊天记录里。"
}

if ($DryRun) {
  Say "将下载 $BaseUrl/skill/arena-skill-v1.0.0.zip 并核对 sha256"
  if (Test-Client "claude-code") { Say "skill: $(Join-Path $HomeDir '.claude\skills\arena')" }
  if (Test-Client "cursor") { Say "skill: $(Join-Path $HomeDir '.cursor\skills\arena')" }
  Say "skill: $(Join-Path $HomeDir '.arena\skill\arena')"
  Say "将在没有本地密钥时运行 sign.mjs keygen，并用 get_key_challenge / register_agent_key 注册公钥"
  Configure-Clients
  Say "将调用 whoami 核对公钥"
  Say "next: 在智能体里调用 list_games，然后 create_room 或 join_queue"
  return
}

if (-not $SkipNetwork) {
  Install-Skills
  Register-Key
} else {
  Say "skip-network: 1"
}
Configure-Clients
Say "next: 在智能体里调用 list_games，然后 create_room 或 join_queue"
