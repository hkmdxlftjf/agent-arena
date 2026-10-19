import { api } from "@/lib/api"
import { Sparkles } from "lucide-react"
import { useEffect, useState } from "react"

type Fresh = { agent_id: string; id: string; name: string }

export function UnlockToast({ refreshKey = 0 }: { refreshKey?: string | number }) {
  const [items, setItems] = useState<Fresh[]>([])

  useEffect(() => {
    let cancel = false
    api<{ items: Fresh[] }>("/api/me/unlocks")
      .then((body) => {
        if (!cancel) setItems(body.items)
      })
      .catch(() => {
        if (!cancel) setItems([])
      })
    return () => {
      cancel = true
    }
  }, [refreshKey])

  if (items.length === 0) return null

  return (
    <div role="status" className="unlock-toast fixed right-4 bottom-4 z-50 w-[min(18rem,calc(100vw-2rem))] p-3.5">
      <p className="flex items-center gap-2 text-sm font-semibold">
        <span className="grid size-7 place-items-center rounded-lg bg-gold/15 text-gold">
          <Sparkles className="size-4" aria-hidden />
        </span>
        新解锁
      </p>
      <ul className="mt-2 space-y-1 text-sm">
        {items.map((item) => (
          <li key={`${item.agent_id}-${item.id}`} className="flex items-center gap-2">
            <span className="size-1.5 rounded-full bg-gold" aria-hidden />
            {item.name}
          </li>
        ))}
      </ul>
      <button
        type="button"
        className="mt-3 -mb-1 py-1 text-sm font-medium text-primary hover:underline"
        onClick={() => {
          setItems([])
          void api("/api/me/unlocks", { method: "POST" }).catch(() => undefined)
        }}
      >
        知道了
      </button>
    </div>
  )
}
