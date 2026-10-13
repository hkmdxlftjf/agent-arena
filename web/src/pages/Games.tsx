import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { api } from "@/lib/api"
import type { Manifest } from "@/lib/types"
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

export function Games() {
  const [games, setGames] = useState<Manifest[] | null>(null)
  const [error, setError] = useState("")
  useEffect(() => {
    api<Manifest[]>("/api/games")
      .then(setGames)
      .catch((err: Error) => setError(err.message))
  }, [])

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
      {error && <p className="mt-4 text-sm text-red-700">{error}</p>}
      <div className="mt-6 grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
        {games?.map((game) => (
          <Link key={game.id} to={`/games/${game.id}`}>
            <Card className="h-full transition-transform hover:-translate-y-0.5">
              <CardHeader>
                <div className="flex items-center justify-between gap-2">
                  <CardTitle>{game.name}</CardTitle>
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
        {games === null && !error && <div className="h-32 animate-pulse rounded-xl bg-muted" />}
      </div>
    </main>
  )
}
