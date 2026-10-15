import { Badge } from "@/components/ui/badge"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { roleOf } from "@/lib/games"
import { reasonText } from "@/lib/reasons"
import type { LogEntry } from "@/lib/types"
import { Check, Server, Timer } from "lucide-react"

function textOf(data: Record<string, unknown> | undefined): string {
  if (!data) return ""
  if (typeof data.at === "string") return data.at
  if (typeof data.from === "string" && typeof data.to === "string") return `${data.from} → ${data.to}`
  if (typeof data.score === "number") return data.score === 0 ? "不叫" : `叫 ${data.score}`
  if (Array.isArray(data.cards)) return data.cards.join(" ")
  if (typeof data.tile === "string") return data.tile
  if (typeof data.suit === "string") return data.suit
  return ""
}

function describe(entry: LogEntry, gameId: string): { who: string; text: string; comment?: string } {
  const seat = entry.payload.seat
  const who = seat == null ? "席" : roleOf(gameId, seat)
  const action = entry.payload.action
  const detail = textOf(action?.data)
  const type = action?.type
  if (entry.kind === "auto_action") {
    const forfeited = entry.payload.policy === "forfeit" || entry.payload.action == null
    return { who, text: forfeited ? "超时判负" : "超时，服务器代走", comment: entry.payload.comment }
  }
  if (entry.kind === "leave") return { who, text: "退出对局，判对方胜" }
  if (entry.kind === "action") {
    const label =
      type === "place" ? detail || "落子" :
      type === "move" ? detail || "走子" :
      type === "pass" ? "过" :
      type === "bid" ? detail || "叫分" :
      type === "play" ? detail || "出牌" :
      type === "deploy_commit" || type === "deploy" ? "布阵" :
      type === "exchange_commit" || type === "exchange" ? "换三张" :
      type === "dingque_commit" || type === "dingque" ? `定缺 ${detail}` :
      type === "discard" ? `打 ${detail}` :
      type === "hu" ? "胡" :
      type === "peng" ? "碰" :
      type === "gang" ? "杠" :
      type === "angang" ? `暗杠 ${detail}` :
      type === "bugang" ? `补杠 ${detail}` :
      type === "resign" ? "认输" :
      detail || type || "动作"
    return { who, text: label, comment: entry.payload.comment }
  }
  if (entry.kind === "resign") return { who, text: "认输" }
  if (entry.kind === "abandon") return { who, text: "超时判负" }
  if (entry.kind === "reveal") return { who: "揭", text: `揭示 ${entry.payload.reveals?.length ?? 0} 份承诺` }
  if (entry.kind === "result") return { who: "终", text: reasonText(entry.payload.result?.reason) }
  if (entry.kind === "genesis") return { who: "开", text: "开局" }
  return { who, text: entry.kind }
}

export function MoveLog({
  entries,
  verified,
  gameId,
}: {
  entries: LogEntry[]
  verified: Record<number, { signature: boolean; ack: boolean } | undefined>
  gameId: string
}) {
  const rows = [...entries].reverse()
  if (rows.length === 0) {
    return <p className="px-4 py-10 text-center text-sm text-muted-foreground">对局开始后，每一步都会出现在这里，并带上签名核验。</p>
  }
  return (
    <ol className="font-data max-h-[420px] divide-y divide-border overflow-auto overscroll-contain text-sm lg:max-h-[640px]">
      {rows.map((entry, index) => {
        const row = describe(entry, gameId)
        const mark = verified[entry.seq]
        const ok = mark?.signature === true
        return (
          <li
            key={entry.seq}
            className={`log-row grid grid-cols-[2.25rem_1.5rem_minmax(0,1fr)_auto] items-start gap-2 px-4 py-2.5 ${index === 0 ? "log-row-latest" : ""}`}
          >
            <span className="text-muted-foreground">{entry.seq}</span>
            <span className="grid size-6 place-items-center rounded-md bg-secondary font-sans text-xs text-foreground">{row.who}</span>
            <span className="min-w-0 break-words">
              <span className="font-sans text-foreground">{row.text}</span>
              {row.comment && <span className="mt-0.5 block font-sans text-xs text-muted-foreground">“{row.comment}”</span>}
            </span>
            <Tooltip>
              <TooltipTrigger asChild>
                <span>
                  {entry.kind === "auto_action" ? (
                    <Badge variant="outline" className="gap-1">
                      <Timer className="size-3" /> 超时
                    </Badge>
                  ) : entry.signer === "server" ? (
                    <Badge variant="secondary" className="gap-1">
                      <Server className="size-3" /> 服务器
                    </Badge>
                  ) : (
                    <Badge variant={ok ? "default" : "outline"} className="gap-1">
                      <Check className="size-3" /> {mark ? (ok ? "已验证" : "未通过") : "核验中"}
                    </Badge>
                  )}
                </span>
              </TooltipTrigger>
              <TooltipContent>
                {entry.kid}
                {mark ? ` · 签名 ${mark.signature ? "通过" : "失败"} · 回执 ${mark.ack ? "通过" : "失败"}` : ""}
              </TooltipContent>
            </Tooltip>
          </li>
        )
      })}
    </ol>
  )
}
