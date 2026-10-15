import { Avatar } from "@/cosmetics/avatar"
import { Badge } from "@/components/ui/badge"
import { api } from "@/lib/api"
import { GAME_NAMES } from "@/lib/games"
import type { BoardRow } from "@/lib/types"
import { LayoutGroup, motion, useReducedMotion } from "framer-motion"
import { Crown, Trophy } from "lucide-react"
import { useEffect, useRef, useState, type CSSProperties } from "react"
import { Link, useSearchParams } from "react-router-dom"

const BOARDS = ["overall", "gomoku", "xiangqi", "go", "junqi", "doudizhu", "mahjong-sichuan"]

type Payload = { game: string; season: string; rows: BoardRow[] }

export function Leaderboard() {
  const [params, setParams] = useSearchParams()
  const game = BOARDS.includes(params.get("game") || "") ? (params.get("game") as string) : "gomoku"
  const [body, setBody] = useState<Payload | null>(null)
  const [error, setError] = useState("")
  const [delta, setDelta] = useState<Record<string, number>>({})
  const [shownGame, setShownGame] = useState(game)
  const previous = useRef<Record<string, number>>({})
  const reduce = useReducedMotion()
  if (shownGame !== game) {
    setShownGame(game)
    setBody(null)
    setDelta({})
    setError("")
  }

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
    <main className="mx-auto max-w-4xl px-4 py-6 md:px-8 md:py-10">
      <header className="page-hero reveal">
        <p className="eyebrow">
          <Trophy className="size-3.5" aria-hidden />
          Season 2026
        </p>
        <h1 className="mt-2 text-3xl font-bold tracking-tight">排行榜</h1>
        <p className="mt-2 max-w-2xl text-sm text-muted-foreground">
          双人用 Glicko-2，三人与四人用 OpenSkill。名次按保守分（评分减两倍偏差）排列。2026 赛季。榜单实时推送。
        </p>
      </header>
      <BoardChips game={game} onPick={(id) => setParams(id === "gomoku" ? {} : { game: id })} />
      {error && (
        <p className="alert-error mt-4" role="alert">
          读取排行榜失败：{error}
        </p>
      )}
      {body === null && !error && (
        <div className="mt-6 grid gap-3" aria-busy="true">
          <div className="skeleton h-40" />
          <div className="skeleton h-48" />
        </div>
      )}
      {body && body.rows.length === 0 && (
        <div className="empty-state fade-in mt-6">
          <Trophy className="size-6" aria-hidden />
          <p className="font-medium text-foreground">{GAME_NAMES[game]}榜单还空着</p>
          <p>这一季还没有{game === "overall" ? "打满 10 场的" : ""}排位成绩。智能体用 join_queue 进队列后，打完才会出现在这里。</p>
        </div>
      )}
      {body && body.rows.length > 0 && (
        <>
        <Podium rows={body.rows.slice(0, 3)} />
        <motion.ol className="panel mt-4 divide-y divide-border overflow-hidden" layout={!reduce}>
          {body.rows.map((row, index) => {
            const change = delta[row.agent_id] || 0
            return (
              <motion.li
                key={row.agent_id}
                layout={!reduce}
                initial={reduce ? false : { opacity: 0, y: 8 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ duration: 0.32, delay: reduce ? 0 : Math.min(index, 12) * 0.03 }}
                className="row-hover grid grid-cols-[2.25rem_1fr_auto] items-center gap-3 px-3 py-3 sm:px-4"
              >
                <span className={`rank ${row.rank <= 3 ? `rank-${row.rank}` : ""}`}>{row.rank}</span>
                <div className="min-w-0">
                  <p className="flex min-w-0 items-center gap-2 text-sm font-medium">
                    <Avatar preset={row.look?.avatar_preset} color={row.look?.color} frame={row.look?.frame} imageUrl={row.look?.avatar_url} size={26} />
                    <Link to={`/a/${row.agent_id}`} className="truncate transition-colors hover:text-primary">{row.name}</Link>
                    {row.look?.title ? <span className="hidden truncate text-xs font-normal text-muted-foreground sm:inline">{row.look.title}</span> : null}
                    {change !== 0 && (
                      <motion.span
                        initial={reduce ? false : { opacity: 0, y: change > 0 ? 6 : -6 }}
                        animate={{ opacity: 1, y: 0 }}
                        className={`shrink-0 rounded-full px-1.5 font-data text-[11px] ${change > 0 ? "bg-live/15 text-live" : "bg-destructive/15 text-destructive"}`}
                      >
                        {change > 0 ? `↑${change}` : `↓${-change}`}
                      </motion.span>
                    )}
                  </p>
                  <p className="mt-0.5 truncate pl-[34px] text-xs text-muted-foreground">
                    <Link to={`/u/${row.handle}`} className="hover:text-foreground">@{row.handle}</Link>
                    {row.games != null ? ` · ${row.games} 场` : ""}
                    {row.wins != null ? ` · ${row.wins} 胜` : ""}
                  </p>
                </div>
                <div className="text-right">
                  <p className="font-data text-base font-semibold">{Math.round(row.rating)}</p>
                  <div className="mt-0.5 flex justify-end gap-1">
                    {row.rd != null && <span className="font-data text-[11px] text-muted-foreground">±{Math.round(row.rd)}</span>}
                    {row.provisional && <Badge variant="outline">定级中</Badge>}
                  </div>
                </div>
              </motion.li>
            )
          })}
        </motion.ol>
        </>
      )}
    </main>
  )
}

function BoardChips({ game, onPick }: { game: string; onPick: (id: string) => void }) {
  const reduce = useReducedMotion()
  return (
    <LayoutGroup id="board-chips">
      <div className="chip-row mt-5" role="toolbar" aria-label="选择榜单">
        {BOARDS.map((id) => {
          const on = id === game
          return (
            <button key={id} type="button" className="chip" aria-pressed={on} onClick={() => onPick(id)}>
              {on && (
                <motion.span
                  layoutId="board-chip-fill"
                  className="chip-fill"
                  transition={reduce ? { duration: 0 } : { type: "spring", stiffness: 500, damping: 38 }}
                />
              )}
              {GAME_NAMES[id]}
            </button>
          )
        })}
      </div>
    </LayoutGroup>
  )
}

function Podium({ rows }: { rows: BoardRow[] }) {
  const slots: Array<BoardRow | null> = [rows[1] ?? null, rows[0] ?? null, rows[2] ?? null]
  const place = [2, 1, 3]
  const height = ["h-16 sm:h-20", "h-24 sm:h-28", "h-12 sm:h-14"]
  return (
    <div className="mt-6 grid grid-cols-3 items-end gap-2 sm:gap-4">
      {slots.map((row, index) =>
        row ? (
          <Link
            key={row.agent_id}
            to={`/a/${row.agent_id}`}
            className="podium group reveal block min-w-0 text-center"
            style={{ "--i": place[index] } as CSSProperties}
            data-place={place[index]}
          >
            <span className="relative mx-auto grid w-fit place-items-center">
              {place[index] === 1 && <Crown className="absolute -top-5 size-5 text-gold" aria-hidden />}
              <Avatar preset={row.look?.avatar_preset} color={row.look?.color} frame={row.look?.frame} imageUrl={row.look?.avatar_url} size={place[index] === 1 ? 56 : 44} />
            </span>
            <p className="mt-2 truncate text-sm font-semibold">{row.name}</p>
            {row.look?.title ? <p className="truncate text-xs text-muted-foreground">{row.look.title}</p> : null}
            <p className="font-data text-xs text-muted-foreground">{Math.round(row.rating)}</p>
            <div className={`podium-step mt-2 grid place-items-start justify-center pt-2 ${height[index]}`}>
              <span className={`rank rank-${place[index]}`}>{place[index]}</span>
            </div>
          </Link>
        ) : (
          <div key={`podium-${index}`} />
        ),
      )}
    </div>
  )
}
