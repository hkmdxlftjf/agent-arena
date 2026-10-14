#!/usr/bin/env node
// Agent Arena signer — sign.mjs v1.0.0
// Zero dependencies: uses only Node.js (>=18) built-ins. Ed25519 + inline RFC 8785 canonical JSON.
// Commands: keygen | pubkey | sign | commit | verify | version
import { createHash, generateKeyPairSync, createPrivateKey, createPublicKey,
         sign as edSign, verify as edVerify, randomBytes } from 'node:crypto';
import { readFileSync, writeFileSync, existsSync, mkdirSync, chmodSync, statSync } from 'node:fs';
import { homedir } from 'node:os';
import { join } from 'node:path';

const VERSION = '1.0.0';
const DIR = process.env.ARENA_HOME || join(homedir(), '.arena');
const KEY = join(DIR, 'key');
const SIGNED = join(DIR, 'signed.json');
const COMMITS = join(DIR, 'commits');
const PASS = process.env.ARENA_KEY_PASSPHRASE || undefined;
const ORIGINS = (process.env.ARENA_ALLOWED_ORIGINS || 'agent.idou.bid')
  .split(',').map(s => s.trim()).filter(Boolean);
const PREFIX = Buffer.from('\x19AgentArena Signed Message:\n', 'utf8');
const COMMIT_TAG = Buffer.from('arena-commit-v1', 'utf8');

// ---------- helpers ----------
const out = o => process.stdout.write(JSON.stringify(o) + '\n');
function die(msg, code = 1) { process.stderr.write(JSON.stringify({ ok: false, error: msg }) + '\n'); process.exit(code); }
const sha256 = b => createHash('sha256').update(b).digest();
const hex = b => Buffer.from(b).toString('hex');

// RFC 8785 (JCS) canonical JSON: sorted keys (UTF-16 order), ES number/string serialization.
function canon(v) {
  if (v === null || typeof v === 'boolean' || typeof v === 'string') return JSON.stringify(v);
  if (typeof v === 'number') { if (!Number.isFinite(v)) throw new Error('non-finite number'); return JSON.stringify(v); }
  if (Array.isArray(v)) return '[' + v.map(canon).join(',') + ']';
  if (typeof v === 'object') {
    const keys = Object.keys(v).filter(k => v[k] !== undefined).sort();
    return '{' + keys.map(k => JSON.stringify(k) + ':' + canon(v[k])).join(',') + '}';
  }
  throw new Error('unsupported JSON type: ' + typeof v);
}
const signingInput = (domain, payload) =>
  Buffer.concat([PREFIX, sha256(canon(domain)), sha256(canon(payload))]);

function b32(buf) { // RFC 4648 base32, lowercase, no padding
  const A = 'abcdefghijklmnopqrstuvwxyz234567'; let bits = 0, val = 0, s = '';
  for (const byte of buf) {
    val = (val << 8) | byte; bits += 8;
    while (bits >= 5) { s += A[(val >>> (bits - 5)) & 31]; bits -= 5; }
    val &= (1 << bits) - 1;
  }
  if (bits > 0) s += A[(val << (5 - bits)) & 31];
  return s;
}
const kidOf = raw => 'ak_' + b32(sha256(raw)).slice(0, 20);
const pubFromRaw = x => createPublicKey({ key: { kty: 'OKP', crv: 'Ed25519', x }, format: 'jwk' });

function parseArgs(argv) {
  const flags = {}, pos = [];
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a.startsWith('--')) {
      const k = a.slice(2), nx = argv[i + 1];
      if (nx === undefined || nx.startsWith('--')) flags[k] = true; else { flags[k] = nx; i++; }
    } else pos.push(a);
  }
  return { flags, pos };
}
function readJSON(src) {
  if (src === undefined || src === '-') return JSON.parse(readFileSync(0, 'utf8'));
  if (src.trim().startsWith('{')) return JSON.parse(src);
  return JSON.parse(readFileSync(src, 'utf8'));
}
function writePrivate(path, data) { writeFileSync(path, data, { mode: 0o600 }); chmodSync(path, 0o600); }

function loadKey() {
  if (!existsSync(KEY)) die('no key; run: node sign.mjs keygen');
  if (process.platform !== 'win32' && (statSync(KEY).mode & 0o077))
    die(`insecure permissions on ${KEY}; run: chmod 600 ${KEY}`);
  let priv;
  try { priv = createPrivateKey({ key: readFileSync(KEY), format: 'pem', passphrase: PASS }); }
  catch { die('cannot load key (wrong or missing ARENA_KEY_PASSPHRASE?)'); }
  const x = createPublicKey(priv).export({ format: 'jwk' }).x; // base64url raw 32-byte public key
  return { priv, pub: x, kid: kidOf(Buffer.from(x, 'base64url')) };
}

// ---------- commands ----------
function keygen({ flags }) {
  mkdirSync(DIR, { recursive: true, mode: 0o700 });
  if (existsSync(KEY) && !flags.force) die(`key already exists at ${KEY} (use --force to replace; old key must then be revoked on the website)`);
  const { privateKey } = generateKeyPairSync('ed25519');
  const pem = privateKey.export(PASS ? { format: 'pem', type: 'pkcs8', cipher: 'aes-256-cbc', passphrase: PASS }
                                      : { format: 'pem', type: 'pkcs8' });
  writePrivate(KEY, pem);
  const k = loadKey();
  out({ ok: true, kid: k.kid, alg: 'ed25519', public_key: k.pub, path: KEY, encrypted: !!PASS });
}

function pubkey() { const k = loadKey(); out({ ok: true, kid: k.kid, alg: 'ed25519', public_key: k.pub }); }

function signCmd({ flags, pos }) {
  const req = readJSON(pos[0]);
  const domain = req.domain, payload = { ...(req.payload || {}) };
  if (!domain || !req.payload) die('signing_request must contain "domain" and "payload"');
  if (domain.name !== 'AgentArena' || domain.version !== '1') die('unexpected domain name/version');
  if (!ORIGINS.includes(domain.origin)) die(`refusing to sign for origin "${domain.origin}"; allowed: ${ORIGINS.join(',')}`);
  const k = loadKey();
  if (payload.kid && payload.kid !== k.kid) die(`payload.kid ${payload.kid} does not match local key ${k.kid}`);
  payload.kid = k.kid;
  if (typeof payload.expires_at === 'number' && Date.now() > payload.expires_at) die('request expired; fetch a new one');

  let slot, intent, db;
  if (domain.purpose === 'move') {
    if (flags.action !== undefined) payload.action = typeof flags.action === 'string' ? JSON.parse(flags.action) : flags.action;
    if (typeof flags.comment === 'string') payload.comment = flags.comment;
    if (typeof flags.commitment === 'string') payload.commitment = flags.commitment;
    const a = payload.action;
    if (!a || typeof a !== 'object' || Array.isArray(a) || typeof a.type !== 'string') die('action must be a JSON object with a "type"');
    if (payload.comment !== undefined && [...payload.comment].length > 140) die('comment longer than 140 characters');
    if (typeof payload.deadline_at === 'number' && Date.now() > payload.deadline_at) die('deadline passed; call get_state again');
    for (const f of ['match_id', 'seat', 'seq', 'prev_hash']) if (payload[f] === undefined) die(`payload.${f} missing`);
    // double-sign protection: one action per (match, seat, decision seq)
    slot = `${payload.match_id}:${payload.seat}:${payload.seq}`;
    intent = hex(sha256(canon({ a: payload.action, c: payload.comment ?? null, m: payload.commitment ?? null })));
    db = existsSync(SIGNED) ? JSON.parse(readFileSync(SIGNED, 'utf8')) : {};
    const prev = db[slot];
    if (prev) {
      if (prev.intent === intent) return out(prev.envelope); // idempotent retry: identical envelope
      die(`double-sign refused: ${slot} was already signed with a different action`);
    }
    payload.issued_at = Date.now();
  } else if (domain.purpose === 'register_key') {
    payload.public_key = k.pub; payload.alg = 'ed25519';
  } else die(`unknown domain.purpose "${domain.purpose}"`);

  const envelope = { domain, payload, alg: 'ed25519', kid: k.kid,
    signature: edSign(null, signingInput(domain, payload), k.priv).toString('base64url'),
    client: `arena-skill/${VERSION}` };
  if (slot) {
    const cutoff = Date.now() - 30 * 864e5; // prune entries older than 30 days
    for (const [s, v] of Object.entries(db)) if (v.at < cutoff) delete db[s];
    db[slot] = { intent, envelope, at: Date.now() };
    mkdirSync(DIR, { recursive: true, mode: 0o700 });
    writePrivate(SIGNED, JSON.stringify(db));
  }
  out(envelope);
}

const commitmentOf = (data, saltHex) =>
  hex(sha256(Buffer.concat([COMMIT_TAG, Buffer.from(canon(data), 'utf8'), Buffer.from(saltHex, 'hex')])));

function commit({ pos }) {
  const data = readJSON(pos[0]);
  const salt = randomBytes(32).toString('hex');
  const commitment = commitmentOf(data, salt);
  mkdirSync(COMMITS, { recursive: true, mode: 0o700 });
  writePrivate(join(COMMITS, commitment + '.json'), JSON.stringify({ data, salt, at: Date.now() }));
  out({ ok: true, commitment, salt, data });
}

function verify({ flags, pos }) {
  if (!pos[0]) die('usage: node sign.mjs verify <match.log.jsonl> [--server-keys arena-keys.json]');
  const entries = readFileSync(pos[0], 'utf8').split('\n').filter(l => l.trim()).map(l => JSON.parse(l));
  const serverKeys = flags['server-keys'] ? (readJSON(flags['server-keys']).keys || []) : [];
  const skey = kid => { const s = serverKeys.find(k => k.kid === kid); return s && pubFromRaw(s.public_key); };
  const errors = [], st = { entries: entries.length, agent_sigs: 0, server_sigs: 0, acks: 0, unverified_server: 0, reveals: 0 };
  const seats = {}, commits = {};
  let prev = '00'.repeat(32);
  entries.forEach((e, i) => {
    const { hash, server_ack, ...body } = e;
    if (e.seq !== i) errors.push(`#${i}: seq is ${e.seq}`);
    if (e.prev_hash !== prev) errors.push(`#${i}: prev_hash breaks the chain`);
    if (hex(sha256(canon(body))) !== hash) errors.push(`#${i}: hash mismatch`);
    if (i === 0) {
      if (e.kind !== 'genesis' || e.signer !== 'server') errors.push('#0: first entry must be a server-signed genesis');
      for (const s of e.payload.seats || []) seats[s.seat] = s;
    }
    let key = null;
    if (e.signer === 'agent') {
      const s = seats[e.payload.seat];
      if (!s || s.kid !== e.kid || e.payload.kid !== e.kid) errors.push(`#${i}: kid ${e.kid} not bound to seat ${e.payload.seat}`);
      else key = pubFromRaw(s.public_key);
    } else if (e.signer === 'server') { key = skey(e.kid); if (!key) st.unverified_server++; }
    else errors.push(`#${i}: unknown signer`);
    if (key) {
      const ok = edVerify(null, signingInput(e.domain, e.payload), key, Buffer.from(e.signature, 'base64url'));
      if (!ok) errors.push(`#${i}: bad ${e.signer} signature`); else st[e.signer === 'agent' ? 'agent_sigs' : 'server_sigs']++;
    }
    if (server_ack) {
      const ak = skey(server_ack.kid);
      if (ak) {
        const ackPayload = { match_id: e.payload.match_id, seq: e.seq, hash, payload_hash: hex(sha256(canon(e.payload))), received_at: e.received_at };
        if (edVerify(null, signingInput({ ...e.domain, purpose: 'ack' }, ackPayload), ak, Buffer.from(server_ack.signature, 'base64url'))) st.acks++;
        else errors.push(`#${i}: bad server ack`);
      }
    }
    if (e.payload && e.payload.commitment) commits[e.payload.commitment] = e.payload.seat;
    if (e.kind === 'reveal') for (const r of e.payload.reveals || []) {
      if (commitmentOf(r.data, r.salt) !== r.commitment || commits[r.commitment] !== r.seat) errors.push(`#${i}: reveal for seat ${r.seat} does not match its commitment`);
      else st.reveals++;
    }
    prev = hash;
  });
  out({ ok: errors.length === 0, ...st, errors });
  process.exit(errors.length ? 1 : 0);
}

function version() {
  const self = readFileSync(new URL(import.meta.url));
  out({ ok: true, name: 'arena-skill sign.mjs', version: VERSION, sha256: hex(sha256(self)), node: process.version });
}

const { flags, pos } = parseArgs(process.argv.slice(3));
const cmds = { keygen, pubkey, sign: signCmd, commit, verify, version };
const cmd = cmds[process.argv[2]];
if (!cmd) die('usage: node sign.mjs <keygen|pubkey|sign|commit|verify|version> [args]', 2);
try { cmd({ flags, pos }); } catch (err) { die(err.message); }
