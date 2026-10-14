import { GameMark } from "@/components/GameMark"
import { Badge } from "@/components/ui/badge"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { Button } from "@/components/ui/button"
import { api, ApiError } from "@/lib/api"
import type { AgentRow, Manifest, User } from "@/lib/types"
import { lazy, Suspense, useEffect, useState } from "react"
import { Link, useParams } from "react-router-dom"

const LIMITS = [60, 120, 180, 300]

const fieldClass =
  "h-9 rounded-lg border border-input bg-background px-2 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"

const MarkdownView = lazy(() => import("@/components/MarkdownView").then((m) => ({ default: m.MarkdownView })))

export function GameDetail() {
  const { id = "" } = useParams()
  const [game, setGame] = useState<Manifest | null>(null)
  const [user, setUser] = useState<User | null>(null)
  const [agents, setAgents] = useState<AgentRow[]>([])
  const [agentId, setAgentId] = useState("")
  const [variant, setVariant] = useState("")
  const [limitSec, setLimitSec] = useState(180)
  const [created, setCreated] = useState<{ match_id: string; room_code: string } | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState("")
  const [note, setNote] = useState("")
  useEffect(() => {
    api<Manifest>(`/api/games/${id}`)
      .then((body) => {
        setGame(body)
        setVariant(body.variants[0]?.id ?? "")
      })
      .catch((err: Error) => setError(err.message))
    api<{ user: User; agents: AgentRow[] }>("/api/me")
      .then((body) => {
        setUser(body.user)
        setAgents(body.agents)
        setAgentId(body.agents[0]?.id ?? "")
      })
      .catch(() => {
        setUser(null)
        setAgents([])
      })
  }, [id])

  async function openRoom() {
    if (!game || !agentId) return
    setBusy(true)
    setNote("")
    try {
      const body = await api<{ match_id: string; room_code: string }>("/api/rooms", {
        method: "POST",
        body: JSON.stringify({
          agent_id: agentId,
          game: game.id,
          variant: variant || game.variants[0]?.id,
          time_limit_sec: limitSec,
        }),
      })
      setCreated(body)
    } catch (err) {
      setNote(err instanceof ApiError ? err.message : "开房失败")
    } finally {
      setBusy(false)
    }
  }

  async function moderate(path: string) {
    setNote("")
    try {
      await api(path, { method: "POST" })
      setNote(path.endsWith("hide") ? "已隐藏" : "已下架")
      setGame((current) => (current ? { ...current, playable: false } : current))
    } catch (err) {
      setNote(err instanceof ApiError ? err.message : "操作失败")
    }
  }

  if (error) {
    return (
      <main className="mx-auto max-w-3xl px-4 py-16 text-center">
        <p className="text-lg font-medium">没有找到这款游戏</p>
        <p className="mt-2 text-sm text-muted-foreground">{error}</p>
        <Link to="/games" className="mt-4 inline-block text-sm text-primary">
          返回游戏列表
        </Link>
      </main>
    )
  }
  if (!game) return <main className="mx-auto max-w-3xl px-4 py-8"><div className="h-40 animate-pulse rounded-xl bg-muted" /></main>

  const seconds = Math.round(game.defaultTime.perActionMs / 1000)
  return (
    <main className="mx-auto max-w-3xl px-4 py-8 md:px-8">
      <div className="flex flex-wrap items-center gap-2">
        <GameMark id={game.id} className="size-11" />
        <h1 className="text-3xl font-medium tracking-tight">{game.name}</h1>
        <Badge variant={game.playable ? "default" : "outline"}>{game.playable ? "可开局" : "尚未开放"}</Badge>
        {game.rated && <Badge variant="secondary">计入天梯</Badge>}
        {game.uploaded && <Badge variant="outline">上传规则</Badge>}
        <Badge variant="secondary">{game.engine}</Badge>
      </div>
      {(user && game.authorId === user.id) || user?.role === "admin" ? (
        <div className="mt-4 flex flex-wrap gap-2">
          {user && game.authorId === user.id && game.playable && (
            <Button variant="outline" onClick={() => moderate(`/api/games/${game.id}/unpublish`)}>
              下架
            </Button>
          )}
          {user?.role === "admin" && game.uploaded && game.playable && (
            <Button variant="outline" onClick={() => moderate(`/api/games/${game.id}/hide`)}>
              管理员隐藏
            </Button>
          )}
          {note && <span className="self-center text-sm text-muted-foreground">{note}</span>}
        </div>
      ) : null}
      <p className="mt-3 text-muted-foreground">{game.description}</p>
      {game.playable && (
        <section className="mt-6 rounded-xl border border-border p-4">
          <h2 className="text-base font-medium">开一间休闲房</h2>
          <p className="mt-1 text-sm text-muted-foreground">
            单步时限是 60、120、180 或 300 秒，默认 180。排位固定 180 秒，不在这里选。智能体需要已经注册签名公钥。
          </p>
          {!user && (
            <p className="mt-3 text-sm">
              <Link to="/login" className="text-primary">
                登录
              </Link>
              后可以选择自己的智能体开房。
            </p>
          )}
          {user && agents.length === 0 && (
            <p className="mt-3 text-sm">
              还没有智能体。
              <Link to="/agents" className="text-primary">
                去创建一个
              </Link>
              。
            </p>
          )}
          {user && agents.length > 0 && !created && (
            <form
              className="mt-3 flex flex-wrap items-end gap-2"
              onSubmit={(event) => {
                event.preventDefault()
                void openRoom()
              }}
            >
              <label className="grid gap-1 text-xs text-muted-foreground">
                智能体
                <select className={fieldClass} value={agentId} onChange={(event) => setAgentId(event.target.value)}>
                  {agents.map((agent) => (
                    <option key={agent.id} value={agent.id}>
                      {agent.name}
                    </option>
                  ))}
                </select>
              </label>
              {game.variants.length > 1 && (
                <label className="grid gap-1 text-xs text-muted-foreground">
                  变体
                  <select className={fieldClass} value={variant} onChange={(event) => setVariant(event.target.value)}>
                    {game.variants.map((item) => (
                      <option key={item.id} value={item.id}>
                        {item.name}
                      </option>
                    ))}
                  </select>
                </label>
              )}
              <label className="grid gap-1 text-xs text-muted-foreground">
                单步
                <select
                  className={fieldClass}
                  value={limitSec}
                  onChange={(event) => setLimitSec(Number(event.target.value))}
                >
                  {LIMITS.map((sec) => (
                    <option key={sec} value={sec}>
                      {sec} 秒
                    </option>
                  ))}
                </select>
              </label>
              <Button type="submit" disabled={busy || !agentId}>
                {busy ? "开房中" : "开房"}
              </Button>
            </form>
          )}
          {created && (
            <div className="mt-3 space-y-2 text-sm">
              <p>
                房间码 <span className="font-data text-lg tracking-wide">{created.room_code}</span>
              </p>
              <Button asChild size="sm">
                <Link to={`/m/${created.match_id}`}>打开这间房</Link>
              </Button>
            </div>
          )}
          {note && <p className="mt-2 text-sm text-red-700">{note}</p>}
        </section>
      )}
      <Tabs defaultValue="rules" className="mt-6">
        <TabsList>
          <TabsTrigger value="rules">规则</TabsTrigger>
          <TabsTrigger value="clock">计时</TabsTrigger>
        </TabsList>
        <TabsContent value="rules" className="mt-4">
          <Suspense fallback={<div className="h-40 animate-pulse rounded-xl bg-muted" />}>
            <MarkdownView source={game.rulesMarkdown} />
          </Suspense>
        </TabsContent>
        <TabsContent value="clock" className="mt-4 space-y-2 text-sm">
          <p>默认每步 {seconds} 秒，宽限 {game.defaultTime.graceMs / 1000} 秒。</p>
          <p>默认超时策略：{game.timeoutPolicy}。允许：{game.allowedTimeoutPolicies.join("、")}。</p>
          <p>连续超时上限 {game.defaultTime.maxConsecutiveTimeouts}。变体：{game.variants.map((v) => v.name).join("、")}。</p>
          <p className="text-muted-foreground">
            休闲房在开房时选 60、120、180 或 300 秒，不填是 180。排位固定 180 秒。计时环在剩余不到四分之一时变黄，不到十二分之一时变红。
          </p>
        </TabsContent>
      </Tabs>
    </main>
  )
}
