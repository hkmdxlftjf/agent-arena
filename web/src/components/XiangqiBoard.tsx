import { AnimatePresence, motion } from "framer-motion"
import { boardPaint, xiangqiDisc } from "@/cosmetics/skins"
import { shade } from "@/lib/color"
import type { XiangqiPiece } from "@/lib/types"

const FILES = "abcdefghi".split("")

const reducedMotion =
  typeof window !== "undefined" && window.matchMedia("(prefers-reduced-motion: reduce)").matches

const GENERAL = 1
const ADVISOR = 2
const ELEPHANT = 3
const HORSE = 4
const CHARIOT = 5
const CANNON = 6

const EASE: [number, number, number, number] = [0.2, 0, 0, 1]

function parseAt(at: string): { f: number; r: number } | null {
  const f = FILES.indexOf(at[0]?.toLowerCase() ?? "")
  const r = Number(at.slice(1))
  if (f < 0 || r < 1 || r > 10) return null
  return { f, r }
}

function entrance(kind: number, side: number, f: number, cell: number) {
  const settled = { opacity: 1, x: 0, y: 0, scale: 1 }
  if (reducedMotion) {
    return { initial: { opacity: 0 }, animate: settled, transition: { duration: 0 } }
  }
  let initial
  let animate
  let duration
  switch (kind) {
    case GENERAL:
      initial = { opacity: 0, y: cell * 0.8, scale: 0.85 }
      animate = { opacity: 1, y: 0, scale: [0.85, 1.07, 1] }
      duration = 0.34
      break
    case ADVISOR:
      initial = { opacity: 0, x: -cell * 0.7, y: cell * 0.7 }
      animate = { opacity: 1, x: 0, y: 0 }
      duration = 0.3
      break
    case ELEPHANT:
      initial = { opacity: 0, x: -cell, y: cell * 0.9 }
      animate = { opacity: 1, x: [-cell, -cell * 0.4, 0], y: [cell * 0.9, cell * 0.35, 0] }
      duration = 0.36
      break
    case HORSE:
      initial = { opacity: 0, x: -cell * 0.5, y: cell, scale: 0.9 }
      animate = {
        opacity: 1,
        x: [-cell * 0.5, cell * 0.15, 0],
        y: [cell, -cell * 0.3, 0],
        scale: [0.9, 1.12, 1],
      }
      duration = 0.32
      break
    case CHARIOT:
      initial = { opacity: 0, x: (f <= 4 ? -1 : 1) * cell * 2.6 }
      animate = { opacity: 1, x: 0 }
      duration = 0.3
      break
    case CANNON:
      initial = { opacity: 0, x: cell * 0.35, y: -cell * 1.5, scale: 0.5 }
      animate = {
        opacity: 1,
        x: [cell * 0.35, 0, 0],
        y: [-cell * 1.5, -cell * 0.2, 0],
        scale: [0.5, 1.25, 1],
      }
      duration = 0.38
      break
    default:
      initial = { opacity: 0, y: side === 0 ? cell * 0.8 : -cell * 0.8 }
      animate = { opacity: 1, y: [initial.y, side === 0 ? -cell * 0.15 : cell * 0.15, 0] }
      duration = 0.3
      break
  }
  return { initial, animate, transition: { duration, ease: EASE } }
}

const pieceExit = {
  opacity: 0,
  scale: 0.55,
  rotate: 10,
  transition: { duration: 0.18, ease: EASE },
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
      <defs>
        <radialGradient id="xiq-face" cx="38%" cy="32%" r="78%">
          <stop offset="0%" stopColor="var(--xiq-face-light, #ffffff)" />
          <stop offset="62%" stopColor="var(--xiq-face-base, #fffaf3)" />
          <stop offset="100%" stopColor="var(--xiq-face-dark, #e7dcc4)" />
        </radialGradient>
      </defs>
      <rect width={w} height={h} fill={paint?.fill || "var(--board-fill)"} />
      <rect x={pad} y={xy(0, 5).y} width={cell * 8} height={xy(0, 6).y - xy(0, 5).y} fill={paint?.river || "color-mix(in srgb, var(--board-edge) 22%, var(--board-fill))"} />
      {Array.from({ length: 10 }, (_, i) => (
        <g key={`rank-${i}`}>{line(0, i + 1, 8, i + 1)}</g>
      ))}
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
      <AnimatePresence>
        {pieces.map((piece) => {
          const pos = parseAt(piece.at)
          if (!pos) return null
          const p = xy(pos.f, pos.r)
          const red = piece.side === 0
          const disc = xiangqiDisc(skins?.[piece.side], red)
          const radius = compact ? 11 : 18
          const marked = marks.has(piece.at)
          const faceLight = shade(disc.fill, 1.06)
          const sideDark = shade(disc.fill, 0.6)
          const ring = red ? "#b42318" : "#1c1917"
          const glyph = red ? "#b42318" : "#1c1917"
          const ghost = shade(disc.fill, 0.9)
          const { initial, animate, transition } = entrance(piece.kind, piece.side, pos.f, cell)
          return (
            <motion.g
              key={piece.at}
              className="piece"
              initial={initial}
              animate={animate}
              exit={pieceExit}
              transition={transition}
              style={{ transformOrigin: `${p.x}px ${p.y}px` }}
            >
              <ellipse cx={p.x} cy={p.y + radius * 0.72} rx={radius * 1.05} ry={radius * 0.42} fill="rgba(0,0,0,0.22)" />
              <ellipse cx={p.x} cy={p.y + radius * 0.52} rx={radius} ry={radius * 0.52} fill={sideDark} />
              <circle cx={p.x} cy={p.y} r={radius} fill="url(#xiq-face)" stroke={disc.stroke} strokeWidth={marked ? 2.4 : 1.4} />
              <circle cx={p.x} cy={p.y} r={radius * 0.78} fill="none" stroke={ring} strokeWidth="0.8" opacity="0.45" />
              <text x={p.x} y={p.y + (compact ? 5 : 7)} textAnchor="middle" fill={ghost} fontSize={compact ? 12 : 16} fontFamily="Noto Sans SC, sans-serif" fontWeight={600}>
                {piece.label}
              </text>
              <text x={p.x} y={p.y + (compact ? 4 : 6)} textAnchor="middle" fill={glyph} fontSize={compact ? 12 : 16} fontFamily="Noto Sans SC, sans-serif" fontWeight={600}>
                {piece.label}
              </text>
              <ellipse cx={p.x} cy={p.y - radius * 0.28} rx={radius * 0.34} ry={radius * 0.14} fill={faceLight} opacity="0.55" />
            </motion.g>
          )
        })}
      </AnimatePresence>
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
