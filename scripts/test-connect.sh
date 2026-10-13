#!/bin/sh
# shellcheck + a dry-run of scripts/connect.sh. Also checks config writes never echo the token.
set -eu
ROOT=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
cd "$ROOT"

shellcheck scripts/connect.sh

TOKEN='super-secret-token-value'
HOME_DIR=$(mktemp -d)
mkdir -p "$HOME_DIR/.claude" "$HOME_DIR/.cursor" "$HOME_DIR/.config/Claude" "$HOME_DIR/.config/Code/User"
printf '%s\n' '{"mcpServers":{"other":{"url":"http://example.test"}}}' > "$HOME_DIR/.cursor/mcp.json"

out=$(sh scripts/connect.sh --dry-run --yes --token "$TOKEN" --home "$HOME_DIR" --base-url "https://agent.idou.bid")
printf '%s\n' "$out" | grep -q 'dry-run: 1'
printf '%s\n' "$out" | grep -q 'client: claude-code'
printf '%s\n' "$out" | grep -q 'client: cursor'
printf '%s\n' "$out" | grep -q 'skill: '"$HOME_DIR"'/.arena/skill/arena'
if printf '%s\n' "$out" | grep -q "$TOKEN"; then
  printf '%s\n' "dry-run printed the token" >&2
  exit 1
fi
if [ -f "$HOME_DIR/.claude.json" ] || [ -f "$HOME_DIR/.cursor/mcp.json.bak."* ]; then
  printf '%s\n' "dry-run wrote client config" >&2
  exit 1
fi
# The seeded cursor file must be untouched.
grep -q 'example.test' "$HOME_DIR/.cursor/mcp.json"
if grep -q agent-arena "$HOME_DIR/.cursor/mcp.json"; then
  printf '%s\n' "dry-run modified cursor config" >&2
  exit 1
fi

apply=$(sh scripts/connect.sh --skip-network --yes --token "$TOKEN" --home "$HOME_DIR" --base-url "https://agent.idou.bid")
if printf '%s\n' "$apply" | grep -q "$TOKEN"; then
  printf '%s\n' "apply printed the token" >&2
  exit 1
fi
printf '%s\n' "$apply" | grep -q 'updated'
grep -q "$TOKEN" "$HOME_DIR/.cursor/mcp.json"
grep -q 'example.test' "$HOME_DIR/.cursor/mcp.json"
grep -q "$TOKEN" "$HOME_DIR/.claude.json"
grep -q "$TOKEN" "$HOME_DIR/.config/Claude/claude_desktop_config.json"
grep -q '"type": "http"' "$HOME_DIR/.config/Code/User/mcp.json"
find "$HOME_DIR/.cursor" -name 'mcp.json.bak.*' | grep -q .

again=$(sh scripts/connect.sh --skip-network --yes --token "$TOKEN" --home "$HOME_DIR" --base-url "https://agent.idou.bid")
printf '%s\n' "$again" | grep -q 'unchanged'
backs=$(find "$HOME_DIR/.cursor" -name 'mcp.json.bak.*' | wc -l | tr -d ' ')
if [ "$backs" != "1" ]; then
  printf '%s\n' "expected one backup, found $backs" >&2
  exit 1
fi

# Unzip the same way connect.sh does, using a tiny zip from python3.
if command -v python3 >/dev/null 2>&1; then
  zip=$HOME_DIR/t.zip
  dest=$HOME_DIR/unz
  mkdir -p "$dest"
  python3 - "$zip" <<'PY'
import sys, zipfile
z = zipfile.ZipFile(sys.argv[1], "w")
z.writestr("arena/SKILL.md", "hello skill")
z.writestr("arena/scripts/sign.mjs", "console.log(1)\n")
z.close()
PY
  ARENA_ZIP=$zip ARENA_DEST=$dest node <<'ARENA_UNZIP'
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
  grep -q 'hello skill' "$dest/arena/SKILL.md"
fi

rm -rf "$HOME_DIR"
printf '%s\n' "connect.sh ok"
