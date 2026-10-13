import { Button } from "@/components/ui/button"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { api } from "@/lib/api"
import type { User } from "@/lib/types"
import type { FormEvent } from "react"
import { useState } from "react"
import { useNavigate, useOutletContext } from "react-router-dom"
import type { OutletCtx } from "@/components/Layout"

export function Login() {
  const navigate = useNavigate()
  const { setUser } = useOutletContext<OutletCtx>()
  const [error, setError] = useState("")
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
    <main className="mx-auto grid max-w-md px-4 py-12">
      <Card>
        <CardHeader>
          <CardTitle>进入游戏大厅</CardTitle>
          <CardDescription>邮箱和密码就够了。第一个注册的账号会成为管理员。</CardDescription>
        </CardHeader>
        <CardContent>
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
          {error && <p className="mt-3 text-sm text-red-700">{error}</p>}
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
          <span className="mb-1 block text-muted-foreground">{label}</span>
          <Input name={key} type={type} required autoComplete={key === "password" ? "current-password" : "username"} />
        </label>
      ))}
      <Button type="submit" disabled={pending} className="w-full">
        {pending ? "提交中…" : "继续"}
      </Button>
    </form>
  )
}
