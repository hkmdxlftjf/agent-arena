import { ArenaView, type SkinSet } from "@/components/ArenaView"
import { Countdown } from "@/components/Countdown"
import { MoveLog } from "@/components/MoveLog"
import { Avatar } from "@/cosmetics/avatar"
import { UnlockToast } from "@/cosmetics/UnlockToast"
import { CopyButton } from "@/components/CopyButton"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import type { OutletCtx } from "@/components/Layout"
import { api } from "@/lib/api"
import { gameTone, roleOf } from "@/lib/games"
import { reasonText, statusText } from "@/lib/reasons"
import type { MatchSnap, SeatCard } from "@/lib/types"
import { ed25519Available, verifyEntry } from "@/lib/verify"
import { motion, useReducedMotion } from "framer-motion"
import { ArrowLeft, Clock, Download, Eye, Flag, History, LogOut, Share2, ShieldCheck, ShieldQuestion, Swords, Trophy } from "lucide-react"
import { useEffect, useRef, useState, type CSSProperties } from "react"
import { Link, useOutletContext, useParams } from "react-router-dom"

function RoomWait({ expiresAt }: { expiresAt: number }) {
  const [now, setNow] = useState(() => Date.now())
  useEffect(() => {
    const id = window.setInterval(() => setNow(Date.now()), 1000)
    return () => window.clearInterval(id)
  }, [])
  const left = Math.max(0, expiresAt - now)
  const label = `${Math.floor(left / 60000)}:${String(Math.floor((left / 1000) % 60)).padStart(2, "0")}`
  return (
    <span className="inline-flex items-center gap-1 text-muted-foreground">
      <Clock className="size-3.5" aria-hidden />
      等待还剩 <span className="font-data">{label}</span>
    </span>
  )
}

export function MatchPage() {
  const { id = "" } = useParams()
  const { user } = useOutletContext<OutletCtx>()
  const [snap, setSnap] = useState<MatchSnap | null>(null)
  const [viewers, setViewers] = useState<number | null>(null)
  const [error, setError] = useState("")
  const [verified, setVerified] = useState<Record<number, { signature: boolean; ack: boolean }>>({})
  const seen = useRef(new Set<number>())
  const [copied, setCopied] = useState(false)
  const [leaving, setLeaving] = useState(false)
  const [plain, setPlain] = useState(false)

  useEffect(() => {
    let closed = false
    let socket: WebSocket | null = null
    let poll: number | undefined
    const apply = (next: MatchSnap) => {
      setSnap((prev) => (prev && next.tip_seq < prev.tip_seq ? prev : next))
      setViewers(next.viewer_count)
    }
    const load = () => {
      api<MatchSnap>(`/api/matches/${id}`)
        .then((body) => {
          if (!closed) apply(body)
        })
        .catch((err: Error) => {
          if (!closed) setError(err.message)
        })
    }
    load()
    const proto = location.protocol === "https:" ? "wss" : "ws"
    try {
      socket = new WebSocket(`${proto}://${location.host}/ws/match/${id}`)
      socket.onmessage = (event) => {
        const msg = JSON.parse(String(event.data)) as { type: string; data?: MatchSnap; count?: number }
        if (msg.type === "snapshot" && msg.data) apply(msg.data)
        if (msg.type === "viewers" && typeof msg.count === "number") setViewers(msg.count)
      }
      socket.onerror = () => {
        poll = window.setInterval(load, 2000)
      }
    } catch {
      poll = window.setInterval(load, 2000)
    }
    return () => {
      closed = true
      socket?.close()
      if (poll) window.clearInterval(poll)
    }
  }, [id])

  useEffect(() => {
    const entries = snap?.entries ?? []
    const keys = snap?.server_keys ?? []
    if (!entries.length || !ed25519Available()) return
    let cancel = false
    const seats = new Map<number, string>()
    for (const seat of snap?.seats ?? []) {
      if (seat.public_key) seats.set(seat.seat, seat.public_key)
    }
    void (async () => {
      for (const entry of entries) {
        if (cancel || seen.current.has(entry.seq)) continue
        seen.current.add(entry.seq)
        const agentKey = entry.payload.seat == null ? undefined : seats.get(entry.payload.seat)
        const result = await verifyEntry(entry, agentKey, keys)
        if (cancel) {
          seen.current.delete(entry.seq)
          return
        }
        setVerified((prev) => ({ ...prev, [entry.seq]: result }))
      }
    })()
    return () => {
      cancel = true
    }
  }, [snap])

  if (error && !snap) {
    return (
      <main className="mx-auto max-w-lg px-4 py-16">
        <div className="empty-state reveal">
          <Swords className="size-7" aria-hidden />
          <p className="text-lg font-semibold text-foreground">这局不存在，或已经无法读取</p>
          <p>{error}</p>
          <div className="mt-2 flex flex-wrap justify-center gap-2">
            <Button asChild variant="outline">
              <Link to="/">回首页</Link>
            </Button>
            <Button asChild variant="ghost">
              <Link to="/games">看看其他房间</Link>
            </Button>
          </div>
        </div>
      </main>
    )
  }
  if (!snap) {
    return (
      <main className="mx-auto max-w-6xl px-4 py-6 md:px-8" aria-busy="true">
        <div className="skeleton h-20" />
        <div className="mt-4 grid gap-4 lg:grid-cols-[minmax(0,720px)_minmax(0,320px)]">
          <div className="skeleton aspect-square" />
          <div className="skeleton h-96" />
        </div>
      </main>
    )
  }

  const acting = new Set(snap.to_act ?? [])
  const checked = Object.values(verified)
  const allOk = checked.length > 0 && checked.every((item) => item.signature)
  const limit = snap.time_limit_ms ?? 180_000
  const ordered = [...snap.seats].sort((a, b) => a.seat - b.seat)
  const head = ordered.length > 1 ? ordered.slice(1) : []
  const foot = ordered[0]
  const ended = snap.status === "finished" || snap.status === "expired" || snap.status === "aborted"
  const turn = acting.size > 0 && snap.status === "running"

  return (
    <main className="mx-auto max-w-6xl px-4 py-6 md:px-8" style={{ "--tone": gameTone(snap.game_id) } as CSSProperties}>
      <Link to="/games" className="-my-1 mb-3 inline-flex items-center gap-1 py-1 text-sm text-muted-foreground transition-colors hover:text-foreground">
        <ArrowLeft className="size-4" aria-hidden />
        大厅
      </Link>
      <header className="room-card reveal mb-4 gap-3 md:flex-row md:items-center">
        <div className="flex min-w-0 flex-1 flex-wrap items-center gap-2 text-sm">
          <h1 className="mr-1 text-xl font-bold tracking-tight">{snap.game_name || "对局"}</h1>
          {snap.status === "running" ? (
            <span className="badge-live">
              <span className="live-dot" aria-hidden />
              {statusText(snap.status)}
            </span>
          ) : (
            <Badge variant={ended ? "outline" : "secondary"}>{statusText(snap.status)}</Badge>
          )}
          <Badge variant="secondary">{snap.mode === "ranked" ? "排位" : "休闲"}</Badge>
          <Badge variant={allOk ? "default" : "outline"}>
            {allOk ? <ShieldCheck aria-hidden /> : <ShieldQuestion aria-hidden />}
            {allOk ? "签名已验证" : "核验签名"}
          </Badge>
          <span className="inline-flex items-center gap-1 text-muted-foreground">
            <Eye className="size-3.5" aria-hidden />
            <span className="font-data">{viewers ?? snap.viewer_count}</span> 人在看
          </span>
          <span className="inline-flex items-center gap-1 text-muted-foreground">
            <Clock className="size-3.5" aria-hidden />
            单步 <span className="font-data">{Math.round(limit / 1000)}s</span>
          </span>
          {snap.status === "waiting" && snap.expires_at ? <RoomWait expiresAt={snap.expires_at} /> : null}
        </div>
        <div className="flex flex-wrap gap-2">
          <Button variant="outline" size="sm" asChild>
            <a href={`/api/matches/${snap.id}/log.jsonl`}>
              <Download aria-hidden />
              下载日志
            </a>
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={() => {
              void navigator.clipboard.writeText(location.href)
              setCopied(true)
            }}
          >
            <Share2 aria-hidden />
            {copied ? "已复制" : "分享"}
          </Button>
          {user &&
            (snap.status === "waiting" || snap.status === "running") &&
            snap.seats.some((seat) => seat.owner_handle === user.handle) && (
              <Button
                variant="destructive"
                size="sm"
                disabled={leaving}
                onClick={() => {
                  setLeaving(true)
                  void api(`/api/matches/${snap.id}/leave`, { method: "POST" })
                    .then(() => api<MatchSnap>(`/api/matches/${snap.id}`))
                    .then((next) => setSnap(next))
                    .catch((err: Error) => setError(err.message))
                    .finally(() => setLeaving(false))
                }}
              >
                <LogOut aria-hidden />
                {leaving ? "退出中" : "退出"}
              </Button>
            )}
        </div>
      </header>

      {error && (
        <p className="alert-error mb-4" role="alert">
          {error}
        </p>
      )}

      <div className="grid min-w-0 grid-cols-1 items-start gap-4 lg:grid-cols-[minmax(0,720px)_minmax(0,320px)]">
        <section className="reveal min-w-0" style={{ "--i": 1 } as CSSProperties} aria-label="棋盘">
          <div className={ordered.length > 2 ? "mb-3 grid gap-2 sm:grid-cols-2" : ""}>
            {(ordered.length > 2 ? ordered : head).map((seat) => (
              <SeatBar
                key={seat.seat}
                seat={seat}
                role={seat.role || roleOf(snap.game_id, seat.seat)}
                active={acting.has(seat.seat) && snap.status === "running"}
                deadline={snap.deadline_at}
                serverTime={snap.server_time}
                limitMs={limit}
                score={snap.result?.scores?.[seat.seat]}
                celebrate={snap.status === "finished" && winnerSeat(snap) === seat.seat}
              />
            ))}
          </div>
          <div className="stage">
            <div className={`board-frame ${turn ? "board-frame-active" : ""}`}>
              <ArenaView gameId={snap.game_id} view={snap.view} skins={plain ? null : skinsOf(snap)} />
            </div>
          </div>
          {ordered.length <= 2 && foot && (
            <SeatBar
              seat={foot}
              role={foot.role || roleOf(snap.game_id, foot.seat)}
              active={acting.has(foot.seat) && snap.status === "running"}
              deadline={snap.deadline_at}
              serverTime={snap.server_time}
              limitMs={limit}
              score={snap.result?.scores?.[foot.seat]}
              celebrate={snap.status === "finished" && winnerSeat(snap) === foot.seat}
            />
          )}
          <label className="mt-1 inline-flex cursor-pointer items-center gap-2 py-1 text-sm text-muted-foreground select-none">
            <input type="checkbox" className="toggle" checked={plain} onChange={(event) => setPlain(event.target.checked)} />
            默认棋子
          </label>
          {snap.status === "waiting" && (
            <div className="panel pop-in mt-3 flex flex-wrap items-center justify-between gap-3 p-4">
              <p className="text-sm text-muted-foreground">{snap.room_code ? "还在等人入座，把房间码发给对手：" : "等待入座"}</p>
              {snap.room_code && (
                <span className="flex items-center gap-2">
                  <span className="font-data text-xl tracking-[0.2em]">{snap.room_code}</span>
                  <CopyButton text={snap.room_code} />
                </span>
              )}
            </div>
          )}
          {ended && (
            <div className="panel pop-in mt-3 flex items-start gap-3 p-4 text-sm">
              <span className="grid size-9 shrink-0 place-items-center rounded-xl bg-accent text-gold">
                {snap.status === "finished" ? <Trophy className="size-4" aria-hidden /> : <Flag className="size-4" aria-hidden />}
              </span>
              <div className="min-w-0">
                <p className="font-semibold">终局：{reasonText(snap.result?.reason || snap.end_reason)}</p>
                {snap.seed ? <span className="font-data mt-1 block break-all text-xs text-muted-foreground">seed {snap.seed}</span> : null}
              </div>
            </div>
          )}
        </section>
        <aside className="panel reveal min-w-0 overflow-hidden lg:sticky lg:top-24" style={{ "--i": 2 } as CSSProperties}>
          <div className="panel-head">
            <span className="flex items-center gap-2">
              <History className="size-4 text-primary" aria-hidden />
              时间线
            </span>
            <span className="font-data text-xs font-normal text-muted-foreground">{snap.entries?.length ?? 0} 条</span>
          </div>
          <MoveLog entries={snap.entries ?? []} verified={verified} gameId={snap.game_id} />
        </aside>
      </div>
      <UnlockToast refreshKey={snap.status === "finished" ? snap.id : 0} />
    </main>
  )
}

function skinsOf(snap: MatchSnap): SkinSet {
  const piece: Record<number, string | undefined> = {}
  const backs: (string | null | undefined)[] = []
  for (const seat of snap.seats) {
    piece[seat.seat] = seat.look?.piece
    backs[seat.seat] = seat.look?.card_back
  }
  return { piece, board: snap.seats[0]?.look?.board, backs }
}

function winnerSeat(snap: MatchSnap): number | null {
  const scores = snap.result?.scores
  if (!scores) return null
  let best = -Infinity
  let seat = -1
  let ties = 0
  scores.forEach((score, index) => {
    if (score == null) return
    if (score > best) {
      best = score
      seat = index
      ties = 1
    } else if (score === best) {
      ties += 1
    }
  })
  return ties === 1 ? seat : null
}

function SeatBar({
  seat,
  role,
  active,
  deadline,
  serverTime,
  limitMs,
  score,
  celebrate = false,
}: {
  seat?: SeatCard
  role: string
  active: boolean
  deadline?: number | null
  serverTime?: number
  limitMs: number
  score?: number | null
  celebrate?: boolean
}) {
  const reduce = useReducedMotion()
  const emote = celebrate ? seat?.look?.emote : null
  return (
    <div className={`seat-bar my-3 ${active ? "seat-bar-active" : ""} ${celebrate ? "seat-bar-win" : ""}`}>
      <Avatar
        preset={seat?.look?.avatar_preset}
        color={seat?.look?.color}
        frame={seat?.look?.frame}
        imageUrl={seat?.look?.avatar_url}
        size={36}
      />
      <div className="min-w-0">
        <p className="truncate text-sm font-medium">
          {seat?.agent_id ? <Link to={`/a/${seat.agent_id}`} className="transition-colors hover:text-primary">{seat.agent_name}</Link> : <span className="text-muted-foreground">空位</span>}
          {celebrate ? <Trophy className="ml-1.5 inline size-3.5 align-[-2px] text-gold" aria-label="胜者" /> : null}
          {seat?.look?.title ? <span className="ml-2 text-xs font-normal text-muted-foreground">{seat.look.title}</span> : null}
          {emote && emote !== "none" && !reduce ? <WinEmote kind={emote} /> : null}
        </p>
        <p className="truncate text-xs text-muted-foreground">
          {role}
          {seat?.owner_handle ? <> · <Link to={`/u/${seat.owner_handle}`}>@{seat.owner_handle}</Link></> : " · 等待入座"}
          {seat?.connected ? <span className="text-live"> · 在线</span> : ""}
          {seat?.model_label ? ` · ${seat.model_label}` : ""}
          {score != null ? ` · ${score > 0 ? "+" : ""}${score}` : ""}
        </p>
      </div>
      <div className="ml-auto shrink-0">
        <Countdown deadline={deadline} serverTime={serverTime} limitMs={limitMs} active={active} />
      </div>
    </div>
  )
}

function WinEmote({ kind }: { kind: string }) {
  if (kind === "bow") {
    return (
      <motion.span className="ml-2 inline-block text-xs" animate={{ y: [0, 6, 0] }} transition={{ duration: 0.8 }}>
        拱
      </motion.span>
    )
  }
  if (kind === "ink") {
    return (
      <motion.span
        className="ml-2 inline-block size-2 rounded-full bg-foreground"
        animate={{ scale: [0.2, 1.8], opacity: [0, 0.75, 0] }}
        transition={{ duration: 0.9 }}
      />
    )
  }
  return (
    <motion.span
      className="ml-2 inline-block size-2 rounded-full bg-primary"
      animate={{ scale: [0.5, 1.5, 1] }}
      transition={{ duration: 0.7 }}
    />
  )
}
