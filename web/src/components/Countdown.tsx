import { useEffect, useState } from "react"

export function Countdown({
  deadline,
  serverTime,
  limitMs,
  active,
}: {
  deadline?: number | null
  serverTime?: number
  limitMs: number
  active: boolean
}) {
  const [now, setNow] = useState(() => Date.now())
  const [anchor, setAnchor] = useState(() => ({ server: serverTime ?? Date.now(), local: Date.now() }))
  useEffect(() => {
    const id = window.setInterval(() => setNow(Date.now()), 200)
    return () => window.clearInterval(id)
  }, [])
  useEffect(() => {
    if (!serverTime) return
    const local = Date.now()
    const id = window.setTimeout(() => setAnchor({ server: serverTime, local }), 0)
    return () => window.clearTimeout(id)
  }, [serverTime])

  const serverNow = anchor.server + (now - anchor.local)
  const remaining = deadline && active ? Math.max(0, deadline - serverNow) : null
  const ratio = remaining == null || limitMs <= 0 ? 1 : Math.min(1, remaining / limitMs)
  const r = 18
  const c = 2 * Math.PI * r
  const dangerAt = limitMs > 0 ? limitMs / 12 : 5_000
  const warnAt = limitMs > 0 ? limitMs / 4 : 15_000
  const tone =
    remaining == null ? "text-muted-foreground" : remaining < dangerAt ? "text-destructive" : remaining < warnAt ? "text-warn" : "text-primary"
  const label =
    remaining == null ? "—" : `${Math.floor(remaining / 60000)}:${String(Math.floor((remaining / 1000) % 60)).padStart(2, "0")}`

  return (
    <div className={`relative grid size-14 place-items-center ${tone}`} aria-label={remaining == null ? "未计时" : `剩余 ${label}`}>
      <svg viewBox="0 0 44 44" className="absolute inset-0 -rotate-90">
        <circle cx="22" cy="22" r={r} fill="none" stroke="currentColor" strokeOpacity="0.18" strokeWidth="3" />
        <circle
          cx="22"
          cy="22"
          r={r}
          fill="none"
          stroke="currentColor"
          strokeWidth="3"
          strokeLinecap="round"
          strokeDasharray={c}
          strokeDashoffset={c * (1 - ratio)}
        />
      </svg>
      <span className="font-data text-xs font-medium text-foreground">{label}</span>
    </div>
  )
}
