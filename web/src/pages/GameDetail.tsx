import { Badge } from "@/components/ui/badge"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { api } from "@/lib/api"
import type { Manifest } from "@/lib/types"
import { useEffect, useState } from "react"
import { Link, useParams } from "react-router-dom"

export function GameDetail() {
  const { id = "" } = useParams()
  const [game, setGame] = useState<Manifest | null>(null)
  const [error, setError] = useState("")
  useEffect(() => {
    api<Manifest>(`/api/games/${id}`)
      .then(setGame)
      .catch((err: Error) => setError(err.message))
  }, [id])

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
        <Badge variant="secondary">{game.engine}</Badge>
      </div>
      <p className="mt-3 text-muted-foreground">{game.description}</p>
      <Tabs defaultValue="rules" className="mt-6">
        <TabsList>
          <TabsTrigger value="rules">规则</TabsTrigger>
          <TabsTrigger value="clock">计时</TabsTrigger>
        </TabsList>
        <TabsContent value="rules" className="mt-4 whitespace-pre-wrap text-sm leading-6">
          {game.rulesMarkdown}
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
