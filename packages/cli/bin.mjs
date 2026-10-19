#!/usr/bin/env node
import { main } from "./lib/connect.mjs";

try {
  const args = process.argv.slice(2);
  const code = args[0] === "serve"
    ? await (await import("./lib/gateway.mjs")).serve(args.slice(1))
    : await main(args);
  process.exitCode = code;
} catch {
  process.stderr.write("agent-arena: startup failed\n");
  process.exitCode = 1;
}
