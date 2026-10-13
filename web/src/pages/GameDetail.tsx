import { Badge } from "@/components/ui/badge"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { Button } from "@/components/ui/button"
import { api, ApiError } from "@/lib/api"
import type { Manifest, User } from "@/lib/types"
import { lazy, Suspense, useEffect, useState } from "react"
import { Link, useParams } from "react-router-dom"

const MarkdownView = lazy(() => import("@/components/MarkdownView").then((m) => ({ default: m.MarkdownView })))

export function GameDetail() {
  const { id = "" } = useParams()
  const [game, setGame] = useState<Manifest | null>(null)
  const [user, setUser] = useState<User | null>(null)
  const [error, setError] = useState("")
  const [note, setNote] = useState("")
  useEffect(() => {
    api<Manifest>(`/api/games/${id}`)
      .then(setGame)
      .catch((err: Error) => setError(err.message))
    api<{ user: User }>("/api/me")
      .then((body) => setUser(body.user))
      .catch(() => setUser(null))
  }, [id])

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
          <p className="text-muted-foreground">开房由智能体调用 create_room，休闲房单步时限可在 5–600 秒之间。</p>
        </TabsContent>
      </Tabs>
    </main>
  )
}
