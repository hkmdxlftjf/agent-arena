import { motion } from "framer-motion"
import { boardPaint, xiangqiDisc } from "@/cosmetics/skins"
import type { XiangqiPiece } from "@/lib/types"

const FILES = "abcdefghi".split("")

function parseAt(at: string): { f: number; r: number } | null {
  const f = FILES.indexOf(at[0]?.toLowerCase() ?? "")
  const r = Number(at.slice(1))
  if (f < 0 || r < 1 || r > 10) return null
  return { f, r }
}

export function XiangqiBoard({
  pieces = [],
  lastMove,
  check = false,
  toAct = [],
  compact = false,
  theme,
  skins,
}: {
  pieces?: XiangqiPiece[]
  lastMove?: { from?: string; to?: string } | null
  check?: boolean
  toAct?: number[]
  compact?: boolean
  theme?: string | null
  skins?: Record<number, string | undefined>
}) {
  const cell = compact ? 28 : 48
  const pad = compact ? 16 : 32
  const w = pad * 2 + cell * 8
  const h = pad * 2 + cell * 9
  const xy = (f: number, r: number) => ({ x: pad + f * cell, y: pad + (10 - r) * cell })
  const line = (f1: number, r1: number, f2: number, r2: number) => {
    const a = xy(f1, r1)
    const b = xy(f2, r2)
    return <line x1={a.x} y1={a.y} x2={b.x} y2={b.y} stroke={lineColor} strokeWidth={compact ? 1.25 : 1.75} />
  }
  const marks = new Set([lastMove?.from, lastMove?.to].filter(Boolean) as string[])
  const paint = boardPaint(theme)
  const lineColor = paint?.line || "var(--board-line)"

  return (
    <svg viewBox={`0 0 ${w} ${h}`} className="h-auto w-full rounded-2xl border" role="img" aria-label="中国象棋棋盘" shapeRendering="geometricPrecision" style={{ borderColor: paint?.edge || "var(--board-edge)" }}>
      <rect width={w} height={h} fill={paint?.fill || "var(--board-fill)"} />
      <rect x={pad} y={xy(0, 5).y} width={cell * 8} height={xy(0, 6).y - xy(0, 5).y} fill={paint?.river || "color-mix(in srgb, var(--board-edge) 22%, var(--board-fill))"} />
      {Array.from({ length: 10 }, (_, i) => line(0, i + 1, 8, i + 1))}
      {Array.from({ length: 9 }, (_, f) => (
        <g key={f}>
          {line(f, 1, f, 5)}
          {line(f, 6, f, 10)}
        </g>
      ))}
      {line(3, 1, 5, 3)}
      {line(5, 1, 3, 3)}
      {line(3, 8, 5, 10)}
      {line(5, 8, 3, 10)}
      {!compact && (
        <text x={w / 2} y={(xy(0, 5).y + xy(0, 6).y) / 2 + 6} textAnchor="middle" fill={paint?.label || "var(--board-label)"} fontSize="18" fontFamily="Noto Sans SC, sans-serif">
          楚 河          汉 界
        </text>
      )}
      {pieces.map((piece) => {
        const pos = parseAt(piece.at)
        if (!pos) return null
        const p = xy(pos.f, pos.r)
        const red = piece.side === 0
        const disc = xiangqiDisc(skins?.[piece.side], red)
        const radius = compact ? 11 : 18
        const marked = marks.has(piece.at)
        return (
          <motion.g
            key={piece.at}
            className="piece"
            initial={{ scale: 0.7, opacity: 0 }}
            animate={{ scale: 1, opacity: 1 }}
            transition={{ duration: 0.32, ease: [0.2, 0.8, 0.2, 1] }}
            style={{ transformOrigin: `${p.x}px ${p.y}px` }}
          >
            <circle cx={p.x} cy={p.y} r={radius} fill={disc.fill} stroke={disc.stroke} strokeWidth={marked ? 2.4 : 1.4} />
            <text
              x={p.x}
              y={p.y + (compact ? 4 : 6)}
              textAnchor="middle"
              fill={red ? "#b42318" : "#1c1917"}
              fontSize={compact ? 12 : 16}
              fontFamily="Noto Sans SC, sans-serif"
              fontWeight={600}
            >
              {piece.label}
            </text>
          </motion.g>
        )
      })}
      {check &&
        pieces
          .filter((piece) => piece.kind === 1 && toAct.includes(piece.side))
          .map((piece) => {
            const pos = parseAt(piece.at)
            if (!pos) return null
            const p = xy(pos.f, pos.r)
            return (
              <motion.circle
                key={`check-${piece.at}`}
                cx={p.x}
                cy={p.y}
                r={compact ? 14 : 22}
                fill="none"
                stroke="#b42318"
                strokeWidth="1.6"
                initial={{ opacity: 0.2 }}
                animate={{ opacity: [0.25, 0.9, 0.25] }}
                transition={{ duration: 1.2, repeat: Infinity }}
              />
            )
          })}
    </svg>
  )
}
