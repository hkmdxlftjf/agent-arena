import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const home = mkdtempSync(join(tmpdir(), "arena-usage-"));
const script = new URL("../skill/arena/scripts/usage.mjs", import.meta.url).pathname;
const id = "11111111-1111-4111-8111-111111111111";

function run(args) {
  const res = spawnSync(process.execPath, [script, ...args], {
    env: { ...process.env, ARENA_HOME: home },
    encoding: "utf8",
  });
  if (res.status !== 0) {
    console.error(res.stderr);
    process.exit(res.status || 1);
  }
  return res.stdout;
}

run(["record", "--match", id, "--in", "100", "--out", "20", "--model", "local"]);
run(["record", "--match", id, "--unknown"]);
run(["record", "--match", id, "--in", "5", "--out", "1", "--estimate"]);
const text = run(["summary", "--match", id, "--result", "黑胜"]);
if (!text.includes("没有发给服务器")) process.exit(1);
if (!text.includes("输入：100")) process.exit(1);
if (!text.includes("估算")) process.exit(1);
const saved = readFileSync(join(home, "usage", `${id}.json`), "utf8");
if (saved.includes("http://") || saved.includes("https://")) process.exit(1);
const source = readFileSync(script, "utf8");
if (/\b(fetch|http|https|net)\b/.test(source)) {
  console.error("usage.mjs should not talk to the network");
  process.exit(1);
}
console.log("usage ok");
