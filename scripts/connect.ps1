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
$nodeCmd = Get-Command node -ErrorAction SilentlyContinue
if (-not $nodeCmd) {
  throw "需要 Node.js >= 18。请先安装 Node 再重试。"
}
$ver = & node -v
if ($ver -notmatch '^v(\d+)') {
  throw "需要 Node.js >= 18，当前是 ${ver}。"
}
if ([int]$Matches[1] -lt 18) {
  throw "需要 Node.js >= 18，当前是 ${ver}。"
}

$nodeArgs = @("connect")
if ($Token) { $nodeArgs += @("--token", $Token) }
if ($Yes) { $nodeArgs += "--yes" }
if ($DryRun) { $nodeArgs += "--dry-run" }
if ($SkipNetwork) { $nodeArgs += "--skip-network" }
if ($BaseUrl) { $nodeArgs += @("--base-url", $BaseUrl.TrimEnd("/")) }
if ($HomeDir) { $nodeArgs += @("--home", $HomeDir) }
if ($Clients) { $nodeArgs += @("--clients", $Clients) }
if ($SkillOnly) { $nodeArgs += "--skill-only" }
if ($McpOnly) { $nodeArgs += "--mcp-only" }

$local = $null
if ($env:ARENA_CONNECT_JS) {
  $local = $env:ARENA_CONNECT_JS
} elseif ($PSCommandPath) {
  $sibling = Join-Path (Split-Path -Parent $PSCommandPath) "..\packages\cli\bin\agent-arena.mjs"
  if (Test-Path $sibling) { $local = $sibling }
}

if ($local) {
  & node $local @nodeArgs
} else {
  $npx = Get-Command npx -ErrorAction SilentlyContinue
  if (-not $npx) { throw "需要 npx（随 Node.js 18 一起提供）。" }
  & npx --yes agent-arena@latest @nodeArgs
}
if ($LASTEXITCODE -ne 0) {
  throw "agent-arena connect 退出码 $LASTEXITCODE"
}
