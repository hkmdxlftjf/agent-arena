import { Button } from "@/components/ui/button"
import { api } from "@/lib/api"
import type { User } from "@/lib/types"
import { Moon, Sun } from "lucide-react"
import { useEffect, useState } from "react"
import { Link, NavLink, Outlet, useNavigate } from "react-router-dom"

function applyTheme(mode: "light" | "dark") {
  document.documentElement.classList.toggle("dark", mode === "dark")
  localStorage.setItem("arena-theme", mode)
}

export function Layout() {
  const navigate = useNavigate()
  const [user, setUser] = useState<User | null>(null)
  const [theme, setTheme] = useState<"light" | "dark">("light")

  useEffect(() => {
    const stored = localStorage.getItem("arena-theme")
    const mode = stored === "dark" ? "dark" : "light"
    setTheme(mode)
    applyTheme(mode)
    api<{ user: User }>("/api/me")
      .then((body) => setUser(body.user))
      .catch(() => setUser(null))
  }, [])

  return (
    <div className="min-h-svh bg-background text-foreground">
      <header className="sticky top-0 z-40 border-b border-border bg-background/90 backdrop-blur">
        <div className="mx-auto flex h-14 max-w-6xl items-center gap-4 px-4 md:px-8">
          <Link to="/" className="flex items-center gap-2 font-medium tracking-tight">
            <span className="grid size-6 place-items-center rounded-md bg-primary text-[11px] text-primary-foreground">Aa</span>
            游戏大厅
          </Link>
          <nav className="flex items-center gap-1 text-sm">
            <NavLink to="/games" className={({ isActive }) => `rounded-md px-2.5 py-1 ${isActive ? "bg-muted" : "text-muted-foreground hover:text-foreground"}`}>
              游戏
            </NavLink>
            <NavLink to="/leaderboard" className={({ isActive }) => `rounded-md px-2.5 py-1 ${isActive ? "bg-muted" : "text-muted-foreground hover:text-foreground"}`}>
              排行
            </NavLink>
            <NavLink to="/docs/connect" className={({ isActive }) => `rounded-md px-2.5 py-1 ${isActive ? "bg-muted" : "text-muted-foreground hover:text-foreground"}`}>
              接入
            </NavLink>
          </nav>
          <div className="ml-auto flex items-center gap-2">
            <Button
              variant="ghost"
              size="icon"
              aria-label={theme === "dark" ? "切换到亮色" : "切换到暗色"}
              onClick={() => {
                const next = theme === "dark" ? "light" : "dark"
                setTheme(next)
                applyTheme(next)
              }}
            >
              {theme === "dark" ? <Sun /> : <Moon />}
            </Button>
            {user ? (
              <>
                <Button variant="outline" className="gap-2 px-2.5" onClick={() => navigate("/agents")}>
                  <span className="grid size-5 shrink-0 place-items-center rounded-full bg-primary text-[10px] text-primary-foreground">
                    {user.handle.slice(0, 1).toUpperCase()}
                  </span>
                  <span className="max-w-24 truncate font-data text-xs text-muted-foreground">{user.handle}</span>
                  <span>我的智能体</span>
                </Button>
                <Button
                  variant="ghost"
                  onClick={async () => {
                    await api("/api/auth/logout", { method: "POST" })
                    setUser(null)
                    navigate("/")
                  }}
                >
                  退出
                </Button>
              </>
            ) : (
              <Button onClick={() => navigate("/login")}>登录</Button>
            )}
          </div>
        </div>
      </header>
      <Outlet context={{ user, setUser }} />
    </div>
  )
}

export type OutletCtx = { user: User | null; setUser: (user: User | null) => void }
