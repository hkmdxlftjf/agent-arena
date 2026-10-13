import { motion } from "framer-motion"
import type { Meld } from "@/lib/types"

const SUIT: Record<string, string> = { m: "万", s: "条", p: "筒" }

function label(code: string): string {
  if (code.length < 2) return code
  return `${code[0]}${SUIT[code[1]] || code[1]}`
}

function Tile({ code, hidden = false, hot = false }: { code?: string; hidden?: boolean; hot?: boolean }) {
  if (hidden) {
    return <span className="inline-block h-8 w-6 rounded-sm border border-[#1e3a5f] bg-[#1e3a5f]" />
  }
  return (
    <motion.span
      initial={{ y: 6, opacity: 0 }}
      animate={{ y: 0, opacity: 1 }}
      transition={{ duration: 0.28 }}
      className={`inline-flex h-9 min-w-7 items-center justify-center rounded-sm border bg-[#f6f1e7] px-1 text-[11px] text-zinc-800 ${hot ? "border-primary" : "border-[#d6d3d1]"}`}
    >
      {code ? label(code) : ""}
    </motion.span>
  )
}

function Row({ tiles, count }: { tiles: string[] | null | undefined; count: number }) {
  if (!tiles) {
    return (
      <div className="flex flex-wrap gap-0.5">
        {Array.from({ length: Math.min(count, 14) }, (_, i) => (
          <Tile key={i} hidden />
        ))}
      </div>
    )
  }
  return (
    <div className="flex flex-wrap gap-0.5">
      {tiles.map((tile, i) => (
        <Tile key={`${tile}-${i}`} code={tile} />
      ))}
    </div>
  )
}

const PHASE: Record<string, string> = {
  exchange: "换三张",
  dingque: "定缺",
  claim: "应牌",
  play: "行牌",
  over: "终局",
}

export function MahjongTable({
  phase,
  wall = 0,
  hands,
  counts = [13, 13, 13, 13],
  melds = [],
  discards = [],
  dingque = [],
  pending,
  hu = [],
  roles = ["东", "南", "西", "北"],
}: {
  phase?: string
  wall?: number
  hands?: (string[] | null)[] | null
  counts?: number[]
  melds?: Meld[][]
  discards?: string[][]
  dingque?: (string | null)[]
  pending?: { tile: string; from: number; kind: string } | null
  hu?: boolean[]
  roles?: string[]
}) {
  const order = [2, 3, 1, 0]
  return (
    <div className="space-y-3 rounded-2xl border border-border bg-[#fafafa] p-3">
      <div className="flex flex-wrap gap-2 text-xs text-muted-foreground">
        <span>{PHASE[phase || ""] || phase || "准备"}</span>
        <span>牌墙 {wall}</span>
        {pending && (
          <span>
            {roles[pending.from] || pending.from} 打出 <Tile code={pending.tile} hot />
          </span>
        )}
      </div>
      {order.map((seat) => (
        <div key={seat} className={`rounded-xl border bg-white p-2 ${hu[seat] ? "border-primary" : "border-border"}`}>
          <p className="mb-1 text-xs text-muted-foreground">
            {roles[seat]}
            {dingque[seat] ? ` · 缺${SUIT[dingque[seat]] || dingque[seat]}` : ""}
            {hu[seat] ? " · 已胡" : ""}
            {!hands?.[seat] ? ` · ${counts[seat] ?? 0} 张` : ""}
          </p>
          <Row tiles={hands?.[seat]} count={counts[seat] ?? 0} />
          {!!melds[seat]?.length && (
            <p className="mt-1 text-[11px] text-muted-foreground">
              副露 {melds[seat].map((meld) => `${meld.kind === "angang" ? "暗杠" : meld.kind === "peng" ? "碰" : "杠"}${label(meld.tile)}`).join(" ")}
            </p>
          )}
          {!!discards[seat]?.length && (
            <div className="mt-1 flex flex-wrap gap-0.5">
              {discards[seat].slice(-8).map((tile, i) => (
                <Tile key={`${tile}-${i}`} code={tile} />
              ))}
            </div>
          )}
        </div>
      ))}
    </div>
  )
}
