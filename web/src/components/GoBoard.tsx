import { motion } from "framer-motion"
import type { GoPoint } from "@/lib/types"

function columns(size: number): string[] {
  return "ABCDEFGHJKLMNOPQRSTUVWXYZ".slice(0, size).split("")
}

function parseAt(at: string, size: number): { x: number; y: number } | null {
  const col = at[0]?.toUpperCase()
  const row = Number(at.slice(1))
  const x = columns(size).indexOf(col ?? "")
  if (x < 0 || row < 1 || row > size) return null
  return { x, y: row - 1 }
}

function stars(size: number): Array<[number, number]> {
  if (size === 9) return [[2, 2], [2, 6], [4, 4], [6, 2], [6, 6]]
  if (size === 13) return [[3, 3], [3, 9], [6, 6], [9, 3], [9, 9]]
  if (size === 19) return [[3, 3], [3, 9], [3, 15], [9, 3], [9, 9], [9, 15], [15, 3], [15, 9], [15, 15]]
  return []
}

export function GoBoard({
  size = 19,
  points = [],
  lastMove,
  compact = false,
}: {
  size?: number
  points?: GoPoint[]
  lastMove?: string | null
  compact?: boolean
}) {
  const cell = compact ? size > 13 ? 14 : 18 : size > 13 ? 26 : 34
  const pad = compact ? 14 : 28
  const span = pad * 2 + cell * (size - 1)
  const xy = (x: number, y: number) => ({ x: pad + x * cell, y: pad + (size - 1 - y) * cell })
  const cols = columns(size)

  return (
    <svg viewBox={`0 0 ${span} ${span}`} className="h-auto w-full rounded-2xl border border-[#d7b98a]" role="img" aria-label={`${size} 路围棋`}>
      <rect width={span} height={span} fill="#e7c590" />
      {Array.from({ length: size }, (_, i) => {
        const a = xy(i, 0)
        const b = xy(0, i)
        return (
          <g key={i}>
            <line x1={xy(0, 0).x} y1={a.y} x2={xy(size - 1, 0).x} y2={a.y} stroke="#6b4f2a" strokeWidth="1" />
            <line x1={b.x} y1={xy(0, 0).y} x2={b.x} y2={xy(0, size - 1).y} stroke="#6b4f2a" strokeWidth="1" />
          </g>
        )
      })}
      {stars(size).map(([x, y]) => {
        const p = xy(x, y)
        return <circle key={`${x}${y}`} cx={p.x} cy={p.y} r={compact ? 1.8 : 2.6} fill="#6b4f2a" />
      })}
      {!compact &&
        cols.map((file, x) => {
          const p = xy(x, 0)
          return (
            <text key={file} x={p.x} y={span - 8} textAnchor="middle" fill="#6b4f2a" fontSize="11" fontFamily="JetBrains Mono, monospace">
              {file}
            </text>
          )
        })}
      {points.map((stone) => {
        const pos = parseAt(stone.at, size)
        if (!pos) return null
        const p = xy(pos.x, pos.y)
        const black = stone.seat === 0
        const radius = compact ? cell * 0.42 : cell * 0.46
        return (
          <motion.g
            key={stone.at}
            initial={{ scale: 0.4, opacity: 0 }}
            animate={{ scale: 1, opacity: 1 }}
            transition={{ duration: 0.32, ease: [0.2, 0.8, 0.2, 1] }}
            style={{ transformOrigin: `${p.x}px ${p.y}px` }}
          >
            <circle cx={p.x} cy={p.y} r={radius} fill={black ? "#1a1a1a" : "#f7f4ef"} stroke={black ? "#1a1a1a" : "#cfc6b8"} />
            {stone.at.toUpperCase() === lastMove?.toUpperCase() && (
              <circle cx={p.x} cy={p.y} r={radius * 0.28} fill={black ? "#f4f4f5" : "#6246ea"} />
            )}
          </motion.g>
        )
      })}
    </svg>
  )
}
