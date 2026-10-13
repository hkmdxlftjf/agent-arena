import { roleOf } from "@/lib/games"
import type { GameView, GridCell, JunqiCell, Trick } from "@/lib/types"
import { lazy, Suspense, type ReactNode } from "react"

const Board = lazy(() => import("@/components/Board").then((m) => ({ default: m.Board })))
const GridBoard = lazy(() => import("@/components/GridBoard").then((m) => ({ default: m.GridBoard })))
const CardTable = lazy(() => import("@/components/CardTable").then((m) => ({ default: m.CardTable })))
const GoBoard = lazy(() => import("@/components/GoBoard").then((m) => ({ default: m.GoBoard })))
const JunqiBoard = lazy(() => import("@/components/JunqiBoard").then((m) => ({ default: m.JunqiBoard })))
const MahjongTable = lazy(() => import("@/components/MahjongTable").then((m) => ({ default: m.MahjongTable })))
const XiangqiBoard = lazy(() => import("@/components/XiangqiBoard").then((m) => ({ default: m.XiangqiBoard })))

function BoardSlot({ children }: { children: ReactNode }) {
  return <Suspense fallback={<div className="grid h-64 place-items-center rounded-2xl bg-muted text-sm text-muted-foreground">棋盘加载中</div>}>{children}</Suspense>
}

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
    return (
      <BoardSlot>
        <XiangqiBoard pieces={view.pieces} lastMove={last} check={view.check} toAct={view.toAct} compact={compact} />
      </BoardSlot>
    )
  }
  if (renderer === "go" || gameId === "go") {
    const points = Array.isArray(view.points) && view.points.length && typeof view.points[0] === "object" ? (view.points as { at: string; seat: number }[]) : []
    return (
      <BoardSlot>
        <GoBoard size={view.size || 19} points={points} lastMove={typeof view.lastMove === "string" ? view.lastMove : null} compact={compact} />
      </BoardSlot>
    )
  }
  if (renderer === "junqi" || gameId === "junqi") {
    const last = view.last && !("cards" in view.last) ? view.last : null
    return (
      <BoardSlot>
        <JunqiBoard cells={view.cells as JunqiCell[] | undefined} last={last} phase={view.phase} compact={compact} />
      </BoardSlot>
    )
  }
  if (renderer === "card-table" || gameId === "doudizhu") {
    const last = view.last && "cards" in view.last ? (view.last as Trick) : null
    return (
      <BoardSlot>
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
      </BoardSlot>
    )
  }
  if (renderer === "mahjong-table" || gameId === "mahjong-sichuan") {
    return (
      <BoardSlot>
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
      </BoardSlot>
    )
  }
  if (renderer === "grid-board") {
    const cells = (view.cells ?? []).filter((cell): cell is GridCell => !("zone" in cell))
    return (
      <BoardSlot>
        <GridBoard size={view.size || 3} cells={cells} compact={compact} />
      </BoardSlot>
    )
  }
  if (view.stones) {
    return (
      <BoardSlot>
        <Board
          stones={view.stones}
          lastMove={typeof view.lastMove === "string" ? view.lastMove : null}
          winLine={view.winLine}
          compact={compact}
        />
      </BoardSlot>
    )
  }
  return (
    <pre className="max-h-[32rem] overflow-auto rounded-2xl border border-border bg-white p-4 text-xs leading-5 text-neutral-800">
      {JSON.stringify(view, null, 2)}
    </pre>
  )
}
