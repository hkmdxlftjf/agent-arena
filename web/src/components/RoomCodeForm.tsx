import { Button } from "@/components/ui/button"
import { DoorOpen, Hash } from "lucide-react"
import { useState } from "react"
import { useNavigate } from "react-router-dom"

export function RoomCodeForm({ compact = false }: { compact?: boolean }) {
  const navigate = useNavigate()
  const [code, setCode] = useState("")
  const id = compact ? "room-code-header" : "room-code-lobby"

  return (
    <form
      className={compact ? "flex items-center gap-1" : "flex items-center gap-2"}
      onSubmit={(event) => {
        event.preventDefault()
        const cleaned = code.trim().toUpperCase()
        if (!cleaned) return
        navigate(`/r/${cleaned}`)
      }}
    >
      <label className={`search-field ${compact ? "h-8 w-28 px-2" : "h-9 w-36"}`} htmlFor={id}>
        <Hash className="size-3.5 shrink-0" aria-hidden />
        <span className="sr-only">房间码</span>
        <input
          id={id}
          value={code}
          onChange={(event) => setCode(event.target.value.toUpperCase())}
          placeholder="房间码"
          autoCapitalize="characters"
          autoComplete="off"
          spellCheck={false}
          maxLength={8}
          className="font-data uppercase tracking-wider"
        />
      </label>
      <Button type="submit" size={compact ? "sm" : "default"} variant={compact ? "outline" : "default"} disabled={!code.trim()}>
        {!compact && <DoorOpen aria-hidden />}
        进入
      </Button>
    </form>
  )
}
