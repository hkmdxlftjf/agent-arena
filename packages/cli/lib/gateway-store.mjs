import { randomBytes } from "node:crypto";
import { hostname } from "node:os";
import { join } from "node:path";
import { mkdir, open, readFile, link, unlink, rename, realpath, lstat } from "node:fs/promises";

export class GatewayError extends Error {
  constructor(code) {
    super(code);
    this.code = code;
  }
}

export const nonce = () => randomBytes(24).toString("hex");
export const canonical = value => {
  if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`;
  if (value && typeof value === "object") {
    return `{${Object.keys(value).filter(key => value[key] !== undefined).sort()
      .map(key => `${JSON.stringify(key)}:${canonical(value[key])}`).join(",")}}`;
  }
  return JSON.stringify(value);
};
export const equal = (a, b) => canonical(a) === canonical(b);

export async function readJSON(path) {
  try { return JSON.parse(await readFile(path, "utf8")); }
  catch (error) {
    if (error.code === "ENOENT") return undefined;
    throw error;
  }
}

async function syncDirectory(path) {
  if (process.platform === "win32") return;
  const directory = await open(path, "r");
  try { await directory.sync(); } finally { await directory.close(); }
}

// A hard link publishes complete, fsynced content without replacing a winner.
export async function saveJSON(directory, name, value, exclusive = false) {
  const temporary = join(directory, `.tmp-${nonce()}`);
  const file = await open(temporary, "wx", 0o600);
  try {
    await file.writeFile(JSON.stringify(value));
    await file.sync();
  } finally { await file.close(); }
  try {
    if (exclusive) await link(temporary, join(directory, name));
    else await rename(temporary, join(directory, name));
    await syncDirectory(directory);
    return true;
  } catch (error) {
    if (exclusive && error.code === "EEXIST") return false;
    throw error;
  } finally {
    await unlink(temporary).catch(error => {
      if (error.code !== "ENOENT") throw error;
    });
  }
}

const queues = new Map();

export class Journal {
  constructor(home) {
    this.home = home;
    this.directory = join(home, "gateway");
  }

  async initialize() {
    await mkdir(this.home, { recursive: true, mode: 0o700 });
    this.home = await realpath(this.home);
    this.directory = join(this.home, "gateway");
    for (const part of ["", "tokens", "submissions", "locks"]) {
      const path = join(this.directory, part);
      await mkdir(path, { recursive: true, mode: 0o700 });
      const stat = await lstat(path);
      if (!stat.isDirectory() || stat.isSymbolicLink()
          || (process.platform !== "win32" && (stat.mode & 0o077))) {
        throw new GatewayError("insecure_gateway_directory");
      }
    }
  }

  async exclusive(fn) {
    const previous = queues.get(this.home) || Promise.resolve();
    let finish;
    const current = new Promise(resolve => { finish = resolve; });
    queues.set(this.home, current);
    await previous;
    let owner;
    try {
      owner = await this.acquire();
      return await fn();
    } finally {
      try {
        if (owner) await saveJSON(join(this.directory, "locks"), `${owner}.released`, true, true);
      } finally {
        finish();
        if (queues.get(this.home) === current) queues.delete(this.home);
      }
    }
  }

  // Immutable successor links avoid the check/unlink race in stale lock stealing.
  // A successor can be installed only after release or positive ESRCH evidence.
  // Live owners, PID reuse, other hosts and uncertain liveness all fail closed.
  async acquire() {
    const directory = join(this.directory, "locks");
    let name = "head.json";
    for (;;) {
      const owner = await readJSON(join(directory, name));
      if (!owner) {
        const mine = { nonce: nonce(), pid: process.pid, host: hostname() };
        if (await saveJSON(directory, name, mine, true)) return mine.nonce;
        continue;
      }
      if (!/^[a-f0-9]{48}$/.test(owner.nonce) || !Number.isSafeInteger(owner.pid) || owner.pid <= 0) {
        throw new GatewayError("gateway_lock_invalid");
      }
      const next = `${owner.nonce}.next.json`;
      if (await readJSON(join(directory, next))) {
        name = next;
        continue;
      }
      const released = await readJSON(join(directory, `${owner.nonce}.released`));
      if (!released) {
        if (owner.host !== hostname()) throw new GatewayError("gateway_busy");
        try {
          process.kill(owner.pid, 0);
          throw new GatewayError("gateway_busy");
        } catch (error) {
          if (error.code !== "ESRCH") throw new GatewayError("gateway_busy");
        }
      }
      name = next;
    }
  }

  async bind(identity) {
    const path = join(this.directory, "identity.json");
    const existing = await readJSON(path);
    if (existing) {
      if (!equal(existing, identity)) throw new GatewayError("identity_mismatch");
      return;
    }
    await saveJSON(this.directory, "identity.json", identity, true);
    if (!equal(await readJSON(path), identity)) throw new GatewayError("identity_mismatch");
  }

  async token(value) {
    const token = nonce();
    await saveJSON(join(this.directory, "tokens"), `${token}.json`, value, true);
    return token;
  }

  async getToken(token) {
    if (typeof token !== "string" || !/^[a-f0-9]{48}$/.test(token)) return undefined;
    return readJSON(join(this.directory, "tokens", `${token}.json`));
  }

  async submission(id) {
    if (typeof id !== "string" || !/^sub_[a-f0-9]{64}$/.test(id)) return undefined;
    return readJSON(join(this.directory, "submissions", `${id}.json`));
  }

  async save(record, exclusive = false) {
    return saveJSON(join(this.directory, "submissions"), `${record.submission_id}.json`, record, exclusive);
  }
}
