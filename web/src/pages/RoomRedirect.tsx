import { Button } from "@/components/ui/button"
import { api } from "@/lib/api"
import type { MatchSnap } from "@/lib/types"
import { DoorClosed, Loader2 } from "lucide-react"
import { useEffect, useState } from "react"
import { Link, useNavigate, useParams } from "react-router-dom"

export function RoomRedirect() {
  const { code = "" } = useParams()
  const navigate = useNavigate()
  const [error, setError] = useState("")

  useEffect(() => {
    let cancel = false
    const cleaned = code.trim()
    api<MatchSnap>(`/api/rooms/${encodeURIComponent(cleaned)}`)
      .then((room) => {
        if (!cancel) navigate(`/m/${room.id}`, { replace: true })
      })
      .catch((err: Error) => {
        if (!cancel) setError(err.message)
      })
    return () => {
      cancel = true
    }
  }, [code, navigate])

  return (
    <main className="mx-auto max-w-lg px-4 py-16">
      {error ? (
        <div className="empty-state reveal">
          <DoorClosed className="size-7" aria-hidden />
          <p className="text-lg font-semibold text-foreground">
            找不到房间 <span className="font-data tracking-widest">{code.trim().toUpperCase()}</span>
          </p>
          <p>{error}</p>
          <Button variant="outline" asChild className="mt-2">
            <Link to="/games">回游戏大厅</Link>
          </Button>
        </div>
      ) : (
        <div className="empty-state fade-in" role="status">
          <Loader2 className="size-6 animate-spin" aria-hidden />
          <p>
            正在打开 <span className="font-data tracking-widest text-foreground">{code.trim().toUpperCase()}</span>
          </p>
        </div>
      )}
    </main>
  )
}
