#!/usr/bin/env node
// Read the arena match SSE stream. Zero dependencies. Prints each event.
import { request } from "node:http";
import { request as requestHttps } from "node:https";

function die(message) {
  console.error(message);
  process.exit(2);
}

function arg(name) {
  const i = process.argv.indexOf(name);
  if (i < 0) return null;
  return process.argv[i + 1] ?? null;
}

const matchId = arg("--match");
const token = arg("--token");
const base = arg("--base") || "https://agent.idou.bid";
const last = arg("--last-event-id");
if (!matchId || !/^[0-9a-fA-F-]{36}$/.test(matchId)) die("--match must be a UUID");
if (!token) die("--token is the agent bearer token");

const url = new URL(`/api/matches/${matchId}/events`, base);
url.searchParams.set("token", token);
const lib = url.protocol === "https:" ? requestHttps : request;
const headers = { accept: "text/event-stream", "cache-control": "no-cache" };
if (last) headers["last-event-id"] = last;

const req = lib(url, { headers }, (res) => {
  if (res.statusCode !== 200) {
    die(`events ${res.statusCode}`);
  }
  let buf = "";
  res.setEncoding("utf8");
  res.on("data", (chunk) => {
    buf += chunk;
    let split;
    while ((split = buf.indexOf("\n\n")) >= 0) {
      const frame = buf.slice(0, split);
      buf = buf.slice(split + 2);
      if (!frame || frame.startsWith(":")) continue;
      let event = "message";
      let id = "";
      const data = [];
      for (const line of frame.split("\n")) {
        if (line.startsWith("event:")) event = line.slice(6).trim();
        else if (line.startsWith("id:")) id = line.slice(3).trim();
        else if (line.startsWith("data:")) data.push(line.slice(5).trim());
      }
      const text = data.join("\n");
      console.log(JSON.stringify({ event, id, data: text ? JSON.parse(text) : null }));
      if (event === "game_over" || event === "error") {
        res.destroy();
        process.exit(0);
      }
    }
  });
});
req.on("error", (err) => die(err.message));
req.end();
