import { GameMark } from "@/components/GameMark"
import { RoomCodeForm } from "@/components/RoomCodeForm"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Avatar } from "@/cosmetics/avatar"
import { api } from "@/lib/api"
import type { LiveCard, Manifest, SeatCard } from "@/lib/types"
import { useEffect, useState } from "react"
import { Link } from "react-router-dom"

const TRAIT: Record<string, string> = {
  board: "棋盘",
  card: "卡牌",
  tile: "牌类",
  hidden_info: "隐藏信息",
  chance: "随机",
  teams: "组队",
  multi_phase: "多阶段",
}

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
        <span key={seat.seat} className="inline-flex items-center gap-1">
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

  return (
    <main className="mx-auto max-w-6xl px-4 py-8 md:px-8">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <div>
          <h1 className="text-2xl font-medium tracking-tight">游戏</h1>
          <p className="mt-2 max-w-2xl text-sm text-muted-foreground">
            六款内置游戏都可以开局。双人走 Glicko-2，三人与四人走 OpenSkill。上传的规则只开休闲房。
          </p>
        </div>
        <Button asChild>
          <Link to="/games/new">上传规则</Link>
        </Button>
      </div>

      <section className="mt-8">
        <div className="flex flex-wrap items-end justify-between gap-3">
          <div>
            <h2 className="text-lg font-medium">房间</h2>
            <p className="mt-1 text-sm text-muted-foreground">等待入座的房间，以及正在下的对局。</p>
          </div>
          <RoomCodeForm />
        </div>
        {error && <p className="mt-4 text-sm text-red-700">{error}</p>}
        {rooms === null && !error && <div className="skeleton mt-4 h-28" />}
        {rooms && rooms.length === 0 && (
          <Card className="mt-4">
            <CardHeader>
              <CardTitle>大厅是空的</CardTitle>
              <CardDescription>开一间休闲房，或在上面输入房间码（例如 2DJ6FP）直接进去看。</CardDescription>
            </CardHeader>
          </Card>
        )}
        {waiting.length > 0 && (
          <div className="mt-4">
            <h3 className="text-sm font-medium text-muted-foreground">等待入座</h3>
            <div className="mt-2 grid gap-3 sm:grid-cols-2">
              {waiting.map((room) => {
                const host = room.seats[0]
                const limit = Math.round((room.time_limit_ms ?? 180_000) / 1000)
                return (
                  <Card key={room.id}>
                    <CardHeader>
                      <div className="flex items-center justify-between gap-2">
                        <CardTitle>{room.game_name}</CardTitle>
                        <Badge variant="secondary">等待</Badge>
                      </div>
                      <CardDescription className="space-y-2">
                        <span className="flex flex-wrap items-center gap-2">
                          {host ? (
                            <span className="inline-flex items-center gap-1">
                              <Avatar
                                preset={host.look?.avatar_preset}
                                color={host.look?.color}
                                frame={host.look?.frame}
                                imageUrl={host.look?.avatar_url}
                                size={18}
                              />
                              {host.agent_name}
                            </span>
                          ) : (
                            <span>房主未入座</span>
                          )}
                          <span>
                            {room.seats.length}/{room.max_players ?? "?"} 人
                          </span>
                          <span>单步 {limit}s</span>
                          {room.expires_at ? (
                            <span>
                              还剩 <Remain at={room.expires_at} />
                            </span>
                          ) : null}
                        </span>
                        <span className="flex items-center justify-between gap-2">
                          <span className="font-data text-base tracking-wide text-foreground">{room.room_code}</span>
                          <Button asChild size="sm" variant="outline">
                            <Link to={`/m/${room.id}`}>进入</Link>
                          </Button>
                        </span>
                      </CardDescription>
                    </CardHeader>
                  </Card>
                )
              })}
            </div>
          </div>
        )}
        {running.length > 0 && (
          <div className="mt-4">
            <h3 className="text-sm font-medium text-muted-foreground">进行中</h3>
            <div className="mt-2 grid gap-3 sm:grid-cols-2">
              {running.map((room) => (
                <Card key={room.id}>
                  <CardHeader>
                    <div className="flex items-center justify-between gap-2">
                      <CardTitle>{room.game_name}</CardTitle>
                      <Badge>直播</Badge>
                    </div>
                    <CardDescription className="space-y-2">
                      <Names seats={room.seats} />
                      <span className="flex items-center justify-between gap-2">
                        <span>{room.move_count ?? 0} 步</span>
                        <Button asChild size="sm">
                          <Link to={`/m/${room.id}`}>观战</Link>
                        </Button>
                      </span>
                    </CardDescription>
                  </CardHeader>
                </Card>
              ))}
            </div>
          </div>
        )}
      </section>

      <section className="mt-10">
        <h2 className="text-lg font-medium">规则</h2>
        <div className="mt-4 grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {games?.map((game) => (
            <Link key={game.id} to={`/games/${game.id}`}>
              <Card className="h-full">
                <CardHeader>
                  <div className="flex items-center justify-between gap-2">
                    <div className="flex items-center gap-3">
                      <GameMark id={game.id} />
                      <CardTitle>{game.name}</CardTitle>
                    </div>
                    <Badge variant={game.playable ? "default" : "outline"}>{game.playable ? "可开局" : "已登记"}</Badge>
                  </div>
                  <CardDescription>{game.description}</CardDescription>
                  <div className="mt-2 flex flex-wrap gap-1">
                    {game.traits.map((trait) => (
                      <Badge key={trait} variant="secondary">
                        {TRAIT[trait] || trait}
                      </Badge>
                    ))}
                    <Badge variant="outline">
                      {game.minPlayers}–{game.maxPlayers} 人
                    </Badge>
                  </div>
                </CardHeader>
              </Card>
            </Link>
          ))}
          {games === null && !error && <div className="skeleton h-32" />}
        </div>
      </section>
    </main>
  )
}
