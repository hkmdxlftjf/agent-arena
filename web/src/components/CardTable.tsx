import { motion } from "framer-motion"
import type { Trick } from "@/lib/types"

function face(card: string): { rank: string; suit: string; red: boolean } {
  if (card === "X") return { rank: "小", suit: "王", red: false }
  if (card === "D") return { rank: "大", suit: "王", red: true }
  const rank = card[0] === "T" ? "10" : card[0] || "?"
  const suit = { s: "♠", h: "♥", d: "♦", c: "♣" }[card[1] || ""] || ""
  return { rank, suit, red: card[1] === "h" || card[1] === "d" || card === "D" }
}

function PlayingCard({ card, hidden = false, index = 0 }: { card?: string; hidden?: boolean; index?: number }) {
  if (hidden || !card) {
    return <span className="inline-block h-12 w-8 rounded-md border border-[#312e81] bg-gradient-to-br from-[#4338ca] to-[#312e81] shadow-sm" />
  }
  const f = face(card)
  return (
    <motion.span
      initial={{ y: 10, opacity: 0 }}
      animate={{ y: 0, opacity: 1 }}
      transition={{ duration: 0.28, delay: Math.min(index, 8) * 0.02 }}
      className={`inline-flex h-14 w-10 flex-col items-center justify-center rounded-md border bg-white shadow-sm ${f.red ? "border-red-200 text-red-600" : "border-zinc-200 text-zinc-900"}`}
    >
      <span className="font-data text-sm leading-none font-medium">{f.rank}</span>
      <span className="text-xs leading-none">{f.suit}</span>
    </motion.span>
  )
}

function Hand({ cards, count }: { cards: string[] | null | undefined; count: number }) {
  if (!cards) {
    return (
      <div className="flex gap-1">
        {Array.from({ length: Math.min(count, 12) }, (_, i) => (
          <PlayingCard key={i} hidden />
        ))}
        {count > 12 && <span className="self-center text-xs text-muted-foreground">×{count}</span>}
      </div>
    )
  }
  return (
    <div className="flex flex-wrap gap-1">
      {cards.map((card, i) => (
        <PlayingCard key={`${card}-${i}`} card={card} index={i} />
      ))}
    </div>
  )
}

const KIND: Record<string, string> = {
  single: "单张",
  pair: "对子",
  triple: "三张",
  triple1: "三带一",
  triple2: "三带二",
  straight: "顺子",
  pairseq: "连对",
  plane: "飞机",
  plane1: "飞机带单",
  plane2: "飞机带对",
  four2: "四带二",
  four4: "四带两对",
  bomb: "炸弹",
  rocket: "王炸",
}

export function CardTable({
  hands,
  counts = [17, 17, 17],
  bottom,
  phase,
  bid = 0,
  landlord,
  multiplier = 1,
  last,
  roles = ["甲", "乙", "丙"],
}: {
  hands?: (string[] | null)[] | null
  counts?: number[]
  bottom?: string[] | null
  phase?: string
  bid?: number
  landlord?: number | null
  multiplier?: number
  last?: Trick | null
  roles?: string[]
}) {
  const seats = [1, 2, 0]
  return (
    <div className="space-y-3 rounded-2xl border border-border bg-[#fafafa] p-3 dark:bg-zinc-950">
      <div className="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
        <span>{phase === "bid" ? `叫分中 · 当前 ${bid || "未叫"}` : phase === "play" ? "出牌" : "终局"}</span>
        <span>倍数 ×{multiplier}</span>
        {landlord != null && <span>地主 {roles[landlord] || landlord}</span>}
      </div>
      {seats.map((seat) => (
        <div key={seat} className="rounded-xl border border-border bg-white p-2">
          <p className="mb-1 text-xs text-muted-foreground">
            {roles[seat] || `座${seat}`}
            {landlord === seat ? " · 地主" : ""}
            {hands?.[seat] ? "" : ` · ${counts[seat] ?? 0} 张`}
          </p>
          <Hand cards={hands?.[seat]} count={counts[seat] ?? 0} />
        </div>
      ))}
      <div className="rounded-xl border border-dashed border-border p-3">
        <p className="mb-2 text-xs text-muted-foreground">{last ? `${roles[last.seat] || ""} 出 ${KIND[last.kind] || last.kind}` : "还没有人出牌"}</p>
        <div className="flex flex-wrap gap-1">
          {last?.cards?.map((card, i) => (
            <PlayingCard key={`${card}-${i}`} card={card} index={i} />
          ))}
        </div>
        {bottom && (
          <div className="mt-3">
            <p className="mb-1 text-xs text-muted-foreground">底牌</p>
            <div className="flex gap-1">
              {bottom.map((card, i) => (
                <PlayingCard key={`${card}-${i}`} card={card} />
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  )
}
