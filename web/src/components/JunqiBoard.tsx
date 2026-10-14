import { motion } from "framer-motion"
import type { JunqiCell } from "@/lib/types"

const ZONE: Record<string, string> = {
  hq: "#d7b07a",
  camp: "#f8e7c4",
  rail: "#ecd7ae",
  road: "#f6e6c8",
}

function parse(at: string): { f: number; r: number } | null {
  const f = at.charCodeAt(0) - 97
  const r = Number(at.slice(1))
  if (f < 0 || f > 4 || r < 1 || r > 12) return null
  return { f, r }
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
  const xy = (f: number, r: number) => ({ x: pad + f * cell, y: pad + (12 - r) * cell })

  return (
    <div className="rounded-2xl border border-[#e7d3b0] bg-[#fbf6ec] p-2">
      {phase && (
        <p className="mb-2 px-1 text-xs text-[#6b4f3a]">{phase === "deploy" ? "双方正在布阵，棋子背面朝上" : phase === "over" ? "终局后身份已揭示" : "暗棋对局"}</p>
      )}
      <svg viewBox={`0 0 ${w} ${h}`} className="h-auto w-full" role="img" aria-label="军棋棋盘">
        {cells.map((item) => {
          const pos = parse(item.at)
          if (!pos) return null
          const p = xy(pos.f, pos.r)
          const hot = item.at === last?.from || item.at === last?.to
          return (
            <g key={item.at}>
              <rect x={p.x + 1} y={p.y + 1} width={cell - 2} height={cell - 2} rx="4" fill={ZONE[item.zone] || ZONE.road} stroke={hot ? "#8f2d2a" : "#c4a574"} />
              {item.zone === "camp" && <circle cx={p.x + cell / 2} cy={p.y + cell / 2} r={cell * 0.28} fill="none" stroke="#b08958" />}
              {item.piece && (
                <motion.g className="piece" initial={{ opacity: 0, y: 4 }} animate={{ opacity: 1, y: 0 }} transition={{ duration: 0.32 }}>
                  <rect
                    x={p.x + 4}
                    y={p.y + 6}
                    width={cell - 8}
                    height={cell - 12}
                    rx="5"
                    fill={item.piece.hidden ? "#44403c" : item.piece.owner === 0 ? "#9f1239" : "#1e3a5f"}
                  />
                  <text
                    x={p.x + cell / 2}
                    y={p.y + cell / 2 + 4}
                    textAnchor="middle"
                    fill="#fffaf3"
                    fontSize={compact ? 9 : 12}
                    fontFamily="Noto Sans SC, sans-serif"
                  >
                    {item.piece.name}
                  </text>
                </motion.g>
              )}
            </g>
          )
        })}
      </svg>
      {last?.result && <p className="mt-2 px-1 text-xs text-[#6b4f3a]">上一手：{last.result === "flag" ? "夺旗" : last.result === "both" ? "同尽" : last.result === "eaten" ? "被吃" : last.result === "eat" ? "吃子" : last.result}</p>}
    </div>
  )
}
