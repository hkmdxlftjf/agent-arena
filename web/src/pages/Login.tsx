import { Particles } from "@/components/Ambient"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { api } from "@/lib/api"
import { authError } from "@/lib/authErrors"
import type { User } from "@/lib/types"
import { Loader2, LogIn } from "lucide-react"
import type { FormEvent } from "react"
import { useState } from "react"
import { useNavigate, useOutletContext, useSearchParams } from "react-router-dom"
import type { OutletCtx } from "@/components/Layout"

export function Login() {
  const navigate = useNavigate()
  const [params] = useSearchParams()
  const { setUser } = useOutletContext<OutletCtx>()
  const [error, setError] = useState(authError(params.get("error")))
  const [pending, setPending] = useState(false)

  async function submit(path: string, body: Record<string, string>) {
    setPending(true)
    setError("")
    try {
      const res = await api<{ user: User }>(path, { method: "POST", body: JSON.stringify(body) })
      setUser(res.user)
      navigate("/agents")
    } catch (err) {
      setError(err instanceof Error ? err.message : "登录失败")
    } finally {
      setPending(false)
    }
  }

  return (
    <main className="relative isolate mx-auto grid max-w-md px-4 py-10 md:py-16">
      <div className="pointer-events-none absolute inset-x-0 -top-6 -z-10 h-[30rem] overflow-hidden [mask-image:radial-gradient(closest-side,black,transparent)]" aria-hidden>
        <Particles />
      </div>
      <Card className="stage pop-in border border-border">
        <CardHeader className="text-center">
          <span className="mx-auto mb-2 grid size-12 place-items-center rounded-2xl bg-accent text-accent-foreground">
            <LogIn className="size-5" aria-hidden />
          </span>
          <CardTitle className="text-xl font-bold">进入游戏大厅</CardTitle>
          <CardDescription>邮箱和密码就够了。第一个注册的账号会成为管理员。</CardDescription>
        </CardHeader>
        <CardContent>
          <a
            href="/auth/github"
            className="mb-4 flex h-10 w-full items-center justify-center gap-2 rounded-lg border border-border bg-secondary text-sm font-medium text-foreground transition-[background-color,transform] hover:bg-muted active:scale-[0.98]"
          >
            <svg viewBox="0 0 16 16" className="size-4" aria-hidden="true">
              <path
                fill="currentColor"
                d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0 0 16 8c0-4.42-3.58-8-8-8z"
              />
            </svg>
            使用 GitHub 登录
          </a>
          <div className="mb-4 flex items-center gap-3 text-xs text-muted-foreground" aria-hidden>
            <span className="h-px flex-1 bg-border" />
            或用邮箱
            <span className="h-px flex-1 bg-border" />
          </div>
          <Tabs defaultValue="login">
            <TabsList className="grid w-full grid-cols-2">
              <TabsTrigger value="login">登录</TabsTrigger>
              <TabsTrigger value="register">注册</TabsTrigger>
            </TabsList>
            <TabsContent value="login">
              <Form
                pending={pending}
                onSubmit={(body) => submit("/api/auth/login", body)}
                fields={[
                  ["email", "邮箱", "email"],
                  ["password", "密码", "password"],
                ]}
              />
            </TabsContent>
            <TabsContent value="register">
              <Form
                pending={pending}
                onSubmit={(body) => submit("/api/auth/register", body)}
                fields={[
                  ["handle", "用户名", "text"],
                  ["email", "邮箱", "email"],
                  ["password", "密码（至少 8 位）", "password"],
                ]}
              />
            </TabsContent>
          </Tabs>
          {error && (
            <p className="alert-error pop-in mt-3" role="alert">
              {error}
            </p>
          )}
        </CardContent>
      </Card>
    </main>
  )
}

function Form({
  fields,
  pending,
  onSubmit,
}: {
  fields: [string, string, string][]
  pending: boolean
  onSubmit: (body: Record<string, string>) => void
}) {
  function handle(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const data = new FormData(event.currentTarget)
    const body: Record<string, string> = {}
    for (const [key] of fields) body[key] = String(data.get(key) || "")
    onSubmit(body)
  }
  return (
    <form className="mt-4 space-y-3" onSubmit={handle}>
      {fields.map(([key, label, type]) => (
        <label key={key} className="block text-sm">
          <span className="mb-1 block text-xs font-medium text-muted-foreground">{label}</span>
          <Input name={key} type={type} required autoComplete={key === "password" ? "current-password" : "username"} className="h-10" />
        </label>
      ))}
      <Button type="submit" size="lg" disabled={pending} className="w-full">
        {pending && <Loader2 className="animate-spin" aria-hidden />}
        {pending ? "提交中…" : "继续"}
      </Button>
    </form>
  )
}
