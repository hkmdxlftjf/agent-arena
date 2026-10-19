import { GameCover } from "@/components/GameCover"
import { Badge } from "@/components/ui/badge"
import { gameTone, players, TRAIT } from "@/lib/games"
import type { Manifest } from "@/lib/types"
import { ArrowRight, Users } from "lucide-react"
import type { CSSProperties } from "react"
import { Link } from "react-router-dom"

export function GameTile({ game, live = 0, traits = false }: { game: Manifest; live?: number; traits?: boolean }) {
  return (
    <Link
      to={`/games/${game.id}`}
      className="tile group"
      style={{ "--tone": gameTone(game.id) } as CSSProperties}
      aria-label={`${game.name}，${game.playable ? "可开局" : "已登记"}${live > 0 ? `，${live} 桌进行中` : ""}`}
    >
      <div className="tile-art">
        <GameCover id={game.id} name={game.name} />
        <div className="absolute top-2.5 left-2.5 flex items-center gap-1.5">
          {live > 0 && (
            <span className="badge-overlay">
              <span className="live-dot" />
              {live} 桌进行中
            </span>
          )}
        </div>
        <div className="absolute top-2.5 right-2.5">
          {game.playable ? (
            <span className="badge-overlay text-[color:var(--tone)]">可开局</span>
          ) : (
            <span className="badge-overlay opacity-80">已登记</span>
          )}
        </div>
        <span className="badge-overlay absolute bottom-2.5 left-2.5 font-data">
          <Users className="size-3" aria-hidden />
          {players(game)}
        </span>
      </div>
      <div className="flex flex-1 flex-col gap-1.5 p-4">
        <div className="flex items-center justify-between gap-2">
          <h3 className="truncate text-base font-semibold">{game.name}</h3>
          {game.rated && <span className="shrink-0 text-[11px] font-medium text-gold">天梯</span>}
        </div>
        <p className="line-clamp-2 text-sm text-muted-foreground">{game.description}</p>
        {traits && game.traits.length > 0 && (
          <div className="flex flex-wrap gap-1 pt-1">
            {game.traits.map((trait) => (
              <Badge key={trait} variant="secondary">
                {TRAIT[trait] || trait}
              </Badge>
            ))}
            {game.uploaded && <Badge variant="outline">上传规则</Badge>}
          </div>
        )}
        <span className="tile-cta mt-auto pt-2">
          查看规则与开局
          <ArrowRight className="size-3.5" aria-hidden />
        </span>
      </div>
    </Link>
  )
}
