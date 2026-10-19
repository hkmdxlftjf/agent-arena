import { GameTile } from "@/components/GameTile"
import type { Manifest } from "@/lib/types"
import { AnimatePresence, LayoutGroup, motion, useReducedMotion } from "framer-motion"
import { SearchX, Search, X } from "lucide-react"
import { useId, useMemo, useState } from "react"

type Category = { id: string; label: string; match: (game: Manifest) => boolean }

const CATEGORIES: Category[] = [
  { id: "all", label: "全部", match: () => true },
  { id: "board", label: "棋盘", match: (g) => g.traits.includes("board") },
  { id: "cards", label: "牌类", match: (g) => g.traits.includes("card") || g.traits.includes("tile") },
  { id: "multi", label: "多人", match: (g) => g.maxPlayers > 2 },
  { id: "hidden", label: "隐藏信息", match: (g) => g.traits.includes("hidden_info") },
  { id: "rated", label: "天梯", match: (g) => g.rated },
  { id: "uploaded", label: "上传", match: (g) => Boolean(g.uploaded) },
]

export function GameLibrary({
  games,
  liveByGame = {},
  search = true,
  traits = false,
}: {
  games: Manifest[]
  liveByGame?: Record<string, number>
  search?: boolean
  traits?: boolean
}) {
  const [category, setCategory] = useState("all")
  const [query, setQuery] = useState("")
  const reduce = useReducedMotion()
  const group = useId()
  const searchId = useId()

  const available = useMemo(
    () => CATEGORIES.filter((item) => item.id === "all" || games.some(item.match)).map((item) => ({ ...item, count: games.filter(item.match).length })),
    [games],
  )
  const active = available.find((item) => item.id === category) ?? available[0]
  const needle = query.trim().toLowerCase()
  const shown = games.filter(
    (game) =>
      active.match(game) &&
      (!needle || game.name.toLowerCase().includes(needle) || game.id.includes(needle) || game.description.toLowerCase().includes(needle)),
  )

  return (
    <div>
      <div className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
        <LayoutGroup id={group}>
          <div className="chip-row min-w-0" role="toolbar" aria-label="游戏分类">
            {available.map((item) => {
              const on = item.id === active.id
              return (
                <button key={item.id} type="button" className="chip" aria-pressed={on} onClick={() => setCategory(item.id)}>
                  {on && (
                    <motion.span
                      layoutId="chip-fill"
                      className="chip-fill"
                      transition={reduce ? { duration: 0 } : { type: "spring", stiffness: 500, damping: 38 }}
                    />
                  )}
                  {item.label}
                  <span className="chip-count">{item.count}</span>
                </button>
              )
            })}
          </div>
        </LayoutGroup>
        {search && (
          <label className="search-field w-full md:w-72" htmlFor={searchId}>
            <Search className="size-4 shrink-0" aria-hidden />
            <span className="sr-only">搜索游戏</span>
            <input
              id={searchId}
              type="search"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder="搜索游戏名称或玩法"
              autoComplete="off"
            />
            {query && (
              <button type="button" className="grid size-6 place-items-center rounded-md hover:bg-muted" aria-label="清空搜索" onClick={() => setQuery("")}>
                <X className="size-3.5" />
              </button>
            )}
          </label>
        )}
      </div>

      <p className="sr-only" aria-live="polite">
        共 {shown.length} 款游戏
      </p>

      {shown.length === 0 ? (
        <div className="empty-state mt-4">
          <SearchX className="size-6" aria-hidden />
          <p className="font-medium text-foreground">没有符合条件的游戏</p>
          <p>换个关键词，或切回“全部”。</p>
        </div>
      ) : (
        <motion.ul layout={!reduce} className="mt-4 grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
          <AnimatePresence mode="popLayout" initial={false}>
            {shown.map((game, index) => (
              <motion.li
                key={game.id}
                layout={!reduce}
                initial={reduce ? false : { opacity: 0, scale: 0.96, y: 10 }}
                animate={{ opacity: 1, scale: 1, y: 0 }}
                exit={reduce ? { opacity: 0, transition: { duration: 0 } } : { opacity: 0, scale: 0.96, transition: { duration: 0.15 } }}
                transition={{ duration: 0.3, delay: reduce ? 0 : Math.min(index, 8) * 0.035, ease: [0.2, 0.8, 0.2, 1] }}
              >
                <GameTile game={game} live={liveByGame[game.id] ?? 0} traits={traits} />
              </motion.li>
            ))}
          </AnimatePresence>
        </motion.ul>
      )}
    </div>
  )
}
