import { Avatar } from "@/cosmetics/avatar"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { api } from "@/lib/api"
import { GAME_NAMES, gameTone, roleOf } from "@/lib/games"
import type { RatingRow, RecentMatch } from "@/lib/types"
import { Bot, ChevronRight, History, Trophy, UserRound } from "lucide-react"
import { useEffect, useState, type CSSProperties } from "react"
import { Link, useParams } from "react-router-dom"

type UserBody = {
  user: { handle: string; created_at: number }
  agents: { id: string; name: string; model_label?: string | null; title?: string | null; bio?: string | null; look?: { avatar_preset?: string; color?: string; frame?: string | null; avatar_url?: string | null; title?: string | null }; ratings: RatingRow[] }[]
  recent: RecentMatch[]
}

type AgentBody = {
  agent: { id: string; name: string; description?: string | null; model_label?: string | null; handle: string; title?: string | null; bio?: string | null; look?: { avatar_preset?: string; color?: string; frame?: string | null; avatar_url?: string | null } }
  ratings: RatingRow[]
  recent: RecentMatch[]
}

export function UserProfile() {
  const { handle = "" } = useParams()
  const [body, setBody] = useState<UserBody | null>(null)
  const [error, setError] = useState("")
  const [shown, setShown] = useState(handle)
  if (shown !== handle) {
    setShown(handle)
    setBody(null)
    setError("")
  }
  useEffect(() => {
    let cancel = false
    api<UserBody>(`/api/users/${encodeURIComponent(handle)}`)
      .then((next) => {
        if (!cancel) setBody(next)
      })
      .catch((err: Error) => {
        if (!cancel) setError(err.message)
      })
    return () => {
      cancel = true
    }
  }, [handle])
  if (error) return <Missing title="没有这个用户" detail={error} />
  if (!body) return <ProfileSkeleton />
  const rated = body.agents.reduce((sum, agent) => sum + agent.ratings.reduce((n, row) => n + row.games, 0), 0)
  return (
    <main className="mx-auto max-w-4xl px-4 py-6 md:px-8 md:py-10">
      <header className="page-hero reveal flex flex-col gap-5 sm:flex-row sm:items-center sm:justify-between">
        <div className="flex items-center gap-4">
          <span className="grid size-16 shrink-0 place-items-center rounded-2xl bg-accent text-2xl font-bold text-accent-foreground">
            {body.user.handle.slice(0, 1).toUpperCase()}
          </span>
          <div className="min-w-0">
            <p className="eyebrow">
              <UserRound className="size-3.5" aria-hidden />
              Player
            </p>
            <h1 className="mt-1 truncate text-3xl font-bold tracking-tight">@{body.user.handle}</h1>
            <p className="mt-1 text-sm text-muted-foreground">加入于 {new Date(body.user.created_at).toLocaleDateString("zh-CN")}</p>
          </div>
        </div>
        <dl className="grid grid-cols-3 gap-3 sm:w-80">
          <div className="stat">
            <dt className="stat-label">智能体</dt>
            <dd className="stat-value">{body.agents.length}</dd>
          </div>
          <div className="stat">
            <dt className="stat-label">排位场次</dt>
            <dd className="stat-value">{rated}</dd>
          </div>
          <div className="stat">
            <dt className="stat-label">最近对局</dt>
            <dd className="stat-value">{body.recent.length}</dd>
          </div>
        </dl>
      </header>
      <section className="mt-10 reveal" style={{ "--i": 2 } as CSSProperties} aria-labelledby="agents-title">
        <h2 id="agents-title" className="section-title mb-4">智能体</h2>
        {body.agents.length === 0 && (
          <div className="empty-state">
            <Bot className="size-6" aria-hidden />
            <p>还没有登记智能体。</p>
          </div>
        )}
        <div className="grid gap-3 md:grid-cols-2">
          {body.agents.map((agent) => (
            <Link key={agent.id} to={`/a/${agent.id}`} className="room-card group">
              <div className="flex items-center gap-3">
                <Avatar preset={agent.look?.avatar_preset} color={agent.look?.color} frame={agent.look?.frame} imageUrl={agent.look?.avatar_url} size={40} />
                <div className="min-w-0 flex-1">
                  <h3 className="truncate font-semibold">{agent.name}</h3>
                  <p className="truncate text-sm text-muted-foreground">{agent.title || agent.model_label || "未标注模型"}</p>
                </div>
                <ChevronRight className="size-4 text-muted-foreground transition-transform group-hover:translate-x-0.5" aria-hidden />
              </div>
              <Ratings ratings={agent.ratings} />
            </Link>
          ))}
        </div>
      </section>
      <Recent recent={body.recent} />
    </main>
  )
}

export function AgentProfile() {
  const { id = "" } = useParams()
  const [body, setBody] = useState<AgentBody | null>(null)
  const [error, setError] = useState("")
  const [shown, setShown] = useState(id)
  if (shown !== id) {
    setShown(id)
    setBody(null)
    setError("")
  }
  useEffect(() => {
    let cancel = false
    api<AgentBody>(`/api/agents/${id}/profile`)
      .then((next) => {
        if (!cancel) setBody(next)
      })
      .catch((err: Error) => {
        if (!cancel) setError(err.message)
      })
    return () => {
      cancel = true
    }
  }, [id])
  if (error) return <Missing title="没有这个智能体" detail={error} />
  if (!body) return <ProfileSkeleton />
  const best = body.ratings.reduce<RatingRow | null>((top, row) => (!top || row.rating > top.rating ? row : top), null)
  const games = body.ratings.reduce((n, row) => n + row.games, 0)
  const wins = body.ratings.reduce((n, row) => n + row.wins, 0)
  return (
    <main className="mx-auto max-w-4xl px-4 py-6 md:px-8 md:py-10">
      <header className="hero-lobby reveal p-5 sm:p-8">
        <div className="flex flex-col gap-6 sm:flex-row sm:items-center sm:justify-between">
          <div className="flex min-w-0 items-center gap-4">
            <span className="stage shrink-0">
              <Avatar preset={body.agent.look?.avatar_preset} color={body.agent.look?.color} frame={body.agent.look?.frame} imageUrl={body.agent.look?.avatar_url} size={72} />
            </span>
            <div className="min-w-0">
              <p className="eyebrow">
                <Bot className="size-3.5" aria-hidden />
                Agent ·{" "}
                <Link className="normal-case tracking-normal hover:underline" to={`/u/${body.agent.handle}`}>
                  @{body.agent.handle}
                </Link>
              </p>
              <h1 className="mt-1 truncate text-3xl font-bold tracking-tight">{body.agent.name}</h1>
              {body.agent.title ? <p className="text-sm font-medium text-gold">{body.agent.title}</p> : null}
            </div>
          </div>
          <dl className="grid grid-cols-3 gap-3 sm:w-80">
            <div className="stat">
              <dt className="stat-label">最高评分</dt>
              <dd className="stat-value">{best ? Math.round(best.rating) : "—"}</dd>
            </div>
            <div className="stat">
              <dt className="stat-label">排位场次</dt>
              <dd className="stat-value">{games}</dd>
            </div>
            <div className="stat">
              <dt className="stat-label">胜场</dt>
              <dd className="stat-value text-live">{wins}</dd>
            </div>
          </dl>
        </div>
        <p className="mt-5 max-w-2xl text-sm text-muted-foreground">{body.agent.bio || body.agent.description || body.agent.model_label || "还没有简介。"}</p>
      </header>
      <section className="mt-10 reveal" style={{ "--i": 2 } as CSSProperties} aria-labelledby="ratings-title">
        <h2 id="ratings-title" className="section-title mb-4">评分</h2>
        <Ratings ratings={body.ratings} />
      </section>
      <Recent recent={body.recent} />
    </main>
  )
}

function Ratings({ ratings }: { ratings: RatingRow[] }) {
  if (!ratings.length) return <p className="text-sm text-muted-foreground">还没有排位评分。休闲对局不计入天梯。</p>
  return (
    <ul className="grid gap-2 sm:grid-cols-2">
      {ratings.map((row) => (
        <li
          key={row.game_id}
          className="relative overflow-hidden rounded-xl border border-border bg-background/40 px-3 py-2.5"
          style={{ "--tone": gameTone(row.game_id) } as CSSProperties}
        >
          <span className="absolute inset-y-0 left-0 w-0.5 bg-[var(--tone)]" aria-hidden />
          <div className="flex items-center justify-between gap-2">
            <span className="text-sm font-medium">{GAME_NAMES[row.game_id] || row.game_id}</span>
            {row.provisional && <Badge variant="outline">定级中</Badge>}
          </div>
          <p className="font-data mt-1 text-xl font-semibold">
            {Math.round(row.rating)} <span className="text-xs font-normal text-muted-foreground">±{Math.round(row.rd)}</span>
          </p>
          <p className="text-xs text-muted-foreground">{row.games} 场 · {row.wins} 胜 · {row.system === "glicko2" ? "Glicko-2" : "OpenSkill"}</p>
        </li>
      ))}
    </ul>
  )
}

function Recent({ recent }: { recent: RecentMatch[] }) {
  return (
    <section className="mt-10 reveal" style={{ "--i": 4 } as CSSProperties} aria-labelledby="recent-title">
      <h2 id="recent-title" className="section-title mb-4">最近对局</h2>
      {recent.length === 0 ? (
        <div className="empty-state">
          <History className="size-6" aria-hidden />
          <p>还没有对局记录。</p>
        </div>
      ) : (
        <ul className="panel divide-y divide-border overflow-hidden">
          {recent.map((match) => (
            <li key={`${match.id}-${match.seat}`}>
              <Link to={`/m/${match.id}`} className="row-hover flex items-center justify-between gap-3 px-4 py-3 text-sm">
                <span className="flex min-w-0 items-center gap-2.5">
                  <span className="size-2 shrink-0 rounded-full" style={{ background: gameTone(match.game_id) }} aria-hidden />
                  <span className="truncate">
                    {GAME_NAMES[match.game_id] || match.game_id}
                    <span className="text-muted-foreground"> · {roleOf(match.game_id, match.seat)}</span>
                  </span>
                </span>
                <span className="flex shrink-0 items-center gap-2 text-muted-foreground">
                  {match.status === "finished" ? match.end_reason || "结束" : match.status}
                  {match.score != null && (
                    <span className={`font-data ${match.score > 0 ? "text-live" : match.score < 0 ? "text-destructive" : ""}`}>
                      {match.score > 0 ? "+" : ""}
                      {match.score}
                    </span>
                  )}
                  <ChevronRight className="size-4" aria-hidden />
                </span>
              </Link>
            </li>
          ))}
        </ul>
      )}
    </section>
  )
}

function ProfileSkeleton() {
  return (
    <main className="mx-auto max-w-4xl px-4 py-6 md:px-8 md:py-10" aria-busy="true">
      <div className="skeleton h-40" />
      <div className="mt-8 grid gap-3 md:grid-cols-2">
        <div className="skeleton h-32" />
        <div className="skeleton h-32" />
      </div>
    </main>
  )
}

function Missing({ title, detail }: { title: string; detail: string }) {
  return (
    <main className="mx-auto max-w-lg px-4 py-16">
      <div className="empty-state reveal">
        <Trophy className="size-7" aria-hidden />
        <p className="text-lg font-semibold text-foreground">{title}</p>
        <p>{detail}</p>
        <Button variant="outline" asChild className="mt-2">
          <Link to="/leaderboard">去排行榜</Link>
        </Button>
      </div>
    </main>
  )
}
