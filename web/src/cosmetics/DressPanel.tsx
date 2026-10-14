import { Avatar } from "@/cosmetics/avatar"
import { boardPaint, gomokuStone, xiangqiDisc } from "@/cosmetics/skins"
import { Button } from "@/components/ui/button"
import { api } from "@/lib/api"
import type { AgentRow } from "@/lib/types"
import { useEffect, useState } from "react"

type Item = {
  id: string
  kind: string
  game_id: string | null
  name: string
  rarity: string
  asset: string
  unlock_rule: string
}

type Preset = { id: string; name: string }

type Bundle = {
  agent_id: string
  name: string
  title: string | null
  bio: string | null
  color: string
  avatar_preset: string
  avatar_url: string | null
  frame_id: string | null
  frame: string | null
  loadout: { game_id: string; kind: string; item_id: string }[]
  unlocks: string[]
  items: Item[]
  presets: Preset[]
}

const GAMES = [
  { id: "gomoku", name: "五子棋" },
  { id: "xiangqi", name: "象棋" },
  { id: "go", name: "围棋" },
  { id: "doudizhu", name: "斗地主" },
  { id: "mahjong-sichuan", name: "麻将" },
  { id: "junqi", name: "军棋" },
]

const RULE_TEXT: Record<string, string> = {
  default: "一开始就有",
  first_win: "第一场胜利",
  "wins:10": "胜 10 场",
  "rating:1600": "评分到 1600",
  "rating:1800": "评分到 1800",
  "rating:2000": "评分到 2000",
  "streak:3": "连胜 3 场",
  all_games: "六款内置游戏都打过",
  season_top10: "赛季前十",
}

export function DressPanel({ agents }: { agents: AgentRow[] }) {
  const [agentId, setAgentId] = useState(agents[0]?.id ?? "")
  const [bundle, setBundle] = useState<Bundle | null>(null)
  const [error, setError] = useState("")
  const [game, setGame] = useState("gomoku")
  const [title, setTitle] = useState("")
  const [bio, setBio] = useState("")
  const [color, setColor] = useState("#8c3a2f")

  useEffect(() => {
    if (!agentId) return
    let cancel = false
    api<Bundle>(`/api/agents/${agentId}/cosmetics`)
      .then((body) => {
        if (cancel) return
        setBundle(body)
        setTitle(body.title || "")
        setBio(body.bio || "")
        setColor(body.color || "#8c3a2f")
        setError("")
      })
      .catch((err: Error) => {
        if (!cancel) setError(err.message)
      })
    return () => {
      cancel = true
    }
  }, [agentId])

  async function save(patch: Record<string, unknown>) {
    setError("")
    try {
      const next = await api<Bundle>(`/api/agents/${agentId}/cosmetics`, {
        method: "POST",
        body: JSON.stringify(patch),
      })
      setBundle(next)
      setTitle(next.title || "")
      setBio(next.bio || "")
      setColor(next.color || "#8c3a2f")
    } catch (err) {
      setError(err instanceof Error ? err.message : "没有保存")
    }
  }

  if (agents.length === 0) {
    return <p className="mt-6 text-sm text-muted-foreground">先创建一个智能体，再给它挑装扮。</p>
  }

  const unlocked = new Set(bundle?.unlocks ?? [])
  const equipped = (kind: string) => bundle?.loadout.find((row) => row.game_id === game && row.kind === kind)?.item_id
  const piece = bundle?.items.find((item) => item.id === equipped("piece"))?.asset
  const board = bundle?.items.find((item) => item.id === equipped("board"))?.asset
  const frame = bundle?.frame

  return (
    <div className="mt-6 grid gap-6 lg:grid-cols-[280px_1fr]">
      <Preview game={game} color={color} preset={bundle?.avatar_preset} imageUrl={bundle?.avatar_url} frame={frame} title={title} piece={piece} board={board} />
      <div className="space-y-5">
        {error && <p className="text-sm text-destructive">{error}</p>}
        <div className="flex flex-wrap gap-2">
          {agents.map((agent) => (
            <button
              key={agent.id}
              type="button"
              onClick={() => setAgentId(agent.id)}
              className={`rounded-full px-3 py-1 text-sm ${agent.id === agentId ? "bg-primary text-primary-foreground" : "bg-muted text-muted-foreground"}`}
            >
              {agent.name}
            </button>
          ))}
        </div>
        {bundle && (
          <>
            <section>
              <h2 className="text-sm font-medium">头像</h2>
              <div className="mt-2 flex flex-wrap gap-2">
                {bundle.presets.map((preset) => (
                  <button
                    key={preset.id}
                    type="button"
                    title={preset.name}
                    onClick={() => void save({ avatar_preset: preset.id })}
                    className={`rounded-full p-0.5 ${bundle.avatar_preset === preset.id ? "ring-2 ring-primary" : ""}`}
                  >
                    <Avatar preset={preset.id} color={color} size={40} />
                  </button>
                ))}
              </div>
              <label className="mt-3 inline-flex cursor-pointer items-center gap-2 text-sm text-muted-foreground">
                上传图片
                <input
                  type="file"
                  accept="image/png,image/jpeg,image/gif,image/webp"
                  className="max-w-48 text-xs"
                  onChange={(event) => {
                    const file = event.target.files?.[0]
                    event.target.value = ""
                    if (!file) return
                    void fetch(`/api/agents/${agentId}/avatar`, {
                      method: "POST",
                      credentials: "include",
                      headers: { "content-type": file.type || "application/octet-stream" },
                      body: file,
                    })
                      .then(async (res) => {
                        if (!res.ok) {
                          const data = (await res.json()) as { message?: string }
                          throw new Error(data.message || "上传失败")
                        }
                        const next = await api<Bundle>(`/api/agents/${agentId}/cosmetics`)
                        setBundle(next)
                      })
                      .catch((err: Error) => setError(err.message))
                  }}
                />
              </label>
              {bundle.avatar_url && (
                <button type="button" className="ml-3 text-sm text-primary" onClick={() => void save({ clear_avatar: true })}>
                  改回头像预设
                </button>
              )}
            </section>
            <form
              className="grid gap-2 sm:grid-cols-2"
              onSubmit={(event) => {
                event.preventDefault()
                void save({ title, bio, color })
              }}
            >
              <label className="text-sm">
                称号
                <input value={title} maxLength={12} onChange={(event) => setTitle(event.target.value)} className="mt-1 w-full rounded-md border border-border bg-background px-2 py-1.5" />
              </label>
              <label className="text-sm">
                颜色
                <input type="color" value={color} onChange={(event) => setColor(event.target.value)} className="mt-1 h-9 w-full" />
              </label>
              <label className="text-sm sm:col-span-2">
                短简介
                <input value={bio} maxLength={80} onChange={(event) => setBio(event.target.value)} className="mt-1 w-full rounded-md border border-border bg-background px-2 py-1.5" />
              </label>
              <Button type="submit" size="sm" className="w-fit">
                保存外观
              </Button>
            </form>
            <ItemRow
              title="边框"
              items={bundle.items.filter((item) => item.kind === "frame")}
              unlocked={unlocked}
              selected={bundle.frame_id}
              onPick={(item) => void save({ frame_id: item?.id ?? "" })}
            />
            <div className="flex gap-2 overflow-x-auto">
              {GAMES.map((row) => (
                <button
                  key={row.id}
                  type="button"
                  onClick={() => setGame(row.id)}
                  className={`shrink-0 rounded-full px-3 py-1 text-sm ${game === row.id ? "bg-secondary text-foreground" : "text-muted-foreground"}`}
                >
                  {row.name}
                </button>
              ))}
            </div>
            <ItemRow
              title="棋子"
              items={bundle.items.filter((item) => item.kind === "piece" && item.game_id === game)}
              unlocked={unlocked}
              selected={equipped("piece")}
              onPick={(item) => void save({ equip: [{ game_id: game, kind: "piece", item_id: item?.id ?? "" }] })}
            />
            {(game === "doudizhu" || game === "mahjong-sichuan") && (
              <ItemRow
                title="牌背"
                items={bundle.items.filter((item) => item.kind === "card_back")}
                unlocked={unlocked}
                selected={equipped("card_back")}
                onPick={(item) => void save({ equip: [{ game_id: game, kind: "card_back", item_id: item?.id ?? "" }] })}
              />
            )}
            {(game === "gomoku" || game === "xiangqi" || game === "go") && (
              <ItemRow
                title="棋盘"
                items={bundle.items.filter((item) => item.kind === "board")}
                unlocked={unlocked}
                selected={equipped("board")}
                onPick={(item) => void save({ equip: [{ game_id: game, kind: "board", item_id: item?.id ?? "" }] })}
              />
            )}
            <ItemRow
              title="胜利表情"
              items={bundle.items.filter((item) => item.kind === "emote")}
              unlocked={unlocked}
              selected={equipped("emote")}
              onPick={(item) => void save({ equip: [{ game_id: game, kind: "emote", item_id: item?.id ?? "" }] })}
            />
          </>
        )}
      </div>
    </div>
  )
}

function ItemRow({
  title,
  items,
  unlocked,
  selected,
  onPick,
}: {
  title: string
  items: Item[]
  unlocked: Set<string>
  selected?: string | null
  onPick: (item: Item | null) => void
}) {
  if (items.length === 0) return null
  return (
    <section>
      <h2 className="text-sm font-medium">{title}</h2>
      <div className="mt-2 flex flex-wrap gap-2">
        {items.map((item) => {
          const open = item.unlock_rule === "default" || unlocked.has(item.id)
          const active = selected === item.id || (!selected && item.unlock_rule === "default" && item.asset !== "none")
          return (
            <button
              key={item.id}
              type="button"
              disabled={!open}
              onClick={() => onPick(active && selected === item.id ? null : item)}
              className={`rounded-lg border px-2 py-1 text-left text-sm ${active ? "border-primary" : "border-border"} ${open ? "" : "opacity-50"}`}
            >
              <span className="block font-medium">{item.name}</span>
              <span className="block text-xs text-muted-foreground">{open ? "已解锁" : RULE_TEXT[item.unlock_rule] || item.unlock_rule}</span>
            </button>
          )
        })}
      </div>
    </section>
  )
}

function Preview({
  game,
  color,
  preset,
  imageUrl,
  frame,
  title,
  piece,
  board,
}: {
  game: string
  color: string
  preset?: string
  imageUrl?: string | null
  frame?: string | null
  title: string
  piece?: string
  board?: string
}) {
  const paint = boardPaint(board) || { fill: "#f3e6c8", line: "#8a7b6a", label: "#6b6258", river: "#f3d7b5", edge: "#c4a574" }
  const dark = gomokuStone(piece, 0, board)
  const light = gomokuStone(piece, 1, board)
  const red = xiangqiDisc(piece, true)
  const black = xiangqiDisc(piece, false)
  return (
    <div className="rounded-2xl border border-border bg-card p-4">
      <div className="mb-3 flex items-center gap-2">
        <Avatar preset={preset} color={color} frame={frame} imageUrl={imageUrl} size={48} />
        <div>
          <p className="text-sm font-medium">{title || "未设称号"}</p>
          <p className="text-xs text-muted-foreground">预览 · {GAMES.find((row) => row.id === game)?.name}</p>
        </div>
      </div>
      <svg viewBox="0 0 180 180" className="w-full rounded-xl border" style={{ borderColor: paint.edge }} role="img" aria-label="装扮预览">
        <rect width="180" height="180" fill={paint.fill} />
        {Array.from({ length: 7 }, (_, i) => {
          const p = 18 + i * 24
          return (
            <g key={i}>
              <line x1="18" y1={p} x2="162" y2={p} stroke={paint.line} />
              <line x1={p} y1="18" x2={p} y2="162" stroke={paint.line} />
            </g>
          )
        })}
        {game === "xiangqi" ? (
          <>
            <circle cx="66" cy="90" r="14" fill={red.fill} stroke={red.stroke} />
            <text x="66" y="95" textAnchor="middle" fontSize="14" fill="#b42318">
              帅
            </text>
            <circle cx="114" cy="90" r="14" fill={black.fill} stroke={black.stroke} />
            <text x="114" y="95" textAnchor="middle" fontSize="14" fill="#1c1917">
              将
            </text>
          </>
        ) : (
          <>
            <circle cx="66" cy="90" r="11" fill={dark.fill} stroke={dark.stroke} />
            <circle cx="114" cy="90" r="11" fill={light.fill} stroke={light.stroke} />
          </>
        )}
      </svg>
    </div>
  )
}
