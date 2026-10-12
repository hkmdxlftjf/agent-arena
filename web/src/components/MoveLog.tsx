import { Badge } from "@/components/ui/badge"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import type { LogEntry } from "@/lib/types"
import { Check, Server, Timer } from "lucide-react"

function describe(entry: LogEntry): { who: string; text: string; comment?: string } {
  const seat = entry.payload.seat
  const who = seat === 0 ? "黑" : seat === 1 ? "白" : "席"
  if (entry.kind === "action") {
    const at = entry.payload.action?.data?.at
    return { who, text: at ? at.toLowerCase() : entry.payload.action?.type || "动作", comment: entry.payload.comment }
  }
  if (entry.kind === "auto_action") {
    const at = entry.payload.action?.data?.at
    const policy = entry.payload.policy === "forfeit" ? "超时判负" : at ? `超时落子 ${at}` : "超时"
    return { who, text: policy }
  }
  if (entry.kind === "resign") return { who, text: "认输" }
  if (entry.kind === "result") return { who: "终", text: entry.payload.result?.reason || "结束" }
  if (entry.kind === "genesis") return { who: "开", text: "开局" }
  return { who, text: entry.kind }
}

export function MoveLog({
  entries,
  verified,
}: {
  entries: LogEntry[]
  verified: Record<number, { signature: boolean; ack: boolean } | undefined>
}) {
  const rows = [...entries].reverse()
  if (rows.length === 0) {
    return <p className="px-4 py-8 text-sm text-muted-foreground">对局开始后，每一步都会出现在这里，并带上签名核验。</p>
  }
  return (
    <ol className="font-data max-h-[640px] divide-y divide-border overflow-auto text-sm">
      {rows.map((entry) => {
        const row = describe(entry)
        const mark = verified[entry.seq]
        const ok = mark?.signature && mark.ack
        return (
          <li key={entry.seq} className="grid grid-cols-[2.5rem_1.5rem_1fr_auto] items-start gap-2 px-4 py-2.5">
            <span className="text-muted-foreground">{entry.seq}</span>
            <span className={entry.payload.seat === 0 ? "text-foreground" : "text-primary"}>{row.who}</span>
            <span>
              <span className="text-foreground">{row.text}</span>
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
                      <Check className="size-3" /> {mark ? (ok ? "已核验" : "未通过") : "核验中"}
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
