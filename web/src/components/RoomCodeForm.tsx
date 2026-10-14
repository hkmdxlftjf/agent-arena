import { Button } from "@/components/ui/button"
import { useState } from "react"
import { useNavigate } from "react-router-dom"

export function RoomCodeForm({ compact = false }: { compact?: boolean }) {
  const navigate = useNavigate()
  const [code, setCode] = useState("")

  return (
    <form
      className={compact ? "flex items-center gap-1" : "flex flex-wrap items-center gap-2"}
      onSubmit={(event) => {
        event.preventDefault()
        const cleaned = code.trim().toUpperCase()
        if (!cleaned) return
        navigate(`/r/${cleaned}`)
      }}
    >
      <label className="sr-only" htmlFor={compact ? "room-code-header" : "room-code-lobby"}>
        房间码
      </label>
      <input
        id={compact ? "room-code-header" : "room-code-lobby"}
        value={code}
        onChange={(event) => setCode(event.target.value.toUpperCase())}
        placeholder="房间码"
        autoCapitalize="characters"
        maxLength={8}
        className={`rounded-lg border border-input bg-background px-2 font-data text-sm uppercase outline-none focus-visible:ring-2 focus-visible:ring-ring ${compact ? "h-8 w-24" : "h-9 w-32"}`}
      />
      <Button type="submit" size={compact ? "sm" : "default"} variant={compact ? "outline" : "default"}>
        进入
      </Button>
    </form>
  )
}
