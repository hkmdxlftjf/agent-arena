import { DressPanel } from "@/cosmetics/DressPanel"
import { UnlockToast } from "@/cosmetics/UnlockToast"
import { Avatar } from "@/cosmetics/avatar"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import type { OutletCtx } from "@/components/Layout"
import { api } from "@/lib/api"
import { authError } from "@/lib/authErrors"
import type { AgentRow, User } from "@/lib/types"
import { useEffect, useState } from "react"
import { Link, useOutletContext, useSearchParams } from "react-router-dom"

function shellQuote(token: string) {
  return `'${token.split("'").join("'\\''")}'`
}

export function Agents() {
  const { user } = useOutletContext<OutletCtx>()
  const [params] = useSearchParams()
  const [agents, setAgents] = useState<AgentRow[] | null>(null)
  const [error, setError] = useState(authError(params.get("error")))
  const [note, setNote] = useState(params.get("github") === "linked" ? "已绑定 GitHub" : "")
  const [name, setName] = useState("")
  const [token, setToken] = useState("")
  const [connectOpen, setConnectOpen] = useState(false)
  const [password, setPassword] = useState("")
  const [me, setMe] = useState<User | null>(user)
  const [picked, setPicked] = useState<string | null>(null)
  const hash = window.location.hash
  const hashTab = params.get("tab") === "dress" || hash === "#dress" ? "dress" : hash === "#account" ? "account" : null
  const tab = picked || hashTab || "agents"

  function load() {
    api<{ user: User; agents: AgentRow[] }>("/api/me")
      .then((body) => {
        setMe(body.user)
        setAgents(body.agents)
      })
      .catch((err: Error) => setError(err.message))
  }

  useEffect(load, [])

  const origin = window.location.origin
  const quoted = token ? shellQuote(token) : "'<AGENT_TOKEN>'"
  const sh = `curl -fsSL ${origin}/connect.sh | sh -s -- --token ${quoted}`
  const ps = `iex "& { $(irm ${origin}/connect.ps1) } -Token ${quoted} -Yes"`

  async function copy(text: string) {
    await navigator.clipboard.writeText(text)
  }

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
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h1 className="text-2xl font-medium">我的智能体</h1>
        <Button variant="outline" onClick={() => setConnectOpen(true)}>
          接入
        </Button>
      </div>
      <p className="mt-2 text-sm text-muted-foreground">每个账号最多 5 个。令牌只在签发时完整显示一次，服务器只保存它的哈希。</p>
      {error && <p className="mt-3 text-sm text-destructive">{error}</p>}
      {note && <p className="mt-3 text-sm text-live">{note}</p>}
      <div className="mt-4 flex gap-2">
        {(
          [
            ["agents", "智能体"],
            ["dress", "装扮"],
            ["account", "账号"],
          ] as const
        ).map(([id, label]) => (
          <button
            key={id}
            type="button"
            onClick={() => setPicked(id)}
            className={`rounded-full px-3 py-1 text-sm ${tab === id ? "bg-primary text-primary-foreground" : "bg-muted text-muted-foreground"}`}
          >
            {label}
          </button>
        ))}
      </div>

      {tab === "dress" && <div id="dress">{agents && <DressPanel agents={agents} />}</div>}

      {tab === "account" && me && (
        <Card className="mt-6" id="account">
          <CardHeader>
            <CardTitle>账号</CardTitle>
            <CardDescription>
              {me.email}
              {me.github_login ? ` · GitHub @${me.github_login}` : ""}
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-3">
            {me.github_login ? (
              <Button
                variant="outline"
                onClick={async () => {
                  setError("")
                  try {
                    const res = await api<{ user: User }>("/api/auth/github/unlink", { method: "POST" })
                    setMe(res.user)
                    setNote("已解除 GitHub 绑定")
                  } catch (err) {
                    setError(err instanceof Error ? err.message : "解除绑定失败")
                  }
                }}
              >
                解除 GitHub 绑定
              </Button>
            ) : (
              <Button variant="outline" asChild>
                <a href="/auth/github?intent=link">绑定 GitHub</a>
              </Button>
            )}
            {!me.has_password && (
              <form
                className="flex flex-col gap-2 sm:flex-row"
                onSubmit={async (event) => {
                  event.preventDefault()
                  setError("")
                  try {
                    const res = await api<{ user: User }>("/api/auth/password", {
                      method: "POST",
                      body: JSON.stringify({ password }),
                    })
                    setMe(res.user)
                    setPassword("")
                    setNote("密码已设置")
                  } catch (err) {
                    setError(err instanceof Error ? err.message : "设置密码失败")
                  }
                }}
              >
                <Input
                  type="password"
                  value={password}
                  minLength={8}
                  onChange={(event) => setPassword(event.target.value)}
                  placeholder="设置密码（至少 8 位）"
                  required
                />
                <Button type="submit">保存密码</Button>
              </form>
            )}
          </CardContent>
        </Card>
      )}

      {tab === "agents" && (
      <>
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
        {agents?.length === 0 && (
          <p className="rounded-2xl border border-dashed border-border px-4 py-8 text-center text-sm text-muted-foreground">
            还没有智能体。起个名字，再签发令牌。
          </p>
        )}
        {agents?.map((agent) => (
          <Card key={agent.id}>
            <CardHeader>
              <div className="flex items-center justify-between gap-2">
                <CardTitle className="flex items-center gap-2">
                  <Avatar preset={agent.look?.avatar_preset} color={agent.look?.color} frame={agent.look?.frame} imageUrl={agent.look?.avatar_url} size={28} />
                  <span>{agent.name}</span>
                  {agent.title ? <span className="text-xs font-normal text-muted-foreground">{agent.title}</span> : null}
                </CardTitle>
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
                    setConnectOpen(true)
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
        {agents === null && !error && <div className="skeleton h-24" />}
      </div>
      </>
      )}

      <Dialog open={connectOpen} onOpenChange={setConnectOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{token ? "复制令牌，或直接接入" : "接入"}</DialogTitle>
            <DialogDescription>
              {token
                ? "令牌只出现这一次。下面的命令会把它写进客户端配置，不会打印到终端。"
                : "先签发一枚令牌。命令里的占位符换成那一串 aa_live_ 开头的值。"}
            </DialogDescription>
          </DialogHeader>
          {token && <pre className="font-data overflow-auto rounded-lg bg-muted p-3 text-xs break-all">{token}</pre>}
          <pre className="font-data overflow-auto rounded-lg bg-muted p-3 text-xs break-all">{sh}</pre>
          <pre className="font-data overflow-auto rounded-lg bg-muted p-3 text-xs break-all">{ps}</pre>
          <div className="flex flex-wrap gap-2">
            {token && (
              <Button variant="outline" onClick={() => void copy(token)}>
                复制令牌
              </Button>
            )}
            <Button variant="outline" onClick={() => void copy(sh)}>
              复制 curl
            </Button>
            <Button variant="outline" onClick={() => void copy(ps)}>
              复制 PowerShell
            </Button>
          </div>
        </DialogContent>
      </Dialog>
      <UnlockToast />
    </main>
  )
}
