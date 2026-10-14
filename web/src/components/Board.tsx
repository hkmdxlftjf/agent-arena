import { boardPaint, gomokuStone } from "@/cosmetics/skins"
import { motion } from "framer-motion"

const FILES = "abcdefghijklmno".split("")

function parseAt(at: string): { c: number; r: number } | null {
  const file = at[0]?.toLowerCase()
  const rank = Number(at.slice(1))
  const c = FILES.indexOf(file ?? "")
  if (c < 0 || rank < 1 || rank > 15) return null
  return { c, r: rank - 1 }
}

type Stone = { at: string; seat: number }

export function Board({
  stones,
  lastMove,
  winLine = [],
  demo = false,
  compact = false,
  theme,
  skins,
}: {
  stones: Stone[]
  lastMove?: string | null
  winLine?: string[]
  demo?: boolean
  compact?: boolean
  theme?: string | null
  skins?: Record<number, string | undefined>
}) {
  const n = 15
  const pad = compact ? 10 : 28
  const cell = compact ? 16 : 36
  const size = pad * 2 + cell * (n - 1)
  const wins = new Set(winLine)
  const paint = boardPaint(theme)
  const xy = (c: number, r: number) => ({
    x: pad + c * cell,
    y: pad + (n - 1 - r) * cell,
  })

  return (
    <div className="relative">
      {demo && (
        <span className="absolute top-3 left-3 z-10 rounded-full border border-border bg-background/90 px-2 py-0.5 text-xs text-muted-foreground">
          演示
        </span>
      )}
      <svg
        viewBox={`0 0 ${size} ${size}`}
        className="paper-board h-auto w-full rounded-2xl border"
        style={paint ? { background: paint.fill, borderColor: paint.edge } : undefined}
        role="img"
        aria-label="五子棋棋盘"
      >
        {Array.from({ length: n }, (_, i) => {
          const p = xy(i, 0)
          const q = xy(0, i)
          return (
            <g key={i} className="text-zinc-400">
              <line x1={xy(0, 0).x} y1={p.y} x2={xy(n - 1, 0).x} y2={p.y} stroke={paint?.line || "var(--board-line)"} strokeWidth="1" />
              <line x1={q.x} y1={xy(0, 0).y} x2={q.x} y2={xy(0, n - 1).y} stroke={paint?.line || "var(--board-line)"} strokeWidth="1" />
            </g>
          )
        })}
        {!compact &&
          FILES.map((file, c) => {
            const p = xy(c, 0)
            return (
              <text key={file} x={p.x} y={size - 8} textAnchor="middle" fill={paint?.label || "var(--board-label)"} fontSize="11">
                {file}
              </text>
            )
          })}
        {!compact &&
          Array.from({ length: n }, (_, r) => {
            const p = xy(0, r)
            return (
              <text key={r} x={10} y={p.y + 4} textAnchor="middle" fill={paint?.label || "var(--board-label)"} fontSize="11">
                {r + 1}
              </text>
            )
          })}
        {[3, 7, 11].flatMap((r) =>
          [3, 7, 11].map((c) => {
            const p = xy(c, r)
            return <circle key={`${c}${r}`} cx={p.x} cy={p.y} r={compact ? 1.6 : 2.4} fill={paint?.line || "var(--board-line)"} />
          }),
        )}
        {stones.map((stone) => {
          const pos = parseAt(stone.at)
          if (!pos) return null
          const p = xy(pos.c, pos.r)
          const win = wins.has(stone.at)
          const tone = gomokuStone(skins?.[stone.seat], stone.seat, theme)
          const radius = compact ? 6.2 : 14
          return (
            <motion.g
              key={stone.at}
              className="piece"
              initial={{ scale: 0.55, opacity: 0 }}
              animate={{ scale: 1, opacity: 1 }}
              transition={{ duration: 0.32, ease: [0.2, 0.8, 0.2, 1] }}
              style={{ transformOrigin: `${p.x}px ${p.y}px` }}
            >
              <circle
                cx={p.x}
                cy={p.y}
                r={radius}
                fill={tone.fill}
                stroke={tone.stroke}
                strokeWidth={compact ? 1 : 1.5}
              />
              {stone.at === lastMove && (
                <circle cx={p.x} cy={p.y} r={compact ? 2 : 3.5} fill={tone.mark} />
              )}
              {win && (
                <motion.circle
                  cx={p.x}
                  cy={p.y}
                  r={radius + (compact ? 2 : 4)}
                  fill="none"
                  stroke="#8f2d2a"
                  strokeWidth={compact ? 1.4 : 2}
                  initial={{ opacity: 0 }}
                  animate={{ opacity: [0.2, 1, 0.55] }}
                  transition={{ duration: 1.1, repeat: Infinity, repeatType: "mirror" }}
                />
              )}
            </motion.g>
          )
        })}
      </svg>
    </div>
  )
}

export const DEMO_STONES = [
  { at: "h8", seat: 0 },
  { at: "i8", seat: 1 },
  { at: "h9", seat: 0 },
  { at: "i9", seat: 1 },
  { at: "h10", seat: 0 },
  { at: "i10", seat: 1 },
  { at: "h11", seat: 0 },
  { at: "i11", seat: 1 },
  { at: "h12", seat: 0 },
]
