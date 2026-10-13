import { Button } from "@/components/ui/button"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { api, ApiError } from "@/lib/api"
import Editor, { loader, type Monaco } from "@monaco-editor/react"
import * as monaco from "monaco-editor"
import editorWorker from "monaco-editor/editor/editor.worker.js?worker"
import { useEffect, useState } from "react"
import { Link, useSearchParams } from "react-router-dom"

self.MonacoEnvironment = {
  getWorker: () => new editorWorker(),
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
    if (sdk) {
      instance.languages.typescript.javascriptDefaults.addExtraLib(sdk, "file:///game.d.ts")
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

  return (
    <main className="mx-auto max-w-5xl px-4 py-8 md:px-8" data-testid="upload-wizard" data-step={step}>
      <h1 className="text-2xl font-medium tracking-tight">上传规则</h1>
      <p className="mt-2 max-w-2xl text-sm text-muted-foreground">
        用 JavaScript 写一局游戏。服务器在隔离的 QuickJS 里跑它：没有网络、没有时钟，随机数只来自对局种子。发布后的版本不能改，对局日志记下规则哈希。
      </p>
      <ol className="mt-6 flex gap-2 text-sm">
        {STEPS.map((label, index) => {
          const n = index + 1
          const active = step === n
          return (
            <li key={label} className={`rounded-full px-3 py-1 ${active ? "bg-primary text-primary-foreground" : "bg-muted text-muted-foreground"}`}>
              {n}. {label}
            </li>
          )
        })}
      </ol>

      {step === 1 && (
        <Card className="mt-6">
          <CardHeader>
            <CardTitle>这局游戏叫什么</CardTitle>
            <CardDescription>标识会变成游戏 id，形如 u_tictactoe。名称留空就用代码里的 manifest。</CardDescription>
          </CardHeader>
          <CardContent className="grid gap-4">
            <label className="grid gap-1 text-sm">
              标识
              <Input value={slug} onChange={(event) => setSlug(event.target.value.trim().toLowerCase())} placeholder="tictactoe" />
            </label>
            <label className="grid gap-1 text-sm">
              显示名称
              <Input value={name} onChange={(event) => setName(event.target.value)} placeholder="井字棋" />
            </label>
            {!slugOk && <p className="text-sm text-red-700">标识需为 3–32 位，以小写字母开头，只能含小写字母、数字和连字符。</p>}
            <div className="flex justify-end">
              <Button disabled={!slugOk} onClick={() => setStep(2)}>
                下一步
              </Button>
            </div>
          </CardContent>
        </Card>
      )}

      {step === 2 && (
        <div className="mt-6">
          <div className="overflow-hidden rounded-xl border border-border bg-white">
            <Editor
              height="560px"
              language="javascript"
              theme="arena-light"
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
        <Card className="mt-6">
          <CardHeader>
            <CardTitle>校验并发布</CardTitle>
            <CardDescription>同一粒种子会自对两遍。通过之前不能发布。每天最多校验 40 次、发布 10 次。上传的规则不进排位。</CardDescription>
          </CardHeader>
          <CardContent className="grid gap-4">
            <div className="flex gap-2">
              <Button variant="outline" disabled={busy || !source} onClick={runCheck}>
                {busy ? "正在跑" : "跑一遍校验"}
              </Button>
              <Button disabled={busy || !report?.ok || !slugOk || Boolean(published)} onClick={publish}>
                发布
              </Button>
            </div>
            {report && (
              <ul className="grid gap-2 text-sm">
                {report.checks.map((check) => (
                  <li key={check.id} className={check.ok ? "text-emerald-800" : "text-red-700"}>
                    {check.ok ? "通过" : "未过"} · {check.detail}
                  </li>
                ))}
              </ul>
            )}
            {error && (
              <p className="text-sm text-red-700">
                {error}
                {error.includes("登录") && (
                  <>
                    {" "}
                    <Link to="/login" className="underline">
                      去登录
                    </Link>
                  </>
                )}
              </p>
            )}
            {published && (
              <p className="text-sm">
                已发布 {published.name} {published.version}。规则哈希 {published.rules_hash.slice(0, 12)}…{" "}
                <Link to={`/games/${published.id}`} className="text-primary underline">
                  查看规则
                </Link>
              </p>
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
