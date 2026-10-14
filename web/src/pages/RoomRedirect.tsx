import { api } from "@/lib/api"
import type { MatchSnap } from "@/lib/types"
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
    <main className="mx-auto max-w-lg px-4 py-16 text-center">
      {error ? (
        <>
          <p className="text-lg font-medium">找不到房间 {code.trim().toUpperCase()}</p>
          <p className="mt-2 text-sm text-muted-foreground">{error}</p>
          <Link to="/games" className="mt-4 inline-block text-sm text-primary">
            回游戏大厅
          </Link>
        </>
      ) : (
        <p className="text-sm text-muted-foreground">正在打开 {code.trim().toUpperCase()}</p>
      )}
    </main>
  )
}
