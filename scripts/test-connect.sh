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
printf '%s\n' "$out" | grep -q 'skill: claude-code link '"$HOME_DIR"'/.claude/skills/arena'
printf '%s\n' "$out" | grep -q 'skill: claude-desktop zip '"$HOME_DIR"'/.arena/skill/arena-skill.zip'
printf '%s\n' "$out" | grep -q 'skill: vscode rules '"$HOME_DIR"'/.copilot/instructions/arena.instructions.md'
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

# Every supported writer, including ones that were not "installed".
all_home=$(mktemp -d)
mkdir -p "$all_home/.codex" "$all_home/.config/goose" "$all_home/.config/zed" "$all_home/.config/opencode"
printf '%s\n' 'model = "keep-me"' > "$all_home/.codex/config.toml"
printf '%s\n' '[mcp_servers.other]' >> "$all_home/.codex/config.toml"
printf '%s\n' 'command = "echo"' >> "$all_home/.codex/config.toml"
printf '%s\n' 'provider: keep' > "$all_home/.config/goose/config.yaml"
printf '%s\n' 'extensions:' >> "$all_home/.config/goose/config.yaml"
printf '%s\n' '  other:' >> "$all_home/.config/goose/config.yaml"
printf '%s\n' '    command: echo' >> "$all_home/.config/goose/config.yaml"
printf '%s\n' '{"theme":"keep","context_servers":{"other":{"command":"echo"}}}' > "$all_home/.config/zed/settings.json"
printf '%s\n' '{"mcp":{"other":{"type":"local","command":["echo"]}}}' > "$all_home/.config/opencode/opencode.json"

clients="claude-code,claude-desktop,cursor,vscode,opencode,codex,gemini,windsurf,cline,roo,zed,qwen,kiro,trae,continue,goose"
applied=$(sh scripts/connect.sh --skip-network --clients "$clients" --token "$TOKEN" --home "$all_home" --base-url "https://agent.idou.bid")
if printf '%s\n' "$applied" | grep -q "$TOKEN"; then
  printf '%s\n' "per-client apply printed the token" >&2
  exit 1
fi
printf '%s\n' "$applied" | grep -q 'updated'

node - "$all_home" "$TOKEN" <<'CHECK'
const fs = require("node:fs");
const path = require("node:path");
const home = process.argv[2];
const token = process.argv[3];
const auth = "Bearer " + token;
const mcp = "https://agent.idou.bid/mcp";
function read(rel) { return JSON.parse(fs.readFileSync(path.join(home, rel), "utf8")); }
function entry(rel, key) {
  const doc = read(rel);
  const got = doc[key]["agent-arena"];
  if (!got) throw new Error("missing " + rel);
  return got;
}
const claude = entry(".claude.json", "mcpServers");
if (claude.url !== mcp || claude.headers.Authorization !== auth) throw new Error("claude-code");
const desk = entry(".config/Claude/claude_desktop_config.json", "mcpServers");
if (desk.command !== "npx" || !desk.args.includes("mcp-remote") || !desk.args.includes("Authorization: " + auth)) throw new Error("claude-desktop");
const cursor = entry(".cursor/mcp.json", "mcpServers");
if (cursor.url !== mcp || cursor.headers.Authorization !== auth) throw new Error("cursor");
const code = entry(".config/Code/User/mcp.json", "servers");
if (code.type !== "http" || code.url !== mcp) throw new Error("vscode");
const oc = entry(".config/opencode/opencode.json", "mcp");
if (oc.type !== "remote" || oc.oauth !== false || oc.headers.Authorization !== auth || !oc.enabled) throw new Error("opencode");
const ocDoc = read(".config/opencode/opencode.json");
if (ocDoc.mcp.other.command[0] !== "echo") throw new Error("opencode other");
if ((ocDoc.permission.external_directory || {})["~/.arena/**"] !== "allow") throw new Error("opencode external_directory permission");
if ((ocDoc.permission.bash || {})["node *sign.mjs*"] !== "allow") throw new Error("opencode bash permission");
const gem = entry(".gemini/settings.json", "mcpServers");
if (gem.httpUrl !== mcp || gem.url) throw new Error("gemini");
const wind = entry(".codeium/windsurf/mcp_config.json", "mcpServers");
if (wind.serverUrl !== mcp || wind.headers.Authorization !== auth) throw new Error("windsurf");
const cline = entry(".config/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json", "mcpServers");
if (cline.type !== "streamableHttp" || cline.url !== mcp) throw new Error("cline");
const roo = entry(".config/Code/User/globalStorage/rooveterinaryinc.roo-cline/settings/mcp_settings.json", "mcpServers");
if (roo.type !== "streamable-http" || roo.url !== mcp) throw new Error("roo");
const zed = read(".config/zed/settings.json");
if (zed.theme !== "keep" || zed.context_servers.other.command !== "echo") throw new Error("zed kept");
if (zed.context_servers["agent-arena"].url !== mcp) throw new Error("zed");
const qwen = entry(".qwen/settings.json", "mcpServers");
if (qwen.httpUrl !== mcp) throw new Error("qwen");
const kiro = entry(".kiro/settings/mcp.json", "mcpServers");
if (kiro.url !== mcp || kiro.headers.Authorization !== auth) throw new Error("kiro");
const trae = entry(".config/Trae/User/mcp.json", "mcpServers");
if (trae.url !== mcp) throw new Error("trae");
const toml = fs.readFileSync(path.join(home, ".codex/config.toml"), "utf8");
if (!toml.includes('model = "keep-me"') || !toml.includes("[mcp_servers.other]") || !toml.includes('command = "echo"')) throw new Error("codex kept");
if (!toml.includes("[mcp_servers.agent-arena]") || !toml.includes(auth) || !toml.includes(mcp)) throw new Error("codex");
const goose = fs.readFileSync(path.join(home, ".config/goose/config.yaml"), "utf8");
if (!goose.includes("provider: keep") || !goose.includes("command: echo")) throw new Error("goose kept");
if (!goose.includes("type: streamable_http") || !goose.includes(auth)) throw new Error("goose");
const cont = fs.readFileSync(path.join(home, ".continue/mcpServers/agent-arena.yaml"), "utf8");
if (!cont.includes("type: streamable-http") || !cont.includes(auth) || !cont.includes("schema: v1")) throw new Error("continue");
CHECK

again_all=$(sh scripts/connect.sh --skip-network --clients "$clients" --token "$TOKEN" --home "$all_home" --base-url "https://agent.idou.bid")
printf '%s\n' "$again_all" | grep -q 'unchanged'
if printf '%s\n' "$again_all" | grep -q 'updated'; then
  printf '%s\n' "second apply still updated a client" >&2
  exit 1
fi
codex_backs=$(find "$all_home/.codex" -name 'config.toml.bak.*' | wc -l | tr -d ' ')
goose_backs=$(find "$all_home/.config/goose" -name 'config.yaml.bak.*' | wc -l | tr -d ' ')
zed_backs=$(find "$all_home/.config/zed" -name 'settings.json.bak.*' | wc -l | tr -d ' ')
if [ "$codex_backs" != "1" ] || [ "$goose_backs" != "1" ] || [ "$zed_backs" != "1" ]; then
  printf '%s\n' "expected one backup each for codex/goose/zed ($codex_backs $goose_backs $zed_backs)" >&2
  exit 1
fi

# opencode.jsonc: comments/trailing commas tolerated, .json never created,
# ~/.arena 与 sign.mjs 预授权，其他 server 与用户 permission 保留。
jsonc_home=$(mktemp -d)
mkdir -p "$jsonc_home/.config/opencode"
printf '%s\n' '// 用户注释' '{' '  "mcp": {' '    "other": {' '      "type": "local",' '      "command": ["echo"],' '    },' '  },' '  "permission": {' '    "webextension": { "*": "ask" },' '  },' '}' > "$jsonc_home/.config/opencode/opencode.jsonc"

applied_jsonc=$(sh scripts/connect.sh --skip-network --clients opencode --token "$TOKEN" --home "$jsonc_home" --base-url "https://agent.idou.bid")
if printf '%s\n' "$applied_jsonc" | grep -q "$TOKEN"; then
  printf '%s\n' "jsonc apply printed the token" >&2
  exit 1
fi
printf '%s\n' "$applied_jsonc" | grep -q 'updated'
node - "$jsonc_home" "$TOKEN" <<'CHECKJSONC'
const fs = require("node:fs");
const path = require("node:path");
const home = process.argv[2];
const token = process.argv[3];
const auth = "Bearer " + token;
const mcp = "https://agent.idou.bid/mcp";
const file = path.join(home, ".config/opencode/opencode.jsonc");
const doc = JSON.parse(fs.readFileSync(file, "utf8"));
if (fs.existsSync(file.replace(/\.jsonc$/, ".json"))) throw new Error("opencode.json was created");
const oc = doc.mcp["agent-arena"];
if (!oc || oc.type !== "remote" || oc.oauth !== false || oc.headers.Authorization !== auth || !oc.enabled) throw new Error("jsonc opencode entry");
const other = doc.mcp.other;
if (!other || other.type !== "local" || other.command[0] !== "echo") throw new Error("jsonc other lost");
if ((doc.permission.external_directory || {})["~/.arena/**"] !== "allow") throw new Error("jsonc external_directory permission");
if ((doc.permission.bash || {})["node *sign.mjs*"] !== "allow") throw new Error("jsonc bash permission");
if ((doc.permission.webextension || {})["*"] !== "ask") throw new Error("jsonc user permission lost");
CHECKJSONC
find "$jsonc_home/.config/opencode" -name 'opencode.jsonc.bak.*' | grep -q .

again_jsonc=$(sh scripts/connect.sh --skip-network --clients opencode --token "$TOKEN" --home "$jsonc_home" --base-url "https://agent.idou.bid")
printf '%s\n' "$again_jsonc" | grep -q 'unchanged'
jsonc_backs=$(find "$jsonc_home/.config/opencode" -name 'opencode.jsonc.bak.*' | wc -l | tr -d ' ')
if [ "$jsonc_backs" != "1" ]; then
  printf '%s\n' "expected one jsonc backup, found $jsonc_backs" >&2
  exit 1
fi

# --clients writes only the named client.
only=$(mktemp -d)
mkdir -p "$only/.claude"
sh scripts/connect.sh --skip-network --clients opencode --token "$TOKEN" --home "$only" --base-url "https://agent.idou.bid" >/dev/null
if [ -f "$only/.claude.json" ]; then
  printf '%s\n' "--clients opencode also wrote claude" >&2
  exit 1
fi
grep -q "$TOKEN" "$only/.config/opencode/opencode.json"

# No TTY and no --clients/--yes must fail instead of blocking.
if setsid -w sh scripts/connect.sh --dry-run --token "$TOKEN" --home "$only" --base-url "https://agent.idou.bid" >/tmp/arena-notty.out 2>/tmp/arena-notty.err; then
  printf '%s\n' "no-tty run should have failed" >&2
  exit 1
fi
grep -q '没有终端' /tmp/arena-notty.err

# Interactive menu: empty enter keeps the detected set; "n" writes nothing.
menu_home=$(mktemp -d)
mkdir -p "$menu_home/.claude" "$menu_home/.cursor"
python3 - "$menu_home" "$TOKEN" <<'PY'
import os, pty, select, sys, time
home, token = sys.argv[1], sys.argv[2]
# 机器上可能全局装着 opencode；从子进程 PATH 剥掉含 opencode 二进制的目录，
# 让检测结果只来自 HOME_DIR，测试才可复现。
clean = [p for p in os.environ.get("PATH", "").split(os.pathsep) if p and not os.path.exists(os.path.join(p, "opencode"))]
env = dict(os.environ, PATH=os.pathsep.join(clean))
pid, fd = pty.fork()
if pid == 0:
    os.execvpe("sh", ["sh", "scripts/connect.sh", "--skip-network", "--token", token, "--home", home, "--base-url", "https://agent.idou.bid"], env)
data = b""
sent = False
deadline = time.time() + 15
while time.time() < deadline:
    readable, _, _ = select.select([fd], [], [], max(0, deadline - time.time()))
    if fd in readable:
        try:
            chunk = os.read(fd, 4096)
        except OSError:
            break
        if not chunk:
            break
        data += chunk
        if (not sent) and "输入编号".encode() in data:
            os.write(fd, b"\n")
            sent = True
    else:
        os.kill(pid, 15)
        raise SystemExit("menu timed out")
else:
    os.kill(pid, 15)
    raise SystemExit("menu timed out")
_, status = os.waitpid(pid, 0)
text = data.decode("utf-8", "replace")
if token in text:
    raise SystemExit("menu output printed the token")
if not sent:
    raise SystemExit("menu prompt missing\n" + text)
if os.WEXITSTATUS(status) != 0:
    raise SystemExit("menu script failed\n" + text)
open("/tmp/arena-menu.out", "w").write(text)
PY
grep -q 'client: claude-code' /tmp/arena-menu.out
grep -q 'MCP '"$menu_home"'/.claude.json + 技能 '"$menu_home"'/.claude/skills/arena' /tmp/arena-menu.out
grep -q 'client: cursor' /tmp/arena-menu.out
grep -q "$TOKEN" "$menu_home/.claude.json"
grep -q "$TOKEN" "$menu_home/.cursor/mcp.json"
if [ -f "$menu_home/.config/opencode/opencode.json" ]; then
  printf '%s\n' "empty menu wrote an undetected client" >&2
  exit 1
fi

skip_home=$(mktemp -d)
mkdir -p "$skip_home/.claude"
python3 - "$skip_home" "$TOKEN" <<'PY'
import os, pty, select, sys, time
home, token = sys.argv[1], sys.argv[2]
clean = [p for p in os.environ.get("PATH", "").split(os.pathsep) if p and not os.path.exists(os.path.join(p, "opencode"))]
env = dict(os.environ, PATH=os.pathsep.join(clean))
pid, fd = pty.fork()
if pid == 0:
    os.execvpe("sh", ["sh", "scripts/connect.sh", "--skip-network", "--token", token, "--home", home, "--base-url", "https://agent.idou.bid"], env)
data = b""
sent = False
deadline = time.time() + 15
while time.time() < deadline:
    readable, _, _ = select.select([fd], [], [], max(0, deadline - time.time()))
    if fd in readable:
        try:
            chunk = os.read(fd, 4096)
        except OSError:
            break
        if not chunk:
            break
        data += chunk
        if (not sent) and "输入编号".encode() in data:
            os.write(fd, b"n\n")
            sent = True
    else:
        os.kill(pid, 15)
        raise SystemExit("skip menu timed out")
else:
    os.kill(pid, 15)
    raise SystemExit("skip menu timed out")
os.waitpid(pid, 0)
if not sent:
    raise SystemExit("skip prompt missing")
PY
if [ -f "$skip_home/.claude.json" ]; then
  printf '%s\n' "n wrote a client config" >&2
  exit 1
fi

# Skill install: symlink or rules file for every selected client. Central tree is the source.
skill_home=$(mktemp -d)
skill_src=$(mktemp -d)
cp -R "$ROOT/skills/arena" "$skill_src/arena"
clients="claude-code,claude-desktop,cursor,vscode,opencode,codex,gemini,windsurf,cline,roo,zed,qwen,kiro,trae,continue,goose"
# A previous copy should be replaced by a symlink.
mkdir -p "$skill_home/.claude/skills/arena"
printf '%s\n' "old copy" > "$skill_home/.claude/skills/arena/SKILL.md"
ARENA_SKILL_SRC="$skill_src/arena" sh scripts/connect.sh --skip-network --skill-only --clients "$clients" --token "$TOKEN" --home "$skill_home" --base-url "https://agent.idou.bid" >/tmp/arena-skill.out
central="$skill_home/.arena/skill/arena"
if [ ! -f "$central/SKILL.md" ] || [ ! -f "$central/scripts/sign.mjs" ] || [ ! -f "$central/scripts/sign-lock.mjs" ]; then
  printf '%s\n' "central skill missing" >&2
  exit 1
fi
for link in \
  .claude/skills/arena \
  .cursor/skills/arena \
  .config/opencode/skills/arena \
  .agents/skills/arena \
  .gemini/skills/arena \
  .codeium/windsurf/skills/arena \
  .qwen/skills/arena \
  .kiro/skills/arena \
  .trae/skills/arena
do
  if [ ! -L "$skill_home/$link" ]; then
    printf '%s\n' "expected symlink $link" >&2
    exit 1
  fi
  if [ "$(readlink "$skill_home/$link")" != "$central" ]; then
    printf '%s\n' "symlink $link does not point at the central skill" >&2
    exit 1
  fi
done
if [ -f "$skill_home/.claude.json" ]; then
  printf '%s\n' "--skill-only wrote MCP config" >&2
  exit 1
fi
for rules in \
  .copilot/instructions/arena.instructions.md \
  .cline/rules/arena.md \
  .roo/rules/arena.md \
  .continue/rules/arena.md
do
  grep -q "$central/SKILL.md" "$skill_home/$rules"
  grep -q "$central/scripts/sign.mjs" "$skill_home/$rules"
done
python3 - "$skill_home/.arena/skill/arena-skill.zip" <<'PY'
import sys, zipfile
z = zipfile.ZipFile(sys.argv[1])
names = z.namelist()
if not {"arena/SKILL.md", "arena/scripts/sign.mjs", "arena/scripts/sign-lock.mjs"}.issubset(names):
    raise SystemExit("desktop zip missing skill files: " + ",".join(names))
if any(not n.startswith("arena/") for n in names):
    raise SystemExit("desktop zip root is not arena/")
PY
grep -q '手动上传' /tmp/arena-skill.out
# Re-run updates the central tree in place and leaves the symlink.
printf '\n%s\n' "second pass" >> "$skill_src/arena/SKILL.md"
again_skill=$(ARENA_SKILL_SRC="$skill_src/arena" sh scripts/connect.sh --skip-network --skill-only --clients claude-code,codex,goose,zed --token "$TOKEN" --home "$skill_home" --base-url "https://agent.idou.bid")
printf '%s\n' "$again_skill" | grep -q 'unchanged'
grep -q 'second pass' "$skill_home/.claude/skills/arena/SKILL.md"
grep -q 'second pass' "$skill_home/.agents/skills/arena/SKILL.md"

mcp_only=$(mktemp -d)
mkdir -p "$mcp_only/.cursor"
ARENA_SKILL_SRC="$skill_src/arena" sh scripts/connect.sh --skip-network --mcp-only --clients cursor --token "$TOKEN" --home "$mcp_only" --base-url "https://agent.idou.bid" >/dev/null
if [ -e "$mcp_only/.arena" ] || [ -e "$mcp_only/.cursor/skills" ]; then
  printf '%s\n' "--mcp-only installed a skill" >&2
  exit 1
fi
grep -q "$TOKEN" "$mcp_only/.cursor/mcp.json"
if sh scripts/connect.sh --dry-run --skill-only --mcp-only --yes --token "$TOKEN" --home "$mcp_only" --base-url "https://agent.idou.bid" >/dev/null 2>&1; then
  printf '%s\n' "both skill flags should fail" >&2
  exit 1
fi

# Wrappers stay thin and call the package. The writer lives in one place.
python3 - <<'PY'
from pathlib import Path
sh = Path("scripts/connect.sh").read_text()
ps = Path("scripts/connect.ps1").read_text()
js = Path("packages/cli/lib/write.cjs").read_text()
if "/* arena-connect-writer */" in sh or "/* arena-connect-writer */" in ps:
    raise SystemExit("wrappers should not embed the writer")
if "npx -y github:hkmdxlftjf/agent-arena" not in sh:
    raise SystemExit("connect.sh should call npx github:hkmdxlftjf/agent-arena")
if "github:hkmdxlftjf/agent-arena" not in ps:
    raise SystemExit("connect.ps1 should call npx github:hkmdxlftjf/agent-arena")
if "agent-arena@latest" in sh or "agent-arena@latest" in ps:
    raise SystemExit("wrappers should not call the npm registry")
if not js.startswith("/* arena-connect-writer */"):
    raise SystemExit("package writer missing")
pkg = Path("packages/cli/package.json").read_text()
if '"dependencies"' in pkg:
    raise SystemExit("agent-arena package must stay dependency-free")
PY

node --test scripts/test-connect-key.mjs
node "$central/scripts/sign.mjs" version >/dev/null

rm -rf "$HOME_DIR" "$all_home" "$only" "$menu_home" "$skip_home" "$skill_home" "$skill_src" "$mcp_only" "$jsonc_home"
printf '%s\n' "connect.sh ok"
