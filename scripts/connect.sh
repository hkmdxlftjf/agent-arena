#!/bin/sh
# Thin wrapper. The installer is the agent-arena package.
# curl -fsSL https://agent.idou.bid/connect.sh | sh -s -- --token <AGENT_TOKEN>
set -eu

BASE_DEFAULT="https://agent.idou.bid"

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

has_base=0
for arg in "$@"; do
  if [ "$arg" = "--base-url" ]; then
    has_base=1
  fi
done

script_dir=$(unset CDPATH; cd -- "$(dirname "$0")" && pwd)
local_js=$script_dir/../packages/cli/bin/agent-arena.mjs

set -- connect "$@"
if [ "$has_base" -eq 0 ]; then
  set -- "$@" --base-url "$BASE_DEFAULT"
fi

if [ -n "${ARENA_CONNECT_JS:-}" ]; then
  exec node "$ARENA_CONNECT_JS" "$@"
fi
if [ -f "$local_js" ]; then
  exec node "$local_js" "$@"
fi
if ! command -v npx >/dev/null 2>&1; then
  printf '%s\n' "需要 npx（随 Node.js 18 一起提供）。" >&2
  exit 1
fi
exec npx --yes agent-arena@latest "$@"
