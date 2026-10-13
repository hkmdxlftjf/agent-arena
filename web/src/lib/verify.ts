import type { LogEntry } from "./types"

const PREFIX = new TextEncoder().encode("\x19AgentArena Signed Message:\n")

function canon(value: unknown): string {
  if (value === null || typeof value === "boolean" || typeof value === "string") return JSON.stringify(value)
  if (typeof value === "number") {
    if (!Number.isFinite(value)) throw new Error("non-finite")
    return JSON.stringify(value)
  }
  if (Array.isArray(value)) return `[${value.map(canon).join(",")}]`
  if (typeof value === "object") {
    const obj = value as Record<string, unknown>
    const keys = Object.keys(obj)
      .filter((key) => obj[key] !== undefined)
      .sort()
    return `{${keys.map((key) => `${JSON.stringify(key)}:${canon(obj[key])}`).join(",")}}`
  }
  throw new Error("unsupported")
}

function b64urlToBytes(input: string): Uint8Array<ArrayBuffer> {
  const pad = input.length % 4 === 0 ? "" : "=".repeat(4 - (input.length % 4))
  const b64 = input.replace(/-/g, "+").replace(/_/g, "/") + pad
  const bin = atob(b64)
  const out = new Uint8Array(bin.length)
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i)
  return out
}

async function sha256(bytes: BufferSource): Promise<Uint8Array<ArrayBuffer>> {
  const digest = await crypto.subtle.digest("SHA-256", bytes)
  return new Uint8Array(digest)
}

async function signingInput(domain: unknown, payload: unknown): Promise<Uint8Array<ArrayBuffer>> {
  const enc = new TextEncoder()
  const left = await sha256(enc.encode(canon(domain)))
  const right = await sha256(enc.encode(canon(payload)))
  const out = new Uint8Array(PREFIX.length + left.length + right.length)
  out.set(PREFIX, 0)
  out.set(left, PREFIX.length)
  out.set(right, PREFIX.length + left.length)
  return out
}

const keyCache = new Map<string, CryptoKey>()

async function publicKey(x: string): Promise<CryptoKey> {
  const cached = keyCache.get(x)
  if (cached) return cached
  const key = await crypto.subtle.importKey(
    "jwk",
    { kty: "OKP", crv: "Ed25519", x },
    { name: "Ed25519" },
    false,
    ["verify"],
  )
  keyCache.set(x, key)
  return key
}

export function ed25519Available(): boolean {
  return typeof crypto !== "undefined" && !!crypto.subtle
}

export async function verifyEntry(
  entry: LogEntry,
  agentKey: string | undefined,
  serverKeys: { kid: string; public_key: string }[],
): Promise<{ signature: boolean; ack: boolean }> {
  const server = serverKeys.find((key) => key.kid === entry.kid)?.public_key
  const pub = entry.signer === "agent" ? agentKey : server
  let signature = false
  if (pub && entry.domain && entry.payload && entry.signature) {
    try {
      const key = await publicKey(pub)
      const input = await signingInput(entry.domain, entry.payload)
      signature = await crypto.subtle.verify(
        { name: "Ed25519" },
        key,
        b64urlToBytes(entry.signature),
        input,
      )
    } catch {
      signature = false
    }
  }
  let ack = false
  if (entry.server_ack && entry.payload?.match_id && entry.domain) {
    const ackKey = serverKeys.find((key) => key.kid === entry.server_ack?.kid)?.public_key
    if (ackKey) {
      try {
        const payloadHash = [...(await sha256(new TextEncoder().encode(canon(entry.payload))))]
          .map((b) => b.toString(16).padStart(2, "0"))
          .join("")
        const ackPayload = {
          hash: entry.hash,
          match_id: entry.payload.match_id,
          payload_hash: payloadHash,
          received_at: entry.received_at,
          seq: entry.seq,
        }
        const key = await publicKey(ackKey)
        const input = await signingInput({ ...entry.domain, purpose: "ack" }, ackPayload)
        ack = await crypto.subtle.verify(
          { name: "Ed25519" },
          key,
          b64urlToBytes(entry.server_ack.signature),
          input,
        )
      } catch {
        ack = false
      }
    }
  }
  return { signature, ack }
}
