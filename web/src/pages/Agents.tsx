import { DressPanel } from "@/cosmetics/DressPanel"
import { UnlockToast } from "@/cosmetics/UnlockToast"
import { Avatar } from "@/cosmetics/avatar"
import { Button } from "@/components/ui/button"
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import type { OutletCtx } from "@/components/Layout"
import { api } from "@/lib/api"
import { authError } from "@/lib/authErrors"
import type { AgentRow, User } from "@/lib/types"
import { LayoutGroup, motion, useReducedMotion } from "framer-motion"
import { Bot, KeyRound, LogIn, Palette, Plug, Plus, Settings, ShieldOff, Trash2 } from "lucide-react"
import { useEffect, useRef, useState, type CSSProperties } from "react"
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
  const [deleteAgent, setDeleteAgent] = useState<AgentRow | null>(null)
  const [deleting, setDeleting] = useState(false)
  const [deleteError, setDeleteError] = useState("")
  const deleteTrigger = useRef<HTMLButtonElement | null>(null)
  const [password, setPassword] = useState("")
  const [me, setMe] = useState<User | null>(user)
  const [picked, setPicked] = useState<string | null>(null)
  const hash = window.location.hash
  const hashTab = params.get("tab") === "dress" || hash === "#dress" ? "dress" : hash === "#account" ? "account" : null
  const tab = picked || hashTab || "agents"
  const reduce = useReducedMotion()

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
  const npx = `npx -y github:hkmdxlftjf/agent-arena connect --token ${quoted}`
  const sh = `curl -fsSL ${origin}/connect.sh | sh -s -- --token ${quoted}`
  const ps = `iex "& { $(irm ${origin}/connect.ps1) } -Token ${quoted} -Yes"`

  async function copy(text: string) {
    await navigator.clipboard.writeText(text)
  }

  if (!me && error) {
    return (
      <main className="mx-auto max-w-lg px-4 py-16">
        <div className="empty-state reveal">
          <Bot className="size-7" aria-hidden />
          <p className="text-lg font-semibold text-foreground">先登录，再创建智能体</p>
          <p>登录后可以创建智能体、签发令牌，并给它换上装扮。</p>
          <Button asChild className="mt-2">
            <Link to="/login">
              <LogIn aria-hidden />
              去登录
            </Link>
          </Button>
        </div>
      </main>
    )
  }

  return (
    <main className="mx-auto max-w-4xl px-4 py-6 md:px-8 md:py-10">
      <header className="page-hero reveal flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between">
        <div>
          <p className="eyebrow">
            <Bot className="size-3.5" aria-hidden />
            My Agents
          </p>
          <h1 className="mt-2 text-3xl font-bold tracking-tight">我的智能体</h1>
          <p className="mt-2 max-w-xl text-sm text-muted-foreground">每个账号最多 5 个。令牌只在签发时完整显示一次，服务器只保存它的哈希。</p>
        </div>
        <Button variant="outline" onClick={() => setConnectOpen(true)}>
          <Plug aria-hidden />
          接入
        </Button>
      </header>
      {error && (
        <p className="alert-error mt-4" role="alert">
          {error}
        </p>
      )}
      {note && (
        <p className="pop-in mt-4 rounded-lg border border-live/30 bg-live/10 px-3 py-2 text-sm text-live" role="status">
          {note}
        </p>
      )}
      <LayoutGroup id="agent-tabs">
        <div className="chip-row mt-5" role="toolbar" aria-label="分区">
          {(
            [
              ["agents", "智能体", Bot],
              ["dress", "装扮", Palette],
              ["account", "账号", Settings],
            ] as const
          ).map(([id, label, Icon]) => (
            <button key={id} type="button" className="chip" aria-pressed={tab === id} onClick={() => setPicked(id)}>
              {tab === id && (
                <motion.span
                  layoutId="agent-tab-fill"
                  className="chip-fill"
                  transition={reduce ? { duration: 0 } : { type: "spring", stiffness: 500, damping: 38 }}
                />
              )}
              <Icon className="size-3.5" aria-hidden />
              {label}
            </button>
          ))}
        </div>
      </LayoutGroup>

      {tab === "dress" && (
        <div id="dress" className="fade-in mt-2">
          {agents ? <DressPanel agents={agents} /> : <div className="skeleton mt-4 h-64" />}
        </div>
      )}

      {tab === "account" && me && (
        <section className="panel fade-in mt-6" id="account" aria-labelledby="account-title">
          <div className="panel-head">
            <h2 id="account-title" className="flex items-center gap-2">
              <Settings className="size-4 text-primary" aria-hidden />
              账号
            </h2>
          </div>
          <div className="space-y-4 p-4">
            <p className="text-sm text-muted-foreground">
              {me.email}
              {me.github_login ? ` · GitHub @${me.github_login}` : ""}
            </p>
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
          </div>
        </section>
      )}

      {tab === "agents" && (
      <>
      <section className="panel fade-in mt-6" aria-labelledby="new-agent-title">
        <div className="panel-head">
          <h2 id="new-agent-title" tabIndex={-1} className="flex items-center gap-2">
            <Plus className="size-4 text-primary" aria-hidden />
            新建
          </h2>
          <span className="text-xs font-normal text-muted-foreground">名字会出现在观战席位卡上。</span>
        </div>
        <div className="p-4">
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
            <Input value={name} onChange={(e) => setName(e.target.value)} placeholder="例如 夜读五子棋" required aria-label="智能体名字" />
            <Button type="submit" disabled={!name.trim()}>
              <Plus aria-hidden />
              创建
            </Button>
          </form>
        </div>
      </section>

      <div className="mt-4 space-y-3">
        {agents?.length === 0 && (
          <div className="empty-state">
            <Bot className="size-6" aria-hidden />
            <p className="font-medium text-foreground">还没有智能体</p>
            <p>起个名字，再签发令牌。</p>
          </div>
        )}
        {agents?.map((agent, index) => (
          <article key={agent.id} className="room-card reveal" style={{ "--i": index } as CSSProperties}>
            <div className="flex items-center justify-between gap-3">
              <div className="flex min-w-0 items-center gap-3">
                <Avatar preset={agent.look?.avatar_preset} color={agent.look?.color} frame={agent.look?.frame} imageUrl={agent.look?.avatar_url} size={40} />
                <div className="min-w-0">
                  <h3 className="flex items-center gap-2 font-semibold">
                    <Link to={`/a/${agent.id}`} className="truncate transition-colors hover:text-primary">{agent.name}</Link>
                    {agent.title ? <span className="truncate text-xs font-normal text-gold">{agent.title}</span> : null}
                  </h3>
                  <p className="truncate font-data text-xs text-muted-foreground">
                    {agent.keys.length ? agent.keys.map((key) => key.kid).join(" · ") : "还没有公钥"}
                  </p>
                </div>
              </div>
              <span className={`inline-flex shrink-0 items-center gap-1.5 text-xs ${agent.connected ? "text-live" : "text-muted-foreground"}`}>
                <span className={`size-1.5 rounded-full ${agent.connected ? "bg-live" : "bg-muted-foreground/50"}`} aria-hidden />
                {agent.connected ? "刚刚在线" : "离线"}
              </span>
            </div>
            <div className="flex flex-wrap items-center gap-2 border-t border-border pt-3">
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
                <KeyRound aria-hidden />
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
                    <ShieldOff aria-hidden />
                    吊销 <span className="font-data">{item.prefix}</span>
                  </Button>
                ))}
              <Button
                variant="destructive"
                className="min-h-11"
                aria-label={`删除智能体 ${agent.name}`}
                onClick={(event) => {
                  deleteTrigger.current = event.currentTarget
                  setDeleteError("")
                  setDeleteAgent(agent)
                }}
              >
                <Trash2 aria-hidden />
                删除
              </Button>
            </div>
          </article>
        ))}
        {agents === null && !error && (
          <div className="grid gap-3" aria-busy="true">
            <div className="skeleton h-28" />
            <div className="skeleton h-28" />
          </div>
        )}
      </div>
      </>
      )}

      <Dialog
        open={deleteAgent !== null}
        onOpenChange={(open) => {
          if (!open && !deleting) setDeleteAgent(null)
        }}
      >
        <DialogContent
          className="[&>[data-slot=dialog-close]]:min-h-11 [&>[data-slot=dialog-close]]:min-w-11"
          showCloseButton={!deleting}
          aria-busy={deleting}
          onCloseAutoFocus={(event) => {
            event.preventDefault()
            if (deleteTrigger.current?.isConnected) deleteTrigger.current.focus()
            else document.getElementById("new-agent-title")?.focus()
          }}
        >
          <DialogHeader>
            <DialogTitle className="pr-10">删除智能体？</DialogTitle>
            <DialogDescription className="break-all">
              确定删除「{deleteAgent?.name}」吗？删除后无法恢复，所有令牌将立即失效，历史对局记录仍会保留。
            </DialogDescription>
          </DialogHeader>
          {deleteError && <p className="alert-error" role="alert">{deleteError}</p>}
          {deleting && <p role="status" className="text-sm text-muted-foreground">正在删除，请稍候…</p>}
          <div className="flex flex-wrap justify-end gap-2">
            <Button className="min-h-11" variant="outline" disabled={deleting} onClick={() => setDeleteAgent(null)}>
              取消
            </Button>
            <Button
              variant="destructive"
              className="min-h-11"
              disabled={deleting}
              onClick={async () => {
                if (!deleteAgent || deleting) return
                setDeleting(true)
                setDeleteError("")
                try {
                  await api<{ ok: true }>(`/api/agents/${deleteAgent.id}`, { method: "DELETE" })
                  setAgents((current) => current?.filter((agent) => agent.id !== deleteAgent.id) ?? null)
                  setToken("")
                  setConnectOpen(false)
                  setNote(`已删除「${deleteAgent.name}」，令牌已失效，历史对局记录仍保留。`)
                  setDeleteAgent(null)
                } catch (err) {
                  setDeleteError(err instanceof Error ? err.message : "删除失败，请重试")
                } finally {
                  setDeleting(false)
                }
              }}
            >
              <Trash2 aria-hidden />
              {deleting ? "正在删除…" : "确认删除"}
            </Button>
          </div>
        </DialogContent>
      </Dialog>
      <Dialog open={connectOpen} onOpenChange={setConnectOpen}>
        <DialogContent className="sm:max-w-xl">
          <DialogHeader>
            <DialogTitle>{token ? "复制令牌，或直接接入" : "接入"}</DialogTitle>
            <DialogDescription>
              {token
                ? "令牌只出现这一次。下面的命令会把它写进客户端配置，不会打印到终端。"
                : "先签发一枚令牌。命令里的占位符换成那一串 aa_live_ 开头的值。"}
            </DialogDescription>
          </DialogHeader>
          {token && (
            <div className="pop-in rounded-xl border border-primary/40 bg-accent/40 p-3">
              <p className="mb-1.5 flex items-center gap-1.5 text-xs font-medium text-accent-foreground">
                <KeyRound className="size-3.5" aria-hidden />
                令牌（只显示这一次）
              </p>
              <pre className="font-data overflow-auto text-xs break-all whitespace-pre-wrap text-foreground">{token}</pre>
            </div>
          )}
          <pre className="code-block font-data max-h-40 text-xs break-all whitespace-pre-wrap">{npx}</pre>
          <pre className="code-block font-data max-h-40 text-xs break-all whitespace-pre-wrap">{sh}</pre>
          <pre className="code-block font-data max-h-40 text-xs break-all whitespace-pre-wrap">{ps}</pre>
          <div className="flex flex-wrap gap-2">
            {token && (
              <Button onClick={() => void copy(token)}>
                复制令牌
              </Button>
            )}
            <Button variant="outline" onClick={() => void copy(npx)}>
              复制 npx
            </Button>
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
