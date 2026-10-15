import { Button } from "@/components/ui/button"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { api, ApiError } from "@/lib/api"
import Editor, { loader, type Monaco } from "@monaco-editor/react"
import * as monaco from "monaco-editor"
import editorWorker from "monaco-editor/editor/editor.worker.js?worker"
import tsWorker from "monaco-editor/language/typescript/ts.worker.js?worker"
import { Check as CheckIcon, CircleCheck, CircleX, FileCode2, Loader2, Rocket } from "lucide-react"
import { useEffect, useState, type CSSProperties } from "react"
import { Link, useSearchParams } from "react-router-dom"

self.MonacoEnvironment = {
  getWorker: (_id: string, label: string) => (label === "typescript" || label === "javascript" ? new tsWorker() : new editorWorker()),
}
loader.config({ monaco })

type Check = { id: string; ok: boolean; detail: string }
type Report = { ok: boolean; checks: Check[]; manifest?: { name?: string; id?: string } }
type Published = { id: string; version: string; rules_hash: string; name: string }

const STEPS = ["元数据", "规则", "预览发布"]

function stepFromQuery(value: string | null) {
  const n = Number(value)
  return n === 2 || n === 3 ? n : 1
}

export function Upload() {
  const [params] = useSearchParams()
  const [step, setStep] = useState(stepFromQuery(params.get("step")))
  const [slug, setSlug] = useState("tictactoe")
  const [name, setName] = useState("")
  const [source, setSource] = useState("")
  const [sdk, setSdk] = useState("")
  const [report, setReport] = useState<Report | null>(null)
  const [published, setPublished] = useState<Published | null>(null)
  const [error, setError] = useState("")
  const [busy, setBusy] = useState(false)

  useEffect(() => {
    fetch("/examples/tic-tac-toe.js")
      .then((res) => res.text())
      .then(setSource)
      .catch(() => setError("示例规则没有加载出来"))
    fetch("/sdk/game.ts")
      .then((res) => res.text())
      .then(setSdk)
      .catch(() => setSdk(""))
  }, [])

  function beforeMount(instance: Monaco) {
    instance.editor.defineTheme("arena-light", {
      base: "vs",
      inherit: true,
      rules: [],
      colors: { "editor.background": "#FFFFFF" },
    })
    instance.editor.defineTheme("arena-dark", {
      base: "vs-dark",
      inherit: true,
      rules: [],
      colors: { "editor.background": "#0b1019", "editor.lineHighlightBackground": "#141a27", "editorGutter.background": "#0b1019" },
    })
    if (sdk) {
      monaco.typescript.javascriptDefaults.addExtraLib(sdk, "file:///game.d.ts")
    }
  }

  async function runCheck() {
    setBusy(true)
    setError("")
    setReport(null)
    try {
      const next = await api<Report>("/api/uploads/check", {
        method: "POST",
        body: JSON.stringify({ source }),
      })
      setReport(next)
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "校验失败")
    } finally {
      setBusy(false)
    }
  }

  async function publish() {
    setBusy(true)
    setError("")
    try {
      const next = await api<Published>("/api/uploads/publish", {
        method: "POST",
        body: JSON.stringify({ source, slug, name: name.trim() || undefined }),
      })
      setPublished(next)
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "发布失败")
    } finally {
      setBusy(false)
    }
  }

  const slugOk = /^[a-z][a-z0-9-]{2,31}$/.test(slug)
  const dark = document.documentElement.classList.contains("dark")

  return (
    <main className="mx-auto max-w-5xl px-4 py-6 md:px-8 md:py-10" data-testid="upload-wizard" data-step={step}>
      <header className="page-hero reveal">
        <p className="eyebrow">
          <FileCode2 className="size-3.5" aria-hidden />
          Upload Rules
        </p>
        <h1 className="mt-2 text-3xl font-bold tracking-tight">上传规则</h1>
        <p className="mt-2 max-w-2xl text-sm text-muted-foreground">
          用 JavaScript 写一局游戏。服务器在隔离的 QuickJS 里跑它：没有网络、没有时钟，随机数只来自对局种子。发布后的版本不能改，对局日志记下规则哈希。
        </p>
      </header>
      <ol className="mt-6 flex items-center gap-2 text-sm" aria-label="步骤">
        {STEPS.map((label, index) => {
          const n = index + 1
          const active = step === n
          const done = step > n
          return (
            <li key={label} className="flex min-w-0 flex-1 items-center gap-2" aria-current={active ? "step" : undefined}>
              <span
                className={`grid size-7 shrink-0 place-items-center rounded-full font-data text-xs font-semibold transition-colors duration-300 ${
                  active ? "bg-primary text-primary-foreground shadow-[0_0_0_4px_color-mix(in_srgb,var(--primary)_18%,transparent)]" : done ? "bg-accent text-accent-foreground" : "bg-muted text-muted-foreground"
                }`}
              >
                {done ? <CheckIcon className="size-3.5" aria-hidden /> : n}
              </span>
              <span className={`truncate ${active ? "font-semibold text-foreground" : "text-muted-foreground"}`}>{label}</span>
              {n < STEPS.length && (
                <span className="relative h-px min-w-4 flex-1 overflow-hidden bg-border" aria-hidden>
                  <span className={`absolute inset-0 origin-left bg-primary transition-transform duration-500 ${done ? "scale-x-100" : "scale-x-0"}`} />
                </span>
              )}
            </li>
          )
        })}
      </ol>

      {step === 1 && (
        <Card className="fade-in mt-6">
          <CardHeader>
            <CardTitle>这局游戏叫什么</CardTitle>
            <CardDescription>标识会变成游戏 id，形如 u_tictactoe。名称留空就用代码里的 manifest。</CardDescription>
          </CardHeader>
          <CardContent className="grid gap-4">
            <label className="grid gap-1 text-xs font-medium text-muted-foreground">
              标识
              <Input className="h-10 font-data" aria-invalid={!slugOk} value={slug} onChange={(event) => setSlug(event.target.value.trim().toLowerCase())} placeholder="tictactoe" />
            </label>
            <label className="grid gap-1 text-xs font-medium text-muted-foreground">
              显示名称
              <Input className="h-10" value={name} onChange={(event) => setName(event.target.value)} placeholder="井字棋" />
            </label>
            {!slugOk && (
              <p className="alert-error" role="alert">
                标识需为 3–32 位，以小写字母开头，只能含小写字母、数字和连字符。
              </p>
            )}
            <div className="flex justify-end">
              <Button disabled={!slugOk} onClick={() => setStep(2)}>
                下一步
              </Button>
            </div>
          </CardContent>
        </Card>
      )}

      {step === 2 && (
        <div className="fade-in mt-6">
          <div className="panel overflow-hidden">
            <div className="panel-head">
              <span className="flex items-center gap-2 font-data text-xs">
                <FileCode2 className="size-4 text-primary" aria-hidden />
                rules.js
              </span>
              <span className="text-xs font-normal text-muted-foreground">{source.split("\n").length} 行</span>
            </div>
            <Editor
              height="560px"
              language="javascript"
              theme={dark ? "arena-dark" : "arena-light"}
              loading={<div className="skeleton m-4 h-[528px]" />}
              value={source}
              beforeMount={beforeMount}
              onChange={(value) => setSource(value ?? "")}
              options={{ minimap: { enabled: false }, fontSize: 13, scrollBeyondLastLine: false }}
            />
          </div>
          <div className="mt-4 flex justify-between">
            <Button variant="outline" onClick={() => setStep(1)}>
              上一步
            </Button>
            <Button
              disabled={!source}
              onClick={() => {
                setStep(3)
                setReport(null)
                setPublished(null)
              }}
            >
              下一步
            </Button>
          </div>
        </div>
      )}

      {step === 3 && (
        <Card className="fade-in mt-6">
          <CardHeader>
            <CardTitle>校验并发布</CardTitle>
            <CardDescription>同一粒种子会自对两遍。通过之前不能发布。每天最多校验 40 次、发布 10 次。上传的规则不进排位。</CardDescription>
          </CardHeader>
          <CardContent className="grid gap-4">
            <div className="flex flex-wrap gap-2">
              <Button variant="outline" disabled={busy || !source} onClick={runCheck}>
                {busy && <Loader2 className="animate-spin" aria-hidden />}
                {busy ? "正在跑" : "跑一遍校验"}
              </Button>
              <Button disabled={busy || !report?.ok || !slugOk || Boolean(published)} onClick={publish}>
                <Rocket aria-hidden />
                发布
              </Button>
            </div>
            {report && (
              <ul className="grid gap-1.5 text-sm" aria-live="polite">
                {report.checks.map((check, index) => (
                  <li
                    key={check.id}
                    className={`reveal flex items-start gap-2 rounded-lg border px-3 py-2 ${check.ok ? "border-live/25 bg-live/5" : "border-destructive/30 bg-destructive/5"}`}
                    style={{ "--i": index } as CSSProperties}
                  >
                    {check.ok ? <CircleCheck className="mt-0.5 size-4 shrink-0 text-live" aria-hidden /> : <CircleX className="mt-0.5 size-4 shrink-0 text-destructive" aria-hidden />}
                    <span>
                      <span className={`font-medium ${check.ok ? "text-live" : "text-destructive"}`}>{check.ok ? "通过" : "未过"}</span>
                      <span className="text-muted-foreground"> · {check.detail}</span>
                    </span>
                  </li>
                ))}
              </ul>
            )}
            {error && (
              <p className="alert-error" role="alert">
                {error}
                {error.includes("登录") && (
                  <>
                    {" "}
                    <Link to="/login" className="font-medium underline">
                      去登录
                    </Link>
                  </>
                )}
              </p>
            )}
            {published && (
              <div className="pop-in flex flex-wrap items-center justify-between gap-3 rounded-xl border border-primary/40 bg-accent/40 p-4 text-sm" role="status">
                <p>
                  已发布 <span className="font-semibold">{published.name}</span> {published.version}。规则哈希{" "}
                  <span className="font-data">{published.rules_hash.slice(0, 12)}…</span>
                </p>
                <Button asChild size="sm">
                  <Link to={`/games/${published.id}`}>查看规则</Link>
                </Button>
              </div>
            )}
            <div>
              <Button variant="outline" onClick={() => setStep(2)}>
                返回编辑
              </Button>
            </div>
          </CardContent>
        </Card>
      )}
    </main>
  )
}
