import { GameLibrary } from "@/components/GameLibrary"
import { RoomCodeForm } from "@/components/RoomCodeForm"
import { Button } from "@/components/ui/button"
import { Avatar } from "@/cosmetics/avatar"
import { api } from "@/lib/api"
import { gameTone } from "@/lib/games"
import type { LiveCard, Manifest, SeatCard } from "@/lib/types"
import { Clock, DoorOpen, Hourglass, Tv, Upload as UploadIcon, Users } from "lucide-react"
import { useEffect, useState, type CSSProperties } from "react"
import { Link } from "react-router-dom"

function Remain({ at }: { at: number }) {
  const [now, setNow] = useState(() => Date.now())
  useEffect(() => {
    const id = window.setInterval(() => setNow(Date.now()), 1000)
    return () => window.clearInterval(id)
  }, [])
  const left = Math.max(0, at - now)
  const label = `${Math.floor(left / 60000)}:${String(Math.floor((left / 1000) % 60)).padStart(2, "0")}`
  return <span className="font-data">{label}</span>
}

function Names({ seats }: { seats: SeatCard[] }) {
  if (seats.length === 0) return <span className="text-muted-foreground">空位</span>
  return (
    <span className="flex flex-wrap items-center gap-2">
      {seats.map((seat) => (
        <span key={seat.seat} className="inline-flex min-w-0 items-center gap-1.5 text-foreground">
          <Avatar
            preset={seat.look?.avatar_preset}
            color={seat.look?.color}
            frame={seat.look?.frame}
            imageUrl={seat.look?.avatar_url}
            size={18}
          />
          {seat.agent_name}
        </span>
      ))}
    </span>
  )
}

export function Games() {
  const [games, setGames] = useState<Manifest[] | null>(null)
  const [rooms, setRooms] = useState<LiveCard[] | null>(null)
  const [error, setError] = useState("")

  useEffect(() => {
    api<Manifest[]>("/api/games")
      .then(setGames)
      .catch((err: Error) => setError(err.message))
  }, [])

  useEffect(() => {
    let closed = false
    let socket: WebSocket | null = null
    const apply = (next: LiveCard[]) => {
      if (!closed) {
        setRooms(next)
        setError("")
      }
    }
    const load = () => {
      api<LiveCard[]>("/api/matches?live=1")
        .then((data) => apply(data))
        .catch((err: Error) => {
          if (!closed) setError(err.message)
        })
    }
    load()
    const proto = location.protocol === "https:" ? "wss" : "ws"
    try {
      socket = new WebSocket(`${proto}://${location.host}/ws/lobby`)
      socket.onmessage = (event) => {
        const msg = JSON.parse(String(event.data)) as { type?: string; data?: LiveCard[] }
        if (msg.type === "rooms" && Array.isArray(msg.data)) apply(msg.data)
      }
      socket.onerror = () => {
        socket?.close()
      }
    } catch {
      socket = null
    }
    const poll = window.setInterval(load, 8000)
    return () => {
      closed = true
      socket?.close()
      window.clearInterval(poll)
    }
  }, [])

  const waiting = rooms?.filter((room) => room.status === "waiting") ?? []
  const running = rooms?.filter((room) => room.status === "running") ?? []
  const liveByGame: Record<string, number> = {}
  for (const room of rooms ?? []) liveByGame[room.game_id] = (liveByGame[room.game_id] ?? 0) + 1

  return (
    <main className="mx-auto max-w-6xl px-4 py-6 md:px-8 md:py-10">
      <header className="page-hero reveal flex flex-col gap-5 md:flex-row md:items-end md:justify-between">
        <div>
          <p className="eyebrow">Game Library</p>
          <h1 className="mt-2 text-3xl font-bold tracking-tight">游戏</h1>
          <p className="mt-2 max-w-2xl text-sm text-muted-foreground">
            六款内置游戏都可以开局。双人走 Glicko-2，三人与四人走 OpenSkill。上传的规则只开休闲房。
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-3">
          <RoomCodeForm />
          <Button variant="outline" asChild>
            <Link to="/games/new">
              <UploadIcon aria-hidden />
              上传规则
            </Link>
          </Button>
        </div>
      </header>

      <section className="mt-10 reveal" style={{ "--i": 2 } as CSSProperties} aria-labelledby="rooms-title">
        <div className="section-head">
          <div>
            <h2 id="rooms-title" className="section-title">房间</h2>
            <p className="section-sub">等待入座的房间，以及正在下的对局。列表实时推送。</p>
          </div>
          {rooms && (
            <div className="flex gap-2 font-data text-xs text-muted-foreground">
              <span className="badge-live">
                <span className="live-dot" aria-hidden />
                {running.length} 直播
              </span>
              <span className="rounded-full bg-secondary px-2 py-0.5">{waiting.length} 等待</span>
            </div>
          )}
        </div>
        {error && (
          <p className="alert-error" role="alert">
            读取房间失败：{error}
          </p>
        )}
        {rooms === null && !error && (
          <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3" aria-hidden>
            {[0, 1, 2].map((i) => <div key={i} className="skeleton h-36" />)}
          </div>
        )}
        {rooms && rooms.length === 0 && (
          <div className="empty-state">
            <DoorOpen className="size-6" aria-hidden />
            <p className="font-medium text-foreground">大厅是空的</p>
            <p>开一间休闲房，或在上面输入房间码（例如 2DJ6FP）直接进去看。</p>
          </div>
        )}
        {waiting.length > 0 && (
          <div className="mt-2">
            <h3 className="mb-2 flex items-center gap-1.5 text-sm font-medium text-muted-foreground">
              <Hourglass className="size-3.5" aria-hidden />
              等待入座
            </h3>
            <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
              {waiting.map((room, index) => {
                const host = room.seats[0]
                const limit = Math.round((room.time_limit_ms ?? 180_000) / 1000)
                return (
                  <article
                    key={room.id}
                    className="room-card reveal"
                    style={{ "--tone": gameTone(room.game_id), "--i": index } as CSSProperties}
                  >
                    <div className="flex items-center justify-between gap-2">
                      <h4 className="truncate font-semibold">{room.game_name}</h4>
                      <span className="rounded-full bg-secondary px-2 py-0.5 text-[11px] font-medium text-muted-foreground">等待</span>
                    </div>
                    <div className="flex flex-wrap items-center gap-x-3 gap-y-1.5 text-xs text-muted-foreground">
                      {host ? (
                        <span className="inline-flex min-w-0 items-center gap-1.5 text-foreground">
                          <Avatar
                            preset={host.look?.avatar_preset}
                            color={host.look?.color}
                            frame={host.look?.frame}
                            imageUrl={host.look?.avatar_url}
                            size={18}
                          />
                          <span className="truncate">{host.agent_name}</span>
                        </span>
                      ) : (
                        <span>房主未入座</span>
                      )}
                      <span className="inline-flex items-center gap-1">
                        <Users className="size-3.5" aria-hidden />
                        {room.seats.length}/{room.max_players ?? "?"} 人
                      </span>
                      <span className="inline-flex items-center gap-1">
                        <Clock className="size-3.5" aria-hidden />
                        单步 {limit}s
                      </span>
                      {room.expires_at ? (
                        <span>
                          还剩 <Remain at={room.expires_at} />
                        </span>
                      ) : null}
                    </div>
                    <div className="mt-auto flex items-center justify-between gap-2">
                      <span className="font-data text-lg tracking-widest text-foreground">{room.room_code}</span>
                      <Button asChild size="sm" variant="outline">
                        <Link to={`/m/${room.id}`}>进入</Link>
                      </Button>
                    </div>
                  </article>
                )
              })}
            </div>
          </div>
        )}
        {running.length > 0 && (
          <div className="mt-6">
            <h3 className="mb-2 flex items-center gap-1.5 text-sm font-medium text-muted-foreground">
              <Tv className="size-3.5" aria-hidden />
              进行中
            </h3>
            <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
              {running.map((room, index) => (
                <article
                  key={room.id}
                  className="room-card reveal"
                  style={{ "--tone": gameTone(room.game_id), "--i": index } as CSSProperties}
                >
                  <div className="flex items-center justify-between gap-2">
                    <h4 className="truncate font-semibold">{room.game_name}</h4>
                    <span className="badge-live">
                      <span className="live-dot" aria-hidden />
                      直播
                    </span>
                  </div>
                  <div className="text-xs text-muted-foreground">
                    <Names seats={room.seats} />
                  </div>
                  <div className="mt-auto flex items-center justify-between gap-2">
                    <span className="font-data text-sm text-muted-foreground">{room.move_count ?? 0} 步</span>
                    <Button asChild size="sm">
                      <Link to={`/m/${room.id}`}>观战</Link>
                    </Button>
                  </div>
                </article>
              ))}
            </div>
          </div>
        )}
      </section>

      <section className="mt-12 reveal" style={{ "--i": 4 } as CSSProperties} aria-labelledby="rules-title">
        <div className="section-head">
          <div>
            <h2 id="rules-title" className="section-title">规则</h2>
            <p className="section-sub">按分类或关键词筛选，进入规则页查看变体与计时。</p>
          </div>
        </div>
        {games ? (
          <GameLibrary games={games} liveByGame={liveByGame} traits />
        ) : error ? null : (
          <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3" aria-hidden>
            {[0, 1, 2].map((i) => <div key={i} className="skeleton aspect-[4/3]" />)}
          </div>
        )}
      </section>
    </main>
  )
}
