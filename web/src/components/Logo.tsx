import { Link } from "react-router-dom"

export function Logo({ compact = false }: { compact?: boolean }) {
  return (
    <Link to="/" className="logo group flex min-w-0 items-center gap-2.5 font-semibold tracking-tight" aria-label={compact ? "游戏大厅首页" : undefined}>
      <svg viewBox="0 0 32 32" className="size-8 shrink-0 transition-transform duration-300 group-hover:rotate-[-6deg]" aria-hidden="true">
        <rect width="32" height="32" rx="9" fill="#0d1320" />
        <rect x="0.5" y="0.5" width="31" height="31" rx="8.5" fill="none" stroke="color-mix(in srgb, var(--primary) 45%, transparent)" />
        <path d="M8 8h16M8 16h16M8 24h16M8 8v16M16 8v16M24 8v16" stroke="#e7ebf3" strokeOpacity="0.55" strokeWidth="1.2" />
        <circle cx="16" cy="16" r="3.4" fill="#e7ebf3" />
        <circle cx="24" cy="8" r="2.6" fill="var(--primary)" className="logo-spark" />
      </svg>
      {!compact && <span className="truncate text-base">游戏大厅</span>}
    </Link>
  )
}
