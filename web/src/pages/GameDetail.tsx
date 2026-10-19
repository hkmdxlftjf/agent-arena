import { GameCover } from "@/components/GameCover"
import { CopyButton } from "@/components/CopyButton"
import { Badge } from "@/components/ui/badge"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { Button } from "@/components/ui/button"
import { api, ApiError } from "@/lib/api"
import { gameTone, players, TRAIT } from "@/lib/games"
import type { AgentRow, Manifest, User } from "@/lib/types"
import { ArrowLeft, BookOpen, Bot, Clock, DoorOpen, Gamepad2, Timer, Trophy, Users } from "lucide-react"
import { lazy, Suspense, useEffect, useState, type CSSProperties } from "react"
import { Link, useParams } from "react-router-dom"

const LIMITS = [60, 120, 180, 300]

const fieldClass = "field w-full"

const MarkdownView = lazy(() => import("@/components/MarkdownView").then((m) => ({ default: m.MarkdownView })))

export function GameDetail() {
  const { id = "" } = useParams()
  const [game, setGame] = useState<Manifest | null>(null)
  const [user, setUser] = useState<User | null>(null)
  const [agents, setAgents] = useState<AgentRow[]>([])
  const [agentId, setAgentId] = useState("")
  const [variant, setVariant] = useState("")
  const [limitSec, setLimitSec] = useState(180)
  const [created, setCreated] = useState<{ match_id: string; room_code: string; vsAi?: boolean } | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState("")
  const [note, setNote] = useState("")
  const [ai, setAi] = useState<{ enabled: boolean; name: string; model: string; games: string[] } | null>(null)
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
    api<{ enabled: boolean; name: string; model: string; games: string[] }>("/api/ai")
      .then(setAi)
      .catch(() => setAi(null))
  }, [id])

  async function openRoom(vsAi = false) {
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
          ...(vsAi ? { opponent: "ai" } : {}),
        }),
      })
      setCreated({ ...body, vsAi })
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
      <main className="mx-auto max-w-3xl px-4 py-16">
        <div className="empty-state reveal">
          <Gamepad2 className="size-7" aria-hidden />
          <p className="text-lg font-semibold text-foreground">没有找到这款游戏</p>
          <p>{error}</p>
          <Button variant="outline" asChild className="mt-2">
            <Link to="/games">
              <ArrowLeft aria-hidden />
              返回游戏列表
            </Link>
          </Button>
        </div>
      </main>
    )
  }
  if (!game) {
    return (
      <main className="mx-auto max-w-6xl px-4 py-6 md:px-8 md:py-10" aria-busy="true">
        <div className="skeleton h-64" />
        <div className="mt-6 grid gap-6 lg:grid-cols-[1fr_22rem]">
          <div className="skeleton h-72" />
          <div className="skeleton h-56" />
        </div>
      </main>
    )
  }

  const seconds = Math.round(game.defaultTime.perActionMs / 1000)
  const canModerate = (user && game.authorId === user.id) || user?.role === "admin"
  return (
    <main className="mx-auto max-w-6xl px-4 py-6 md:px-8 md:py-10" style={{ "--tone": gameTone(game.id) } as CSSProperties}>
      <Link to="/games" className="-my-1 mb-4 inline-flex items-center gap-1 py-1 text-sm text-muted-foreground transition-colors hover:text-foreground">
        <ArrowLeft className="size-4" aria-hidden />
        全部游戏
      </Link>
      <header className="hero-lobby reveal grid overflow-hidden md:grid-cols-[minmax(0,1.1fr)_minmax(0,1fr)]">
        <div className="relative aspect-[16/9] md:aspect-auto md:min-h-72">
          <GameCover id={game.id} name={game.name} className="absolute inset-0" />
        </div>
        <div className="flex flex-col justify-center gap-4 p-5 sm:p-7">
          <div className="flex flex-wrap items-center gap-1.5">
            <Badge variant={game.playable ? "default" : "outline"}>{game.playable ? "可开局" : "尚未开放"}</Badge>
            {game.rated && (
              <Badge variant="secondary" className="text-gold">
                <Trophy aria-hidden />
                计入天梯
              </Badge>
            )}
            {game.uploaded && <Badge variant="outline">上传规则</Badge>}
            <Badge variant="secondary" className="font-data">{game.engine}</Badge>
          </div>
          <h1 className="text-3xl font-bold tracking-tight sm:text-4xl">{game.name}</h1>
          <p className="text-muted-foreground">{game.description}</p>
          <dl className="grid grid-cols-3 gap-3">
            <div className="stat">
              <dt className="stat-label">人数</dt>
              <dd className="stat-value text-lg">{players(game)}</dd>
            </div>
            <div className="stat">
              <dt className="stat-label">默认单步</dt>
              <dd className="stat-value text-lg">{seconds}s</dd>
            </div>
            <div className="stat">
              <dt className="stat-label">变体</dt>
              <dd className="stat-value text-lg">{game.variants.length}</dd>
            </div>
          </dl>
          {game.traits.length > 0 && (
            <div className="flex flex-wrap gap-1">
              {game.traits.map((trait) => (
                <Badge key={trait} variant="outline">
                  {TRAIT[trait] || trait}
                </Badge>
              ))}
            </div>
          )}
          {game.playable && (
            <div>
              <Button size="lg" asChild>
                <a href="#open-room">
                  <DoorOpen aria-hidden />
                  开一间休闲房
                </a>
              </Button>
            </div>
          )}
        </div>
      </header>

      {canModerate ? (
        <div className="panel mt-4 flex flex-wrap items-center gap-2 p-3">
          <span className="text-sm font-medium text-muted-foreground">管理</span>
          {user && game.authorId === user.id && game.playable && (
            <Button variant="outline" size="sm" onClick={() => moderate(`/api/games/${game.id}/unpublish`)}>
              下架
            </Button>
          )}
          {user?.role === "admin" && game.uploaded && game.playable && (
            <Button variant="destructive" size="sm" onClick={() => moderate(`/api/games/${game.id}/hide`)}>
              管理员隐藏
            </Button>
          )}
          {note && <span className="text-sm text-muted-foreground" role="status">{note}</span>}
        </div>
      ) : null}

      <div className="mt-6 grid items-start gap-6 lg:grid-cols-[minmax(0,1fr)_22rem]">
        <Tabs defaultValue="rules" className="reveal min-w-0" style={{ "--i": 2 } as CSSProperties}>
          <TabsList>
            <TabsTrigger value="rules">
              <BookOpen aria-hidden />
              规则
            </TabsTrigger>
            <TabsTrigger value="clock">
              <Clock aria-hidden />
              计时
            </TabsTrigger>
          </TabsList>
          <TabsContent value="rules" className="panel mt-3 p-5 sm:p-6">
            <Suspense fallback={<div className="skeleton h-40" />}>
              <MarkdownView source={game.rulesMarkdown} />
            </Suspense>
          </TabsContent>
          <TabsContent value="clock" className="panel mt-3 space-y-3 p-5 text-sm sm:p-6">
            <dl className="grid gap-3 sm:grid-cols-2">
              <div className="stat">
                <dt className="stat-label">默认每步</dt>
                <dd className="font-data text-base">{seconds} 秒，宽限 {game.defaultTime.graceMs / 1000} 秒</dd>
              </div>
              <div className="stat">
                <dt className="stat-label">连续超时上限</dt>
                <dd className="font-data text-base">{game.defaultTime.maxConsecutiveTimeouts}</dd>
              </div>
              <div className="stat">
                <dt className="stat-label">默认超时策略</dt>
                <dd>{game.timeoutPolicy}（允许：{game.allowedTimeoutPolicies.join("、")}）</dd>
              </div>
              <div className="stat">
                <dt className="stat-label">变体</dt>
                <dd>{game.variants.map((v) => v.name).join("、")}</dd>
              </div>
            </dl>
            <p className="text-muted-foreground">
              休闲房在开房时选 60、120、180 或 300 秒，不填是 180。排位固定 180 秒。计时环在剩余不到四分之一时变黄，不到十二分之一时变红。
            </p>
          </TabsContent>
        </Tabs>

        {game.playable && (
          <section id="open-room" className="panel reveal scroll-mt-24 lg:sticky lg:top-24" style={{ "--i": 3 } as CSSProperties} aria-labelledby="open-room-title">
            <div className="panel-head">
              <h2 id="open-room-title" className="flex items-center gap-2">
                <DoorOpen className="size-4 text-primary" aria-hidden />
                开一间休闲房
              </h2>
            </div>
            <div className="p-4">
              <p className="text-sm text-muted-foreground">
                单步时限是 60、120、180 或 300 秒，默认 180。排位固定 180 秒，不在这里选。智能体需要已经注册签名公钥。
              </p>
              {!user && (
                <Button asChild className="mt-4 w-full">
                  <Link to="/login">登录后选择智能体开房</Link>
                </Button>
              )}
              {user && agents.length === 0 && (
                <div className="mt-4 rounded-lg border border-dashed border-border p-3 text-sm">
                  还没有智能体。
                  <Link to="/agents" className="font-medium text-primary hover:underline">
                    去创建一个
                  </Link>
                  。
                </div>
              )}
              {user && agents.length > 0 && !created && (
                <form
                  className="mt-4 grid gap-3"
                  onSubmit={(event) => {
                    event.preventDefault()
                    void openRoom()
                  }}
                >
                  <label className="grid gap-1 text-xs font-medium text-muted-foreground">
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
                    <label className="grid gap-1 text-xs font-medium text-muted-foreground">
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
                  <fieldset className="grid gap-1">
                    <legend className="mb-1 flex items-center gap-1 text-xs font-medium text-muted-foreground">
                      <Timer className="size-3.5" aria-hidden />
                      单步
                    </legend>
                    <div className="grid grid-cols-4 gap-1.5">
                      {LIMITS.map((sec) => (
                        <button
                          key={sec}
                          type="button"
                          className="chip chip-solid justify-center font-data"
                          aria-pressed={limitSec === sec}
                          onClick={() => setLimitSec(sec)}
                        >
                          {sec}s
                        </button>
                      ))}
                    </div>
                  </fieldset>
                  <Button type="submit" size="lg" disabled={busy || !agentId}>
                    <Users aria-hidden />
                    {busy ? "开房中…" : "开房"}
                  </Button>
                  {ai?.enabled && ai.games.includes(game.id) && game.maxPlayers === 2 && (
                    <div className="grid gap-2">
                      <Button type="button" variant="secondary" size="lg" disabled={busy || !agentId} onClick={() => void openRoom(true)}>
                        <Bot aria-hidden />
                        {busy ? "开房中…" : `和 ${ai.name} 对战`}
                      </Button>
                      <p className="text-xs text-muted-foreground">
                        服务器内置对手（{ai.model}）自动入座，房间会立刻开始；把房间码交给你的智能体就能开下。
                      </p>
                    </div>
                  )}
                </form>
              )}
              {created && (
                <div className="pop-in mt-4 grid gap-3 rounded-xl border border-primary/40 bg-accent/40 p-4 text-sm">
                  <p className="text-muted-foreground">
                    {created.vsAi
                      ? "AI 已入座，对局已经开始；把房间码交给你的智能体："
                      : "房间已开好，把房间码发给对手："}
                  </p>
                  <div className="flex items-center justify-between gap-2">
                    <span className="font-data text-2xl tracking-[0.2em] text-foreground">{created.room_code}</span>
                    <CopyButton text={created.room_code} />
                  </div>
                  <Button asChild>
                    <Link to={`/m/${created.match_id}`}>打开这间房</Link>
                  </Button>
                </div>
              )}
              {note && (
                <p className="alert-error mt-3" role="alert">
                  {note}
                </p>
              )}
            </div>
          </section>
        )}
      </div>
    </main>
  )
}
