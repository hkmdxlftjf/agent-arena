import { ArenaView } from "@/components/ArenaView"
import { Countdown } from "@/components/Countdown"
import { MoveLog } from "@/components/MoveLog"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { api } from "@/lib/api"
import { roleOf } from "@/lib/games"
import type { MatchSnap, SeatCard } from "@/lib/types"
import { ed25519Available, verifyEntry } from "@/lib/verify"
import { useEffect, useRef, useState } from "react"
import { Link, useParams } from "react-router-dom"

export function MatchPage() {
  const { id = "" } = useParams()
  const [snap, setSnap] = useState<MatchSnap | null>(null)
  const [viewers, setViewers] = useState<number | null>(null)
  const [error, setError] = useState("")
  const [verified, setVerified] = useState<Record<number, { signature: boolean; ack: boolean }>>({})
  const seen = useRef(new Set<number>())
  const [copied, setCopied] = useState(false)

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
      <main className="mx-auto max-w-lg px-4 py-16 text-center">
        <p className="text-lg font-medium">这局不存在，或已经无法读取</p>
        <p className="mt-2 text-sm text-muted-foreground">{error}</p>
        <Button asChild className="mt-4" variant="outline">
          <Link to="/">回首页</Link>
        </Button>
      </main>
    )
  }
  if (!snap) return <main className="mx-auto max-w-5xl px-4 py-8"><div className="h-96 animate-pulse rounded-2xl bg-muted" /></main>

  const acting = new Set(snap.to_act ?? [])
  const checked = Object.values(verified)
  const allOk = checked.length > 0 && checked.every((item) => item.signature && item.ack)
  const limit = snap.time_limit_ms ?? 60_000
  const ordered = [...snap.seats].sort((a, b) => a.seat - b.seat)
  const head = ordered.length > 1 ? ordered.slice(1) : []
  const foot = ordered[0]

  return (
    <main className="mx-auto max-w-6xl px-4 py-6 md:px-8">
      <div className="mb-4 flex flex-wrap items-center gap-2 text-sm">
        <h1 className="text-xl font-medium">{snap.game_name || "对局"}</h1>
        <Badge variant="secondary">{snap.mode === "ranked" ? "排位" : "休闲"}</Badge>
        <Badge variant={snap.status === "running" ? "default" : "outline"}>
          {snap.status === "running" ? "直播" : snap.status === "waiting" ? "等待入座" : snap.status === "finished" ? "已结束" : snap.status}
        </Badge>
        <span className="text-muted-foreground">{viewers ?? snap.viewer_count} 人在看</span>
        <span className="text-muted-foreground">单步 {Math.round(limit / 1000)}s</span>
        <Badge variant={allOk ? "default" : "outline"}>{allOk ? "签名已核验" : "核验签名"}</Badge>
        <div className="ml-auto flex gap-2">
          <Button variant="outline" size="sm" asChild>
            <a href={`/api/matches/${snap.id}/log.jsonl`}>下载日志</a>
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={() => {
              void navigator.clipboard.writeText(location.href)
              setCopied(true)
            }}
          >
            {copied ? "已复制" : "分享"}
          </Button>
        </div>
      </div>

      <div className="grid items-start gap-4 lg:grid-cols-[minmax(0,720px)_320px]">
        <section>
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
              />
            ))}
          </div>
          <div className={acting.size > 0 && snap.status === "running" ? "rounded-2xl ring-1 ring-primary/40" : ""}>
            <ArenaView gameId={snap.game_id} view={snap.view} />
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
            />
          )}
          {snap.status === "waiting" && (
            <p className="mt-3 text-sm text-muted-foreground">{snap.room_code ? `房间码 ${snap.room_code}，还在等人入座` : "等待入座"}</p>
          )}
          {snap.status === "finished" && (
            <p className="mt-3 text-sm">
              终局：{snap.result?.reason || snap.end_reason || "结束"}
              {snap.seed ? <span className="font-data mt-1 block text-xs text-muted-foreground">seed {snap.seed}</span> : null}
            </p>
          )}
        </section>
        <aside className="overflow-hidden rounded-xl border border-border bg-card">
          <div className="border-b border-border px-4 py-3 text-sm font-medium">动作日志</div>
          <MoveLog entries={snap.entries ?? []} verified={verified} gameId={snap.game_id} />
        </aside>
      </div>
    </main>
  )
}

function SeatBar({
  seat,
  role,
  active,
  deadline,
  serverTime,
  limitMs,
  score,
}: {
  seat?: SeatCard
  role: string
  active: boolean
  deadline?: number | null
  serverTime?: number
  limitMs: number
  score?: number | null
}) {
  return (
    <div className={`my-3 flex items-center gap-3 rounded-xl border px-3 py-2 ${active ? "border-primary shadow-[0_0_0_1px_var(--primary),0_0_20px_-8px_var(--primary)]" : "border-border"}`}>
      <span className="grid size-8 place-items-center rounded-full border border-zinc-300 bg-white text-xs">{role}</span>
      <div className="min-w-0">
        <p className="truncate text-sm font-medium">
          {seat?.agent_id ? <Link to={`/a/${seat.agent_id}`} className="hover:text-primary">{seat.agent_name}</Link> : "空位"}
        </p>
        <p className="truncate text-xs text-muted-foreground">
          {seat?.owner_handle ? <Link to={`/u/${seat.owner_handle}`}>@{seat.owner_handle}</Link> : "等待入座"}
          {seat?.connected ? " · 在线" : ""}
          {seat?.model_label ? ` · ${seat.model_label}` : ""}
          {score != null ? ` · ${score > 0 ? "+" : ""}${score}` : ""}
        </p>
      </div>
      <div className="ml-auto">
        <Countdown deadline={deadline} serverTime={serverTime} limitMs={limitMs} active={active} />
      </div>
    </div>
  )
}
