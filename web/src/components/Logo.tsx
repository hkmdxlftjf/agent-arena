import { Link } from "react-router-dom"

export function Logo() {
  return (
    <Link to="/" className="flex min-w-0 items-center gap-2 font-medium tracking-tight">
      <svg viewBox="0 0 32 32" className="size-8 shrink-0" aria-hidden="true">
        <rect width="32" height="32" rx="8" fill="var(--ink)" />
        <path d="M8 8h16M8 16h16M8 24h16M8 8v16M16 8v16M24 8v16" stroke="#f3eee6" strokeWidth="1.2" />
        <circle cx="16" cy="16" r="3.2" fill="#f6efe6" />
        <circle cx="24" cy="8" r="2.4" fill="var(--primary)" />
      </svg>
      <span className="truncate text-base">游戏大厅</span>
    </Link>
  )
}
