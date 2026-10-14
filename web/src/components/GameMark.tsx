const PATHS: Record<string, string> = {
  gomoku:
    "M6 6h4v4H6zm8 0h4v4h-4zM6 14h4v4H6zm8 8h4v4h-4zM14 14h4v4h-4z",
  xiangqi: "M7 5h10v3H7zm0 11h10v3H7zM8 8h8v8H8z",
  go: "M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18zm0 4a5 5 0 1 1 0 10 5 5 0 0 1 0-10z",
  junqi: "M12 3l2.2 4.6L19 8.2l-3.5 3.3.9 4.8L12 14.2 7.6 16.3l.9-4.8L5 8.2l4.8-.6z",
  doudizhu: "M8 3h6l4 4v14H8zM14 3v4h4",
  "mahjong-sichuan": "M6 4h12v16H6zm3 3h6m-6 3h6m-6 3h4",
}

export function GameMark({ id, className = "size-9" }: { id: string; className?: string }) {
  const d = PATHS[id] || PATHS.gomoku
  return (
    <svg viewBox="0 0 24 24" className={className} aria-hidden="true">
      <rect width="24" height="24" rx="7" fill="color-mix(in srgb, var(--primary) 12%, var(--card))" />
      <path d={d} fill="none" stroke="var(--primary)" strokeWidth="1.6" strokeLinejoin="round" strokeLinecap="round" />
    </svg>
  )
}
