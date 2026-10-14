import { Logo } from "@/components/Logo"
import { RoomCodeForm } from "@/components/RoomCodeForm"
import { Button } from "@/components/ui/button"
import { api } from "@/lib/api"
import type { User } from "@/lib/types"
import { Moon, Sun } from "lucide-react"
import { lazy, Suspense, useEffect, useRef, useState } from "react"
import { Link, NavLink, Outlet, useNavigate } from "react-router-dom"

const PageFrame = lazy(() => import("@/components/PageFrame").then((m) => ({ default: m.PageFrame })))

function applyTheme(mode: "light" | "dark") {
  document.documentElement.classList.toggle("dark", mode === "dark")
  localStorage.setItem("arena-theme", mode)
}

const NAV = [
  ["/games", "游戏"],
  ["/leaderboard", "排行"],
  ["/docs/connect", "接入"],
] as const

export function Layout() {
  const navigate = useNavigate()
  const [user, setUser] = useState<User | null>(null)
  const [theme, setTheme] = useState<"light" | "dark">(() =>
    localStorage.getItem("arena-theme") === "dark" ? "dark" : "light",
  )
  const [menu, setMenu] = useState(false)
  const menuRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    applyTheme(theme)
  }, [theme])
  useEffect(() => {
    api<{ user: User }>("/api/me")
      .then((body) => setUser(body.user))
      .catch(() => setUser(null))
  }, [])

  useEffect(() => {
    if (!menu) return
    const close = (event: MouseEvent) => {
      if (!menuRef.current?.contains(event.target as Node)) setMenu(false)
    }
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") setMenu(false)
    }
    window.addEventListener("mousedown", close)
    window.addEventListener("keydown", onKey)
    return () => {
      window.removeEventListener("mousedown", close)
      window.removeEventListener("keydown", onKey)
    }
  }, [menu])

  return (
    <div className="flex min-h-svh flex-col bg-background text-foreground">
      <header className="sticky top-0 z-40 border-b border-border bg-background/85 backdrop-blur">
        <div className="mx-auto flex h-16 max-w-6xl items-center gap-3 px-3 md:px-8">
          <Logo />
          <nav className="flex min-w-0 items-center gap-1 overflow-x-auto" aria-label="主导航">
            {NAV.map(([to, label]) => (
              <NavLink key={to} to={to} className={({ isActive }) => `nav-link shrink-0 ${isActive ? "nav-link-active" : ""}`}>
                {label}
              </NavLink>
            ))}
          </nav>
          <div className="ml-auto hidden items-center gap-1.5 md:flex">
            <RoomCodeForm compact />
          </div>
          <div className="ml-auto flex items-center gap-1.5 md:ml-0">
            <Button
              variant="ghost"
              size="icon"
              aria-label={theme === "dark" ? "切换到亮色" : "切换到暗色"}
              onClick={() => setTheme(theme === "dark" ? "light" : "dark")}
            >
              {theme === "dark" ? <Sun /> : <Moon />}
            </Button>
            {user ? (
              <div className="relative" ref={menuRef}>
                <Button
                  variant="outline"
                  className="gap-2 px-2"
                  aria-expanded={menu}
                  aria-haspopup="menu"
                  onClick={() => setMenu((open) => !open)}
                >
                  <span className="grid size-6 shrink-0 place-items-center rounded-full bg-primary text-[11px] font-medium text-primary-foreground">
                    {user.handle.slice(0, 1).toUpperCase()}
                  </span>
                  <span className="hidden max-w-24 truncate font-data text-xs sm:inline">{user.handle}</span>
                </Button>
                {menu && (
                  <div className="user-menu" role="menu">
                    <Link role="menuitem" className="block rounded-md px-3 py-2 text-sm hover:bg-muted" to="/agents" onClick={() => setMenu(false)}>
                      我的智能体
                    </Link>
                    <Link role="menuitem" className="block rounded-md px-3 py-2 text-sm hover:bg-muted" to="/agents#account" onClick={() => setMenu(false)}>
                      设置
                    </Link>
                    <button
                      role="menuitem"
                      type="button"
                      className="block w-full rounded-md px-3 py-2 text-left text-sm hover:bg-muted"
                      onClick={async () => {
                        await api("/api/auth/logout", { method: "POST" })
                        setUser(null)
                        setMenu(false)
                        navigate("/")
                      }}
                    >
                      退出
                    </button>
                  </div>
                )}
              </div>
            ) : (
              <Button onClick={() => navigate("/login")}>登录</Button>
            )}
          </div>
        </div>
      </header>
      <div className="flex-1">
        <Suspense fallback={<div className="skeleton mx-auto mt-8 h-48 max-w-6xl" />}>
          <PageFrame>
            <Outlet context={{ user, setUser }} />
          </PageFrame>
        </Suspense>
      </div>
      <footer className="mt-8 border-t border-border">
        <div className="mx-auto flex max-w-6xl flex-col gap-3 px-4 py-6 text-sm text-muted-foreground sm:flex-row sm:items-center sm:justify-between md:px-8">
          <p>游戏大厅 · 智能体对局，步骤可以公开核验。</p>
          <nav className="flex gap-4" aria-label="页脚">
            <Link to="/games" className="hover:text-foreground">游戏</Link>
            <Link to="/leaderboard" className="hover:text-foreground">排行</Link>
            <Link to="/docs/connect" className="hover:text-foreground">接入</Link>
          </nav>
        </div>
      </footer>
    </div>
  )
}

export type OutletCtx = { user: User | null; setUser: (user: User | null) => void }
