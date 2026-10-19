import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdtempSync, readFileSync, readdirSync, rmSync, mkdirSync, writeFileSync,
         statSync, utimesSync, chmodSync, openSync, closeSync } from 'node:fs';
import { hostname, tmpdir } from 'node:os';
import { join } from 'node:path';
import { createPublicKey, verify, createHash } from 'node:crypto';
import { test } from 'node:test';

const signer = new URL('../skills/arena/scripts/sign.mjs', import.meta.url);
const helper = new URL('../skills/arena/scripts/sign-lock.mjs', import.meta.url);
const request = {
  domain: { name: 'AgentArena', version: '1', origin: 'agent.idou.bid', purpose: 'move' },
  payload: { match_id: 'lock-test', seat: 0, seq: 0, prev_hash: '00'.repeat(32),
    deadline_at: Number.MAX_SAFE_INTEGER },
};
const action = { type: 'place', data: { at: 'h8' } };
const options = { timeout: 20000 };

function child(t, home, args) {
  const p = spawn(process.execPath, args, {
    env: { ...process.env, ARENA_HOME: home, ARENA_KEY_PASSPHRASE: '',
      ARENA_ALLOWED_ORIGINS: 'agent.idou.bid' },
    stdio: ['pipe', 'pipe', 'pipe', 'ipc'],
  });
  let stdout = '', stderr = '';
  p.stdout.on('data', data => { stdout += data; });
  p.stderr.on('data', data => { stderr += data; });
  const done = new Promise((resolve, reject) => {
    p.once('error', reject);
    p.once('close', (code, signal) => resolve({ code, signal, stdout, stderr }));
  });
  t.after(() => { if (p.exitCode === null && p.signalCode === null) p.kill('SIGKILL'); });
  return { p, done };
}

async function run(t, home, args, input) {
  const c = child(t, home, [signer.pathname, ...args]);
  c.p.stdin.end(input);
  return c.done;
}

async function setup(t) {
  const home = mkdtempSync(join(tmpdir(), 'arena-sign-lock-'));
  t.after(() => rmSync(home, { recursive: true, force: true }));
  const result = await run(t, home, ['keygen']);
  assert.equal(result.code, 0, result.stderr);
  return { home, key: JSON.parse(result.stdout) };
}

function sign(t, home, req = request, move = action, flags = []) {
  return run(t, home, ['sign', '-', '--action', JSON.stringify(move), ...flags], JSON.stringify(req));
}

function journal(home) { return JSON.parse(readFileSync(join(home, 'signed.json'), 'utf8')); }
function unlocked(home) { assert.ok(!readdirSync(home).includes('signed.lock')); }
function canonical(v) {
  if (Array.isArray(v)) return '[' + v.map(canonical).join(',') + ']';
  if (v !== null && typeof v === 'object')
    return '{' + Object.keys(v).sort().map(k => JSON.stringify(k) + ':' + canonical(v[k])).join(',') + '}';
  return JSON.stringify(v);
}
function validSignature(envelope, pub) {
  const hash = v => createHash('sha256').update(canonical(v)).digest();
  const bytes = Buffer.concat([Buffer.from('\x19AgentArena Signed Message:\n'),
    hash(envelope.domain), hash(envelope.payload)]);
  assert.ok(verify(null, bytes, createPublicKey({
    key: { kty: 'OKP', crv: 'Ed25519', x: pub }, format: 'jwk',
  }), Buffer.from(envelope.signature, 'base64url')));
}

async function holder(t, home) {
  const c = child(t, home, ['--input-type=module', '-e', `
    import { acquireSignLock } from ${JSON.stringify(helper.href)};
    const release = acquireSignLock(process.env.ARENA_HOME);
    process.once('message', () => { release(); process.disconnect(); });
    process.send('locked');
  `]);
  assert.deepEqual(await once(c.p, 'message'), ['locked', undefined]);
  return c;
}

// Every contender subscribes before the parent releases the start barrier.
async function contend(t, home, moves) {
  const children = moves.map(move => child(t, home, ['--input-type=module', '-e', `
    process.argv = ['node', ${JSON.stringify(signer.pathname)}, 'sign', '-',
      '--action', ${JSON.stringify(JSON.stringify(move))}];
    process.once('message', async () => {
      await import(${JSON.stringify(signer.href)});
      process.disconnect();
    });
    process.send('ready');
  `]));
  await Promise.all(children.map(async c => {
    const ready = once(c.p, 'message');
    c.p.stdin.end(JSON.stringify(request));
    assert.equal((await ready)[0], 'ready');
  }));
  for (const c of children) c.p.send('sign');
  return Promise.all(children.map(c => c.done));
}

test('concurrent conflicting signers expose only one durable intent and signature', options, async t => {
  const { home, key } = await setup(t);
  const results = await contend(t, home, Array.from({ length: 12 }, (_, i) =>
    ({ type: 'place', data: { at: i % 2 ? 'h8' : 'h9' } })));
  const winners = results.filter(r => r.code === 0);
  assert.ok(winners.length >= 1);
  const envelope = JSON.parse(winners[0].stdout);
  for (const r of winners) assert.equal(r.stdout, winners[0].stdout);
  for (const r of results.filter(r => r.code !== 0)) {
    assert.equal(r.stdout, '');
    assert.match(r.stderr, /double-sign refused|signing lock busy|incomplete ownership metadata/);
  }
  assert.deepEqual(journal(home)['lock-test:0:0'].envelope, envelope);
  validSignature(envelope, key.public_key);
  const retry = await sign(t, home, request, envelope.payload.action);
  assert.equal(retry.stdout, winners[0].stdout);
  const conflict = await sign(t, home, request, { type: 'different' });
  assert.match(conflict.stderr, /double-sign refused/);
  unlocked(home);
});

test('matching concurrent signers replay the exact saved envelope', options, async t => {
  const { home } = await setup(t);
  const first = await sign(t, home);
  assert.equal(first.code, 0, first.stderr);
  const results = await contend(t, home, Array(8).fill(action));
  for (const r of results) {
    if (r.code === 0) assert.equal(r.stdout, first.stdout);
    else {
      assert.equal(r.stdout, '');
      assert.match(r.stderr, /signing lock busy|incomplete ownership metadata/);
    }
  }
  assert.ok(results.some(r => r.code === 0));
  unlocked(home);
});

test('action, comment and commitment are locked even when request context changes', options, async t => {
  const { home } = await setup(t);
  const flags = ['--comment', 'hello', '--commitment', 'abc'];
  const first = await sign(t, home, request, action, flags);
  assert.equal(first.code, 0, first.stderr);
  const refreshed = { ...request, payload: { ...request.payload, prev_hash: 'ff'.repeat(32) } };
  const replay = await sign(t, home, refreshed, { data: { at: 'h8' }, type: 'place' }, flags);
  assert.equal(replay.stdout, first.stdout);
  for (const [move, extra] of [
    [{ type: 'other' }, flags],
    [action, ['--comment', 'changed', '--commitment', 'abc']],
    [action, ['--comment', 'hello', '--commitment', 'def']],
    [action, []],
  ]) {
    const refused = await sign(t, home, refreshed, move, extra);
    assert.equal(refused.code, 1);
    assert.equal(refused.stdout, '');
    assert.match(refused.stderr, /double-sign refused/);
    unlocked(home);
  }
});

test('a live owner cannot be stolen by age; a dead owner is recovered', options, async t => {
  const { home } = await setup(t);
  const owner = await holder(t, home);
  const lock = join(home, 'signed.lock');
  utimesSync(lock, 1, 1);
  const before = readdirSync(lock);
  const blocked = await sign(t, home);
  assert.equal(blocked.code, 1);
  assert.equal(blocked.stdout, '');
  assert.match(blocked.stderr, /signing lock busy/);
  assert.deepEqual(readdirSync(lock), before);
  if (process.platform !== 'win32') {
    assert.equal(statSync(lock).mode & 0o777, 0o700);
    assert.equal(statSync(join(lock, before[0])).mode & 0o777, 0o600);
  }
  owner.p.kill('SIGKILL');
  assert.equal((await owner.done).signal, 'SIGKILL');
  const results = await contend(t, home, [action, { type: 'other' }, action, { type: 'other' }]);
  const winners = results.filter(r => r.code === 0);
  assert.ok(winners.length >= 1);
  for (const r of winners) assert.equal(r.stdout, winners[0].stdout);
  for (const r of results.filter(r => r.code !== 0)) {
    assert.equal(r.stdout, '');
    assert.match(r.stderr, /double-sign refused|signing lock busy|incomplete ownership metadata/);
  }
  unlocked(home);
});

test('incomplete, malformed and foreign ownership fail closed', options, async t => {
  const { home } = await setup(t);
  const lock = join(home, 'signed.lock');
  for (const metadata of [undefined, '{', '{}', '{"pid":0}', JSON.stringify({ pid: 1, host: hostname() + '-other' })]) {
    mkdirSync(lock, { mode: 0o700 });
    if (metadata !== undefined)
      writeFileSync(join(lock, 'owner-' + 'a'.repeat(32)), metadata, { mode: 0o600 });
    const before = readdirSync(lock);
    const result = await sign(t, home);
    assert.equal(result.code, 1);
    assert.equal(result.stdout, '');
    assert.deepEqual(readdirSync(lock), before);
    assert.ok(!readdirSync(home).includes('signed.json'));
    rmSync(lock, { recursive: true });
  }
});

test('two IPC-gated dead-owner reclaimers cannot remove the new live owner', options, async t => {
  const { home } = await setup(t);
  const old = await holder(t, home);
  old.p.kill('SIGKILL');
  assert.equal((await old.done).signal, 'SIGKILL');
  const lock = join(home, 'signed.lock');
  const oldOwner = join(lock, readdirSync(lock)[0]);
  const moves = [action, { type: 'different' }];
  const contenders = moves.map(move => {
    // Run the actual signer in a worker so its synchronous filesystem calls can
    // pause while the child process's main thread still handles parent IPC.
    const worker = `
      const fs = require('node:fs');
      const { syncBuiltinESMExports } = require('node:module');
      const { parentPort, workerData } = require('node:worker_threads');
      const gates = new Int32Array(workerData);
      function gate(index, event) {
        parentPort.postMessage(event);
        if (Atomics.wait(gates, index, 0, 10000) === 'timed-out')
          throw new Error('parent did not release ' + event);
      }
      const unlink = fs.unlinkSync, write = fs.writeFileSync;
      fs.unlinkSync = path => {
        if (path === ${JSON.stringify(oldOwner)})
          gate(0, 'dead-owner-observed');
        return unlink(path);
      };
      fs.writeFileSync = (...args) => {
        const result = write(...args);
        if (typeof args[0] === 'string' && args[0].startsWith(${JSON.stringify(lock + '/owner-')}))
          gate(1, 'successor-published');
        return result;
      };
      syncBuiltinESMExports();
      process.argv = ['node', ${JSON.stringify(signer.pathname)}, 'sign', '-',
        '--action', ${JSON.stringify(JSON.stringify(move))}];
      import(${JSON.stringify(signer.href)}).catch(err => { throw err; });
    `;
    return child(t, home, ['--input-type=module', '-e', `
      import { Worker } from 'node:worker_threads';
      const buffer = new SharedArrayBuffer(8);
      const gates = new Int32Array(buffer);
      process.on('message', index => {
        Atomics.store(gates, index, 1);
        Atomics.notify(gates, index);
      });
      const worker = new Worker(${JSON.stringify(worker)}, {
        eval: true, execArgv: [], workerData: buffer,
      });
      worker.on('message', event => process.send(event));
      worker.on('error', err => { console.error(err); process.exitCode = 1; });
      worker.on('exit', code => { process.exitCode = code; process.disconnect(); });
    `]);
  });
  const nextEvent = c => Promise.race([
    once(c.p, 'message'),
    c.done.then(result => { throw new Error('reclaimer exited before IPC: ' + JSON.stringify(result)); }),
  ]);
  const observed = contenders.map(nextEvent);
  for (const c of contenders) c.p.stdin.end(JSON.stringify(request));
  for (const event of await Promise.all(observed)) assert.equal(event[0], 'dead-owner-observed');
  // Both distinct processes have now confirmed the SAME owner dead and are
  // suspended immediately before unlinking its SAME unique owner path.
  assert.notEqual(contenders[0].p.pid, contenders[1].p.pid);
  const published = nextEvent(contenders[0]);
  contenders[0].p.send(0);
  assert.equal((await published)[0], 'successor-published');
  const successor = readdirSync(lock);
  assert.equal(successor.length, 1);
  assert.notEqual(join(lock, successor[0]), oldOwner);
  assert.equal(JSON.parse(readFileSync(join(lock, successor[0]))).pid, contenders[0].p.pid);

  // Resume the stale unlink only AFTER the first contender has reacquired.
  // It must not remove this live successor or produce the competing signature.
  contenders[1].p.send(0);
  const stale = await contenders[1].done;
  assert.equal(stale.code, 1);
  assert.equal(stale.stdout, '');
  assert.match(stale.stderr, /signing lock busy/);
  assert.deepEqual(readdirSync(lock), successor);
  const bare = await sign(t, home, request, moves[1]);
  assert.equal(bare.code, 1);
  assert.equal(bare.stdout, '');
  assert.match(bare.stderr, /signing lock busy/);

  contenders[0].p.send(1);
  const winner = await contenders[0].done;
  assert.equal(winner.code, 0, winner.stderr);
  assert.deepEqual(JSON.parse(winner.stdout).payload.action, action);
  assert.deepEqual(journal(home)['lock-test:0:0'].envelope, JSON.parse(winner.stdout));
  const conflict = await sign(t, home, request, moves[1]);
  assert.equal(conflict.code, 1);
  assert.equal(conflict.stdout, '');
  assert.match(conflict.stderr, /double-sign refused/);
  unlocked(home);
});

async function faultedSign(t, home, operation, behavior, req) {
  const c = child(t, home, ['--input-type=module', '-e', `
    import fs from 'node:fs';
    import { syncBuiltinESMExports } from 'node:module';
    const original = fs[${JSON.stringify(operation)}];
    fs[${JSON.stringify(operation)}] = (...args) => {
      ${behavior}
    };
    syncBuiltinESMExports();
    process.argv = ['node', ${JSON.stringify(signer.pathname)}, 'sign', '-',
      '--action', ${JSON.stringify(JSON.stringify(action))}];
    await import(${JSON.stringify(signer.href)});
  `]);
  c.p.stdin.end(JSON.stringify(req));
  return c.done;
}

test('atomic replacement preserves old journal on error and releases ownership', options, async t => {
  const { home } = await setup(t);
  assert.equal((await sign(t, home)).code, 0);
  const before = readFileSync(join(home, 'signed.json'), 'utf8');
  const next = { ...request, payload: { ...request.payload, seq: 1 } };
  const failed = await faultedSign(t, home, 'renameSync', 'throw new Error("injected rename failure");', next);
  assert.equal(failed.code, 1);
  assert.equal(failed.stdout, '');
  assert.match(failed.stderr, /injected rename failure/);
  assert.equal(readFileSync(join(home, 'signed.json'), 'utf8'), before);
  assert.ok(!readdirSync(home).some(name => name.endsWith('.tmp')));
  unlocked(home);
  chmodSync(join(home, 'signed.json'), 0o644);
  const oldFile = openSync(join(home, 'signed.json'), 'r');
  try {
    const success = await sign(t, home, next);
    assert.equal(success.code, 0, success.stderr);
    // An already-open reader sees the complete old inode, not an overwritten file.
    assert.equal(readFileSync(oldFile, 'utf8'), before);
  } finally { closeSync(oldFile); }
  assert.equal(Object.keys(journal(home)).length, 2);
  if (process.platform !== 'win32') assert.equal(statSync(join(home, 'signed.json')).mode & 0o777, 0o600);
});

test('crashes on both sides of rename retain a complete private journal', options, async t => {
  for (const after of [false, true]) {
    const { home } = await setup(t);
    assert.equal((await sign(t, home)).code, 0);
    const before = journal(home);
    const next = { ...request, payload: { ...request.payload, seq: 1 } };
    const crashed = await faultedSign(t, home, 'renameSync', `
      ${after ? 'original(...args);' : ''}
      process.kill(process.pid, 'SIGKILL');
    `, next);
    assert.equal(crashed.signal, 'SIGKILL');
    assert.equal(crashed.stdout, '');
    const saved = journal(home);
    assert.deepEqual(saved['lock-test:0:0'], before['lock-test:0:0']);
    assert.equal(Object.keys(saved).length, after ? 2 : 1);
    if (process.platform !== 'win32') {
      for (const name of readdirSync(home).filter(n => n === 'signed.json' || n.endsWith('.tmp')))
        assert.equal(statSync(join(home, name)).mode & 0o777, 0o600);
    }
    const retried = await sign(t, home, next);
    assert.equal(retried.code, 0, retried.stderr);
    if (after) assert.deepEqual(JSON.parse(retried.stdout), saved['lock-test:0:1'].envelope);
    unlocked(home);
  }
});

test('crash before owner publication leaves an incomplete lock closed', options, async t => {
  const { home } = await setup(t);
  const crashed = await faultedSign(t, home, 'mkdirSync', `
    const result = original(...args);
    if (args[0].endsWith('/signed.lock')) process.kill(process.pid, 'SIGKILL');
    return result;
  `, request);
  assert.equal(crashed.signal, 'SIGKILL');
  assert.equal(crashed.stdout, '');
  assert.deepEqual(readdirSync(join(home, 'signed.lock')), []);
  const retry = await sign(t, home);
  assert.equal(retry.code, 1);
  assert.equal(retry.stdout, '');
  assert.match(retry.stderr, /incomplete ownership metadata/);
  assert.ok(!readdirSync(home).includes('signed.json'));
});

test('owner publication failure cleans up ordinary acquisition', options, async t => {
  const { home } = await setup(t);
  const failed = await faultedSign(t, home, 'writeFileSync', `
    if (typeof args[0] === 'string' && args[0].includes('/signed.lock/'))
      throw new Error('injected owner write failure');
    return original(...args);
  `, request);
  assert.equal(failed.code, 1);
  assert.equal(failed.stdout, '');
  assert.match(failed.stderr, /injected owner write failure/);
  unlocked(home);
  assert.equal((await sign(t, home)).code, 0);
});

test('corrupt journal refuses signing and releases the lock', options, async t => {
  const { home } = await setup(t);
  writeFileSync(join(home, 'signed.json'), '{', { mode: 0o600 });
  const failed = await sign(t, home);
  assert.equal(failed.code, 1);
  assert.equal(failed.stdout, '');
  assert.equal(readFileSync(join(home, 'signed.json'), 'utf8'), '{');
  unlocked(home);
});

test('registration and hidden commitment remain independent of move locking', options, async t => {
  const { home, key } = await setup(t);
  const owner = await holder(t, home);
  const req = { domain: { ...request.domain, purpose: 'register_key' }, payload: { challenge: 'test' } };
  const registered = await run(t, home, ['sign', '-'], JSON.stringify(req));
  assert.equal(registered.code, 0, registered.stderr);
  const envelope = JSON.parse(registered.stdout);
  assert.equal(envelope.payload.public_key, key.public_key);
  assert.equal(envelope.payload.alg, 'ed25519');
  validSignature(envelope, key.public_key);
  const data = { fleet: [1, 2, 3] };
  const committed = await run(t, home, ['commit', '-'], JSON.stringify(data));
  assert.equal(committed.code, 0, committed.stderr);
  const result = JSON.parse(committed.stdout);
  assert.equal(result.commitment, createHash('sha256').update(Buffer.concat([
    Buffer.from('arena-commit-v1'), Buffer.from(canonical(data)), Buffer.from(result.salt, 'hex'),
  ])).digest('hex'));
  const saved = JSON.parse(readFileSync(join(home, 'commits', result.commitment + '.json')));
  assert.deepEqual(saved.data, data);
  assert.equal(saved.salt, result.salt);
  owner.p.send('release');
  assert.equal((await owner.done).code, 0);
  unlocked(home);
});
