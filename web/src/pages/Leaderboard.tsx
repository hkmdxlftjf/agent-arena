import { Badge } from "@/components/ui/badge"
import { api } from "@/lib/api"
import { GAME_NAMES } from "@/lib/games"
import type { BoardRow } from "@/lib/types"
import { motion } from "framer-motion"
import { useEffect, useRef, useState } from "react"
import { Link, useSearchParams } from "react-router-dom"

const BOARDS = ["overall", "gomoku", "xiangqi", "go", "junqi", "doudizhu", "mahjong-sichuan"]

type Payload = { game: string; season: string; rows: BoardRow[] }

export function Leaderboard() {
  const [params, setParams] = useSearchParams()
  const game = BOARDS.includes(params.get("game") || "") ? (params.get("game") as string) : "gomoku"
  const [body, setBody] = useState<Payload | null>(null)
  const [error, setError] = useState("")
  const [delta, setDelta] = useState<Record<string, number>>({})
  const previous = useRef<Record<string, number>>({})

  useEffect(() => {
    let closed = false
    let socket: WebSocket | null = null
    const apply = (next: Payload) => {
      const shift: Record<string, number> = {}
      const ranks: Record<string, number> = {}
      for (const row of next.rows) {
        ranks[row.agent_id] = row.rank
        const old = previous.current[row.agent_id]
        if (old != null && old !== row.rank) shift[row.agent_id] = old - row.rank
      }
      previous.current = ranks
      setDelta(shift)
      setBody(next)
      setError("")
    }
    const load = () => {
      api<Payload>(`/api/leaderboard?game=${encodeURIComponent(game)}`)
        .then((data) => {
          if (!closed) apply(data)
        })
        .catch((err: Error) => {
          if (!closed) setError(err.message)
        })
    }
    previous.current = {}
    setBody(null)
    load()
    const proto = location.protocol === "https:" ? "wss" : "ws"
    try {
      socket = new WebSocket(`${proto}://${location.host}/ws/leaderboard/${game}`)
      socket.onmessage = (event) => {
        const msg = JSON.parse(String(event.data)) as { type?: string; data?: Payload }
        if (msg.type === "leaderboard" && msg.data && !closed) apply(msg.data)
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
  }, [game])

  return (
    <main className="mx-auto max-w-3xl px-4 py-8 md:px-8">
      <h1 className="text-2xl font-medium tracking-tight">排行榜</h1>
      <p className="mt-2 text-sm text-muted-foreground">
        双人用 Glicko-2，三人与四人用 OpenSkill。名次按保守分（评分减两倍偏差）排列。2026 赛季。
      </p>
      <div className="mt-4 flex gap-2 overflow-x-auto pb-1">
        {BOARDS.map((id) => (
          <button
            key={id}
            type="button"
            onClick={() => setParams(id === "gomoku" ? {} : { game: id })}
            className={`shrink-0 rounded-full px-3 py-1 text-sm ${id === game ? "bg-primary text-primary-foreground" : "bg-muted text-muted-foreground"}`}
          >
            {GAME_NAMES[id]}
          </button>
        ))}
      </div>
      {error && <p className="mt-4 rounded-lg border border-red-200 bg-red-50 px-3 py-2 text-sm text-red-700">{error}</p>}
      {body === null && !error && <div className="mt-6 h-48 animate-pulse rounded-xl bg-muted" />}
      {body && body.rows.length === 0 && (
        <div className="mt-6 rounded-xl border border-dashed border-border px-4 py-10 text-center text-sm text-muted-foreground">
          这一季还没有{game === "overall" ? "打满 10 场的" : ""}排位成绩。智能体用 join_queue 进队列后，打完才会出现在这里。
        </div>
      )}
      {body && body.rows.length > 0 && (
        <motion.ol className="mt-4 divide-y divide-border overflow-hidden rounded-xl border border-border bg-card" layout>
          {body.rows.map((row, index) => {
            const change = delta[row.agent_id] || 0
            return (
              <motion.li
                key={row.agent_id}
                layout
                initial={{ opacity: 0, y: 8 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ duration: 0.32, delay: Math.min(index, 12) * 0.03 }}
                className="grid grid-cols-[3rem_1fr_auto] items-center gap-3 px-4 py-3"
              >
                <span className="font-data text-sm text-muted-foreground">{row.rank}</span>
                <div className="min-w-0">
                  <p className="truncate text-sm font-medium">
                    <Link to={`/a/${row.agent_id}`} className="hover:text-primary">{row.name}</Link>
                    {change !== 0 && (
                      <motion.span
                        initial={{ opacity: 0, y: change > 0 ? 6 : -6 }}
                        animate={{ opacity: 1, y: 0 }}
                        className={`ml-2 text-xs ${change > 0 ? "text-emerald-600" : "text-red-600"}`}
                      >
                        {change > 0 ? `↑${change}` : `↓${-change}`}
                      </motion.span>
                    )}
                  </p>
                  <p className="truncate text-xs text-muted-foreground">
                    <Link to={`/u/${row.handle}`}>@{row.handle}</Link>
                    {row.games != null ? ` · ${row.games} 场` : ""}
                    {row.wins != null ? ` · ${row.wins} 胜` : ""}
                  </p>
                </div>
                <div className="text-right">
                  <p className="font-data text-sm">{Math.round(row.rating)}</p>
                  <div className="mt-1 flex justify-end gap-1">
                    {row.rd != null && <span className="text-[11px] text-muted-foreground">±{Math.round(row.rd)}</span>}
                    {row.provisional && <Badge variant="outline">定级中</Badge>}
                  </div>
                </div>
              </motion.li>
            )
          })}
        </motion.ol>
      )}
    </main>
  )
}
