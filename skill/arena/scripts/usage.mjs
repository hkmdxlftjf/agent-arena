#!/usr/bin/env node
// Local token notes for one match. This file never opens a network connection.
import { chmodSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

const CAP = 10_000_000;

function die(message) {
  console.error(message);
  process.exit(2);
}

function home() {
  return process.env.ARENA_HOME || join(homedir(), ".arena");
}

function arg(name) {
  const i = process.argv.indexOf(name);
  if (i < 0) return null;
  return process.argv[i + 1] ?? null;
}

function flag(name) {
  return process.argv.includes(name);
}

function matchId() {
  const id = arg("--match");
  if (!id || !/^[0-9a-fA-F-]{36}$/.test(id)) die("--match must be a UUID");
  return id;
}

function fileOf(id) {
  return join(home(), "usage", `${id}.json`);
}

function load(id) {
  try {
    return JSON.parse(readFileSync(fileOf(id), "utf8"));
  } catch {
    return { match_id: id, started_at: new Date().toISOString(), moves: [] };
  }
}

function save(id, doc) {
  const dir = join(home(), "usage");
  mkdirSync(dir, { recursive: true, mode: 0o700 });
  const path = fileOf(id);
  writeFileSync(path, `${JSON.stringify(doc, null, 2)}\n`, { mode: 0o600 });
  try {
    chmodSync(path, 0o600);
  } catch {
    /* windows */
  }
}

function integer(name) {
  const raw = arg(name);
  if (raw == null) return null;
  if (!/^\d+$/.test(raw)) die(`${name} must be a non-negative integer`);
  const n = Number(raw);
  if (n > CAP) die(`${name} is above ${CAP}`);
  return n;
}

function record(id) {
  const doc = load(id);
  const unknown = flag("--unknown");
  const estimate = flag("--estimate");
  const input = integer("--in");
  const output = integer("--out");
  const cached = integer("--cached");
  const model = arg("--model");
  if (model && model.length > 80) die("--model is too long");
  if (!unknown && (input == null || output == null)) {
    die("pass --in and --out, or --unknown when the client has no counts");
  }
  doc.moves.push({
    at: new Date().toISOString(),
    input,
    output,
    cached,
    model: model || null,
    unknown,
    estimate: estimate && !unknown,
  });
  save(id, doc);
  if (unknown) console.log("recorded: client did not expose token counts");
  else if (estimate) console.log("recorded: estimate, not a client reading");
  else console.log("recorded");
}

function duration(doc) {
  const start = Date.parse(doc.started_at);
  if (!Number.isFinite(start)) return "未知";
  const sec = Math.max(0, Math.round((Date.now() - start) / 1000));
  const m = Math.floor(sec / 60);
  const s = sec % 60;
  return m > 0 ? `${m} 分 ${s} 秒` : `${s} 秒`;
}

function summary(id) {
  const doc = load(id);
  const result = arg("--result") || "未注明";
  const moves = doc.moves || [];
  const counted = moves.filter((m) => !m.unknown && m.input != null && m.output != null);
  const measured = counted.filter((m) => !m.estimate);
  const estimates = counted.filter((m) => m.estimate);
  const unknown = moves.filter((m) => m.unknown).length;
  const use = measured.length ? measured : estimates;
  const label = measured.length ? "" : estimates.length ? "（估算，不是客户端读数）" : "";
  const input = use.reduce((n, m) => n + m.input, 0);
  const output = use.reduce((n, m) => n + m.output, 0);
  const n = Number(arg("--moves") || moves.length || use.length || 0);
  const avgIn = use.length ? Math.round(input / use.length) : 0;
  const avgOut = use.length ? Math.round(output / use.length) : 0;
  const model = [...use].reverse().find((m) => m.model)?.model;
  const lines = [
    `本局用量（只留在这台机器，没有发给服务器）${label}`,
    `结果：${result}`,
    `用时：${duration(doc)}`,
    `步数：${n}`,
  ];
  if (!use.length) {
    lines.push("客户端没有提供 token 计数，这里无法统计。");
  } else {
    lines.push(`输入：${input}`);
    lines.push(`输出：${output}`);
    lines.push(`每步平均：输入 ${avgIn}，输出 ${avgOut}`);
    if (model) lines.push(`模型：${model}`);
  }
  if (unknown) lines.push(`其中 ${unknown} 步客户端没有给出计数。`);
  if (measured.length && estimates.length) {
    lines.push(`另有 ${estimates.length} 步标成估算，没有算进上面的合计。`);
  }
  console.log(lines.join("\n"));
}

const cmd = process.argv[2];
if (cmd === "record") record(matchId());
else if (cmd === "summary") summary(matchId());
else {
  die("usage: usage.mjs record|summary --match <uuid> [--in N --out N | --unknown] [--estimate] [--result text]");
}
