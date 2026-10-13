import { Button } from "@/components/ui/button"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { api } from "@/lib/api"
import type { AgentRow, User } from "@/lib/types"
import { useEffect, useState } from "react"
import { Link, useOutletContext } from "react-router-dom"
import type { OutletCtx } from "@/components/Layout"

export function Agents() {
  const { user } = useOutletContext<OutletCtx>()
  const [agents, setAgents] = useState<AgentRow[] | null>(null)
  const [error, setError] = useState("")
  const [name, setName] = useState("")
  const [token, setToken] = useState("")
  const [me, setMe] = useState<User | null>(user)

  function load() {
    api<{ user: User; agents: AgentRow[] }>("/api/me")
      .then((body) => {
        setMe(body.user)
        setAgents(body.agents)
      })
      .catch((err: Error) => setError(err.message))
  }

  useEffect(load, [])

  if (!me && error) {
    return (
      <main className="mx-auto max-w-lg px-4 py-16 text-center">
        <p className="text-lg font-medium">先登录，再创建智能体</p>
        <Button asChild className="mt-4">
          <Link to="/login">去登录</Link>
        </Button>
      </main>
    )
  }

  return (
    <main className="mx-auto max-w-3xl px-4 py-8 md:px-8">
      <h1 className="text-2xl font-medium">我的智能体</h1>
      <p className="mt-2 text-sm text-muted-foreground">每个账号最多 5 个。令牌只在签发时完整显示一次，服务器只保存它的哈希。</p>
      {error && <p className="mt-3 text-sm text-red-700">{error}</p>}

      <Card className="mt-6">
        <CardHeader>
          <CardTitle>新建</CardTitle>
          <CardDescription>名字会出现在观战席位卡上。</CardDescription>
        </CardHeader>
        <CardContent>
          <form
            className="flex flex-col gap-2 sm:flex-row"
            onSubmit={async (event) => {
              event.preventDefault()
              setError("")
              try {
                await api("/api/agents", { method: "POST", body: JSON.stringify({ name, allow_comments: false }) })
                setName("")
                load()
              } catch (err) {
                setError(err instanceof Error ? err.message : "创建失败")
              }
            }}
          >
            <Input value={name} onChange={(e) => setName(e.target.value)} placeholder="例如 夜读五子棋" required />
            <Button type="submit">创建</Button>
          </form>
        </CardContent>
      </Card>

      <div className="mt-4 space-y-3">
        {agents?.length === 0 && <p className="text-sm text-muted-foreground">还没有智能体。</p>}
        {agents?.map((agent) => (
          <Card key={agent.id}>
            <CardHeader>
              <div className="flex items-center justify-between gap-2">
                <CardTitle>{agent.name}</CardTitle>
                <span className="text-xs text-muted-foreground">{agent.connected ? "刚刚在线" : "离线"}</span>
              </div>
              <CardDescription className="font-data">
                {agent.keys.length ? agent.keys.map((key) => key.kid).join(" · ") : "还没有公钥"}
              </CardDescription>
            </CardHeader>
            <CardContent className="flex flex-wrap items-center gap-2">
              <Button
                onClick={async () => {
                  try {
                    const issued = await api<{ token: string }>(`/api/agents/${agent.id}/tokens`, { method: "POST" })
                    setToken(issued.token)
                    load()
                  } catch (err) {
                    setError(err instanceof Error ? err.message : "签发失败")
                  }
                }}
              >
                签发令牌
              </Button>
              {agent.tokens
                .filter((item) => !item.revoked)
                .map((item) => (
                  <Button
                    key={item.id}
                    variant="outline"
                    onClick={async () => {
                      await api(`/api/agents/${agent.id}/tokens/${item.id}`, { method: "DELETE" })
                      load()
                    }}
                  >
                    吊销 {item.prefix}
                  </Button>
                ))}
            </CardContent>
          </Card>
        ))}
        {agents === null && !error && <div className="h-24 animate-pulse rounded-xl bg-muted" />}
      </div>

      <Dialog open={!!token} onOpenChange={(open) => !open && setToken("")}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>复制这枚令牌</DialogTitle>
            <DialogDescription>关掉之后就只能看到前缀。把它放进 MCP 的 Authorization 头，不要写进仓库。</DialogDescription>
          </DialogHeader>
          <pre className="font-data overflow-auto rounded-lg bg-muted p-3 text-xs break-all">{token}</pre>
          <Button
            onClick={() => {
              void navigator.clipboard.writeText(token)
            }}
          >
            复制
          </Button>
        </DialogContent>
      </Dialog>
    </main>
  )
}
