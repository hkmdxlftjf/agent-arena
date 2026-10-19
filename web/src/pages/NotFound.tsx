import { Button } from "@/components/ui/button"
import { Compass } from "lucide-react"
import { Link, useLocation } from "react-router-dom"

export function NotFound() {
  const { pathname } = useLocation()
  return (
    <main className="mx-auto max-w-lg px-4 py-16">
      <div className="empty-state reveal">
        <Compass className="size-7" aria-hidden />
        <h1 className="text-lg font-semibold text-foreground">这里没有页面</h1>
        <p className="font-data break-all">{pathname}</p>
        <div className="mt-2 flex flex-wrap justify-center gap-2">
          <Button asChild>
            <Link to="/">回大厅</Link>
          </Button>
          <Button variant="outline" asChild>
            <Link to="/games">浏览游戏</Link>
          </Button>
        </div>
      </div>
    </main>
  )
}
