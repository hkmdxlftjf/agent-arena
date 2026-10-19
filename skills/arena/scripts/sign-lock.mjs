// Node >=18, zero dependencies. All signers sharing a home use this lock.
import { randomBytes } from 'node:crypto';
import { hostname } from 'node:os';
import { dirname, join } from 'node:path';
import { closeSync, fsyncSync, mkdirSync, openSync, readFileSync, readdirSync,
         renameSync, rmdirSync, unlinkSync, writeFileSync } from 'node:fs';

export function acquireSignLock(home) {
  const lock = join(home, 'signed.lock');
  mkdirSync(home, { recursive: true, mode: 0o700 });
  for (;;) {
    try {
      mkdirSync(lock, { mode: 0o700 });
      break;
    } catch (err) {
      if (err.code !== 'EEXIST') throw err;
    }
    let names, owner;
    try {
      names = readdirSync(lock);
      if (names.length !== 1 || !/^owner-[0-9a-f]{32}$/.test(names[0]))
        throw new Error('signing lock has incomplete ownership metadata');
      owner = JSON.parse(readFileSync(join(lock, names[0]), 'utf8'));
    } catch (err) {
      if (err.code === 'ENOENT') continue;
      if (err instanceof SyntaxError) throw new Error('signing lock has incomplete ownership metadata');
      throw err;
    }
    if (!Number.isSafeInteger(owner?.pid) || owner.pid <= 0 || owner.host !== hostname())
      throw new Error('signing lock has invalid or foreign ownership metadata');
    try {
      process.kill(owner.pid, 0);
      throw new Error(`signing lock busy: process ${owner.pid}`);
    } catch (err) {
      if (err.code !== 'ESRCH') throw err;
    }
    // Only the process that unlinks this unique owner's file may rmdir.
    // Successful unlink elects one remover; losers never remove an empty lock.
    // No successor can mkdir until that elected remover finishes rmdir.
    // A stale observer cannot unlink a successor's different owner file.
    // Never recursively remove the directory: an empty lock fails closed.
    try { unlinkSync(join(lock, names[0])); }
    catch (err) { if (err.code === 'ENOENT') continue; throw err; }
    rmdirSync(lock);
  }
  const ownerPath = join(lock, `owner-${randomBytes(16).toString('hex')}`);
  try {
    writeFileSync(ownerPath, JSON.stringify({ pid: process.pid, host: hostname() }),
      { flag: 'wx', mode: 0o600 });
  } catch (err) {
    try { unlinkSync(ownerPath); } catch (cleanup) { if (cleanup.code !== 'ENOENT') throw cleanup; }
    rmdirSync(lock);
    throw err;
  }
  return () => {
    unlinkSync(ownerPath);
    rmdirSync(lock);
  };
}

export function writeSignedAtomic(path, data) {
  const temp = `${path}.${randomBytes(16).toString('hex')}.tmp`;
  const fd = openSync(temp, 'wx', 0o600);
  try {
    try { writeFileSync(fd, data); fsyncSync(fd); }
    finally { closeSync(fd); }
    renameSync(temp, path);
    if (process.platform !== 'win32') {
      const dir = openSync(dirname(path), 'r');
      try { fsyncSync(dir); } finally { closeSync(dir); }
    }
  } finally {
    try { unlinkSync(temp); } catch (err) { if (err.code !== 'ENOENT') throw err; }
  }
}
