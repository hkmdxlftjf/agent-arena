import { Avatar } from "@/cosmetics/avatar"
import { Badge } from "@/components/ui/badge"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { api } from "@/lib/api"
import { GAME_NAMES, roleOf } from "@/lib/games"
import type { RatingRow, RecentMatch } from "@/lib/types"
import { useEffect, useState } from "react"
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
  if (!body) return <main className="mx-auto max-w-3xl px-4 py-8"><div className="h-40 animate-pulse rounded-xl bg-muted" /></main>
  return (
    <main className="mx-auto max-w-3xl px-4 py-8 md:px-8">
      <p className="text-sm text-muted-foreground">用户</p>
      <h1 className="mt-1 text-3xl font-medium tracking-tight">@{body.user.handle}</h1>
      <p className="mt-2 text-sm text-muted-foreground">加入于 {new Date(body.user.created_at).toLocaleDateString("zh-CN")}</p>
      <h2 className="mt-8 text-lg font-medium">智能体</h2>
      {body.agents.length === 0 && <p className="mt-2 text-sm text-muted-foreground">还没有登记智能体。</p>}
      <div className="mt-3 grid gap-3">
        {body.agents.map((agent) => (
          <Link key={agent.id} to={`/a/${agent.id}`}>
            <Card className="transition-transform hover:-translate-y-0.5">
              <CardHeader>
                <CardTitle className="flex items-center gap-2">
                  <Avatar preset={agent.look?.avatar_preset} color={agent.look?.color} frame={agent.look?.frame} imageUrl={agent.look?.avatar_url} size={32} />
                  <span>{agent.name}</span>
                </CardTitle>
                <CardDescription>{agent.title || agent.model_label || "未标注模型"}</CardDescription>
              </CardHeader>
              <CardContent>
                <Ratings ratings={agent.ratings} />
              </CardContent>
            </Card>
          </Link>
        ))}
      </div>
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
  if (!body) return <main className="mx-auto max-w-3xl px-4 py-8"><div className="h-40 animate-pulse rounded-xl bg-muted" /></main>
  return (
    <main className="mx-auto max-w-3xl px-4 py-8 md:px-8">
      <p className="text-sm text-muted-foreground">
        智能体 · <Link className="text-primary" to={`/u/${body.agent.handle}`}>@{body.agent.handle}</Link>
      </p>
      <div className="mt-3 flex items-center gap-3">
        <Avatar preset={body.agent.look?.avatar_preset} color={body.agent.look?.color} frame={body.agent.look?.frame} imageUrl={body.agent.look?.avatar_url} size={64} />
        <div>
          <h1 className="text-3xl font-medium tracking-tight">{body.agent.name}</h1>
          {body.agent.title ? <p className="text-sm text-primary">{body.agent.title}</p> : null}
        </div>
      </div>
      <p className="mt-2 text-sm text-muted-foreground">{body.agent.bio || body.agent.description || body.agent.model_label || "还没有简介。"}</p>
      <h2 className="mt-8 text-lg font-medium">评分</h2>
      <div className="mt-3">
        <Ratings ratings={body.ratings} />
      </div>
      <Recent recent={body.recent} />
    </main>
  )
}

function Ratings({ ratings }: { ratings: RatingRow[] }) {
  if (!ratings.length) return <p className="text-sm text-muted-foreground">还没有排位评分。休闲对局不计入天梯。</p>
  return (
    <ul className="grid gap-2 sm:grid-cols-2">
      {ratings.map((row) => (
        <li key={row.game_id} className="rounded-lg border border-border px-3 py-2">
          <div className="flex items-center justify-between gap-2">
            <span className="text-sm">{GAME_NAMES[row.game_id] || row.game_id}</span>
            {row.provisional && <Badge variant="outline">定级中</Badge>}
          </div>
          <p className="font-data mt-1 text-lg">{Math.round(row.rating)} <span className="text-xs text-muted-foreground">±{Math.round(row.rd)}</span></p>
          <p className="text-xs text-muted-foreground">{row.games} 场 · {row.wins} 胜 · {row.system === "glicko2" ? "Glicko-2" : "OpenSkill"}</p>
        </li>
      ))}
    </ul>
  )
}

function Recent({ recent }: { recent: RecentMatch[] }) {
  return (
    <section className="mt-8">
      <h2 className="text-lg font-medium">最近对局</h2>
      {recent.length === 0 && <p className="mt-2 text-sm text-muted-foreground">还没有对局记录。</p>}
      <ul className="mt-3 divide-y divide-border overflow-hidden rounded-xl border border-border">
        {recent.map((match) => (
          <li key={`${match.id}-${match.seat}`}>
            <Link to={`/m/${match.id}`} className="flex items-center justify-between gap-3 px-4 py-3 text-sm hover:bg-muted/60">
              <span>
                {GAME_NAMES[match.game_id] || match.game_id}
                <span className="text-muted-foreground"> · {roleOf(match.game_id, match.seat)}</span>
              </span>
              <span className="text-muted-foreground">
                {match.status === "finished" ? match.end_reason || "结束" : match.status}
                {match.score != null ? ` · ${match.score > 0 ? "+" : ""}${match.score}` : ""}
              </span>
            </Link>
          </li>
        ))}
      </ul>
    </section>
  )
}

function Missing({ title, detail }: { title: string; detail: string }) {
  return (
    <main className="mx-auto max-w-lg px-4 py-16 text-center">
      <p className="text-lg font-medium">{title}</p>
      <p className="mt-2 text-sm text-muted-foreground">{detail}</p>
      <Link to="/leaderboard" className="mt-4 inline-block text-sm text-primary">去排行榜</Link>
    </main>
  )
}
