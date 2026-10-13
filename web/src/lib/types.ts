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
  authorId?: string
  uploaded?: boolean
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
    action?: { type?: string; data?: Record<string, unknown> }
    comment?: string
    policy?: string
    decision_seq?: number
    result?: { scores?: number[]; ranking?: number[][]; reason?: string }
    reason?: string
    reveals?: unknown[]
    commitment?: string
  }
  server_ack?: { kid: string; signature: string }
  domain?: { purpose?: string }
}

export type XiangqiPiece = { at: string; side: number; kind: number; label: string }
export type GoPoint = { at: string; seat: number }
export type JunqiCell = {
  at: string
  zone: string
  piece: { owner: number; name: string; hidden: boolean } | null
}
export type GridCell = { at: string; seat?: number; label?: string }
export type Trick = { seat: number; cards: string[]; kind: string; rank: number; len: number }
export type Meld = { kind: string; tile: string }

export type GameView = {
  renderer?: string
  size?: number
  stones?: { at: string; seat: number }[]
  lastMove?: string | { from?: string; to?: string } | null
  winLine?: string[]
  toAct?: number[]
  winner?: number | null
  reason?: string | null
  moves?: number
  variant?: string
  pieces?: XiangqiPiece[]
  check?: boolean
  quiet?: number
  komi?: number
  points?: GoPoint[] | number[]
  passes?: number
  score?: [number, number] | null
  phase?: string
  cells?: Array<JunqiCell | GridCell>
  deployed?: boolean[]
  last?: Trick | { from?: string; to?: string; result?: string; seat?: number; type?: string } | null
  hands?: (string[] | null)[] | null
  counts?: number[]
  bottom?: string[] | null
  bid?: number
  landlord?: number | null
  multiplier?: number
  wall?: number
  melds?: Meld[][]
  discards?: string[][]
  dingque?: (string | null)[]
  lastDraw?: string | null
  pending?: { tile: string; from: number; kind: string } | null
  hu?: boolean[]
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
  view: GameView | null
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
  view: GameView | null
}

export type User = {
  id: string
  handle: string
  email: string
  role: string
  github_login?: string | null
  has_password?: boolean
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

export type RatingRow = {
  game_id: string
  rating: number
  rd: number
  games: number
  wins: number
  system: string
  provisional: boolean
}

export type BoardRow = {
  rank: number
  agent_id: string
  name: string
  handle: string
  rating: number
  rd: number | null
  games: number | null
  wins?: number
  system?: string
  provisional: boolean
  conservative: number
}

export type RecentMatch = {
  id: string
  game_id: string
  status: string
  end_reason?: string | null
  score?: number | null
  seat: number
}
