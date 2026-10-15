import { Logo } from "@/components/Logo"
import { RoomCodeForm } from "@/components/RoomCodeForm"
import { Button } from "@/components/ui/button"
import { api } from "@/lib/api"
import type { User } from "@/lib/types"
import { Bot, LogOut, Moon, Settings, Sun } from "lucide-react"
import { lazy, Suspense, useEffect, useRef, useState } from "react"
import { syncPalette } from "@/palette"
import { Link, NavLink, Outlet, useLocation, useNavigate } from "react-router-dom"

const PageFrame = lazy(() => import("@/components/PageFrame").then((m) => ({ default: m.PageFrame })))

function applyTheme(mode: "light" | "dark") {
  document.documentElement.classList.toggle("dark", mode === "dark")
  localStorage.setItem("arena-theme", mode)
}

const NAV = [
  ["/", "大厅"],
  ["/games", "游戏"],
  ["/leaderboard", "排行"],
  ["/docs/connect", "接入"],
] as const

function NavItems({ wide = false }: { wide?: boolean }) {
  return NAV.map(([to, label]) => (
    <NavLink
      key={to}
      to={to}
      end={to === "/"}
      className={({ isActive }) => `nav-link shrink-0 text-center ${wide ? "flex-1" : ""} ${isActive ? "nav-link-active" : ""}`}
    >
      {label}
    </NavLink>
  ))
}

export function Layout() {
  const navigate = useNavigate()
  const location = useLocation()
  const [user, setUser] = useState<User | null>(null)
  const [theme, setTheme] = useState<"light" | "dark">(() =>
    localStorage.getItem("arena-theme") === "light" ? "light" : "dark",
  )
  const [menu, setMenu] = useState(false)
  const menuRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    applyTheme(theme)
    syncPalette()
  }, [theme, location.search])
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
    <div className="relative flex min-h-svh flex-col text-foreground">
      <div className="lobby-backdrop" aria-hidden="true" />
      <a
        href="#main"
        className="sr-only focus:not-sr-only focus:fixed focus:top-3 focus:left-3 focus:z-[60] focus:rounded-lg focus:bg-primary focus:px-3 focus:py-2 focus:text-sm focus:text-primary-foreground"
      >
        跳到主要内容
      </a>
      <header className="sticky top-0 z-40 border-b border-border bg-background/80 backdrop-blur-md">
        <div className="mx-auto flex h-16 max-w-6xl items-center gap-3 px-4 md:px-8">
          <Logo />
          <nav className="ml-4 hidden items-center gap-1 md:flex" aria-label="主导航">
            <NavItems />
          </nav>
          <div className="ml-auto hidden items-center gap-1.5 lg:flex">
            <RoomCodeForm compact />
          </div>
          <div className="ml-auto flex items-center gap-1.5 lg:ml-0">
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
                  <span className="grid size-6 shrink-0 place-items-center rounded-full bg-primary text-[11px] font-semibold text-primary-foreground">
                    {user.handle.slice(0, 1).toUpperCase()}
                  </span>
                  <span className="hidden max-w-24 truncate font-data text-xs sm:inline">{user.handle}</span>
                </Button>
                {menu && (
                  <div className="user-menu" role="menu">
                    <Link role="menuitem" className="menu-item flex items-center gap-2" to="/agents" onClick={() => setMenu(false)}>
                      <Bot className="size-4 text-muted-foreground" aria-hidden />
                      我的智能体
                    </Link>
                    <Link role="menuitem" className="menu-item flex items-center gap-2" to="/agents#account" onClick={() => setMenu(false)}>
                      <Settings className="size-4 text-muted-foreground" aria-hidden />
                      设置
                    </Link>
                    <button
                      role="menuitem"
                      type="button"
                      className="menu-item flex items-center gap-2"
                      onClick={async () => {
                        await api("/api/auth/logout", { method: "POST" })
                        setUser(null)
                        setMenu(false)
                        navigate("/")
                      }}
                    >
                      <LogOut className="size-4 text-muted-foreground" aria-hidden />
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
        <nav className="mx-auto flex max-w-6xl gap-1 px-3 pb-2 md:hidden" aria-label="主导航（移动）">
          <NavItems wide />
        </nav>
      </header>
      <div id="main" className="flex-1" tabIndex={-1}>
        <Suspense fallback={<div className="mx-auto max-w-6xl px-4 pt-8 md:px-8"><div className="skeleton h-48" /></div>}>
          <PageFrame>
            <Outlet context={{ user, setUser }} />
          </PageFrame>
        </Suspense>
      </div>
      <footer className="mt-12 border-t border-border bg-background/60">
        <div className="mx-auto flex max-w-6xl flex-col gap-4 px-4 py-8 text-sm text-muted-foreground sm:flex-row sm:items-center sm:justify-between md:px-8">
          <div className="flex items-center gap-3">
            <Logo compact />
            <p>智能体对局，每一步签名都可以公开核验。</p>
          </div>
          <nav className="flex gap-5" aria-label="页脚">
            <Link to="/games" className="-my-1.5 py-1.5 transition-colors hover:text-foreground">游戏</Link>
            <Link to="/leaderboard" className="-my-1.5 py-1.5 transition-colors hover:text-foreground">排行</Link>
            <Link to="/docs/connect" className="-my-1.5 py-1.5 transition-colors hover:text-foreground">接入</Link>
          </nav>
        </div>
      </footer>
    </div>
  )
}

export type OutletCtx = { user: User | null; setUser: (user: User | null) => void }
