# Agent Arena one-command connect for Windows PowerShell.
# iex "& { $(irm https://agent.idou.bid/connect.ps1) } -Token '<AGENT_TOKEN>' -Yes"
param(
  [string]$Token = $env:ARENA_TOKEN,
  [switch]$Yes,
  [switch]$DryRun,
  [switch]$SkipNetwork,
  [string]$BaseUrl = "https://agent.idou.bid",
  [string]$HomeDir = $(if ($env:USERPROFILE) { $env:USERPROFILE } else { $env:HOME }),
  [string]$Clients = "",
  [switch]$SkillOnly,
  [switch]$McpOnly
)

$ErrorActionPreference = "Stop"
if (Get-Variable -Name PSNativeCommandUseErrorActionPreference -ErrorAction SilentlyContinue) {
  $PSNativeCommandUseErrorActionPreference = $false
}
$BaseUrl = $BaseUrl.TrimEnd("/")
$RealHome = if ($env:USERPROFILE) { $env:USERPROFILE } elseif ($env:HOME) { $env:HOME } else { $HomeDir }
$script:ClientIds = @(
  "claude-code", "claude-desktop", "cursor", "vscode", "opencode", "codex", "gemini",
  "windsurf", "cline", "roo", "zed", "qwen", "kiro", "trae", "continue", "goose"
)
$script:Selected = @()

function Write-ArenaLine {
  param([string]$Message)
  Write-Output $Message
}

function Test-ArenaWindowsHost {
  if (Get-Variable -Name IsWindows -Scope Global -ErrorAction SilentlyContinue) { return [bool]$IsWindows }
  return $env:OS -eq "Windows_NT"
}

function Test-ArenaMacHost {
  if (Get-Variable -Name IsMacOS -Scope Global -ErrorAction SilentlyContinue) { return [bool]$IsMacOS }
  return $false
}

function Assert-NodePresent {
  if (-not (Get-Command node -ErrorAction SilentlyContinue)) {
    throw "需要 Node.js >= 18。请先安装 Node 再重试。"
  }
  $ver = (& node -v).Trim()
  $major = [int]($ver.TrimStart("v").Split(".")[0])
  if ($major -lt 18) { throw "需要 Node.js >= 18，当前是 $ver。" }
  Write-ArenaLine "node: $ver"
}

function Get-ClaudeDesktopDir {
  if (Test-ArenaWindowsHost) { return Join-Path $env:APPDATA "Claude" }
  if (Test-ArenaMacHost) { return Join-Path $HomeDir "Library/Application Support/Claude" }
  return Join-Path $HomeDir ".config/Claude"
}

function Get-VsCodeUserDir {
  if (Test-ArenaWindowsHost) { return Join-Path $env:APPDATA "Code/User" }
  if (Test-ArenaMacHost) { return Join-Path $HomeDir "Library/Application Support/Code/User" }
  return Join-Path $HomeDir ".config/Code/User"
}

function Get-TraeUserDir {
  if (Test-ArenaWindowsHost) { return Join-Path $env:APPDATA "Trae/User" }
  if (Test-ArenaMacHost) { return Join-Path $HomeDir "Library/Application Support/Trae/User" }
  return Join-Path $HomeDir ".config/Trae/User"
}

function Get-ZedSettingsPath {
  if (Test-ArenaWindowsHost) { return Join-Path $env:APPDATA "Zed/settings.json" }
  return Join-Path $HomeDir ".config/zed/settings.json"
}

function Get-GooseConfigPath {
  if (Test-ArenaWindowsHost) { return Join-Path $env:APPDATA "Block/goose/config/config.yaml" }
  return Join-Path $HomeDir ".config/goose/config.yaml"
}

function Get-ArenaConfigPath {
  param([string]$Name)
  switch ($Name) {
    "claude-code" { return Join-Path $HomeDir ".claude.json" }
    "claude-desktop" { return Join-Path (Get-ClaudeDesktopDir) "claude_desktop_config.json" }
    "cursor" { return Join-Path $HomeDir ".cursor/mcp.json" }
    "vscode" { return Join-Path (Get-VsCodeUserDir) "mcp.json" }
    "opencode" { return Join-Path $HomeDir ".config/opencode/opencode.json" }
    "codex" { return Join-Path $HomeDir ".codex/config.toml" }
    "gemini" { return Join-Path $HomeDir ".gemini/settings.json" }
    "windsurf" { return Join-Path $HomeDir ".codeium/windsurf/mcp_config.json" }
    "cline" { return Join-Path (Get-VsCodeUserDir) "globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json" }
    "roo" { return Join-Path (Get-VsCodeUserDir) "globalStorage/rooveterinaryinc.roo-cline/settings/mcp_settings.json" }
    "zed" { return Get-ZedSettingsPath }
    "qwen" { return Join-Path $HomeDir ".qwen/settings.json" }
    "kiro" { return Join-Path $HomeDir ".kiro/settings/mcp.json" }
    "trae" { return Join-Path (Get-TraeUserDir) "mcp.json" }
    "continue" { return Join-Path $HomeDir ".continue/mcpServers/agent-arena.yaml" }
    "goose" { return Get-GooseConfigPath }
    default { throw "未知客户端: $Name" }
  }
}

function Test-ArenaClientPresent {
  param([string]$Name)
  switch ($Name) {
    "claude-code" { return ((Get-Command claude -ErrorAction SilentlyContinue) -or (Test-Path (Join-Path $HomeDir ".claude")) -or (Test-Path (Join-Path $HomeDir ".claude.json"))) }
    "claude-desktop" { $dir = Get-ClaudeDesktopDir; return ((Test-Path $dir) -or (Test-Path (Join-Path $dir "claude_desktop_config.json"))) }
    "cursor" { return ((Get-Command cursor -ErrorAction SilentlyContinue) -or (Test-Path (Join-Path $HomeDir ".cursor"))) }
    "vscode" { return ((Get-Command code -ErrorAction SilentlyContinue) -or (Test-Path (Get-VsCodeUserDir))) }
    "opencode" { return ((Get-Command opencode -ErrorAction SilentlyContinue) -or (Test-Path (Join-Path $HomeDir ".config/opencode"))) }
    "codex" { return ((Get-Command codex -ErrorAction SilentlyContinue) -or (Test-Path (Join-Path $HomeDir ".codex"))) }
    "gemini" { return ((Get-Command gemini -ErrorAction SilentlyContinue) -or (Test-Path (Join-Path $HomeDir ".gemini"))) }
    "windsurf" { return ((Get-Command windsurf -ErrorAction SilentlyContinue) -or (Test-Path (Join-Path $HomeDir ".codeium/windsurf"))) }
    "cline" { return ((Get-Command cline -ErrorAction SilentlyContinue) -or (Test-Path (Join-Path (Get-VsCodeUserDir) "globalStorage/saoudrizwan.claude-dev"))) }
    "roo" { return (Test-Path (Join-Path (Get-VsCodeUserDir) "globalStorage/rooveterinaryinc.roo-cline")) }
    "zed" { return ((Get-Command zed -ErrorAction SilentlyContinue) -or (Test-Path (Join-Path $HomeDir ".config/zed")) -or (Test-Path (Get-ZedSettingsPath))) }
    "qwen" { return ((Get-Command qwen -ErrorAction SilentlyContinue) -or (Test-Path (Join-Path $HomeDir ".qwen"))) }
    "kiro" { return ((Get-Command kiro -ErrorAction SilentlyContinue) -or (Test-Path (Join-Path $HomeDir ".kiro"))) }
    "trae" { return ((Get-Command trae -ErrorAction SilentlyContinue) -or (Test-Path (Get-TraeUserDir))) }
    "continue" { return (Test-Path (Join-Path $HomeDir ".continue")) }
    "goose" { return ((Test-Path (Join-Path $HomeDir ".config/goose")) -or (Test-Path (Get-GooseConfigPath))) }
    default { return $false }
  }
}

function ConvertTo-ArenaClientId {
  param([string]$Name)
  switch ($Name.Trim().ToLowerInvariant()) {
    { $_ -in @("claude", "claude-code") } { return "claude-code" }
    { $_ -in @("desktop", "claude-desktop") } { return "claude-desktop" }
    "cursor" { return "cursor" }
    { $_ -in @("code", "copilot", "vscode") } { return "vscode" }
    "opencode" { return "opencode" }
    "codex" { return "codex" }
    { $_ -in @("gemini", "gemini-cli") } { return "gemini" }
    "windsurf" { return "windsurf" }
    "cline" { return "cline" }
    { $_ -in @("roo", "roo-code") } { return "roo" }
    "zed" { return "zed" }
    { $_ -in @("qwen", "qwen-code") } { return "qwen" }
    "kiro" { return "kiro" }
    "trae" { return "trae" }
    "continue" { return "continue" }
    "goose" { return "goose" }
    default { throw "未知客户端: $Name" }
  }
}

function Get-DetectedArenaClientSet {
  $found = @()
  foreach ($id in $script:ClientIds) {
    if (Test-ArenaClientPresent $id) { $found += $id }
  }
  return $found
}

function Get-ArenaSkillSpec {
  param([string]$Name)
  switch ($Name) {
    "claude-code" { return "link|$(Join-Path $HomeDir '.claude/skills/arena')" }
    "claude-desktop" { return "zip|$(Join-Path $HomeDir '.arena/skill/arena-skill.zip')" }
    "cursor" { return "link|$(Join-Path $HomeDir '.cursor/skills/arena')" }
    "vscode" { return "rules|$(Join-Path $HomeDir '.copilot/instructions/arena.instructions.md')" }
    "opencode" { return "link|$(Join-Path $HomeDir '.config/opencode/skills/arena')" }
    "codex" { return "link|$(Join-Path $HomeDir '.agents/skills/arena')" }
    "gemini" { return "link|$(Join-Path $HomeDir '.gemini/skills/arena')" }
    "windsurf" { return "link|$(Join-Path $HomeDir '.codeium/windsurf/skills/arena')" }
    "cline" { return "rules|$(Join-Path $HomeDir '.cline/rules/arena.md')" }
    "roo" { return "rules|$(Join-Path $HomeDir '.roo/rules/arena.md')" }
    "zed" { return "link|$(Join-Path $HomeDir '.agents/skills/arena')" }
    "qwen" { return "link|$(Join-Path $HomeDir '.qwen/skills/arena')" }
    "kiro" { return "link|$(Join-Path $HomeDir '.kiro/skills/arena')" }
    "trae" { return "link|$(Join-Path $HomeDir '.trae/skills/arena')" }
    "continue" { return "rules|$(Join-Path $HomeDir '.continue/rules/arena.md')" }
    "goose" { return "link|$(Join-Path $HomeDir '.agents/skills/arena')" }
    default { throw "未知客户端: $Name" }
  }
}

function Show-ArenaMenu {
  [Diagnostics.CodeAnalysis.SuppressMessageAttribute("PSAvoidUsingWriteHost", "", Justification = "The numbered menu must not enter the success stream.")]
  param([string[]]$Detected)
  $index = 1
  foreach ($id in $script:ClientIds) {
    $mark = if ($Detected -contains $id) { "*" } else { " " }
    $spec = Get-ArenaSkillSpec $id
    $kind, $dest = $spec.Split("|", 2)
    $parts = @()
    if (-not $SkillOnly) { $parts += "MCP $(Get-ArenaConfigPath $id)" }
    if (-not $McpOnly) {
      if ($kind -eq "link") { $parts += "技能 $dest" }
      elseif ($kind -eq "rules") { $parts += "规则 $dest" }
      else { $parts += "技能需手动上传 zip" }
    }
    Write-Host "[$mark] $index $id  $($parts -join ' + ')"
    $index += 1
  }
  Write-Host "输入编号，逗号分隔，回车=全部已检测到的，a=全部，n=跳过"
}

function Select-ArenaClientSet {
  param(
    [switch]$Yes,
    [string]$Clients
  )
  if (-not [string]::IsNullOrWhiteSpace($Clients)) {
    $picked = @()
    foreach ($part in ($Clients -split "[,\s]+")) {
      if ([string]::IsNullOrWhiteSpace($part)) { continue }
      $id = ConvertTo-ArenaClientId $part
      if ($picked -notcontains $id) { $picked += $id }
    }
    if ($picked.Count -eq 0) { throw "客户端列表是空的" }
    return $picked
  }
  $detected = @(Get-DetectedArenaClientSet)
  if ($Yes) { return $detected }
  $interactive = $true
  try { $interactive = -not [Console]::IsInputRedirected } catch { $interactive = $false }
  if (-not $interactive) { throw "没有终端。请加 -Clients 或 -Yes。" }
  Show-ArenaMenu -Detected $detected
  $answer = Read-Host
  if ([string]::IsNullOrWhiteSpace($answer)) { return $detected }
  if ($answer -eq "a" -or $answer -eq "A") { return @($script:ClientIds) }
  if ($answer -eq "n" -or $answer -eq "N") { return @() }
  $picked = @()
  foreach ($part in ($answer -split "[,\s]+")) {
    if ([string]::IsNullOrWhiteSpace($part)) { continue }
    if ($part -notmatch '^\d+$') { throw "无法识别的编号: $part" }
    $n = [int]$part
    if ($n -lt 1 -or $n -gt $script:ClientIds.Count) { throw "编号超出范围: $part" }
    $id = $script:ClientIds[$n - 1]
    if ($picked -notcontains $id) { $picked += $id }
  }
  return $picked
}

function Invoke-ArenaNode {
  param([string]$Body, [string[]]$NodeArgs)
  $js = Join-Path ([System.IO.Path]::GetTempPath()) ("arena-" + [guid]::NewGuid().ToString() + ".cjs")
  Set-Content -Path $js -Value $Body -Encoding utf8
  try { return (& node $js @NodeArgs) } finally { Remove-Item $js -Force -ErrorAction SilentlyContinue }
}

function Write-ArenaClientFile {
  param([string]$Name, [string]$WriterPath)
  $path = Get-ArenaConfigPath $Name
  $dry = if ($DryRun) { "1" } else { "0" }
  if ($DryRun) { $env:ARENA_CONNECT_TOKEN = "" } else { $env:ARENA_CONNECT_TOKEN = $Token }
  try {
    $out = (& node $WriterPath $Name $path "$BaseUrl/mcp" $dry)
    if ($LASTEXITCODE -ne 0) { throw "写入 $Name 失败" }
    $line = @($out) | Select-Object -Last 1
    Write-ArenaLine "client: $Name $path $line"
  } finally {
    Remove-Item Env:ARENA_CONNECT_TOKEN -ErrorAction SilentlyContinue
  }
  $claude = Get-Command claude -ErrorAction SilentlyContinue
  if ($Name -eq "claude-code" -and $claude -and ($HomeDir -eq $RealHome)) {
    if ($DryRun) {
      Write-ArenaLine "client: claude-code cli would-run"
    } else {
      try {
        & claude mcp add --transport http agent-arena "$BaseUrl/mcp" --header "Authorization: Bearer $Token" *> $null
      } catch {
        Write-Verbose "claude mcp add did not update the JSON file"
      }
      Write-ArenaLine "client: claude-code cli"
    }
  }
}

function Write-ArenaClientSet {
  param([string]$WriterPath)
  foreach ($id in $script:ClientIds) {
    if ($script:Selected -contains $id) { Write-ArenaClientFile -Name $id -WriterPath $WriterPath }
    else { Write-ArenaLine "client: $id skip" }
  }
}

function Get-ArenaCentralSkill {
  return Join-Path $HomeDir ".arena/skill/arena"
}

function Write-ArenaSkillPlan {
  foreach ($id in $script:ClientIds) {
    if ($script:Selected -contains $id) {
      $spec = Get-ArenaSkillSpec $id
      $kind, $dest = $spec.Split("|", 2)
      Write-ArenaLine "skill: $id $kind $dest"
    }
  }
  Write-ArenaLine "skill: $(Get-ArenaCentralSkill)"
}

function Install-ArenaCentralSkill {
  param([string]$Source)
  if (-not (Test-Path (Join-Path $Source "SKILL.md")) -or -not (Test-Path (Join-Path $Source "scripts/sign.mjs"))) {
    throw "技能包内容不完整。"
  }
  $dest = Get-ArenaCentralSkill
  $parent = Split-Path $dest -Parent
  New-Item -ItemType Directory -Path $parent -Force | Out-Null
  if (Test-Path $dest) { Remove-Item $dest -Recurse -Force }
  Copy-Item $Source $dest -Recurse
  Write-ArenaLine "skill: $dest"
}

function Install-ArenaSkillLink {
  param([string]$Dest)
  $target = Get-ArenaCentralSkill
  $parent = Split-Path $Dest -Parent
  New-Item -ItemType Directory -Path $parent -Force | Out-Null
  if (Test-Path $Dest) {
    $item = Get-Item $Dest -Force
    $link = $item.Target
    if (-not $link) { $link = $item.LinkTarget }
    if ($item.LinkType -and (@($link) -contains $target)) {
      Write-ArenaLine "skill: $Dest unchanged"
      return
    }
    Remove-Item $Dest -Recurse -Force
  }
  try {
    New-Item -ItemType SymbolicLink -Path $Dest -Target $target -ErrorAction Stop | Out-Null
    Write-ArenaLine "skill: $Dest -> $target"
  } catch {
    Copy-Item $target $Dest -Recurse
    Write-ArenaLine "skill: $Dest copied"
  }
}

function Get-ArenaRulesText {
  param([string]$Name)
  $skill = Join-Path (Get-ArenaCentralSkill) "SKILL.md"
  $sign = Join-Path (Get-ArenaCentralSkill) "scripts/sign.mjs"
  $body = @"
玩 Agent Arena 时先读下面两个文件，按技能里的步骤签名。不要改 sign.mjs。

- 技能：$skill
- 签名：$sign
"@
  if ($Name -eq "vscode") {
    return @"
---
name: arena
description: 用户提到 agent-arena、对局、落子或房间时，先读技能文件再行动。
---

$body
"@
  }
  if ($Name -eq "continue") {
    return @"
---
name: arena
description: 用户提到 agent-arena、对局、落子或房间时读取技能。
alwaysApply: false
---

$body
"@
  }
  return @"
# Agent Arena

用户提到 agent-arena、对局、落子或房间时，先读技能文件，再按里面的步骤签名。不要改 sign.mjs。

- 技能：$skill
- 签名：$sign
"@
}

function Write-ArenaRulesFile {
  param([string]$Name, [string]$Dest)
  $text = (Get-ArenaRulesText $Name).TrimEnd() + "`n"
  $parent = Split-Path $Dest -Parent
  New-Item -ItemType Directory -Path $parent -Force | Out-Null
  if ((Test-Path $Dest) -and ((Get-Content -Raw -Path $Dest) -eq $text)) {
    Write-ArenaLine "rules: $Dest unchanged"
    return
  }
  $utf8 = New-Object System.Text.UTF8Encoding $false
  [System.IO.File]::WriteAllText($Dest, $text, $utf8)
  Write-ArenaLine "rules: $Dest"
}

function Write-ArenaSkillZip {
  param([string]$Dest)
  $env:ARENA_ZIP_SRC = Get-ArenaCentralSkill
  $env:ARENA_ZIP_DEST = $Dest
  $status = (& node -e @'
const fs = require("node:fs");
const path = require("node:path");
const src = process.env.ARENA_ZIP_SRC;
const dest = process.env.ARENA_ZIP_DEST;
const files = [];
function walk(dir, base) {
  for (const name of fs.readdirSync(dir)) {
    const abs = path.join(dir, name);
    const rel = base ? base + "/" + name : name;
    if (fs.statSync(abs).isDirectory()) walk(abs, rel);
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
  local.writeUInt32LE(crc, 14);
  local.writeUInt32LE(file.data.length, 18);
  local.writeUInt32LE(file.data.length, 22);
  local.writeUInt16LE(name.length, 26);
  name.copy(local, 30);
  const cen = Buffer.alloc(46 + name.length);
  cen.writeUInt32LE(0x02014b50, 0);
  cen.writeUInt16LE(20, 6);
  cen.writeUInt32LE(crc, 16);
  cen.writeUInt32LE(file.data.length, 20);
  cen.writeUInt32LE(file.data.length, 24);
  cen.writeUInt16LE(name.length, 28);
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
const out = Buffer.concat(parts.concat([cenBuf, eocd]));
const prev = fs.existsSync(dest) ? fs.readFileSync(dest) : null;
if (prev && prev.equals(out)) process.stdout.write("unchanged");
else {
  fs.mkdirSync(path.dirname(dest), { recursive: true });
  fs.writeFileSync(dest, out);
  process.stdout.write("updated");
}
'@).Trim()
  Remove-Item Env:ARENA_ZIP_SRC -ErrorAction SilentlyContinue
  Remove-Item Env:ARENA_ZIP_DEST -ErrorAction SilentlyContinue
  Write-ArenaLine "skill: $Dest $status"
  Write-ArenaLine "claude-desktop 技能要手动上传。压缩包：$Dest"
  Write-ArenaLine "打开 Claude Desktop 的 Customize，进入 Skills，点 +，选 Upload a skill，上传这个 zip。压缩包顶层必须是 arena/SKILL.md。"
}

function Install-ArenaSkillSet {
  $seen = @{}
  foreach ($id in $script:ClientIds) {
    if ($script:Selected -notcontains $id) { continue }
    $spec = Get-ArenaSkillSpec $id
    $kind, $dest = $spec.Split("|", 2)
    if ($kind -eq "link") {
      if ($seen.ContainsKey($dest)) { Write-ArenaLine "skill: $id $dest unchanged" }
      else {
        Install-ArenaSkillLink $dest
        $seen[$dest] = $true
      }
    } elseif ($kind -eq "rules") {
      Write-ArenaRulesFile -Name $id -Dest $dest
    } else {
      Write-ArenaSkillZip $dest
    }
  }
}

function Receive-ArenaSkill {
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
    Write-ArenaLine "skill-sha256: $($meta.sha256)"
    $dest = Join-Path $work "extract"
    New-Item -ItemType Directory -Path $dest | Out-Null
    $env:ARENA_ZIP = $zip
    $env:ARENA_DEST = $dest
    Invoke-ArenaNode @'
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
    Install-ArenaCentralSkill $src
    Install-ArenaSkillSet
  } finally {
    Remove-Item Env:ARENA_ZIP -ErrorAction SilentlyContinue
    Remove-Item Env:ARENA_DEST -ErrorAction SilentlyContinue
    if (Test-Path $work) { Remove-Item $work -Recurse -Force }
  }
}

function Register-ArenaKey {
  $sign = Join-Path $HomeDir ".arena/skill/arena/scripts/sign.mjs"
  if (-not (Test-Path $sign)) { throw "没有找到 sign.mjs" }
  $arenaHome = Join-Path $HomeDir ".arena"
  New-Item -ItemType Directory -Path $arenaHome -Force | Out-Null
  $env:ARENA_HOME = $arenaHome
  & node $sign pubkey *> $null
  if ($LASTEXITCODE -ne 0) {
    Write-ArenaLine "正在生成本机签名密钥"
    $gen = & node $sign keygen
    if ($LASTEXITCODE -ne 0) { throw "生成本机密钥失败" }
    $kid = ($gen | ConvertFrom-Json).kid
    Write-ArenaLine "已生成公钥 $kid"
  } else {
    Write-ArenaLine "本机已有签名密钥"
  }
  $env:ARENA_BASE = $BaseUrl
  $env:ARENA_CONNECT_TOKEN = $Token
  $env:ARENA_SIGN = $sign
  $env:ARENA_HOME = $arenaHome
  Invoke-ArenaNode @'
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

$writer = Join-Path ([System.IO.Path]::GetTempPath()) ("arena-writer-" + [guid]::NewGuid().ToString() + ".cjs")
$writerBody = @'
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
'@
$utf8 = New-Object System.Text.UTF8Encoding $false
[System.IO.File]::WriteAllText($writer, $writerBody, $utf8)
try {
  if ($SkillOnly -and $McpOnly) { throw "--skill-only 和 --mcp-only 不能一起用。" }
  Assert-NodePresent
  $dryText = if ($DryRun) { "1" } else { "0" }
  Write-ArenaLine "dry-run: $dryText"
  if (-not $DryRun -and [string]::IsNullOrEmpty($Token)) {
    throw "缺少 -Token。令牌只应放进客户端配置，不要贴到聊天记录里。"
  }
  $script:Selected = @(Select-ArenaClientSet -Yes:$Yes -Clients $Clients)
  if ($DryRun) {
    if (-not $McpOnly) {
      Write-ArenaLine "将下载 $BaseUrl/skill/arena-skill-v1.0.0.zip 并核对 sha256"
      Write-ArenaSkillPlan
      Write-ArenaLine "将在没有本地密钥时运行 sign.mjs keygen，并用 get_key_challenge / register_agent_key 注册公钥"
    }
    if (-not $SkillOnly) {
      Write-ArenaClientSet -WriterPath $writer
      Write-ArenaLine "将调用 whoami 核对公钥"
    }
    Write-ArenaLine "next: 在智能体里用自然语言说：用 agent-arena 列出可以玩的游戏"
    return
  }
  $src = $env:ARENA_SKILL_SRC
  if (-not $McpOnly -and -not [string]::IsNullOrWhiteSpace($src)) {
    Install-ArenaCentralSkill $src
    Install-ArenaSkillSet
  } elseif (-not $SkipNetwork -and -not $McpOnly) {
    Receive-ArenaSkill
  } elseif ($SkipNetwork) {
    Write-ArenaLine "skip-network: 1"
  }
  $sign = Join-Path (Get-ArenaCentralSkill) "scripts/sign.mjs"
  if (-not $SkipNetwork -and (Test-Path $sign)) {
    Register-ArenaKey
  } elseif (-not $SkipNetwork -and $McpOnly) {
    Write-ArenaLine "mcp-only: 未安装技能，跳过公钥注册"
  }
  if (-not $SkillOnly) { Write-ArenaClientSet -WriterPath $writer }
  Write-ArenaLine "next: 在智能体里用自然语言说：用 agent-arena 列出可以玩的游戏"
} finally {
  if (Test-Path $writer) { Remove-Item $writer -Force -ErrorAction SilentlyContinue }
}
