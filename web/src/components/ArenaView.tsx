import { Board } from "@/components/Board"
import { CardTable } from "@/components/CardTable"
import { GoBoard } from "@/components/GoBoard"
import { JunqiBoard } from "@/components/JunqiBoard"
import { MahjongTable } from "@/components/MahjongTable"
import { XiangqiBoard } from "@/components/XiangqiBoard"
import { roleOf } from "@/lib/games"
import type { GameView, Trick } from "@/lib/types"

export function ArenaView({ gameId, view, compact = false }: { gameId: string; view: GameView | null; compact?: boolean }) {
  if (!view) {
    return (
      <div className="grid h-64 place-items-center rounded-2xl border border-dashed border-border text-sm text-muted-foreground">
        棋盘还没摆开
      </div>
    )
  }
  const renderer = view.renderer || ""
  if (renderer === "xiangqi" || gameId === "xiangqi") {
    const last = view.lastMove && typeof view.lastMove === "object" ? view.lastMove : null
    return <XiangqiBoard pieces={view.pieces} lastMove={last} check={view.check} toAct={view.toAct} compact={compact} />
  }
  if (renderer === "go" || gameId === "go") {
    const points = Array.isArray(view.points) && view.points.length && typeof view.points[0] === "object" ? (view.points as { at: string; seat: number }[]) : []
    return <GoBoard size={view.size || 19} points={points} lastMove={typeof view.lastMove === "string" ? view.lastMove : null} compact={compact} />
  }
  if (renderer === "junqi" || gameId === "junqi") {
    const last = view.last && !("cards" in view.last) ? view.last : null
    return <JunqiBoard cells={view.cells} last={last} phase={view.phase} compact={compact} />
  }
  if (renderer === "card-table" || gameId === "doudizhu") {
    const last = view.last && "cards" in view.last ? (view.last as Trick) : null
    return (
      <CardTable
        hands={view.hands}
        counts={view.counts}
        bottom={view.bottom}
        phase={view.phase}
        bid={view.bid}
        landlord={view.landlord}
        multiplier={view.multiplier}
        last={last}
        roles={[0, 1, 2].map((seat) => roleOf(gameId, seat))}
      />
    )
  }
  if (renderer === "mahjong-table" || gameId === "mahjong-sichuan") {
    return (
      <MahjongTable
        phase={view.phase}
        wall={view.wall}
        hands={view.hands}
        counts={view.counts}
        melds={view.melds}
        discards={view.discards}
        dingque={view.dingque}
        pending={view.pending}
        hu={view.hu}
        roles={[0, 1, 2, 3].map((seat) => roleOf(gameId, seat))}
      />
    )
  }
  if (view.stones) {
    return (
      <Board
        stones={view.stones}
        lastMove={typeof view.lastMove === "string" ? view.lastMove : null}
        winLine={view.winLine}
        compact={compact}
      />
    )
  }
  return (
    <div className="grid h-64 place-items-center rounded-2xl border border-dashed border-border text-sm text-muted-foreground">
      这一局面还没有可画的棋盘
    </div>
  )
}
