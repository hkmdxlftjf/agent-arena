export type Manifest = {
  id: string
  name: string
  version: string
  description: string
  playable: boolean
  minPlayers: number
  maxPlayers: number
  traits: string[]
  variants: { id: string; name: string }[]
  defaultTime: { perActionMs: number; maxConsecutiveTimeouts: number; graceMs: number }
  timeoutPolicy: string
  allowedTimeoutPolicies: string[]
  rulesMarkdown: string
  rated: boolean
  engine: string
}

export type SeatCard = {
  seat: number
  agent_id?: string
  agent_name: string
  owner_handle: string
  model_label?: string | null
  kid?: string
  public_key?: string
  connected?: boolean
  timeouts?: number
  forfeited?: boolean
  role?: string
  allow_comments?: boolean
}

export type LogEntry = {
  seq: number
  kind: string
  signer: string
  kid: string
  hash: string
  prev_hash: string
  signature: string
  received_at: number
  payload: {
    match_id?: string
    seat?: number
    action?: { type?: string; data?: { at?: string } }
    comment?: string
    policy?: string
    decision_seq?: number
    result?: { scores?: number[]; ranking?: number[][]; reason?: string }
    reason?: string
  }
  server_ack?: { kid: string; signature: string }
  domain?: { purpose?: string }
}

export type BoardView = {
  size: number
  board: number[][]
  stones: { at: string; seat: number }[]
  lastMove: string | null
  winLine: string[]
  toAct: number[]
  winner: number | null
  reason: string | null
  moves: number
  variant: string
}

export type MatchSnap = {
  id: string
  game_id: string
  game_name: string
  game_version?: string
  variant?: string | null
  mode?: string
  status: string
  room_code?: string | null
  time_limit_ms?: number
  grace_ms?: number
  timeout_policy?: string
  tip_seq: number
  viewer_count: number
  server_time?: number
  to_act?: number[]
  deadline_at?: number | null
  end_reason?: string | null
  result?: { scores?: Array<number | null>; ranking?: number[][]; reason?: string } | null
  seed?: string | null
  seed_commit?: string | null
  rules_hash?: string
  view: BoardView | null
  seats: SeatCard[]
  entries?: LogEntry[]
  server_keys?: { kid: string; alg: string; public_key: string }[]
}

export type LiveCard = {
  id: string
  game_id: string
  game_name: string
  status: string
  room_code?: string | null
  variant?: string | null
  tip_seq?: number | null
  viewer_count: number
  seats: SeatCard[]
  view: BoardView | null
}

export type User = {
  id: string
  handle: string
  email: string
  role: string
}

export type AgentRow = {
  id: string
  name: string
  description?: string | null
  model_label?: string | null
  allow_comments: boolean
  last_seen_at?: number | null
  connected: boolean
  keys: { kid: string; status: string; public_key: string; label?: string | null }[]
  tokens: { id: string; prefix: string; created_at: number; revoked: boolean }[]
}
