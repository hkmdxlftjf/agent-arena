import { AnimatePresence, motion } from "framer-motion"
import { shade } from "@/lib/color"
import type { JunqiCell } from "@/lib/types"

const reducedMotion =
  typeof window !== "undefined" && window.matchMedia("(prefers-reduced-motion: reduce)").matches

const RANKS = ["司令", "军长", "师长", "旅长", "团长", "营长", "连长", "排长"]

const EASE: [number, number, number, number] = [0.2, 0, 0, 1]

function parse(at: string): { f: number; r: number } | null {
  const f = at.charCodeAt(0) - 97
  const r = Number(at.slice(1))
  if (f < 0 || f > 4 || r < 1 || r > 12) return null
  return { f, r }
}

function isHq(f: number, r: number) {
  return (r === 1 || r === 12) && (f === 0 || f === 4)
}

function isCamp(f: number, r: number) {
  return (
    ((f === 1 || f === 3) && (r === 3 || r === 5 || r === 8 || r === 10)) ||
    (f === 2 && (r === 4 || r === 9))
  )
}

function onRail(f: number, r: number) {
  if (isHq(f, r) || isCamp(f, r)) return false
  if (f === 0 || f === 4) return r >= 2 && r <= 11
  if (r === 1 || r === 12) return f >= 1 && f <= 3
  return r === 6 || r === 7
}

function isRail(f: number, r: number, nf: number, nr: number) {
  if (!onRail(f, r) || !onRail(nf, nr)) return false
  const df = nf - f
  const dr = nr - r
  if (Math.abs(df) + Math.abs(dr) !== 1) return false
  if (df !== 0) return r === 1 || r === 12 || r === 6 || r === 7
  return f === 0 || f === 4 || ((r === 6 || r === 7) && (nr === 6 || nr === 7))
}

function entrance(name: string, f: number, cell: number) {
  const settled = { opacity: 1, x: 0, y: 0, scale: 1, scaleX: 1 }
  if (reducedMotion) {
    return { initial: { opacity: 0 }, animate: settled, transition: { duration: 0 } }
  }
  switch (name) {
    case "司令":
    case "军长":
      return {
        initial: { opacity: 0, y: cell * 0.5, scale: 0.9 },
        animate: { opacity: 1, y: 0, scale: [0.9, 1.06, 1] },
        transition: { duration: 0.34, ease: EASE },
      }
    case "工兵":
      return {
        initial: { opacity: 0, x: (f <= 2 ? -1 : 1) * cell * 0.8 },
        animate: { opacity: 1, x: 0 },
        transition: { duration: 0.26, ease: EASE },
      }
    case "地雷":
      return {
        initial: { opacity: 0, y: -cell * 0.7, scale: 0.9 },
        animate: { opacity: 1, y: 0, scale: [0.9, 1.18, 1] },
        transition: { duration: 0.3, ease: EASE },
      }
    case "炸弹":
      return {
        initial: { opacity: 0, scale: 0.4 },
        animate: { opacity: 1, scale: [0.4, 1.15, 1] },
        transition: { duration: 0.28, ease: EASE },
      }
    case "军旗":
      return {
        initial: { opacity: 0, scaleX: 0.3 },
        animate: { opacity: 1, scaleX: [0.3, 1.05, 1] },
        transition: { duration: 0.3, ease: EASE },
      }
    default:
      return {
        initial: { opacity: 0, y: RANKS.includes(name) ? cell * 0.35 : cell * 0.2 },
        animate: { opacity: 1, y: 0 },
        transition: { duration: 0.28, ease: EASE },
      }
  }
}

const pieceExit = {
  opacity: 0,
  scale: 0.6,
  transition: { duration: 0.16, ease: EASE },
}

export function JunqiBoard({
  cells = [],
  last,
  phase,
  compact = false,
}: {
  cells?: JunqiCell[]
  last?: { from?: string; to?: string; result?: string; type?: string } | null
  phase?: string
  compact?: boolean
}) {
  const cell = compact ? 28 : 46
  const pad = compact ? 8 : 18
  const w = pad * 2 + cell * 5
  const h = pad * 2 + cell * 12
  const xy = (f: number, r: number) => ({ x: pad + f * cell + cell / 2, y: pad + (12 - r) * cell + cell / 2 })
  const line = "var(--board-line)"
  const roads: Array<[number, number, number, number]> = []
  const rails: Array<[number, number, number, number]> = []
  for (let r = 1; r <= 12; r++) {
    for (let f = 0; f < 5; f++) {
      if (f < 4) {
        const edge: [number, number, number, number] = [f, r, f + 1, r]
        if (isRail(f, r, f + 1, r)) rails.push(edge)
        else roads.push(edge)
      }
      if (r < 12) {
        const edge: [number, number, number, number] = [f, r, f, r + 1]
        if (isRail(f, r, f, r + 1)) rails.push(edge)
        else roads.push(edge)
      }
    }
  }
  const segment = (edge: [number, number, number, number], width: number, key: string) => {
    const [f, r, nf, nr] = edge
    const a = xy(f, r)
    const b = xy(nf, nr)
    return <line key={key} x1={a.x} y1={a.y} x2={b.x} y2={b.y} stroke={line} strokeWidth={width} strokeLinecap="square" />
  }

  return (
    <div className="rounded-2xl border bg-card p-2" style={{ borderColor: "var(--board-edge)" }}>
      {phase && (
        <p className="mb-2 px-1 text-xs text-muted-foreground">{phase === "deploy" ? "双方正在布阵，棋子背面朝上" : phase === "over" ? "终局后身份已揭示" : "暗棋对局"}</p>
      )}
      <svg viewBox={`0 0 ${w} ${h}`} className="h-auto w-full" role="img" aria-label="军棋棋盘" shapeRendering="geometricPrecision">
        <defs>
          <linearGradient id="jq-red" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor={shade("#9f1239", 1.18)} />
            <stop offset="55%" stopColor="#9f1239" />
            <stop offset="100%" stopColor={shade("#9f1239", 0.68)} />
          </linearGradient>
          <linearGradient id="jq-blue" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor={shade("#1e3a5f", 1.2)} />
            <stop offset="55%" stopColor="#1e3a5f" />
            <stop offset="100%" stopColor={shade("#1e3a5f", 0.66)} />
          </linearGradient>
          <linearGradient id="jq-hidden" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor={shade("#44403c", 1.15)} />
            <stop offset="55%" stopColor="#44403c" />
            <stop offset="100%" stopColor={shade("#44403c", 0.62)} />
          </linearGradient>
        </defs>
        <rect width={w} height={h} fill="var(--board-fill)" />
        {roads.map((edge, i) => segment(edge, compact ? 1.25 : 1.7, `road-${i}`))}
        {rails.map((edge, i) => segment(edge, compact ? 2.6 : 3.4, `rail-${i}`))}
        {Array.from({ length: 12 }, (_, i) => i + 1).flatMap((r) =>
          Array.from({ length: 5 }, (_, f) => {
            const p = xy(f, r)
            if (isHq(f, r)) {
              return <rect key={`s${f}${r}`} x={p.x - cell * 0.22} y={p.y - cell * 0.22} width={cell * 0.44} height={cell * 0.44} fill="var(--board-fill)" stroke={line} strokeWidth={compact ? 1.2 : 1.6} />
            }
            if (isCamp(f, r)) {
              return <circle key={`s${f}${r}`} cx={p.x} cy={p.y} r={cell * 0.28} fill="var(--board-fill)" stroke={line} strokeWidth={compact ? 1.2 : 1.6} shapeRendering="geometricPrecision" />
            }
            return <circle key={`s${f}${r}`} cx={p.x} cy={p.y} r={compact ? 1.6 : 2.2} fill={line} shapeRendering="geometricPrecision" />
          }),
        )}
        <AnimatePresence>
          {cells.map((item) => {
            const pos = parse(item.at)
            if (!pos || !item.piece) return null
            const p = xy(pos.f, pos.r)
            const hot = item.at === last?.from || item.at === last?.to
            const bw = compact ? cell * 0.62 : cell * 0.66
            const bh = compact ? cell * 0.5 : cell * 0.52
            const base = item.piece.hidden ? "#44403c" : item.piece.owner === 0 ? "#9f1239" : "#1e3a5f"
            const face = item.piece.hidden ? "url(#jq-hidden)" : item.piece.owner === 0 ? "url(#jq-red)" : "url(#jq-blue)"
            const { initial, animate, transition } = entrance(item.piece.name, pos.f, cell)
            return (
              <motion.g
                key={item.at}
                className="piece"
                initial={initial}
                animate={animate}
                exit={pieceExit}
                transition={transition}
                style={{ transformOrigin: `${p.x}px ${p.y}px` }}
              >
                <ellipse cx={p.x} cy={p.y + bh * 0.62} rx={bw * 0.56} ry={bh * 0.28} fill="rgba(0,0,0,0.22)" />
                {hot && <rect x={p.x - bw / 2 - 2} y={p.y - bh / 2 - 2} width={bw + 4} height={bh + 4} rx="6" fill="none" stroke="var(--primary)" strokeWidth="1.6" />}
                <rect x={p.x - bw / 2} y={p.y - bh / 2 + bh * 0.14} width={bw} height={bh} rx="5" fill={shade(base, 0.55)} />
                <rect x={p.x - bw / 2} y={p.y - bh / 2} width={bw} height={bh} rx="5" fill={face} stroke={shade(base, 0.45)} strokeWidth="0.6" />
                <line x1={p.x - bw / 2 + 2} y1={p.y - bh / 2 + 1.5} x2={p.x + bw / 2 - 2} y2={p.y - bh / 2 + 1.5} stroke={shade(base, 1.4)} strokeWidth="0.7" opacity="0.7" />
                <text x={p.x} y={p.y + 4} textAnchor="middle" fill="#fffaf3" fontSize={compact ? 9 : 12} fontFamily="Noto Sans SC, sans-serif">
                  {item.piece.name}
                </text>
              </motion.g>
            )
          })}
        </AnimatePresence>
      </svg>
      {last?.result && <p className="mt-2 px-1 text-xs text-muted-foreground">上一手：{last.result === "flag" ? "夺旗" : last.result === "both" ? "同尽" : last.result === "eaten" ? "被吃" : last.result === "eat" ? "吃子" : last.result}</p>}
    </div>
  )
}
