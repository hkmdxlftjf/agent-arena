import { Avatar } from "@/cosmetics/avatar"
import { boardPaint, gomokuStone, xiangqiDisc } from "@/cosmetics/skins"
import { Button } from "@/components/ui/button"
import { api } from "@/lib/api"
import type { AgentRow } from "@/lib/types"
import { Check, ImageUp, Lock, Palette } from "lucide-react"
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
    return (
      <div className="empty-state mt-6">
        <Palette className="size-6" aria-hidden />
        <p>先创建一个智能体，再给它挑装扮。</p>
      </div>
    )
  }

  const unlocked = new Set(bundle?.unlocks ?? [])
  const equipped = (kind: string) => bundle?.loadout.find((row) => row.game_id === game && row.kind === kind)?.item_id
  const piece = bundle?.items.find((item) => item.id === equipped("piece"))?.asset
  const board = bundle?.items.find((item) => item.id === equipped("board"))?.asset
  const frame = bundle?.frame

  return (
    <div className="mt-4 grid items-start gap-6 lg:grid-cols-[280px_1fr]">
      <div className="lg:sticky lg:top-24">
        <Preview game={game} color={color} preset={bundle?.avatar_preset} imageUrl={bundle?.avatar_url} frame={frame} title={title} piece={piece} board={board} />
      </div>
      <div className="min-w-0 space-y-4">
        {error && (
          <p className="alert-error" role="alert">
            {error}
          </p>
        )}
        <div className="chip-row" role="toolbar" aria-label="选择智能体">
          {agents.map((agent) => (
            <button key={agent.id} type="button" onClick={() => setAgentId(agent.id)} className="chip chip-solid" aria-pressed={agent.id === agentId}>
              {agent.name}
            </button>
          ))}
        </div>
        {!bundle && !error && <div className="skeleton h-64" aria-busy="true" />}
        {bundle && (
          <div key={agentId} className="fade-in space-y-4">
            <section className="panel p-4">
              <h2 className="text-sm font-semibold">头像</h2>
              <div className="mt-3 flex flex-wrap gap-2">
                {bundle.presets.map((preset) => (
                  <button
                    key={preset.id}
                    type="button"
                    title={preset.name}
                    aria-label={preset.name}
                    aria-pressed={bundle.avatar_preset === preset.id}
                    onClick={() => void save({ avatar_preset: preset.id })}
                    className="loadout-avatar"
                  >
                    <Avatar preset={preset.id} color={color} size={40} />
                  </button>
                ))}
              </div>
              <div className="mt-3 flex flex-wrap items-center gap-3">
              <label className="inline-flex h-8 cursor-pointer items-center gap-2 rounded-lg border border-border bg-secondary px-3 text-sm text-foreground transition-colors focus-within:ring-2 focus-within:ring-ring hover:bg-muted">
                <ImageUp className="size-4" aria-hidden />
                上传图片
                <input
                  type="file"
                  accept="image/png,image/jpeg,image/gif,image/webp"
                  className="sr-only"
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
                <button type="button" className="py-1 text-sm font-medium text-primary hover:underline" onClick={() => void save({ clear_avatar: true })}>
                  改回头像预设
                </button>
              )}
              </div>
            </section>
            <form
              className="panel grid gap-3 p-4 sm:grid-cols-2"
              onSubmit={(event) => {
                event.preventDefault()
                void save({ title, bio, color })
              }}
            >
              <label className="grid gap-1 text-xs font-medium text-muted-foreground">
                称号
                <input value={title} maxLength={12} onChange={(event) => setTitle(event.target.value)} className="field w-full" />
              </label>
              <label className="grid gap-1 text-xs font-medium text-muted-foreground">
                颜色
                <span className="field flex w-full items-center gap-2 px-1.5">
                  <input type="color" value={color} onChange={(event) => setColor(event.target.value)} className="h-6 w-10 cursor-pointer rounded border-0 bg-transparent p-0" />
                  <span className="font-data text-sm text-foreground">{color}</span>
                </span>
              </label>
              <label className="grid gap-1 text-xs font-medium text-muted-foreground sm:col-span-2">
                短简介
                <input value={bio} maxLength={80} onChange={(event) => setBio(event.target.value)} className="field w-full" />
              </label>
              <Button type="submit" className="w-fit">
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
            <div className="chip-row" role="toolbar" aria-label="选择游戏">
              {GAMES.map((row) => (
                <button key={row.id} type="button" onClick={() => setGame(row.id)} className="chip chip-solid" aria-pressed={game === row.id}>
                  {row.name}
                </button>
              ))}
            </div>
            <div key={game} className="fade-in space-y-4">
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
            </div>
          </div>
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
    <section className="panel p-4">
      <h2 className="text-sm font-semibold">{title}</h2>
      <div className="mt-3 grid grid-cols-[repeat(auto-fill,minmax(8.5rem,1fr))] gap-2">
        {items.map((item) => {
          const open = item.unlock_rule === "default" || unlocked.has(item.id)
          const active = selected === item.id || (!selected && item.unlock_rule === "default" && item.asset !== "none")
          return (
            <button
              key={item.id}
              type="button"
              disabled={!open}
              aria-pressed={active}
              onClick={() => onPick(active && selected === item.id ? null : item)}
              className="loadout-item"
              data-rarity={item.rarity}
            >
              <span className="flex items-center justify-between gap-1">
                <span className="truncate font-medium">{item.name}</span>
                {active ? <Check className="size-3.5 shrink-0 text-primary" aria-hidden /> : !open ? <Lock className="size-3.5 shrink-0" aria-hidden /> : null}
              </span>
              <span className="block truncate text-xs text-muted-foreground">{open ? (active ? "已装备" : "已解锁") : RULE_TEXT[item.unlock_rule] || item.unlock_rule}</span>
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
  const paint = boardPaint(board) || { fill: "var(--board-fill)", line: "var(--board-line)", label: "var(--board-label)", river: "var(--board-fill)", edge: "var(--board-edge)" }
  const dark = gomokuStone(piece, 0, board)
  const light = gomokuStone(piece, 1, board)
  const red = xiangqiDisc(piece, true)
  const black = xiangqiDisc(piece, false)
  return (
    <div className="panel stage p-4">
      <p className="eyebrow mb-3">Preview</p>
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
