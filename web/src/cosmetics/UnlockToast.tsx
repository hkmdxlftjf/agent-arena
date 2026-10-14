import { api } from "@/lib/api"
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
    <div role="status" className="fixed right-4 bottom-4 z-50 w-64 rounded-xl border border-border bg-card p-3 shadow-[var(--shadow-card)]">
      <p className="text-sm font-medium">新解锁</p>
      <ul className="mt-1 space-y-0.5 text-sm text-muted-foreground">
        {items.map((item) => (
          <li key={`${item.agent_id}-${item.id}`}>{item.name}</li>
        ))}
      </ul>
      <button
        type="button"
        className="mt-2 text-sm text-primary"
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
